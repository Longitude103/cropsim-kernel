//! [`Kernel`]: runs one year of one crop setup on one soil.

use cropsim_sys as sys;
use serde::Serialize;

use crate::model::{Block, Sim, Soil, State, Tillage, Weather};
use crate::{Error, Fix};

/// Options for one run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, serde::Deserialize)]
#[serde(default)]
pub struct RunOptions {
    /// Fixes to apply; all by default.
    pub fixes: Fix,
    /// The original LIVECOND rule (same SIM, earlier year).
    pub livecond: bool,
    /// Round the returned state like INITIAL.DAT, to match old runs.
    pub legacy_rounding: bool,
}

/// The kernel with its parameters (BLOCK.DAT and TILLAGE.DAT). Every run is
/// independent, and a `Kernel` can be shared across threads.
pub struct Kernel {
    params: Box<sys::csk_params_t>,
}

// The params struct is plain data, only read by the kernel.
unsafe impl Send for Kernel {}
unsafe impl Sync for Kernel {}

impl Kernel {
    /// Kernel with the snow melt base temperature at 32 F (used by the
    /// SNOWMELT_BASE fix).
    pub fn new(block: &Block, tillage: &[Tillage]) -> Result<Kernel, Error> {
        Kernel::with_smtmp(block, tillage, 32.0)
    }

    pub fn with_smtmp(block: &Block, tillage: &[Tillage], smtmp: f32) -> Result<Kernel, Error> {
        let mut p = sys::zeroed_box::<sys::csk_params_t>();
        p.etrfact = block.etrfact;
        p.nfday = block.nfday;
        p.ytrend = block.ytrend;
        p.depth = block.depth;
        p.ydens = block.ydens;
        p.resrat = block.resrat;
        p.spgrav = block.spgrav;
        p.fragil = block.fragil;
        p.restyp = block.restyp;
        p.kcl = block.kcl;
        p.kcu = block.kcu;
        p.tbase = block.tbase;
        p.tceil = block.tceil;
        p.cnfallow = block.cnfallow;
        for crop in 0..22 {
            for stage in 0..3 {
                for pt in 0..10 {
                    p.cc[pt][stage][crop] = block.cc[crop][stage][pt];
                }
            }
            for g in 0..4 {
                p.cn[g][crop] = block.cn[crop][g];
            }
        }
        [
            p.sftmp, p.smfmx, p.smfmn, p.snocovmx, p.snocov1, p.snocov2, p.timp,
        ] = block.snow;
        for row in tillage {
            if !(1..=80).contains(&row.code) {
                return Err(Error::Input(format!(
                    "tillage code {} is outside 1-80",
                    row.code
                )));
            }
            p.tilfac[0][row.code as usize - 1] = row.factor1;
            p.tilfac[1][row.code as usize - 1] = row.factor2;
        }
        p.smtmp = smtmp;
        Ok(Kernel { params: p })
    }

    /// Run one year. A non-zero kernel status is returned as
    /// [`Error::Kernel`]; use [`Kernel::run_year_status`] to keep the
    /// result of a failed run.
    pub fn run_year(
        &self,
        sim: &Sim,
        soil: &Soil,
        weather: &Weather,
        state: &State,
        opts: &RunOptions,
    ) -> Result<YearResult, Error> {
        let r = self.run_year_status(sim, soil, weather, state, opts)?;
        match r.status {
            0 => Ok(r),
            s => Err(Error::Kernel { status: s }),
        }
    }

    /// Run one year and return the result whatever the kernel status.
    /// Errors only for inputs that cannot be passed to the kernel.
    pub fn run_year_status(
        &self,
        sim: &Sim,
        soil: &Soil,
        weather: &Weather,
        state: &State,
        opts: &RunOptions,
    ) -> Result<YearResult, Error> {
        let s = sim_struct(sim)?;
        let so = soil_struct(soil);
        let w = weather_struct(weather)?;
        let st = state_struct(state);
        let o = sys::csk_options_t {
            revert: opts.fixes.revert_bits(),
            livecond: opts.livecond as i32,
            legacy_round: opts.legacy_rounding as i32,
            reserved: 0,
        };
        let mut out_state = st;
        let mut raw = sys::zeroed_box::<sys::csk_result_t>();
        // SAFETY: every pointer is to a live, properly laid out struct
        // (checked against csk_struct_size in cropsim-sys); the kernel only
        // reads the inputs and writes the two outputs.
        let status = unsafe {
            sys::csk_run_year(
                &*self.params,
                &*s,
                &so,
                &*w,
                &st,
                &o,
                &mut out_state,
                &mut *raw,
            )
        };
        Ok(YearResult {
            year: weather.year,
            crop: sim.crop,
            status,
            ndays: weather.days(),
            state: from_state(&out_state),
            raw,
        })
    }
}

