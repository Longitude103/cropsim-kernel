"""Test parcels for kernel validation: Nebraska Panhandle crops, soils and
irrigation, run on real Scottsbluff weather.

There are no State parcel data sets with known answers, so these parcels
are *plausible*, not observed: representative soil profiles (not mapped
SSURGO components), crop calendars and irrigation settings typical of the
North Platte valley around Scottsbluff, and crop coefficient curves shaped
after Wright (1982) where CropSim reads them from BLOCK.DAT. Every value
is set here, in one place, so it can be reviewed and changed.

Weather: Scottsbluff (SCTB) station files from the ESC weather set
(1953-2025), reference ET by Hargreaves (ETRHG). 2001 is a spin-up year;
2002-2025 are reported.
"""

from __future__ import annotations

import copy
from dataclasses import dataclass, field
from pathlib import Path

from cropsim import legacy
from cropsim.model import Block, Sim, Soil, State

ROOT = Path(__file__).resolve().parents[2]
FIX = ROOT / "tests" / "fixtures"

SPINUP = 2001
YEARS = tuple(range(2001, 2026))
REPORT_YEARS = tuple(y for y in YEARS if y > SPINUP)
STATION = "SCTB"


# ---------------------------------------------------------------------------
# Model parameters (BLOCK.DAT)
# ---------------------------------------------------------------------------

def _curve(*pts: float) -> list[float]:
    assert len(pts) == 10
    return list(pts)


# Crop coefficient curves (alfalfa reference) for the crops whose curves
# come from BLOCK.DAT: crops 1-7 by tenths of the time from planting to
# effective cover (stage 1) and by tens of days after it (stage 2); alfalfa
# by tenths of each cutting cycle (1 first cycle, 2 later cycles, 3 regrowth
# after the last cutting). Shapes follow Wright (1982); corn, sunflower and
# crops 11-22 use curves built into the model.
KC = {
    1: (_curve(.25, .30, .38, .48, .60, .72, .84, .95, 1.0, 1.0),     # spring small grain
        _curve(1.0, 1.0, 1.0, 1.0, .95, .85, .65, .40, .20, .15)),
    2: (_curve(.20, .22, .27, .35, .45, .56, .68, .80, .90, .95),     # dry beans
        _curve(.95, .95, .93, .85, .65, .40, .20, .10, .10, .10)),
    3: (_curve(.20, .22, .27, .35, .45, .58, .72, .85, .95, 1.0),     # soybeans
        _curve(1.0, 1.0, 1.0, .98, .90, .75, .50, .30, .15, .10)),
    4: (_curve(.15, .18, .24, .32, .42, .52, .62, .70, .76, .80),     # potatoes
        _curve(.80, .80, .80, .80, .75, .70, .60, .45, .30, .20)),
    5: (_curve(.15, .18, .24, .32, .42, .54, .66, .80, .92, 1.0),     # sugar beets
        _curve(1.0, 1.0, 1.0, 1.0, 1.0, .98, .95, .92, .90, .85)),
    6: (_curve(.15, .18, .24, .32, .42, .54, .66, .78, .88, .95),     # sorghum
        _curve(.95, .95, .93, .88, .80, .65, .45, .30, .20, .15)),
    7: (_curve(.25, .30, .38, .48, .60, .72, .84, .95, 1.0, 1.0),     # winter wheat
        _curve(1.0, 1.0, .98, .90, .70, .40, .20, .15, .15, .15)),
}
KC_ALFALFA = (
    _curve(.30, .40, .50, .60, .70, .80, .90, .95, 1.0, 1.0),        # green-up to cut 1
    _curve(.40, .55, .70, .85, .95, 1.0, 1.0, 1.0, 1.0, 1.0),        # between cuttings
    _curve(.40, .55, .70, .80, .85, .85, .80, .70, .55, .40),        # after last cutting
)

