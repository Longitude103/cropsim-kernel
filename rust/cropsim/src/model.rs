//! Kernel inputs as plain Rust values. Units are the original's: inches,
//! degrees F, lb/ac. These match the Python package's dataclasses
//! (`cropsim.model`) field for field, so either side can fill them from a
//! database, a file or anything else.

use serde::{Deserialize, Serialize};

/// Crop names by crop code - 1.
pub const CROP_NAMES: [&str; 22] = [
    "SM.GRAIN",
    "ED BEANS",
    "SOYBEANS",
    "POTATOES",
    "S. BEETS",
    "SORGHUM",
    "W. WHEAT",
    "CORN",
    "SUNFLOWR",
    "ALFALFA",
    "IRR HAY",
    "IRR PAST",
    "N. RANGE",
    "TURF",
    "FALLOW",
    "R. WOODS",
    "NR. WOOD",
    "CATTAILS",
    "REED/RUSH",
    "SHAL H2O",
    "DEEP H2O",
    "FARMSTED",
];

/// Model parameters from BLOCK.DAT. Per-crop arrays have 22 entries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub etrfact: f32,
    pub nfday: i32,
    pub ytrend: i32,
    pub depth: [f32; 10],
    pub ydens: [f32; 22],
    pub resrat: [f32; 22],
    pub spgrav: [f32; 22],
    pub fragil: [i32; 22],
    pub restyp: [i32; 22],
    /// Crop coefficients `cc[crop - 1][stage][point]`.
    pub cc: [[[f32; 10]; 3]; 22],
    pub kcl: [f32; 22],
    pub kcu: [f32; 22],
    pub tbase: [f32; 22],
    pub tceil: [f32; 22],
    /// SFTMP, SMFMX, SMFMN, SNOCOVMX, SNOCOV1, SNOCOV2, TIMP.
    pub snow: [f32; 7],
    pub cnfallow: [f32; 4],
    /// Curve numbers `cn[crop - 1][hydrologic group A-D]`.
    pub cn: [[f32; 4]; 22],
}

impl Default for Block {
    fn default() -> Self {
        Block {
            etrfact: 1.0,
            nfday: 0,
            ytrend: 0,
            depth: [0.0; 10],
            ydens: [0.0; 22],
            resrat: [0.0; 22],
            spgrav: [0.0; 22],
            fragil: [0; 22],
            restyp: [0; 22],
            cc: [[[0.0; 10]; 3]; 22],
            kcl: [0.0; 22],
            kcu: [0.0; 22],
            tbase: [0.0; 22],
            tceil: [0.0; 22],
            snow: [0.0; 7],
            cnfallow: [0.0; 4],
            cn: [[0.0; 4]; 22],
        }
    }
}

/// One TILLAGE.DAT row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tillage {
    pub code: i32,
    pub factor1: f32,
    pub factor2: f32,
    #[serde(default)]
    pub name: String,
}

/// One crop / irrigation / tillage combination (a .SIM file).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sim {
    pub crop: i32,
    pub layers: i32,
    pub implt: i32,
    pub idplt: i32,
    pub imbg: i32,
    pub idbg: i32,
    pub imend: i32,
    pub idend: i32,
    pub gddveg: f32,
    pub gddflo: f32,
    pub gddripe: f32,
    pub gyform: f32,
    pub gddefc: f32,
    pub gddmat: f32,
    pub ncut: i32,
    pub jdycut: [i32; 5],
    pub rzmin: f32,
    pub rzmax: f32,
    pub tbreak: f32,
    pub gddroot: f32,
    pub rzmgmt: f32,
    pub iyield: i32,
    pub ymax: f32,
    pub ycoeff: f32,
    pub iterrc: i32,
    pub chand: f32,
    pub chanw: f32,
    pub tintrv: f32,
    pub itflag: i32,
    /// (timing code ITILTM, days TILDAY, tillage code TILCOD), at most 20.
    pub tillage_ops: Vec<(i32, i32, i32)>,
    pub irrtyp: i32,
    pub pad: [f32; 5],
    pub rainal: [f32; 5],
    pub syscap: f32,
    pub iper: [f32; 5],
    pub apmin: [f32; 5],
    pub apmax: [f32; 5],
    pub smalli: [f32; 5],
    pub gstart: f32,
    pub gstop: f32,
    pub irrsch: i32,
    pub jfirst: i32,
    pub jstopi: i32,
    pub jgoi: i32,
    pub jdfreq: i32,
    pub ddepth: f32,
    pub eapp: [f32; 5],
    pub ereuse: [f32; 5],
    pub prunof: [f32; 5],
    /// IRRTYP 2 (known irrigation dates): FDFILE name, kept for reference.
    pub fdfile: String,
    pub alpha1: f32,
    pub sim_airdry: [f32; 2],
    pub sim_fieldc: [f32; 10],
    pub sim_pwp: [f32; 10],
    pub sim_bulkd: f32,
    /// (day of year, gross depth in, fraction infiltrated), used when
    /// `irrtyp == 2`; at most 50.
    pub irrigation_events: Vec<(i32, f32, f32)>,
}

