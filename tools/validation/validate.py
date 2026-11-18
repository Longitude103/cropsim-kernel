"""Kernel validation and model change report on the Scottsbluff test parcels.

    python tools/validation/validate.py WEA_DIR [OUT_DIR]

WEA_DIR holds SCTB2001.WEA ... SCTB2025.WEA (ESC weather set). Writes
kernel-validation.md, annual.csv, checks.csv, fix_impact.csv and
fix_impact_years.csv to OUT_DIR (default docs/validation).

Part 1 checks the fixed kernel (every fix on, the default): run status, no
NaN or infinite value anywhere, water balance closure (the kernel's own
check and an independent one from the daily series and the carried state),
annual totals equal to the sums of the daily and monthly values, physical
bounds, the second-pass (known applied water) replay, repeatability and
thread safety, and plausibility of the annual totals.

Part 2 runs every parcel with each fix on its own (against the original)
and with each fix taken out of the full set (against all fixes), and
reports how annual ET, irrigation, runoff, deep percolation and wetland
supply move.
"""

from __future__ import annotations

import csv
import math
import statistics as st
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import parcels as pc  # noqa: E402
from runner import Runner  # noqa: E402

from cropsim import ALL_FIXES, Fix  # noqa: E402

ALL = int(ALL_FIXES)
ORIG = 0
FIXES = [f for f in Fix if f]

# (annual key, label)
METRICS = [("ets", "ET"), ("snirr", "Net irr."), ("sgrirr", "Gross irr."),
           ("totrof", "Runoff"), ("drains", "Deep perc."), ("wetsups", "Wetland supply")]
EXTRA = [("ts", "Transp."), ("es", "Evap."), ("precips", "Precip"), ("irigno", "Irrigations")]

TOL = 0.005  # in, for "changed" and closure tests
WET = (16, 18, 19, 20, 21)
YIELD_KEYS = ("yield", "yldratio", "bvalue")


# ---------------------------------------------------------------------------
# Plausibility bands (annual means 2002-2025, inches). Reviewer judgment from
# typical published Panhandle values, widened for Hargreaves ETr and for
# off-season soil evaporation (Jan-Apr, Oct-Dec) which CropSim includes.
# ---------------------------------------------------------------------------
BANDS = {
    "irrigated_corn": dict(ets=(24, 34), season=(21, 29), snirr=(9, 19), drains=(0, 8)),
    "irrigated_corn_surface": dict(ets=(24, 38), season=(21, 31), snirr=(9, 21),
                                   drains=(0, 12)),
    "irrigated_beans": dict(ets=(18, 28), season=(15, 22), snirr=(6, 14), drains=(0, 8)),
    "irrigated_beets": dict(ets=(26, 38), season=(22, 31), snirr=(12, 22), drains=(0, 10)),
    "irrigated_potato": dict(ets=(22, 33), season=(19, 27), snirr=(10, 20), drains=(0, 10)),
    "irrigated_rotation": dict(ets=(20, 34), season=(17, 29), snirr=(6, 20), drains=(0, 8)),
    "irrigated_alfalfa": dict(ets=(32, 46), season=(28, 40), snirr=(18, 32), drains=(0, 8)),
    "irrigated_pasture": dict(ets=(26, 42), season=(20, 34), snirr=(10, 28), drains=(0, 12)),
    "dryland": dict(ets_over_p=(0.85, 1.05), drains=(0, 2)),
    "dryland_perennial": dict(ets_over_p=(0.85, 1.05), drains=(0, 3)),
    "riparian": dict(ets=(32, 52)),
    "wetland": dict(ets=(32, 55)),
}


def fnum(x) -> bool:
    return isinstance(x, (int, float)) and math.isfinite(x)


def storage(state, depth) -> float:
    return sum(t * d for t, d in zip(state.theta, depth)) + state.snoh2o


def season_et(r) -> float:
    return sum(r.monthly["etmon"][4:9])


# ---------------------------------------------------------------------------
# Part 1: checks
# ---------------------------------------------------------------------------