# Lower and upper KC by crop 1-22.
KCL = [.20, .15, .15, .15, .15, .15, .20, .15, .15, .25, .25, .25, .20, .50, .15,
       .30, .30, .30, .30, 1.05, 1.05, .15]
KCU = [1.0, .95, 1.0, .80, 1.05, .95, 1.0, 1.0, 1.0, 1.0, .95, .90, .80, .85, .20,
       1.0, .90, 1.1, 1.1, 1.1, 1.1, .30]

# SCS curve numbers by hydrologic group A-D (TR-55, average condition).
_ROW = [67, 78, 85, 89]          # row crops, straight row, good
_GRAIN = [63, 75, 83, 87]        # small grain
_LEGUME = [58, 72, 81, 85]       # close-seeded legumes (alfalfa)
_PASTURE = [39, 61, 74, 80]      # pasture, good
_RANGE = [49, 69, 79, 84]        # range, fair
_WOODS = [36, 60, 73, 79]        # woods, fair
_WET = [78, 85, 90, 92]          # wetland / near-saturated
_FARM = [59, 74, 82, 86]         # farmstead
CN = ([_GRAIN, _ROW, _ROW, _ROW, _ROW, _ROW, _GRAIN, _ROW, _ROW, _LEGUME, _PASTURE,
       _PASTURE, _RANGE, _PASTURE, [77, 86, 91, 94], _WOODS, _WOODS, _WET, _WET,
       [98] * 4, [98] * 4, _FARM])


def block() -> Block:
    """BLOCK.DAT for the test parcels: the repo fixture with realistic crop
    coefficient curves, KC bounds and curve numbers, and ETr used as given."""
    b = legacy.read_block(FIX / "BLOCK.DAT")
    b = copy.deepcopy(b)
    b.etrfact = 1.0  # ETRHG is used as given (no field-condition reduction)
    for crop, (s1, s2) in KC.items():
        b.cc[crop - 1][0] = list(s1)
        b.cc[crop - 1][1] = list(s2)
    for stage in range(3):
        b.cc[9][stage] = list(KC_ALFALFA[stage])
    b.kcl = list(KCL)
    b.kcu = list(KCU)
    b.cn = [list(map(float, c)) for c in CN]
    return b


def tillage():
    return legacy.read_tillage(FIX / "TILLAGE.DAT")


# ---------------------------------------------------------------------------
# Soils
# ---------------------------------------------------------------------------
# Code: hundreds = water holding class (about 4 x available water, in/ft),
# tens = hydrologic group (1 A ... 4 D). Field capacity and wilting point
# are volumetric, by layer (BLOCK depths 4, 8, then 12 in).

@dataclass
class SoilSpec:
    code: int
    name: str
    bulkd: float
    orgm: float
    fc_top: float
    fc_sub: float
    pwp_top: float
    pwp_sub: float

    def soil(self) -> Soil:
        fc = [self.fc_top, self.fc_top] + [self.fc_sub] * 8
        pwp = [self.pwp_top, self.pwp_top] + [self.pwp_sub] * 8
        return Soil(soil=self.code, bulkd=self.bulkd, orgm=self.orgm, fieldc=fc, pwp=pwp,
                    airdry=[round(0.5 * self.pwp_top, 3), round(0.7 * self.pwp_top, 3)])

    @property
    def awc_in_ft(self) -> float:
        return round(12 * (self.fc_sub - self.pwp_sub), 2)


SOILS = {
    "fine_sand": SoilSpec(312, "Fine sand (Valentine-like), group A", 1.60, 0.5,
                          .11, .10, .045, .040),
    "loamy_sand": SoilSpec(411, "Loamy fine sand (Dailey/Valent-like), group A", 1.55, 0.8,
                           .16, .15, .07, .067),
    "fine_sandy_loam": SoilSpec(621, "Fine sandy loam (Tripp/Bayard-like), group B", 1.45,
                                1.2, .23, .22, .10, .095),
    "silt_loam": SoilSpec(821, "Very fine sandy loam to silt loam (Mitchell/Keith-like), "
                               "group B", 1.35, 1.5, .29, .28, .12, .113),
    "clay_loam": SoilSpec(731, "Silty clay loam bottomland, group C", 1.40, 2.0,
                          .36, .35, .21, .204),
    "wet_loam": SoilSpec(741, "Loam bottomland, poorly drained, group D", 1.40, 2.5,
                         .36, .35, .21, .204),
}