impl Default for Sim {
    fn default() -> Self {
        Sim {
            crop: 8,
            layers: 10,
            implt: 0,
            idplt: 0,
            imbg: 1,
            idbg: 1,
            imend: 12,
            idend: 31,
            gddveg: 0.0,
            gddflo: 0.0,
            gddripe: 0.0,
            gyform: 0.0,
            gddefc: 0.0,
            gddmat: 0.0,
            ncut: 0,
            jdycut: [0; 5],
            rzmin: 0.0,
            rzmax: 0.0,
            tbreak: 0.0,
            gddroot: 0.0,
            rzmgmt: 0.0,
            iyield: 0,
            ymax: 0.0,
            ycoeff: 0.0,
            iterrc: 0,
            chand: 0.0,
            chanw: 0.0,
            tintrv: 0.0,
            itflag: 0,
            tillage_ops: Vec::new(),
            irrtyp: 1,
            pad: [0.0; 5],
            rainal: [0.0; 5],
            syscap: 0.0,
            iper: [0.0; 5],
            apmin: [0.0; 5],
            apmax: [0.0; 5],
            smalli: [0.0; 5],
            gstart: 0.0,
            gstop: 0.0,
            irrsch: 1,
            jfirst: 0,
            jstopi: 0,
            jgoi: 0,
            jdfreq: 0,
            ddepth: 0.0,
            eapp: [0.0; 5],
            ereuse: [0.0; 5],
            prunof: [0.0; 5],
            fdfile: String::new(),
            alpha1: 0.0,
            sim_airdry: [0.0; 2],
            sim_fieldc: [0.0; 10],
            sim_pwp: [0.0; 10],
            sim_bulkd: 0.0,
            irrigation_events: Vec::new(),
        }
    }
}

impl Sim {
    /// Whether this setup is irrigated (any IRRTYP other than 1, dryland).
    pub fn irrigated(&self) -> bool {
        self.irrtyp != 1
    }

    /// This setup as a known-irrigation run (IRRTYP 2) with the given
    /// events, taking soil properties from `soil` the way IRRTYP 2 needs.
    pub fn with_known_irrigation(&self, soil: &Soil, events: Vec<(i32, f32, f32)>) -> Sim {
        let mut s = self.clone();
        s.irrtyp = 2;
        s.irrigation_events = events;
        s.alpha1 = 0.0;
        s.sim_airdry = soil.airdry;
        s.sim_fieldc = soil.fieldc;
        s.sim_pwp = soil.pwp;
        s.sim_bulkd = soil.bulkd;
        s
    }
}

/// Soil properties for one soil class (a NESOILS.DAT row).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Soil {
    pub soil: i32,
    pub bulkd: f32,
    pub orgm: f32,
    pub fieldc: [f32; 10],
    pub pwp: [f32; 10],
    pub airdry: [f32; 2],
    pub drncoe: f32,
    pub drnday: f32,
}

/// One year of daily weather starting on `start_month`/`start_day`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Weather {
    pub year: i32,
    pub start_month: i32,
    pub start_day: i32,
    pub lat: f32,
    pub lon: f32,
    pub elev: f32,
    pub tmax: Vec<f32>,
    pub tmin: Vec<f32>,
    pub precip: Vec<f32>,
    /// Pan evaporation; no longer collected, so usually zeros.
    pub epan: Vec<f32>,
    /// Reference ET (alfalfa), inches.
    pub etr: Vec<f32>,
}

impl Weather {
    pub fn days(&self) -> usize {
        self.tmax.len()
    }
}

/// Carryover state for one soil between years (the INITIAL.DAT contents).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub theta: [f32; 10],
    pub oldcrp: i32,
    pub oldres: f32,
    pub livecrop: i32,
    pub snotmp: f32,
    pub snoh2o: f32,
    pub soilt: [f32; 10],
}

impl State {
    /// Area-weighted blend of states, for when a parcel's soil components
    /// change between years. Water contents, residue, snow and soil
    /// temperature are averaged; the crop flags come from the largest part.
    pub fn blend(parts: &[(f64, &State)]) -> Option<State> {
        let total: f64 = parts.iter().map(|(w, _)| *w).sum();
        let (_, big) = parts.iter().max_by(|a, b| a.0.total_cmp(&b.0))?;
        if total <= 0.0 {
            return Some((*big).clone());
        }
        let avg = |f: &dyn Fn(&State) -> f32| -> f32 {
            (parts.iter().map(|(w, s)| w * f(s) as f64).sum::<f64>() / total) as f32
        };
        let mut out = (*big).clone();
        for k in 0..10 {
            out.theta[k] = avg(&|s| s.theta[k]);
            out.soilt[k] = avg(&|s| s.soilt[k]);
        }
        out.oldres = avg(&|s| s.oldres);
        out.snotmp = avg(&|s| s.snotmp);
        out.snoh2o = avg(&|s| s.snoh2o);
        Some(out)
    }
}
