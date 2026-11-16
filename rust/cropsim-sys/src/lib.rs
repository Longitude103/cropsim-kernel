//! Raw FFI to the CropSim compute kernel (`fortran/kernel/cropsim_kernel.h`).
//!
//! The structs mirror the header (and the BIND(C) types in
//! `csk_types.f90`) field for field. Fortran arrays are column-major, so
//! the Fortran `cc(22,3,10)` is `cc[10][3][22]` here. The layout is checked
//! against the kernel's own `csk_struct_size` in the tests.
//!
//! Use the `cropsim` crate for a safe interface.
#![allow(non_camel_case_types)]

/// Interface version this crate was written for (`CSK_API_VERSION`).
pub const CSK_API_VERSION: i32 = 3;

pub const CSK_OK: i32 = 0;
pub const CSK_ERR_RUNOFF: i32 = 1;
pub const CSK_ERR_EFF_PRECIP: i32 = 2;
pub const CSK_ERR_TRANS_CONV: i32 = 3;
pub const CSK_ERR_NEG_RUNOFF: i32 = 4;
pub const CSK_ERR_BAD_INPUT: i32 = 11;

/// `csk_options_t.et_method`: CropSim's own E/T split (the default).
pub const CSK_ET_ORIGINAL: i32 = 0;
/// `csk_options_t.et_method`: FAO-56 dual crop coefficient.
pub const CSK_ET_FAO56_DUAL: i32 = 1;

/// `csk_options_t.revert` value that turns every fix off (original v8.0).
pub const CSK_REVERT_ALL: i32 = 0x7FFF_FFFF;

/// Where the linked kernel came from: "compiled" (from fortran/kernel by
/// build.rs) or "prebuilt" (a precompiled static library).
pub const KERNEL_SOURCE: &str = env!("CROPSIM_KERNEL_SOURCE");