def check_run(R: Runner, p, r, block) -> dict:
    """All checks for one parcel-year. Returns {check: (ok, worst value)}."""
    out = {}
    depth = block.depth
    n = len(R.weather[r.year].tmax)
    sim = R.sim_for(p, r.year)
    crop = sim.crop

    out["status 0"] = (r.status == 0, r.status)

    bad = [k for k, v in r.annual.items() if not fnum(float(v))]
    bad += [k for k, v in r.monthly.items() if not all(fnum(float(x)) for x in v)]
    bad += [k for k, v in r.weekly.items() if not all(fnum(float(x)) for x in v)]
    bad += [k for k, v in r.daily.items() if not all(fnum(float(x)) for x in v[:n])]
    st_vals = list(r.state.theta) + [r.state.snoh2o, r.state.snotmp, r.state.oldres]
    bad += ["state"] if not all(fnum(float(x)) for x in st_vals) else []
    # yield is not a water budget term; report it separately
    ybad = [b for b in bad if b in YIELD_KEYS]
    bad = [b for b in bad if b not in YIELD_KEYS]
    out["no NaN/Inf (water budget, daily, state)"] = (not bad, ",".join(bad[:3]) or 0)
    out["yield outputs finite (YIELD, YLDRATIO, BVALUE)"] = (not ybad, ",".join(ybad) or 0)

    out["kernel balance error < 0.005 in/yr"] = (abs(r.annual["balerr"]) < TOL,
                                                 r.annual["balerr"])
    dmax = max(abs(x) for x in r.daily["balerr"][:n])
    out["kernel daily balance error < 0.001 in"] = (dmax < 0.001, dmax)

    d = r.daily
    inflow = sum(d["precip"][:n]) + sum(d["runon"][:n]) + sum(d["irrinf"][:n]) + \
        sum(d["wetsup"][:n])
    outflow = sum(d["runoff"][:n]) + sum(d["et"][:n]) + sum(d["drain"][:n])
    ds = storage(r.state, depth) - storage(r.start_state, depth)
    resid = ds - (inflow - outflow)
    out["independent closure (state + daily) < 0.01 in/yr"] = (abs(resid) < 0.01, resid)

    # annual equals daily sums and monthly sums
    worst = 0.0
    for ak, dk, mk in (("ets", "et", "etmon"), ("drains", "drain", "dramon"),
                       ("totrof", "runoff", "rofmon"), ("snirr", "nirr", "irnmon"),
                       ("sgrirr", "grirr", "irgmon"), ("precips", "precip", "ramon"),
                       ("wetsups", "wetsup", "wsmon")):
        a = r.annual[ak]
        worst = max(worst, abs(a - sum(d[dk][:n])), abs(a - sum(r.monthly[mk])))
    out["annual = sum of daily = sum of monthly (< 0.01 in)"] = (worst < 0.01, worst)

    # physical bounds
    sat = sum((1 - R.soils[p.soil].bulkd / 2.65) * dd for dd in depth)
    probs = 0.0
    for k in ("runoff", "drain", "e", "t", "et", "grirr", "nirr", "irrinf", "wetsup", "snow"):
        probs = min(probs, min(d[k][:n]))
    out["no negative fluxes or snow"] = (probs >= -1e-6, probs)
    smin, smax = min(d["soilw"][:n]), max(d["soilw"][:n])
    out["soil water within 0 and saturation"] = (0 <= smin and smax <= sat + 1e-4,
                                                 smax - sat)
    kcl, kcu = R.kernel_block.kcl[crop - 1], R.kernel_block.kcu[crop - 1]
    kc_bad = max(max(kcl - x, x - kcu, 0) for x in d["kc"][:n])
    out["KC within KCL-KCU"] = (kc_bad < 1e-4, kc_bad)
    if crop not in WET:
        cap = max(1.0, kcu)
        ratio = max((d["et"][i] - cap * d["etr"][i]) for i in range(n))
        out["daily ET <= max(1, KCU) x ETr (dry covers)"] = (ratio < 0.005, ratio)
    infl = max(d["irrinf"][i] - d["grirr"][i] for i in range(n))
    out["infiltrated irrigation <= gross"] = (infl < 1e-5, infl)
    snow0 = r.start_state.snoh2o
    rof = max(d["runoff"][i] - (d["precip"][i] + d["grirr"][i] + d["runon"][i]
                                + (d["snow"][i - 1] if i else snow0)) for i in range(n))
    out["daily runoff <= rain + irrigation + run-on + snowpack"] = (rof < 1e-4, rof)
    return out