# ---------------------------------------------------------------------------
# Crop and irrigation setups (SIM files)
# ---------------------------------------------------------------------------

def _base(crop: int) -> Sim:
    """A dryland SIM for the crop, from the fixtures, with the calendar set below."""
    if crop == 7:
        return legacy.read_sim(FIX / "sim" / "WWTDRY01.SIM")
    if crop >= 10:
        s = legacy.read_sim(FIX / "sim" / "NRANGE01.SIM")
        s.crop = crop
        return s
    s = legacy.read_sim(FIX / "sim" / "CRNDRY01.SIM")
    s.crop = crop
    return s


# Planting (green-up for perennials), GDD to effective cover and maturity
# (base and ceiling from BLOCK.DAT), roots. GDD targets were checked against
# Scottsbluff 2001-2025: e.g. corn planted May 5 averages 2,580 GDD (50/86 F)
# to Sep 25.
CALENDAR = {
    # crop: (month, day, gddveg, gddflo, gddripe, gyform, gddefc, gddmat, rzmin, rzmax, rzmgmt)
    8: (5, 5, 700, 1350, 1950, 1300, 1200, 2450, 4, 48, 36),     # corn
    2: (6, 1, 500, 900, 1500, 1000, 950, 1750, 4, 36, 24),       # dry beans
    3: (5, 15, 600, 1100, 1800, 1200, 1100, 2300, 4, 48, 36),    # soybeans
    4: (4, 25, 500, 1000, 2000, 1200, 1000, 2700, 4, 24, 18),    # potatoes
    5: (4, 15, 800, 1800, 3500, 2000, 1700, 4100, 4, 60, 36),    # sugar beets
    9: (6, 1, 600, 1100, 1700, 1200, 1000, 2100, 4, 60, 48),     # sunflower
}
# Perennials: green-up, GDD to cover and "maturity" (end of FAO curve).
PERENNIAL = {
    10: (4, 1, 0, 0, 6, 72, 48),        # alfalfa
    12: (4, 1, 600, 4300, 4, 36, 24),   # irrigated pasture
    13: (4, 1, 600, 4300, 4, 60, 36),   # native range / meadow
    16: (4, 1, 600, 4300, 6, 96, 48),   # riparian woodland
    18: (4, 1, 600, 4300, 4, 36, 24),   # cattail wetland
}


def crop_sim(crop: int) -> Sim:
    s = _base(crop)
    if crop in CALENDAR:
        (s.implt, s.idplt, s.gddveg, s.gddflo, s.gddripe, s.gyform, s.gddefc, s.gddmat,
         s.rzmin, s.rzmax, s.rzmgmt) = CALENDAR[crop]
    elif crop in PERENNIAL:
        s.implt, s.idplt, s.gddefc, s.gddmat, s.rzmin, s.rzmax, s.rzmgmt = PERENNIAL[crop]
    if crop == 10:
        s.ncut = 3
        s.jdycut = [158, 198, 240, 0, 0]
        s.gddefc, s.gddmat = 0.0, 0.0
    return s


def pivot(s: Sim, *, gstart: float, gstop: float, syscap: float = 0.28,
          gross: float = 1.0, eff: float = 0.85) -> Sim:
    """Center pivot, demand scheduling (management allowed depletion)."""
    s.irrtyp, s.irrsch = 3, 1
    s.pad = [0.5] * 5
    s.rainal = [0.25] * 5
    s.syscap = syscap
    s.iper = [0.9] * 5
    s.apmin = [0.75] * 5
    s.apmax = [gross] * 5
    s.smalli = [0.5] * 5
    s.gstart, s.gstop = gstart, gstop
    s.eapp = [eff] * 5
    s.ereuse = [0.0] * 5
    s.prunof = [0.0] * 5
    return s


