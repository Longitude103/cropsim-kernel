//! Bug-fix switches (docs/kernel/fixes.md). Every fix is on by default.

use bitflags::bitflags;

bitflags! {
    /// Bug fixes to apply. [`Fix::all()`] (the default) is the corrected
    /// model; [`Fix::ORIGINAL`] (no fixes) reproduces CropSim v8.0 exactly.
    /// Revert one fix with `Fix::all() - Fix::LEAP_CALENDAR`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
    #[serde(transparent)]
    pub struct Fix: u32 {
        const KC_GDD_LAG = 1;
        const SNOWMELT_BASE = 2;
        const IRRIGATION_COUNT = 4;
        const ROTATION_FREQUENCY = 8;
        const FORAGE_CUTTING_STAGE = 16;
        const ALFALFA_REGROWTH = 32;
        const TRANSPIRATION_LIMIT = 64;
        const WETLAND_BALANCE = 128;
        const DRAINAGE_SPREAD = 256;
        const KNOWN_DATE_RAIN = 512;
        const RESIDUE_CN = 1024;
        const IRRIGATION_DEPTH = 2048;
        const DRAINAGE_CLOCK = 4096;
        const INFILTRATION_OUTPUT = 8192;
        const ET_ACCUMULATION = 16384;
        const FALLOW_CROP = 32768;
        const PERENNIAL_ROOTS = 65536;
        const RZMIN_CAP = 131072;
        const LEAP_CALENDAR = 262144;
        const TAILWATER_FURROW = 524288;
        const FORAGE_CUTOFF = 1048576;
    }
}

impl Fix {
    /// No fixes: the original v8.0 results.
    pub const ORIGINAL: Fix = Fix::empty();

    /// The kernel's `revert` field for this set of fixes. No fixes is
    /// `CSK_REVERT_ALL`, so the original stays the original even for a
    /// kernel with fixes this crate does not know yet.
    pub fn revert_bits(self) -> i32 {
        if self.is_empty() {
            cropsim_sys::CSK_REVERT_ALL
        } else {
            (Fix::all().bits() & !self.bits()) as i32
        }
    }

    /// Fixes from names (as in `Fix::KC_GDD_LAG`), e.g. a run file's list.
    pub fn from_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<Fix, String> {
        let mut f = Fix::empty();
        for n in names {
            f |= Fix::from_name(n.trim()).ok_or_else(|| format!("unknown fix {n:?}"))?;
        }
        Ok(f)
    }
}

impl Default for Fix {
    fn default() -> Self {
        Fix::all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revert_bits() {
        assert_eq!(Fix::all().revert_bits(), 0);
        assert_eq!(Fix::ORIGINAL.revert_bits(), cropsim_sys::CSK_REVERT_ALL);
        assert_eq!((Fix::all() - Fix::LEAP_CALENDAR).revert_bits(), 262144);
        assert_eq!(Fix::all().iter().count(), 21);
    }
}