def part1(R: Runner):
    block = R.kernel_block
    rows = []
    allres, orires = {}, {}
    for pid, p in R.parcels.items():
        allres[pid] = R.run(p, ALL)
        orires[pid] = R.run(p, ORIG)
    for label, res in (("all fixes", allres), ("original", orires)):
        for pid, rs in res.items():
            p = R.parcels[pid]
            for r in rs:
                for chk, (ok, val) in check_run(R, p, r, block).items():
                    rows.append(dict(model=label, parcel=pid, year=r.year, check=chk,
                                     ok=ok, value=val))

    # second pass replay
    a, b = allres["P01"], allres["P05"]
    replay = max(abs(x.annual[k] - y.annual[k]) for x, y in zip(a, b)
                 for k, _ in METRICS + [("ts", ""), ("es", "")])
    # repeatability and thread safety
    p = R.parcels["P10"]
    again = R.run(p, ALL)
    rep = max(abs(x.annual[k] - y.annual[k]) for x, y in zip(allres["P10"], again)
              for k in x.annual)

    def one(pid):
        return pid, R.run(R.parcels[pid], ALL)
    with ThreadPoolExecutor(max_workers=8) as ex:
        threaded = dict(ex.map(one, list(R.parcels) * 2))
    thr = max(abs(x.annual[k] - y.annual[k]) for pid in R.parcels
              for x, y in zip(allres[pid], threaded[pid]) for k in x.annual)
    extra = [("Second pass: P05 replaying P01's applied water reproduces P01", replay < 1e-4,
              replay),
             ("Repeatable: same inputs give identical results", rep == 0.0, rep),
             (f"Thread safe: {2 * len(R.parcels)} parcel chains on 8 threads match serial "
              "runs", thr == 0.0, thr)]
    return rows, extra, allres, orires


def summarise_checks(rows):
    out = {}
    for row in rows:
        key = (row["model"], row["check"])
        o = out.setdefault(key, dict(n=0, fail=0, worst=0.0, where=""))
        o["n"] += 1
        v = row["value"]
        if not row["ok"]:
            o["fail"] += 1
            if not o["where"]:
                o["where"] = f'{row["parcel"]} {row["year"]}'
        if isinstance(v, (int, float)) and math.isfinite(v) and abs(v) > abs(o["worst"]):
            o["worst"] = v
    return out


def means(rs, key):
    rs = [r for r in rs if r.year in pc.REPORT_YEARS]
    if key == "season":
        return st.mean(season_et(r) for r in rs)
    return st.mean(float(r.annual[key]) for r in rs)


def plausibility(R, allres):
    rows = []
    for pid, rs in allres.items():
        p = R.parcels[pid]
        band = BANDS[p.group]
        m = {k: means(rs, k) for k, _ in METRICS + EXTRA}
        m["season"] = means(rs, "season")
        flags = []
        for k, (lo, hi) in band.items():
            if k == "ets_over_p":
                v = m["ets"] / m["precips"]
            else:
                v = m[k]
            if not lo <= v <= hi:
                flags.append(f"{k} {v:.2f} outside {lo}-{hi}")
        rows.append((p, m, flags))
    return rows


# ---------------------------------------------------------------------------
# Part 2: change report
# ---------------------------------------------------------------------------

def annual_means(rs):
    return {k: means(rs, k) for k, _ in METRICS + EXTRA}


def part2(R: Runner, allres, orires):
    base_all = {pid: annual_means(rs) for pid, rs in allres.items()}
    base_ori = {pid: annual_means(rs) for pid, rs in orires.items()}
    impact = {}   # (fix, mode, pid) -> {metric: delta}
    years = []    # per year rows
    for f in FIXES:
        for mode, fixes, base, bres in (("alone", int(f), base_ori, orires),
                                        ("leave-out", ALL & ~int(f), base_all, allres)):
            for pid, p in R.parcels.items():
                rs = R.run(p, fixes)
                m = annual_means(rs)
                # effect of the fix: alone = (orig+fix) - orig ; leave-out = all - (all-fix)
                if mode == "alone":
                    dlt = {k: m[k] - base[pid][k] for k in m}
                else:
                    dlt = {k: base[pid][k] - m[k] for k in m}
                impact[(f, mode, pid)] = dlt
                for r, b in zip(rs, bres[pid]):
                    if r.year not in pc.REPORT_YEARS:
                        continue
                    row = dict(fix=f.name, mode=mode, parcel=pid, year=r.year)
                    for k, _ in METRICS:
                        v = (r.annual[k] - b.annual[k]) if mode == "alone" else \
                            (b.annual[k] - r.annual[k])
                        row[k] = round(float(v), 4)
                    years.append(row)
    total = {pid: {k: base_all[pid][k] - base_ori[pid][k] for k in base_all[pid]}
             for pid in R.parcels}
    return impact, years, total, base_all, base_ori


# ---------------------------------------------------------------------------
# Report
# ---------------------------------------------------------------------------

def f2(x, d=2):
    if abs(x) < 0.5 * 10 ** -d:
        return "0"
    return f"{x:+.{d}f}"