def furrow_demand(s: Sim, *, gstart: float, gstop: float) -> Sim:
    """Gravity furrow from a well, demand scheduling, no tailwater reuse."""
    s.irrtyp, s.irrsch = 4, 1
    s.pad = [0.5] * 5
    s.rainal = [0.25] * 5
    s.syscap = 0.30
    s.iper = [0.9] * 5
    s.apmin = [2.5] * 5
    s.apmax = [3.5] * 5
    s.smalli = [1.5] * 5
    s.gstart, s.gstop = gstart, gstop
    s.eapp = [0.65] * 5
    s.ereuse = [0.0] * 5
    s.prunof = [0.25] * 5
    return s


def canal_rotation(s: Sim, *, irrtyp: int, first: int, every: int, depth: float,
                   eff: float, stop: int = 0, go: int = 0, prunof: float = 0.25) -> Sim:
    """Canal district rotation delivery (IRRSCH 3): a fixed DDEPTH every JDFREQ days
    once the field is dry enough to take it."""
    s.irrtyp, s.irrsch = irrtyp, 3
    s.pad = [0.5] * 5
    s.rainal = [0.25] * 5
    s.syscap = 0.5
    s.iper = [1.0] * 5
    s.apmin = [depth] * 5
    s.apmax = [depth] * 5
    s.smalli = [1.0] * 5
    s.gstart, s.gstop = 0.0, 9999.0
    s.jfirst, s.jstopi, s.jgoi, s.jdfreq, s.ddepth = first, stop, go, every, depth
    s.eapp = [eff] * 5
    s.ereuse = [0.0] * 5
    s.prunof = [prunof] * 5
    return s


# ---------------------------------------------------------------------------
# Parcels
# ---------------------------------------------------------------------------

@dataclass
class Parcel:
    pid: str
    name: str
    soil: str
    sims: list[Sim]
    """One SIM per year in rotation order (a single SIM repeats every year)."""
    group: str
    """Expectation group for the plausibility checks."""
    known_from: str | None = None
    """For a second-pass parcel: the parcel whose applied irrigation it replays."""
    wet_start: bool = False
    notes: str = ""
    sims_by_year: dict = field(default_factory=dict)

    def sim(self, year: int) -> Sim:
        if year in self.sims_by_year:
            return self.sims_by_year[year]
        return self.sims[(year - YEARS[0]) % len(self.sims)]


