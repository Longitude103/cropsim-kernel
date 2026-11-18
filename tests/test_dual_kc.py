"""The FAO-56 dual crop coefficient option (et_method="fao56_dual").

The option replaces CropSim's own split of potential ET into soil
evaporation and transpiration; it is not a bug fix, so the default and the
original (Fix.ORIGINAL) results are unchanged (test_kernel.py). Here:

* every test parcel runs with it, the water balance closes, and nothing
  goes negative or above the reference ET limit;
* the parts behave as FAO-56 describes: no transpiration from fallow,
  evaporation from a wet bare surface at the KCMAX rate falling as the
  surface dries, little evaporation under full cover, and unstressed
  transpiration equal to KCB * ETr;
* an unknown method is bad input.
"""

from __future__ import annotations

import pytest

import scenarios as sc
from cropsim import CropSimError, Kernel

DUAL = "fao56_dual"
KCMAX = 1.0           # alfalfa-reference KCMAX in the kernel
WET = {16, 18, 19, 20, 21}


@pytest.fixture(scope="module")
def inp():
    return sc.inputs()


@pytest.fixture(scope="module")
def kernel(inp):
    return Kernel(inp.block, inp.tillage)


def season(r):
    return range(r.annual["jdybg"] - 1, r.annual["jdyend"])


CASES = [(name, soil) for name in sc.scenarios() for soil in sc.SOILS]


@pytest.mark.parametrize("name,soil", CASES)
def test_every_parcel_runs_and_balances(inp, kernel, name, soil):
    sim = sc.scenarios()[name]
    for r in sc.run_chain(kernel.run_year, sim, inp.soils[soil], inp, et_method=DUAL):
        a, d = r.annual, r.daily
        assert r.status == 0, (name, soil, r.status)
        assert abs(a["balerr"]) < 2e-3, (name, soil, a["balerr"])
        for j in season(r):
            assert d["e"][j] >= 0.0 and d["t"][j] >= -1e-6
            if sim.crop not in WET:
                # Kc = Ks*Kcb + Ke never exceeds max(KCMAX, Kcb + 0.05).
                limit = max(KCMAX, d["kc"][j] + 0.05) * d["etr"][j] + 1e-4
                assert d["e"][j] + d["t"][j] <= limit, (name, soil, j)


def test_original_is_still_the_default(inp, kernel):
    sim = sc.scenarios()["corn_pivot"]
    args = (sim, inp.soils[622], inp.weather[2012], inp.initial)
    base = kernel.run_year(*args)
    assert kernel.run_year(*args, et_method="original").annual == base.annual
    assert kernel.run_year(*args, et_method=DUAL).annual != base.annual


def test_wet_covers_are_unchanged(inp, kernel):
    """Wetlands and open water keep ET = KC * ETr."""
    for name, sim in sc.scenarios().items():
        if sim.crop in WET:
            args = (sim, inp.soils[622], inp.weather[2012], inp.initial)
            assert kernel.run_year(*args, et_method=DUAL).annual == kernel.run_year(*args).annual, name


def test_fallow_is_evaporation_only(inp, kernel):
    fallow = [s for s in sc.scenarios().values() if s.crop == 15][0]
    r = kernel.run_year(fallow, inp.soils[622], inp.weather[2012], inp.initial, et_method=DUAL)
    assert r.annual["ts"] == 0.0
    assert r.annual["es"] > 0.0


def test_bare_surface_dries_like_fao56(inp, kernel):
    """After soaking rain on fallow, E starts near KCMAX * ETr (times the
    residue factor) and falls once the readily evaporable water is gone."""
    fallow = [s for s in sc.scenarios().values() if s.crop == 15][0]
    r = kernel.run_year(fallow, inp.soils[622], inp.weather[2012], inp.initial, et_method=DUAL)
    d = r.daily
    days = list(season(r))
    # A wetting day followed by at least six dry days.
    for i, j in enumerate(days[:-7]):
        if d["eprecip"][j] >= 0.5 and all(d["eprecip"][k] == 0 for k in range(j + 1, j + 7)):
            ratio = [d["e"][k] / d["etr"][k] for k in range(j, j + 7) if d["etr"][k] > 0.05]
            assert ratio[0] <= KCMAX + 1e-6
            assert ratio[-1] < ratio[0]          # drying
            assert all(b <= a + 1e-6 for a, b in zip(ratio, ratio[1:]))
            break
    else:
        pytest.fail("no wetting event followed by a dry spell in the fixture year")


def test_full_cover_is_mostly_transpiration(inp, kernel):
    """Mid-season irrigated corn: E is a small share of ET, and
    unstressed transpiration is KCB * ETr."""
    sim = sc.scenarios()["corn_pivot"]
    r = kernel.run_year(sim, inp.soils[622], inp.weather[2012], inp.initial, et_method=DUAL)
    d = r.daily
    peak = [j for j in season(r) if d["kc"][j] >= 0.95 and d["etr"][j] > 0.1]
    assert len(peak) > 20
    e = sum(d["e"][j] for j in peak)
    et = sum(d["e"][j] + d["t"][j] for j in peak)
    assert e / et < 0.15
    ratio = [d["t"][j] / (d["kc"][j] * d["etr"][j]) for j in peak]
    assert max(ratio) <= 1.0 + 1e-4                    # T never above KCB * ETr
    assert sum(abs(x - 1.0) < 1e-4 for x in ratio) > 20  # and equal to it unstressed


def test_seasonal_totals_are_comparable(inp, kernel):
    """The dual method moves water between E and T but not wildly: the
    six-year ET of the main row crops stays within 25% of the original
    split's."""
    for name in ("corn_pivot", "corn_dry", "soy_dry", "wheat_fallow", "alfalfa_pivot"):
        sim = sc.scenarios()[name]
        a = sc.run_chain(kernel.run_year, sim, inp.soils[622], inp)
        b = sc.run_chain(kernel.run_year, sim, inp.soils[622], inp, et_method=DUAL)
        eta = sum(r.annual["ets"] for r in a)
        etb = sum(r.annual["ets"] for r in b)
        assert abs(etb / eta - 1.0) < 0.25, (name, eta, etb)


def test_unknown_method_is_an_error(inp, kernel):
    sim = sc.scenarios()["corn_pivot"]
    with pytest.raises(ValueError):
        kernel.run_year(sim, inp.soils[622], inp.weather[2012], inp.initial, et_method="pan")
