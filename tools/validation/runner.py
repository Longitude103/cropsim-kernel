"""Run the validation parcels through the kernel, carrying each parcel's soil
state from year to year."""

from __future__ import annotations

from pathlib import Path

from cropsim import ALL_FIXES, Kernel
from cropsim.model import Result

import parcels as pc


class Runner:
    def __init__(self, wea_dir: Path):
        self.weather = pc.weather(wea_dir)
        self.kernel_block = pc.block()
        self.kernel = Kernel(self.kernel_block, pc.tillage())
        self.parcels = {p.pid: p for p in pc.parcels()}
        self.soils = {k: s.soil() for k, s in pc.SOILS.items()}
        self._events: dict[str, dict[int, list]] = {}

    def known_events(self, pid: str) -> dict[int, list]:
        """Applied irrigation of a parcel run with every fix: the 'metered'
        water a second-pass parcel replays, (day, gross in, fraction in)."""
        if pid not in self._events:
            p = self.parcels[pid]
            ev = {}
            for r in self.run(p, int(ALL_FIXES)):
                s = p.sim(r.year)
                eff = s.eapp[0]
                ev[r.year] = [(d + 1, round(g, 3), eff) for d, g in enumerate(r.daily["grirr"])
                              if g > 0][:50]
            self._events[pid] = ev
        return self._events[pid]

    def sim_for(self, p: pc.Parcel, year: int):
        s = p.sim(year)
        if p.known_from:
            s = s.copy()
            soil = self.soils[p.soil]
            s.irrtyp = 2
            s.irrigation_events = self.known_events(p.known_from)[year]
            s.alpha1 = 0.0
            s.sim_airdry = list(soil.airdry)
            s.sim_fieldc = list(soil.fieldc)
            s.sim_pwp = list(soil.pwp)
            s.sim_bulkd = soil.bulkd
        return s

    def run(self, p: pc.Parcel, fixes: int) -> list[Result]:
        soil = self.soils[p.soil]
        state = pc.initial_state(p, soil)
        out = []
        for y in pc.YEARS:
            r = self.kernel.run_year(self.sim_for(p, y), soil, self.weather[y], state,
                                     fixes=fixes, raise_on_error=False)
            r.start_state = state
            state = r.state
            out.append(r)
        return out