fn sim_struct(s: &Sim) -> Result<Box<sys::csk_sim_t>, Error> {
    if s.tillage_ops.len() > 20 {
        return Err(Error::Input("at most 20 tillage operations".into()));
    }
    if s.irrigation_events.len() > 50 {
        return Err(Error::Input("at most 50 irrigation events".into()));
    }
    let mut t = sys::zeroed_box::<sys::csk_sim_t>();
    t.crop = s.crop;
    t.layers = s.layers;
    t.implt = s.implt;
    t.idplt = s.idplt;
    t.imbg = s.imbg;
    t.idbg = s.idbg;
    t.imend = s.imend;
    t.idend = s.idend;
    t.gddveg = s.gddveg;
    t.gddflo = s.gddflo;
    t.gddripe = s.gddripe;
    t.gyform = s.gyform;
    t.gddefc = s.gddefc;
    t.gddmat = s.gddmat;
    t.ncut = s.ncut;
    t.jdycut = s.jdycut;
    t.rzmin = s.rzmin;
    t.rzmax = s.rzmax;
    t.tbreak = s.tbreak;
    t.gddroot = s.gddroot;
    t.rzmgmt = s.rzmgmt;
    t.iyield = s.iyield;
    t.ymax = s.ymax;
    t.ycoeff = s.ycoeff;
    t.iterrc = s.iterrc;
    t.chand = s.chand;
    t.chanw = s.chanw;
    t.tintrv = s.tintrv;
    t.itflag = s.itflag;
    t.ntills = s.tillage_ops.len() as i32;
    for (k, &(tm, day, code)) in s.tillage_ops.iter().enumerate() {
        t.itiltm[k] = tm;
        t.tilday[k] = day;
        t.tilcod[k] = code;
    }
    t.irrtyp = s.irrtyp;
    t.pad = s.pad;
    t.rainal = s.rainal;
    t.syscap = s.syscap;
    t.iper = s.iper;
    t.apmin = s.apmin;
    t.apmax = s.apmax;
    t.smalli = s.smalli;
    t.gstart = s.gstart;
    t.gstop = s.gstop;
    t.irrsch = s.irrsch;
    t.jfirst = s.jfirst;
    t.jstopi = s.jstopi;
    t.jgoi = s.jgoi;
    t.jdfreq = s.jdfreq;
    t.ddepth = s.ddepth;
    t.eapp = s.eapp;
    t.ereuse = s.ereuse;
    t.prunof = s.prunof;
    t.alpha1 = s.alpha1;
    t.airdry = s.sim_airdry;
    t.fieldc = s.sim_fieldc;
    t.pwp = s.sim_pwp;
    t.bulkd = s.sim_bulkd;
    t.kirr = s.irrigation_events.len() as i32;
    for (k, &(doy, gross, frac)) in s.irrigation_events.iter().enumerate() {
        t.fdirrig[k] = [doy as f32, gross, frac];
    }
    Ok(t)
}

fn soil_struct(s: &Soil) -> sys::csk_soil_t {
    sys::csk_soil_t {
        soil: s.soil,
        bulkd: s.bulkd,
        orgm: s.orgm,
        fieldc: s.fieldc,
        pwp: s.pwp,
        airdry: s.airdry,
        drncoe: s.drncoe,
        drnday: s.drnday,
    }
}

fn weather_struct(w: &Weather) -> Result<Box<sys::csk_weather_t>, Error> {
    let n = w.days();
    if !(1..=366).contains(&n) {
        return Err(Error::Input(format!(
            "weather must have 1 to 366 days, got {n}"
        )));
    }
    for (name, v) in [
        ("tmin", &w.tmin),
        ("precip", &w.precip),
        ("epan", &w.epan),
        ("etr", &w.etr),
    ] {
        if v.len() != n {
            return Err(Error::Input(format!(
                "weather.{name} has {} days, tmax has {n}",
                v.len()
            )));
        }
    }
    let mut t = sys::zeroed_box::<sys::csk_weather_t>();
    t.year = w.year;
    t.imstr = w.start_month;
    t.idstr = w.start_day;
    t.ndays = n as i32;
    t.lat = w.lat;
    t.lon = w.lon;
    t.elev = w.elev;
    t.tmax[..n].copy_from_slice(&w.tmax);
    t.tmin[..n].copy_from_slice(&w.tmin);
    t.precip[..n].copy_from_slice(&w.precip);
    t.epan[..n].copy_from_slice(&w.epan);
    t.etr[..n].copy_from_slice(&w.etr);
    Ok(t)
}

