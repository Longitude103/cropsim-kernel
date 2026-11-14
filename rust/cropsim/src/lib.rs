//! Safe Rust interface to the CropSim compute kernel.
//!
//! The kernel (`fortran/kernel/`) is CropSim v8.0 rewritten as a pure,
//! thread-safe compute library. This crate gives it plain Rust inputs
//! ([`Block`], [`Sim`], [`Soil`], [`Weather`], [`State`]), runs one year at a
//! time with [`Kernel::run_year`], and returns a [`YearResult`] with annual,
//! monthly and daily values and the end-of-year [`State`] to carry forward.
//!
//! It mirrors the Python `cropsim.Kernel` class: the same inputs give the
//! same results, bit for bit, because both call the same compiled kernel.
//!
//! ```no_run
//! use cropsim::{legacy, Fix, Kernel, RunOptions};
//!
//! let kernel = Kernel::new(&legacy::read_block("BLOCK.DAT")?, &legacy::read_tillage("TILLAGE.DAT")?)?;
//! let soil = legacy::read_soils("NESOILS.DAT")?.remove(&622).unwrap();
//! let sim = legacy::read_sim("CRNIRR01.SIM")?;
//! let mut state = legacy::read_initial("INITIAL.DAT")?;
//! for year in 2010..=2014 {
//!     let weather = legacy::read_weather(format!("AGAT{year}.WEA"), Some(year))?;
//!     let r = kernel.run_year(&sim, &soil, &weather, &state, &RunOptions::default())?;
//!     state = r.state.clone();
//!     println!("{year} ET {:.2} in", r.annual().ets);
//! }
//! # Ok::<(), cropsim::Error>(())
//! ```

mod fix;
mod kernel;
pub mod legacy;
mod model;

pub use fix::Fix;
pub use kernel::{Annual, Daily, Kernel, Monthly, RunOptions, YearResult};
pub use model::{Block, Sim, Soil, State, Tillage, Weather, CROP_NAMES};

/// Kernel interface version (`CSK_API_VERSION`).
pub fn kernel_api_version() -> i32 {
    // SAFETY: no arguments, no state.
    unsafe { cropsim_sys::csk_version() }
}

/// Where the linked kernel came from: "compiled" or "prebuilt".
pub const KERNEL_SOURCE: &str = cropsim_sys::KERNEL_SOURCE;

/// Errors from reading inputs or running the kernel.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {source}")]
    Io { path: String, source: std::io::Error },
    #[error("{path}: {message}")]
    Parse { path: String, message: String },
    #[error("invalid input: {0}")]
    Input(String),
    #[error("CropSim status {status}: {}", status_message(*.status))]
    Kernel { status: i32 },
}

/// Message for a kernel status code (the interface's status-code contract).
pub fn status_message(status: i32) -> &'static str {
    match status {
        0 => "ok",
        1 => "negative runoff or effective precipitation above rain (EFPRECIP check)",
        2 => "effective precipitation above rain (DEPLT check)",
        3 => "transpiration loop did not converge",
        4 => "negative monthly runoff",
        11 => "an input is out of range",
        _ => "unknown error",
    }
}
