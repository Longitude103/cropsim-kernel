"""Change table for the FAO-56 dual crop coefficient option: every test
parcel (tests/scenarios.py, synthetic fixtures) on the three fixture soils
for six years, with the original E/T split and with et_method="fao56_dual".

    python tools/dual_kc_report.py > table.md

Prints a Markdown table of six-year mean annual inches per scenario.
Wetlands and open water are left out (the option does not change them).
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tests"))

import scenarios as sc  # noqa: E402
from cropsim import Kernel  # noqa: E402

WET = {16, 18, 19, 20, 21}
KEYS = [("es", "E"), ("ts", "T"), ("ets", "ET"), ("sgrirr", "Gross irr."),
        ("drains", "DP"), ("totrof", "Runoff")]


def mean(kernel, inp, sim, method):
    tot = {k: 0.0 for k, _ in KEYS}
    n = 0
    for soil in sc.SOILS:
        for r in sc.run_chain(kernel.run_year, sim, inp.soils[soil], inp, et_method=method):
            for k, _ in KEYS:
                tot[k] += r.annual[k]
            n += 1
    return {k: v / n for k, v in tot.items()}


def main():
    inp = sc.inputs()
    kernel = Kernel(inp.block, inp.tillage)
    head = " | ".join(f"{lab} orig | {lab} dual" for _, lab in KEYS)
    print(f"| Scenario | {head} |")
    print("|---|" + "---:|" * (2 * len(KEYS)))
    for name, sim in sc.scenarios().items():
        if sim.crop in WET:
            continue
        a = mean(kernel, inp, sim, "original")
        b = mean(kernel, inp, sim, "fao56_dual")
        cells = " | ".join(f"{a[k]:.1f} | {b[k]:.1f}" for k, _ in KEYS)
        print(f"| {name} | {cells} |")


if __name__ == "__main__":
    main()