#[repr(C)]
#[derive(Clone, Copy)]
pub struct csk_params_t {
    pub etrfact: f32,
    pub nfday: i32,
    pub ytrend: i32,
    pub depth: [f32; 10],
    pub ydens: [f32; 22],
    pub resrat: [f32; 22],
    pub spgrav: [f32; 22],
    pub fragil: [i32; 22],
    pub restyp: [i32; 22],
    pub cc: [[[f32; 22]; 3]; 10],
    pub kcl: [f32; 22],
    pub kcu: [f32; 22],
    pub tbase: [f32; 22],
    pub tceil: [f32; 22],
    pub sftmp: f32,
    pub smfmx: f32,
    pub smfmn: f32,
    pub snocovmx: f32,
    pub snocov1: f32,
    pub snocov2: f32,
    pub timp: f32,
    pub cnfallow: [f32; 4],
    pub cn: [[f32; 22]; 4],
    /// `[factor][tillage code - 1]`
    pub tilfac: [[f32; 80]; 2],
    pub smtmp: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct csk_sim_t {
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
    pub ntills: i32,
    pub itflag: i32,
    pub itiltm: [i32; 20],
    pub tilday: [i32; 20],
    pub tilcod: [i32; 20],
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
    pub alpha1: f32,
    pub airdry: [f32; 2],
    pub fieldc: [f32; 10],
    pub pwp: [f32; 10],
    pub bulkd: f32,
    pub kirr: i32,
    /// `[event][day of year, gross in, fraction infiltrated]`
    pub fdirrig: [[f32; 3]; 50],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct csk_soil_t {
    pub soil: i32,
    pub bulkd: f32,
    pub orgm: f32,
    pub fieldc: [f32; 10],
    pub pwp: [f32; 10],
    pub airdry: [f32; 2],
    pub drncoe: f32,
    pub drnday: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct csk_weather_t {
    pub year: i32,
    pub imstr: i32,
    pub idstr: i32,
    pub ndays: i32,
    pub lat: f32,
    pub lon: f32,
    pub elev: f32,
    pub tmax: [f32; 366],
    pub tmin: [f32; 366],
    pub precip: [f32; 366],
    pub epan: [f32; 366],
    pub etr: [f32; 366],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct csk_state_t {
    pub theta: [f32; 10],
    pub oldcrp: i32,
    pub livecrop: i32,
    pub oldres: f32,
    pub snotmp: f32,
    pub snoh2o: f32,
    pub soilt: [f32; 10],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct csk_options_t {
    pub revert: i32,
    pub livecond: i32,
    pub legacy_round: i32,
    /// `CSK_ET_*`: how potential ET is split into E and T.
    pub et_method: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct csk_result_t {
    pub status: i32,
    pub crop_yr: i32,
    pub crop_mon: i32,
    pub imbg: i32,
    pub imend: i32,
    pub jdyplt: i32,
    pub jfirst: i32,
    pub jdyefc: i32,
    pub jdymat: i32,
    pub jdyfrz: i32,
    pub irigno: i32,
    pub itflag: i32,
    pub yield_: f32,
    pub yldratio: f32,
    pub bvalue: f32,
    pub etmax: f32,
    pub etyld: f32,
    pub ets: f32,
    pub etrs: f32,
    pub es: f32,
    pub ts: f32,
    pub sgrirr: f32,
    pub snirr: f32,
    pub precips: f32,
    pub eprecips: f32,
    pub drains: f32,
    pub dplbg: f32,
    pub totrof: f32,
    pub etrmon: [f32; 12],
    pub emon: [f32; 12],
    pub tmon: [f32; 12],
    pub etmon: [f32; 12],
    pub ermon: [f32; 12],
    pub ramon: [f32; 12],
    pub rofmon: [f32; 12],
    pub ronmon: [f32; 12],
    pub irnmon: [f32; 12],
    pub irgmon: [f32; 12],
    pub infmon: [f32; 12],
    pub dramon: [f32; 12],
    pub dplmon: [f32; 12],
    pub numirr: [i32; 12],
    pub etrwk: [f32; 53],
    pub etwk: [f32; 53],
    pub erwk: [f32; 53],
    pub rawk: [f32; 53],
    pub irnwk: [f32; 53],
    pub irgwk: [f32; 53],
    pub jdybg: i32,
    pub jdyend: i32,
    pub wetsups: f32,
    pub wsmon: [f32; 12],
    pub balerr: f32,
    pub d_gdd: [f32; 366],
    pub d_kc: [f32; 366],
    pub d_rzd: [f32; 366],
    pub d_etr: [f32; 366],
    pub d_precip: [f32; 366],
    pub d_eprecip: [f32; 366],
    pub d_runoff: [f32; 366],
    pub d_runon: [f32; 366],
    pub d_grirr: [f32; 366],
    pub d_nirr: [f32; 366],
    pub d_irrinf: [f32; 366],
    pub d_e: [f32; 366],
    pub d_t: [f32; 366],
    pub d_et: [f32; 366],
    pub d_drain: [f32; 366],
    pub d_wetsup: [f32; 366],
    pub d_snow: [f32; 366],
    pub d_soilw: [f32; 366],
    pub d_dpln: [f32; 366],
    pub d_awdpln: [f32; 366],
    pub d_balerr: [f32; 366],
}

extern "C" {
    pub fn csk_version() -> i32;
    /// 1 params, 2 sim, 3 soil, 4 weather, 5 state, 6 options, 7 result
    pub fn csk_struct_size(which: i32) -> i32;
    pub fn csk_run_year(
        params: *const csk_params_t,
        sim: *const csk_sim_t,
        soil: *const csk_soil_t,
        weather: *const csk_weather_t,
        state_in: *const csk_state_t,
        options: *const csk_options_t,
        state_out: *mut csk_state_t,
        result: *mut csk_result_t,
    ) -> i32;
}

/// Every struct here is plain old data, so all-zero bytes is a valid value.
/// Boxed, as the result struct is about 40 KB.
pub fn zeroed_box<T: Copy>() -> Box<T> {
    // SAFETY: only used for the repr(C) POD structs in this module, for
    // which the all-zero bit pattern is valid (integers and floats).
    Box::new(unsafe { std::mem::zeroed() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn version_matches() {
        assert_eq!(unsafe { csk_version() }, CSK_API_VERSION);
    }

    #[test]
    fn struct_sizes_match_the_kernel() {
        let sizes = [
            (1, size_of::<csk_params_t>()),
            (2, size_of::<csk_sim_t>()),
            (3, size_of::<csk_soil_t>()),
            (4, size_of::<csk_weather_t>()),
            (5, size_of::<csk_state_t>()),
            (6, size_of::<csk_options_t>()),
            (7, size_of::<csk_result_t>()),
        ];
        for (which, rust) in sizes {
            let fortran = unsafe { csk_struct_size(which) } as usize;
            assert_eq!(
                rust, fortran,
                "struct {which}: Rust {rust} bytes, kernel {fortran}"
            );
        }
    }
}
