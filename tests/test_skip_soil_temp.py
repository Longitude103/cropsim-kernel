"""skip_soil_temperature: the soil temperature routine never feeds the
water balance, so skipping it must leave every result identical and only
hand the carried-over soil temperature (state.soilt) back unchanged.

Checked on every test parcel, soil and year, with every fix on, with the
original model and with the FAO-56 dual crop coefficient.
"""

from __future__ import annotations

import dataclasses

import pytest

import scenarios as sc
from cropsim import ALL_FIXES, Fix, Kernel

MODES = {
    "fixed": dict(fixes=ALL_FIXES),
    "original": dict(fixes=Fix.ORIGINAL),
    "dual": dict(et_method="fao56_dual"),
}


@pytest.fixture(scope="module")
def inp():
    return sc.inputs()


@pytest.fixture(scope="module")
def kernel(inp):
    return Kernel(inp.block, inp.tillage)


def _same(a, b) -> bool:
    return repr(a) == repr(b)        # NaN-safe exact comparison


@pytest.mark.parametrize("mode", MODES)
@pytest.mark.parametrize("name", sc.scenarios())
def test_skipping_changes_only_soil_temperature(inp, kernel, name, mode):
    sim = sc.scenarios()[name]
    for soil in sc.SOILS:
        full = sc.run_chain(kernel.run_year, sim, inp.soils[soil], inp, **MODES[mode])
        fast = sc.run_chain(kernel.run_year, sim, inp.soils[soil], inp,
                            skip_soil_temperature=True, **MODES[mode])
        soilt_in = inp.initial.soilt
        for a, b in zip(full, fast):
            where = (name, mode, soil, a.year)
            assert a.status == b.status, where
            assert _same(a.annual, b.annual), where
            assert _same(a.monthly, b.monthly), where
            assert _same(a.weekly, b.weekly), where
            assert _same(a.daily, b.daily), where
            assert _same(dataclasses.replace(a.state, soilt=None),
                         dataclasses.replace(b.state, soilt=None)), where
            assert list(b.state.soilt) == list(soilt_in), where
            soilt_in = b.state.soilt


def test_default_computes_soil_temperature(inp, kernel):
    sim = sc.scenarios()["corn_pivot"]
    args = (sim, inp.soils[622], inp.weather[2012], inp.initial)
    assert kernel.run_year(*args).state.soilt != list(inp.initial.soilt)
    assert kernel.run_year(*args, skip_soil_temperature=True).state.soilt == list(inp.initial.soilt)
