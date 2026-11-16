"""Write the kernel benchmark cases: every test parcel run, as raw C structs.

The cases are the kernel test scenarios (tests/scenarios.py): 36 setups x 3
soils x 6 years, each run three ways (every fix on, Fix.ORIGINAL, and the
FAO-56 dual crop coefficient), 1,944 runs. Each year starts from the state
the year before ended with, as the parcel layer runs them.

    python tools/kernel_bench/dump_cases.py cases.bin [--skip-soil-temperature]

With --skip-soil-temperature every case sets csk_options_t.skip_soil_temp.

File layout: int32 case count, csk_params_t, then per case csk_sim_t,
csk_soil_t, csk_weather_t, csk_state_t (state in) and csk_options_t.
"""

from __future__ import annotations

import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tests"))

import scenarios as sc  # noqa: E402
from cropsim import ALL_FIXES, Engine, Fix, Kernel, _lib  # noqa: E402
from cropsim.kernel import REVERT_ALL  # noqa: E402

MODES = {
    "fixed": dict(fixes=ALL_FIXES, et_method="original"),
    "original": dict(fixes=Fix.ORIGINAL, et_method="original"),
    "dual": dict(fixes=ALL_FIXES, et_method="fao56_dual"),
}


def main(out_path: str, skip_soil_temp: bool = False) -> None:
    inp = sc.inputs()
    kernel = Kernel(inp.block, inp.tillage)
    cases = []
    for mode in MODES.values():
        for sim in sc.scenarios().values():
            for soil in sc.SOILS:
                state = inp.initial.copy()
                for year in sc.YEARS:
                    opt = _lib.OptionsT(revert=~int(mode["fixes"]) & REVERT_ALL,
                                        et_method=1 if mode["et_method"] == "fao56_dual" else 0,
                                        skip_soil_temp=int(skip_soil_temp))
                    cases.append(b"".join([
                        bytes(Engine._sim_struct(sim)),
                        bytes(Engine._soil_struct(inp.soils[soil])),
                        bytes(Engine._weather_struct(inp.weather[year])),
                        bytes(Engine._state_struct(state)),
                        bytes(opt)]))
                    r = kernel.run_year(sim, inp.soils[soil], inp.weather[year], state,
                                        raise_on_error=False, **mode)
                    state = r.state
    with open(out_path, "wb") as f:
        f.write(struct.pack("i", len(cases)))
        f.write(bytes(kernel.params))
        for c in cases:
            f.write(c)
    print(f"{len(cases)} cases written to {out_path}")


if __name__ == "__main__":
    main(sys.argv[1], "--skip-soil-temperature" in sys.argv[2:])