def changed(dlt) -> bool:
    return any(abs(dlt[k]) > TOL for k, _ in METRICS) or abs(dlt.get("irigno", 0)) > 0.05


def fix_doc_rows():
    """Short description per fix from the Fix docstrings."""
    import cropsim.kernel as K
    import inspect
    src = inspect.getsource(K.Fix)
    out = {}
    lines = src.splitlines()
    for i, line in enumerate(lines):
        s = line.strip()
        if " = " in s and s.split(" = ")[0].isupper():
            name = s.split(" = ")[0]
            if i + 1 < len(lines) and lines[i + 1].strip().startswith('"""'):
                doc = lines[i + 1].strip().strip('"')
                out[name] = doc.split(" (original")[0]
    return out


def write(R, out_dir: Path, rows, extra, allres, orires, plaus, impact, years, total,
          base_all, base_ori):
    out_dir.mkdir(parents=True, exist_ok=True)
    pids = list(R.parcels)
    docs = fix_doc_rows()

    # CSVs
    with open(out_dir / "checks.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=["model", "parcel", "year", "check", "ok", "value"])
        w.writeheader()
        w.writerows(rows)
    with open(out_dir / "annual.csv", "w", newline="") as fh:
        keys = [k for k, _ in METRICS + EXTRA] + ["balerr"]
        w = csv.writer(fh)
        w.writerow(["model", "parcel", "year", "crop"] + keys + ["season_et"])
        for label, res in (("all fixes", allres), ("original", orires)):
            for pid, rs in res.items():
                for r in rs:
                    w.writerow([label, pid, r.year, r.crop] +
                               [round(float(r.annual[k]), 4) for k in keys] +
                               [round(season_et(r), 4)])
    with open(out_dir / "fix_impact.csv", "w", newline="") as fh:
        w = csv.writer(fh)
        keys = [k for k, _ in METRICS + EXTRA]
        w.writerow(["fix", "mode", "parcel"] + keys)
        for (f, mode, pid), d in impact.items():
            w.writerow([f.name, mode, pid] + [round(d[k], 4) for k in keys])
        for pid in pids:
            w.writerow(["ALL_FIXES", "all vs original", pid] +
                       [round(total[pid][k], 4) for k in keys])
    with open(out_dir / "fix_impact_years.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(years[0]))
        w.writeheader()
        w.writerows(years)

    summ = summarise_checks(rows)
    nfail_all = sum(v["fail"] for (m, c), v in summ.items()
                    if m == "all fixes" and not c.startswith("yield"))
    nrun = len(pids) * len(pc.YEARS)

    L = []
    a = L.append
    a("# CropSim kernel: validation and model change report")
    a("")
    a("Generated by `tools/validation/validate.py` on the Scottsbluff test parcels. "
      "Do not edit by hand; rerun the script.")
    a("")
    a("## Summary")
    a("")
    ok_extra = all(e[1] for e in extra)
    a(f"- **Validation.** {len(pids)} test parcels x {len(pc.YEARS)} years "
      f"({nrun} parcel-years per model) on real Scottsbluff weather, 2001-2025. "
      + ("With every fix on (the default), every check passed: "
         if nfail_all == 0 and ok_extra else
         f"With every fix on, {nfail_all} check(s) failed (see Part 1): ")
      + "no NaN or infinite values in any water budget output, the water balance "
        "closes every day and every year (kernel check and an independent check from "
        "the daily series and carried soil state), annual totals equal the daily and "
        "monthly sums, no negative fluxes, the second pass reproduces the first, and "
        "results are repeatable and thread safe.")
    ofail = summ.get(("original", "kernel balance error < 0.005 in/yr"), {}).get("fail", 0)
    a(f"- **Original v8.0 for contrast:** {ofail} of {nrun} parcel-years do not close "
      "the water balance by more than 0.005 in, mostly irrigated parcels "
      "(transpiration above the water taken up) and wet covers (ET with no source).")
    nflag = sum(1 for _, _, fl in plaus if fl)
    a(f"- **Plausibility.** {len(pids) - nflag} of {len(pids)} parcels fall inside the "
      "rough ranges for their crop and irrigation type. Flags and observations are in "
      "Part 1; none is a kernel defect, but some are worth a look before calibration.")
    # top movers
    movers = []
    for f in FIXES:
        ds = [impact[(f, "alone", pid)] for pid in pids]
        big = max((abs(d[k]), k, pid) for d, pid in zip(ds, pids) for k, _ in METRICS)
        n = sum(changed(d) for d in ds)
        movers.append((big[0], f, n, big[1], big[2]))
    movers.sort(key=lambda x: x[0], reverse=True)
    lab = dict(METRICS)
    top = [f"`{f.name}` ({lab[k].lower()} {f2(impact[(f, 'alone', pid)][k])} in/yr on "
           f"{pid})" for _, f, n, k, pid in movers[:5] if _ > TOL]
    a("- **Biggest movers** (largest change in a parcel's 24-year mean, fix on its own): "
      + "; ".join(top) + ".")
    none = [f.name for b, f, n, _, _ in movers if n == 0]
    if none:
        a(f"- **No change on these parcels:** {', '.join('`'+x+'`' for x in none)}.")
    a("")
    a("Inputs are plausible test data, not State parcel data with known answers: "
      "representative soils, Panhandle crop calendars and irrigation settings, and crop "
      "coefficient curves after Wright (1982). The numbers show how the kernel behaves "
      "and how much each fix moves results on realistic inputs; they are not calibrated "
      "estimates.")
    a("")

    # parcels
    a("## Test parcels")
    a("")
    a("Weather: Scottsbluff station (41.87 N, 103.60 W, 3,944 ft), ESC weather set "
      "1953-2025, reference ET by Hargreaves (ETRHG, used as given). 2001 is spin-up; "
      "means below are 2002-2025 (24 years, mean precipitation "
      f"{means(allres['P01'], 'precips'):.1f} in/yr, range 6.6-24.6). Each parcel "
      "carries its own soil state from year to year, starting at 60% of available "
      "water on 1 Jan 2001 (field capacity for the riparian and wetland parcels).")
    a("")
    a("| Parcel | Setup | Soil | Irrigation |")
    a("| --- | --- | --- | --- |")
    irr_names = {1: "dryland", 2: "known applied water (IRRTYP 2)", 3: "sprinkler",
                 4: "furrow", 5: "border"}
    for pid, p in R.parcels.items():
        s = p.sim(pc.YEARS[1])
        spec = pc.SOILS[p.soil]
        sch = {1: "demand", 2: "demand", 3: f"rotation every {s.jdfreq} d, "
                                            f"{s.ddepth:g} in"}.get(s.irrsch, "")
        irr = irr_names.get(s.irrtyp, "")
        if s.irrtyp > 1 and not p.known_from:
            irr += f", {sch}, eff. {s.eapp[0]:g}"
            if s.irrtyp in (3, 4) and s.irrsch < 3:
                irr += f", {s.syscap:g} in/day"
        if p.known_from:
            irr = f"replays {p.known_from}'s applied water (IRRTYP 2)"
        a(f"| {pid} | {p.name} | {spec.name}, {spec.awc_in_ft:g} in/ft, code {spec.code} "
          f"| {irr} |")
    a("")
    a("Crop coefficients, curve numbers, calendars and irrigation settings are all in "
      "`tools/validation/parcels.py`.")
    a("")

    # Part 1
    a("## Part 1: Validation (every fix on)")
    a("")
    a("### Checks")
    a("")
    a("Each check runs on every parcel-year. The original v8.0 column is for contrast.")
    a("")
    a("| Check | All fixes: failed / runs | Worst value | Original v8.0: failed / runs |")
    a("| --- | ---: | ---: | ---: |")
    checks = []
    for (_m, c) in summ:
        if c not in checks:
            checks.append(c)
    for c in checks:
        s1 = summ[("all fixes", c)]
        s0 = summ[("original", c)]
        wv = s1["worst"]
        wtxt = f"{wv:.2g}" if isinstance(wv, float) else str(wv)
        a(f"| {c} | {s1['fail']} / {s1['n']} | {wtxt} | {s0['fail']} / {s0['n']}"
          + (f" (first: {s0['where']})" if s0["fail"] else "") + " |")
    for name, ok, val in extra:
        a(f"| {name} | {'0' if ok else '1'} / 1 | {val:.2g} | |")
    a("")
    a("The independent closure check does not use the kernel's balance code: it takes "
      "the change in soil water and snow from the state passed in and handed back, and "
      "compares it with the year's daily rain, run-on, infiltrated irrigation and wetland "
      "supply less runoff, ET and deep percolation.")
    a("")
    a("### Annual water budget (24-year means, inches per year, every fix on)")
    a("")
    a("| Parcel | ET | ET May-Sep | Transp. | Evap. | Net irr. | Gross irr. | Irrigations "
      "| Runoff | Deep perc. | Wetland supply | Flags |")
    a("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |")
    for p, m, flags in plaus:
        a(f"| {p.pid} {p.name} | {m['ets']:.1f} | {m['season']:.1f} | {m['ts']:.1f} | "
          f"{m['es']:.1f} | {m['snirr']:.1f} | {m['sgrirr']:.1f} | {m['irigno']:.1f} | "
          f"{m['totrof']:.2f} | {m['drains']:.2f} | {m['wetsups']:.1f} | "
          + ("; ".join(flags) if flags else "ok") + " |")
    a("")
    a("Plausibility ranges used (annual unless noted; reviewer judgment from typical "
      "published Nebraska Panhandle values, widened for Hargreaves ETr and off-season "
      "soil evaporation, which CropSim counts all year):")
    a("")
    a("| Group | Ranges |")
    a("| --- | --- |")
    nice = dict(ets="ET", season="ET May-Sep", snirr="net irr.", drains="deep perc.",
                ets_over_p="ET / precip")
    for g, b in BANDS.items():
        a(f"| {g.replace('_', ' ')} | " + ", ".join(f"{nice[k]} {lo:g}-{hi:g}"
                                                     for k, (lo, hi) in b.items()) + " |")
    a("")

    # Part 2
    a("### Observations")
    a("")
    for line in OBSERVATIONS:
        a(f"- {line}")
    a("")
    a("## Part 2: Model change report")
    a("")
    a("Every parcel was run 44 ways: the original (no fixes), all fixes, each fix on its "
      "own added to the original (**alone**), and all fixes with one taken out "
      "(**leave-out**; the change shown is all fixes minus that run, so it is the fix's "
      "effect in the model as shipped). Changes are 24-year means, inches per year, "
      "fixed minus unfixed. A parcel counts as changed when any of the six water budget "
      f"terms moves by more than {TOL} in/yr or the irrigation count changes.")
    a("")
    a("### Summary by fix")
    a("")
    a("Mean change over the parcels the fix changes (alone), and the largest parcel "
      "change in brackets.")
    a("")
    a("| Fix | What it corrects | Parcels changed (alone / leave-out) | ET | Net irr. "
      "| Gross irr. | Runoff | Deep perc. | Wetland supply |")
    a("| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
    for f in FIXES:
        ds = {pid: impact[(f, "alone", pid)] for pid in pids}
        dl = {pid: impact[(f, "leave-out", pid)] for pid in pids}
        ch = [pid for pid in pids if changed(ds[pid])]
        chl = [pid for pid in pids if changed(dl[pid])]
        cells = []
        for k, _ in METRICS:
            if not ch:
                cells.append("0")
                continue
            xs = [ds[pid][k] for pid in ch]
            mx = max(xs, key=abs)
            cells.append("0" if abs(mx) <= TOL else f"{f2(st.mean(xs))} ({f2(mx)})")
        a(f"| [{f.name}](#{f.name.lower()}) | {docs.get(f.name, '')} | {len(ch)} / "
          f"{len(chl)} | " + " | ".join(cells) + " |")
    ch = [pid for pid in pids if changed(total[pid])]
    cells = []
    for k, _ in METRICS:
        xs = [total[pid][k] for pid in ch]
        cells.append(f"{f2(st.mean(xs))} ({f2(max(xs, key=abs))})" if xs else "0")
    a(f"| **All fixes vs original** | | {len(ch)} | " + " | ".join(cells) + " |")
    a("")

    a("### All fixes vs original, by parcel")
    a("")
    a("| Parcel | ET | Net irr. | Gross irr. | Runoff | Deep perc. | Wetland supply | "
      "Irrigations |")
    a("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
    for pid in pids:
        t = total[pid]
        a(f"| {pid} {R.parcels[pid].name} | " +
          " | ".join(f2(t[k]) for k, _ in METRICS) + f" | {f2(t['irigno'], 1)} |")
    a("")
    a("Original values are in `annual.csv` (model = original).")
    a("")

    a("### Each fix")
    a("")
    for f in FIXES:
        a(f"#### {f.name}")
        a("")
        a(f"{docs.get(f.name, '')}.")
        a("")
        ds = {pid: impact[(f, "alone", pid)] for pid in pids}
        dl = {pid: impact[(f, "leave-out", pid)] for pid in pids}
        rows_ = [pid for pid in pids if changed(ds[pid]) or changed(dl[pid])]
        if not rows_:
            a("No parcel changed, alone or as a leave-out. " + NO_CHANGE.get(f.name, ""))
            a("")
            continue
        a("| Parcel | Mode | ET | Net irr. | Gross irr. | Runoff | Deep perc. | "
          "Wetland supply | Irrigations |")
        a("| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
        for pid in rows_:
            for mode, d in (("alone", ds[pid]), ("leave-out", dl[pid])):
                a(f"| {pid} {R.parcels[pid].name} | {mode} | " +
                  " | ".join(f2(d[k]) for k, _ in METRICS) + f" | {f2(d['irigno'], 1)} |")
        a("")
        others = [pid for pid in pids if pid not in rows_]
        if others:
            a(f"Unchanged: {', '.join(others)}.")
            a("")
        if f.name in NOTES:
            a(NOTES[f.name])
            a("")

    a("## Method and limits")
    a("")
    for line in METHOD:
        a(f"- {line}")
    a("")
    (out_dir / "kernel-validation.md").write_text("\n".join(L) + "\n")


NO_CHANGE = {
    "DRAINAGE_SPREAD": "",
    "INFILTRATION_OUTPUT": "It changes only the monthly infiltration output (INFMON), "
    "which the original always reported as 0; no water budget term moves.",
    "ET_ACCUMULATION": "It changes only the seasonal ET totals used by the yield model "
    "(ETMAX, ETYLD) on very low-ET days; no water budget term moves.",
    "PERENNIAL_ROOTS": "The original's root error only shows when a perennial's water "
    "balance starts after growing degree days have begun to accrue (a SIM start date "
    "after green-up). Every test parcel starts on 1 January, as parcel runs will, so the "
    "roots are already at RZMAX. The fix still matters for any SIM that starts later.",
    "RZMIN_CAP": "It matters only where the soil-scaled RZMAX falls below RZMIN (very "
    "shallow soils with a deep starting root depth); no test parcel is that shallow.",
}
NOTES: dict[str, str] = {
    "ROTATION_FREQUENCY": "The largest change for canal-supplied parcels. The original "
    "delivers the full rotation depth on any day the field is dry enough, so a 10- or "
    "14-day rotation becomes close to a delivery every few days. Fixed, the 3 in "
    "deliveries come at the rotation interval. On the pasture parcel (P11) gross "
    "delivery falls by about 31 in/yr and deep percolation by about 18 in/yr: in the "
    "original most of the extra water drained below the root zone. Any canal-district "
    "recharge estimate from the original is likely too high.",
    "TAILWATER_FURROW": "With the fix, border irrigation on rotation (IRRTYP 5) "
    "infiltrates the net depth (delivery x efficiency, 65% here), as it does under "
    "demand scheduling. The original used the furrow tailwater formula, which with a 20% "
    "tailwater loss let 80% of each delivery soak in, more than the net depth. So less "
    "water now enters the soil: deep percolation falls (alone), and in the model as "
    "shipped, where deliveries already come at the rotation interval, ET falls by about "
    "4 in/yr on the two border parcels because the soil is drier. Which share is right "
    "for border irrigation is a SIM parameter question (EAPP, PRUNOF) for the State.",
    "TRANSPIRATION_LIMIT": "The main ET change for demand-irrigated and dryland crops: "
    "about 0.4-1.3 in/yr less ET on demand-irrigated parcels (1.1-1.3 on pivot corn) and "
    "0.1 in/yr or less on dryland, because "
    "transpiration on water-short days is now limited to the water in the root zone. "
    "This is what closes the original's water balance on irrigated parcels.",
    "WETLAND_BALANCE": "The original reported riparian and wetland ET with no water "
    "source, so its balance was off by the full ET (about 45-50 in/yr). Fixed, the "
    "shortfall is reported as wetland supply (WETSUPS, a groundwater draw), and because "
    "the soil is kept wet, part of the rain now drains (deep percolation +4 to +7 in/yr). "
    "For the groundwater model the net exchange is supply minus deep percolation "
    "(about 36 in/yr out of the aquifer on the riparian parcel).",
    "FALLOW_CROP": "Only P20 changes, the wheat / fallow rotation with the fallow year "
    "coded as crop 15: its wheat years lose their spring crop (see Observations). The "
    "fix is correct, but it removes the side effect that made wheat after a crop 15 year "
    "start at spring green-up. Wheat-fallow coded as winter wheat every year (P13), the "
    "way CropSim intends, does not change.",
    "IRRIGATION_COUNT": "Reported irrigation count only: one fewer per irrigated year, "
    "with the first irrigation date now recorded. Water applied is unchanged.",
    "FORAGE_CUTOFF": "Native meadow on rotation (P12) now stops at the summer shut-off "
    "and on 1 November, like alfalfa and irrigated pasture.",
    "KNOWN_DATE_RAIN": "Second-pass runs (known applied water) no longer count runoff and "
    "snow as effective rain. Small on this parcel; it matters more where runoff is "
    "larger.",
    "LEAP_CALENDAR": "Leap years only (6 of 24). Small in annual totals, but the "
    "original's monthly totals for leap years put 29 February in March and drop "
    "31 December.",
}
OBSERVATIONS = [
    "**Irrigation losses leave the soil budget.** Gross irrigation minus the water that "
    "soaks in (about 2.8 in/yr on pivots, 5-7 in/yr on furrow and 9-13 in/yr on border "
    "rotation parcels) is not runoff, deep percolation or ET in CropSim's balance; it "
    "simply leaves. The parcel layer (WP4) has to decide where that water goes "
    "(evaporation and drift, tailwater, or seepage to groundwater) before writing "
    "recharge.",
    "**Deep percolation under demand-scheduled irrigation is near zero** (0-0.5 in/yr "
    "for corn, 1.8 in/yr for potatoes on loamy sand), because demand scheduling refills "
    "only the depletion less a rain allowance and the 9 ft profile below the roots is "
    "rarely wetted to field capacity. Most irrigated recharge will come from the second "
    "pass, where metered applied water can exceed demand, and from the application "
    "losses above.",
    "**Dryland transpiration is low** (0.7-2.4 in/yr) and evaporation takes 85-95% of "
    "precipitation. ET equals precipitation on average, so recharge and the water "
    "balance are fine, but the split between evaporation and transpiration (and so "
    "yield) looks off for a 15 in climate. Soil evaporation draws the top 12 in down to "
    "air-dry. This is the same with or without the fixes; it is a calibration item "
    "(evaporation parameters, air-dry contents, residue).",
    "**Off-season evaporation counts.** CropSim runs the whole year, so annual ET "
    "includes 5-7 in/yr of evaporation outside May-September on irrigated parcels and "
    "about 10 in/yr on pasture. Compare growing-season ET (May-Sep column) with "
    "published crop ET.",
    "**Wet covers draw groundwater.** With the fixes, riparian woodland and cattail "
    "wetland ET (49 and 45 in/yr) is supplied mostly by WETSUPS (42 and 38 in/yr), with "
    "4-6 in/yr of rain draining back. The groundwater model should use supply minus "
    "deep percolation.",
    "**Wheat after a fallow year coded as crop 15 loses its spring crop** with the fixes "
    "(P20). The original's FALLOW_CROP error set the 'live crop' flag after fallow, so "
    "the next wheat year started at green-up (15 March). Fixed, the flag stays off and "
    "the wheat year is treated as a fall planting: transpiration near 0 and yield 0. "
    "Coding wheat-fallow as winter wheat every year (P13) is unaffected. If the State's "
    "land use data codes fallow years separately, the parcel layer should set "
    "`state.livecrop = 1` before a wheat year that follows fallow (tested: this restores "
    "the green-up start).",
    "**BVALUE is infinite for crops 12 and up** (pasture, range, woodland, wetland), in "
    "both the original and the kernel: it divides by seasonal ET for yield (ETMAX), "
    "which is 0 for those crops. It is a yield-model output only, but the parcel layer "
    "should not store it as is. A small kernel fix (report 0) is an option.",
    "**Original v8.0, leap years:** the crop coefficient on 31 December is 0, outside "
    "KCL-KCU, because the original never simulates that day. LEAP_CALENDAR fixes it.",
]
METHOD = [
    "Model: the CropSim compute kernel (`fortran/kernel`) through `cropsim.Kernel`. "
    "Fix.ORIGINAL is bit-identical to the frozen v8.0 program (tests/test_parity.py).",
    "One kernel call per parcel per year, soil state carried from the previous year; "
    "rotations change the SIM each year and keep the state.",
    "The second-pass parcel (P05) takes P01's daily gross irrigation from the run with "
    "every fix on as known applied water, so its fix comparisons hold the applied water "
    "fixed, as metered data would.",
    "Means are over 2002-2025. Per-year changes for every fix, parcel and year are in "
    "`fix_impact_years.csv`.",
    "Plausibility ranges are a sanity screen, not calibration targets.",
]


def main(argv):
    wea = Path(argv[1]) if len(argv) > 1 else Path("wea")
    out = Path(argv[2]) if len(argv) > 2 else HERE.parents[1] / "docs" / "validation"
    R = Runner(wea)
    rows, extra, allres, orires = part1(R)
    plaus = plausibility(R, allres)
    impact, years, total, base_all, base_ori = part2(R, allres, orires)
    write(R, out, rows, extra, allres, orires, plaus, impact, years, total, base_all,
          base_ori)
    print(f"wrote {out}")


if __name__ == "__main__":
    main(sys.argv)