def parcels() -> list[Parcel]:
    corn_gdd = dict(gstart=700.0, gstop=2250.0)
    alf = crop_sim(10)
    alf = pivot(alf, gstart=300.0, gstop=4000.0)
    # Alfalfa: tighter management after the first cutting (stage = cutting).
    alf.pad = [0.5, 0.55, 0.55, 0.6, 0.6]
    alf.apmax = [1.0, 0.9, 0.9, 0.8, 0.8]
    alf_dry = crop_sim(10)
    alf_dry.ncut = 2
    alf_dry.jdycut = [165, 225, 0, 0, 0]

    rot = [pivot(crop_sim(8), **corn_gdd),
           pivot(crop_sim(2), gstart=400.0, gstop=1500.0),
           pivot(crop_sim(5), gstart=700.0, gstop=3800.0)]

    out = [
        Parcel("P01", "Corn, center pivot", "fine_sandy_loam",
               [pivot(crop_sim(8), **corn_gdd)], "irrigated_corn"),
        Parcel("P02", "Corn, center pivot, sandy", "loamy_sand",
               [pivot(crop_sim(8), **corn_gdd)], "irrigated_corn"),
        Parcel("P03", "Corn, gravity furrow from a well (demand)", "silt_loam",
               [furrow_demand(crop_sim(8), **corn_gdd)], "irrigated_corn_surface"),
        Parcel("P04", "Corn, furrow, canal rotation (10-day, 3 in)", "silt_loam",
               [canal_rotation(crop_sim(8), irrtyp=4, first=165, every=10, depth=3.0,
                               eff=0.6)], "irrigated_corn_surface"),
        Parcel("P05", "Corn, known applied water (second pass of P01)", "fine_sandy_loam",
               [pivot(crop_sim(8), **corn_gdd)], "irrigated_corn", known_from="P01"),
        Parcel("P06", "Dry beans, center pivot", "silt_loam",
               [pivot(crop_sim(2), gstart=400.0, gstop=1500.0)], "irrigated_beans"),
        Parcel("P07", "Sugar beets, gravity furrow (demand)", "clay_loam",
               [furrow_demand(crop_sim(5), gstart=700.0, gstop=3800.0)], "irrigated_beets"),
        Parcel("P08", "Potatoes, center pivot", "loamy_sand",
               [pivot(crop_sim(4), gstart=200.0, gstop=2500.0, gross=0.8)],
               "irrigated_potato"),
        Parcel("P09", "Corn / dry beans / sugar beets rotation, pivot", "fine_sandy_loam",
               rot, "irrigated_rotation"),
        Parcel("P10", "Alfalfa, center pivot, 3 cuttings", "fine_sandy_loam", [alf],
               "irrigated_alfalfa"),
        Parcel("P11", "Irrigated pasture, border, canal rotation (14-day, 3 in)",
               "clay_loam",
               [canal_rotation(crop_sim(12), irrtyp=5, first=130, every=14, depth=3.0,
                               eff=0.65, prunof=0.2)], "irrigated_pasture"),
        Parcel("P12", "Wet meadow (native), canal rotation with summer shut-off",
               "clay_loam",
               [canal_rotation(crop_sim(13), irrtyp=5, first=120, every=14, depth=2.5,
                               eff=0.65, stop=200, go=240, prunof=0.2)],
               "irrigated_pasture"),
        Parcel("P13", "Winter wheat - fallow, dryland", "silt_loam", [crop_sim(7)],
               "dryland"),
        Parcel("P14", "Corn, dryland", "silt_loam", [crop_sim(8)], "dryland"),
        Parcel("P15", "Sunflower, dryland", "silt_loam", [crop_sim(9)], "dryland"),
        Parcel("P16", "Alfalfa, dryland, 2 cuttings", "fine_sandy_loam", [alf_dry],
               "dryland_perennial"),
        Parcel("P17", "Native range, sands", "fine_sand", [crop_sim(13)],
               "dryland_perennial"),
        Parcel("P18", "Riparian woodland (North Platte River)", "wet_loam",
               [crop_sim(16)], "riparian", wet_start=True),
        Parcel("P19", "Cattail wetland", "wet_loam", [crop_sim(18)], "wetland",
               wet_start=True),
        Parcel("P20", "Winter wheat / summer fallow, fallow year coded as crop 15",
               "silt_loam", [crop_sim(7), crop_sim(15)], "dryland"),
    ]
    return out


def initial_state(p: Parcel, soil: Soil) -> State:
    """Profile at 60% of available water on Jan 1 of the spin-up year
    (field capacity for wet sites)."""
    frac = 1.0 if p.wet_start else 0.6
    theta = [w + frac * (f - w) for f, w in zip(soil.fieldc, soil.pwp)]
    first = p.sim(YEARS[0])
    return State(theta=theta, oldcrp=first.crop, oldres=1500.0 if first.crop <= 9 else 0.0,
                 livecrop=0)


def weather(wea_dir: Path) -> dict:
    return {y: legacy.read_weather(wea_dir / f"{STATION}{y}.WEA") for y in YEARS}