fn state_struct(s: &State) -> sys::csk_state_t {
    sys::csk_state_t {
        theta: s.theta,
        oldcrp: s.oldcrp,
        livecrop: s.livecrop,
        oldres: s.oldres,
        snotmp: s.snotmp,
        snoh2o: s.snoh2o,
        soilt: s.soilt,
    }
}

fn from_state(s: &sys::csk_state_t) -> State {
    State {
        theta: s.theta,
        oldcrp: s.oldcrp,
        oldres: s.oldres,
        livecrop: s.livecrop,
        snotmp: s.snotmp,
        snoh2o: s.snoh2o,
        soilt: s.soilt,
    }
}

/// Annual values of one run (inches unless noted).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Annual {
    pub crop_yr: i32,
    pub crop_mon: i32,
    pub jdyplt: i32,
    pub jfirst: i32,
    pub jdyefc: i32,
    pub jdymat: i32,
    pub jdyfrz: i32,
    pub irigno: i32,
    pub itflag: i32,
    pub yield_: f32,
    pub yldratio: f32,
    pub etmax: f32,
    pub etyld: f32,
    /// ET.
    pub ets: f32,
    /// Reference ET.
    pub etrs: f32,
    /// Evaporation.
    pub es: f32,
    /// Transpiration.
    pub ts: f32,
    /// Gross irrigation.
    pub sgrirr: f32,
    /// Net irrigation.
    pub snirr: f32,
    pub precips: f32,
    pub eprecips: f32,
    /// Deep percolation.
    pub drains: f32,
    pub dplbg: f32,
    /// Runoff.
    pub totrof: f32,
    /// Wetland/open-water supply.
    pub wetsups: f32,
    /// Annual water balance error.
    pub balerr: f32,
}

/// Monthly series (12 values, Jan-Dec).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Monthly {
    Etrmon,
    Emon,
    Tmon,
    Etmon,
    Ermon,
    Ramon,
    Rofmon,
    Ronmon,
    Irnmon,
    Irgmon,
    Infmon,
    Dramon,
    Dplmon,
    Wsmon,
}

impl Monthly {
    pub const ALL: [Monthly; 14] = [
        Monthly::Etrmon,
        Monthly::Emon,
        Monthly::Tmon,
        Monthly::Etmon,
        Monthly::Ermon,
        Monthly::Ramon,
        Monthly::Rofmon,
        Monthly::Ronmon,
        Monthly::Irnmon,
        Monthly::Irgmon,
        Monthly::Infmon,
        Monthly::Dramon,
        Monthly::Dplmon,
        Monthly::Wsmon,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Monthly::Etrmon => "etrmon",
            Monthly::Emon => "emon",
            Monthly::Tmon => "tmon",
            Monthly::Etmon => "etmon",
            Monthly::Ermon => "ermon",
            Monthly::Ramon => "ramon",
            Monthly::Rofmon => "rofmon",
            Monthly::Ronmon => "ronmon",
            Monthly::Irnmon => "irnmon",
            Monthly::Irgmon => "irgmon",
            Monthly::Infmon => "infmon",
            Monthly::Dramon => "dramon",
            Monthly::Dplmon => "dplmon",
            Monthly::Wsmon => "wsmon",
        }
    }
}

/// Daily series (index = day of year - 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Daily {
    Gdd,
    Kc,
    Rzd,
    Etr,
    Precip,
    Eprecip,
    Runoff,
    Runon,
    Grirr,
    Nirr,
    Irrinf,
    E,
    T,
    Et,
    Drain,
    Wetsup,
    Snow,
    Soilw,
    Dpln,
    Awdpln,
    Balerr,
}

impl Daily {
    pub const ALL: [Daily; 21] = [
        Daily::Gdd,
        Daily::Kc,
        Daily::Rzd,
        Daily::Etr,
        Daily::Precip,
        Daily::Eprecip,
        Daily::Runoff,
        Daily::Runon,
        Daily::Grirr,
        Daily::Nirr,
        Daily::Irrinf,
        Daily::E,
        Daily::T,
        Daily::Et,
        Daily::Drain,
        Daily::Wetsup,
        Daily::Snow,
        Daily::Soilw,
        Daily::Dpln,
        Daily::Awdpln,
        Daily::Balerr,
    ];

    /// Name as in the Python package's `Result.daily` keys.
    pub fn name(self) -> &'static str {
        match self {
            Daily::Gdd => "gdd",
            Daily::Kc => "kc",
            Daily::Rzd => "rzd",
            Daily::Etr => "etr",
            Daily::Precip => "precip",
            Daily::Eprecip => "eprecip",
            Daily::Runoff => "runoff",
            Daily::Runon => "runon",
            Daily::Grirr => "grirr",
            Daily::Nirr => "nirr",
            Daily::Irrinf => "irrinf",
            Daily::E => "e",
            Daily::T => "t",
            Daily::Et => "et",
            Daily::Drain => "drain",
            Daily::Wetsup => "wetsup",
            Daily::Snow => "snow",
            Daily::Soilw => "soilw",
            Daily::Dpln => "dpln",
            Daily::Awdpln => "awdpln",
            Daily::Balerr => "balerr",
        }
    }

    /// Whether the series is a flux (inches per day) that adds up when
    /// area-weighting, as opposed to a state or crop property.
    pub fn is_flux(self) -> bool {
        !matches!(self, Daily::Gdd | Daily::Kc | Daily::Rzd)
    }
}

/// Results of one year for one soil.
pub struct YearResult {
    pub year: i32,
    pub crop: i32,
    /// Kernel status (0 = ok); see [`crate::status_message`].
    pub status: i32,
    /// Days of weather given (365 or 366 for a full year).
    pub ndays: usize,
    /// End-of-year state: carry it to this soil's next year.
    pub state: State,
    raw: Box<sys::csk_result_t>,
}

impl YearResult {
    pub fn annual(&self) -> Annual {
        let r = &*self.raw;
        Annual {
            crop_yr: r.crop_yr,
            crop_mon: r.crop_mon,
            jdyplt: r.jdyplt,
            jfirst: r.jfirst,
            jdyefc: r.jdyefc,
            jdymat: r.jdymat,
            jdyfrz: r.jdyfrz,
            irigno: r.irigno,
            itflag: r.itflag,
            yield_: r.yield_,
            yldratio: r.yldratio,
            etmax: r.etmax,
            etyld: r.etyld,
            ets: r.ets,
            etrs: r.etrs,
            es: r.es,
            ts: r.ts,
            sgrirr: r.sgrirr,
            snirr: r.snirr,
            precips: r.precips,
            eprecips: r.eprecips,
            drains: r.drains,
            dplbg: r.dplbg,
            totrof: r.totrof,
            wetsups: r.wetsups,
            balerr: r.balerr,
        }
    }

    pub fn monthly(&self, m: Monthly) -> &[f32; 12] {
        let r = &*self.raw;
        match m {
            Monthly::Etrmon => &r.etrmon,
            Monthly::Emon => &r.emon,
            Monthly::Tmon => &r.tmon,
            Monthly::Etmon => &r.etmon,
            Monthly::Ermon => &r.ermon,
            Monthly::Ramon => &r.ramon,
            Monthly::Rofmon => &r.rofmon,
            Monthly::Ronmon => &r.ronmon,
            Monthly::Irnmon => &r.irnmon,
            Monthly::Irgmon => &r.irgmon,
            Monthly::Infmon => &r.infmon,
            Monthly::Dramon => &r.dramon,
            Monthly::Dplmon => &r.dplmon,
            Monthly::Wsmon => &r.wsmon,
        }
    }

    pub fn numirr(&self) -> &[i32; 12] {
        &self.raw.numirr
    }

    /// Daily series for the days run (`ndays` values).
    pub fn daily(&self, d: Daily) -> &[f32] {
        let r = &*self.raw;
        let a = match d {
            Daily::Gdd => &r.d_gdd,
            Daily::Kc => &r.d_kc,
            Daily::Rzd => &r.d_rzd,
            Daily::Etr => &r.d_etr,
            Daily::Precip => &r.d_precip,
            Daily::Eprecip => &r.d_eprecip,
            Daily::Runoff => &r.d_runoff,
            Daily::Runon => &r.d_runon,
            Daily::Grirr => &r.d_grirr,
            Daily::Nirr => &r.d_nirr,
            Daily::Irrinf => &r.d_irrinf,
            Daily::E => &r.d_e,
            Daily::T => &r.d_t,
            Daily::Et => &r.d_et,
            Daily::Drain => &r.d_drain,
            Daily::Wetsup => &r.d_wetsup,
            Daily::Snow => &r.d_snow,
            Daily::Soilw => &r.d_soilw,
            Daily::Dpln => &r.d_dpln,
            Daily::Awdpln => &r.d_awdpln,
            Daily::Balerr => &r.d_balerr,
        };
        &a[..self.ndays.min(366)]
    }

    /// Weekly series (53 values): etrwk, etwk, erwk, rawk, irnwk, irgwk.
    pub fn weekly(&self, name: &str) -> Option<&[f32; 53]> {
        let r = &*self.raw;
        Some(match name {
            "etrwk" => &r.etrwk,
            "etwk" => &r.etwk,
            "erwk" => &r.erwk,
            "rawk" => &r.rawk,
            "irnwk" => &r.irnwk,
            "irgwk" => &r.irgwk,
            _ => return None,
        })
    }
}
