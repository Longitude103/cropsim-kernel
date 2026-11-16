# FAO-56 dual crop coefficient option

`et_method = "fao56_dual"` (run file `[model]`, `RunOptions.et_method` in
Rust, `et_method=` in Python, `csk_options_t.et_method = 1` in C) replaces
CropSim's own split of potential ET into soil evaporation (E) and
transpiration (T) with the FAO-56 dual crop coefficient method (Allen et
al. 1998, ch. 7). It is a method option, not a bug fix: the default stays
`"original"`, and the original and corrected results are unchanged
(`tests/test_kernel.py`, `rust/cropsim/tests/golden.rs`).

## Why

The original split takes potential evaporation as ETr·min(KCU + KCL − KC, 1),
dries the top two layers in two stages (Ritchie), and gives potential
transpiration ETr·(KC − KCL) raised by any unused evaporation. On the test
parcels that puts most of the water into E: irrigated corn averages 22 in of
E and 14 in of T a year, dryland corn 20 in of E and under 4 in of T. The
kernel validation already listed the low dryland T as a calibration item.

## Method

Reference ET is the tall (alfalfa) ETr the kernel already reads, and every
coefficient is on that basis.

* **Basal coefficient KCB.** The crop's daily KC curve, as the kernel already
  computes it. The Wright (1982) curves (crops 1-7, alfalfa) are basal,
  alfalfa-reference coefficients by design, and the Watts/Stegman corn curve
  and the four-segment curves for the other covers have the same 0.15-0.25
  floor for dry bare soil. Fallow has KCB = 0 (no transpiration).
* **Transpiration.** Potential T = KCB·ETr, through the kernel's existing
  root uptake and water stress (the stress is FAO-56's Ks).
* **Evaporation.** E = KE·ETr, taken from layer 1 (4 in, the FAO-56 surface
  layer depth Ze of 0.10-0.15 m), with
  * KE = min(KR·(KCMAX − KCB), FEW·KCMAX) (Eq. 71), times the residue
    reduction the original applies to evaporation;
  * KCMAX = max(1.0, KCB + 0.05): 1.0 on an alfalfa basis, about 1.2 on a
    grass basis (Eq. 72);
  * KR = 1 until the readily evaporable water REW = 0.35 in is gone, then
    falling linearly to 0 at the total evaporable water TEW of layer 1
    (field capacity to air dry) (Eq. 74);
  * FEW = min(1 − FC, FW): the surface not covered by the crop, FC from KCB
    and crop height (Eq. 76; heights from FAO-56 Table 12), and the fraction
    last wetted, FW (Eq. 75);
  * FW = 1 after 0.12 in or more of rain; after irrigation 1.0 for
    sprinklers, borders and known dates and 0.8 for furrows (Table 20);
  * the surface-layer depletion DE is tracked for the wetted, exposed part
    (Eq. 77): infiltrated rain reduces it, irrigation by I/FW, and
    evaporation adds E/FEW.
* **Wetlands and open water** (crops 16, 18-21) keep ET = KC·ETr.

The constants are in `fortran/kernel/csk_balance.f90` (`KCMAX_ETR`,
`REW_IN`, `FW_RAIN_IN`, `FW_IRR`, `HMAX`).

## What changes

Six-year mean annual inches on the test parcels (synthetic fixtures, three
soils each; `python tools/dual_kc_report.py`). These are not State data:
real-weather numbers need the Scottsbluff validation set
(`tools/validation`), which needs the ESC weather files.

| Scenario | E orig | E dual | T orig | T dual | ET orig | ET dual | Gross irr. orig | Gross irr. dual | DP orig | DP dual | Runoff orig | Runoff dual |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| corn_pivot | 22.3 | 15.8 | 14.3 | 22.7 | 36.5 | 38.4 | 15.3 | 14.3 | 1.7 | 3.3 | 6.1 | 1.6 |
| corn_dry | 20.2 | 15.5 | 3.5 | 12.0 | 23.8 | 27.5 | 0.0 | 0.0 | 0.7 | 2.5 | 7.3 | 1.3 |
| corn_district_demand | 22.3 | 15.8 | 14.3 | 22.7 | 36.5 | 38.4 | 15.3 | 14.3 | 1.7 | 3.3 | 6.1 | 1.6 |
| corn_furrow | 22.5 | 15.8 | 14.3 | 22.6 | 36.8 | 38.4 | 16.0 | 14.8 | 1.9 | 3.3 | 5.9 | 1.6 |
| corn_border | 23.2 | 15.8 | 14.0 | 22.5 | 37.1 | 38.4 | 18.2 | 17.9 | 3.7 | 6.1 | 5.9 | 1.8 |
| corn_known_dates | 25.7 | 15.8 | 8.6 | 16.9 | 34.3 | 32.7 | 6.4 | 6.4 | 1.6 | 2.5 | 0.9 | 1.4 |
| corn_terrace | 26.3 | 15.5 | 5.6 | 12.2 | 32.0 | 27.7 | 0.0 | 0.0 | 16.0 | 15.7 | 2.7 | 0.2 |
| soy_dry | 19.0 | 12.4 | 4.1 | 14.7 | 23.1 | 27.1 | 0.0 | 0.0 | 0.9 | 2.7 | 7.5 | 1.4 |
| soy_rotation | 21.4 | 13.6 | 16.7 | 24.9 | 38.2 | 38.5 | 18.3 | 18.0 | 5.1 | 6.2 | 3.7 | 1.8 |
| sorghum_dry | 20.2 | 13.6 | 3.7 | 14.0 | 23.9 | 27.6 | 0.0 | 0.0 | 0.7 | 2.4 | 7.2 | 1.3 |
| wheat_fallow | 19.6 | 16.4 | 2.7 | 9.1 | 22.2 | 25.5 | 0.0 | 0.0 | 2.0 | 4.2 | 7.2 | 1.4 |
| wheat_irrigated | 17.0 | 12.4 | 11.2 | 18.6 | 28.3 | 31.0 | 6.1 | 5.1 | 1.3 | 3.0 | 7.1 | 1.5 |
| smallgrain_dry | 18.4 | 12.9 | 4.5 | 14.0 | 22.9 | 26.8 | 0.0 | 0.0 | 1.2 | 3.0 | 7.5 | 1.4 |
| beans_pivot | 20.9 | 13.2 | 16.5 | 25.6 | 37.3 | 38.8 | 15.2 | 14.4 | 1.2 | 3.0 | 5.9 | 1.6 |
| potato_pivot | 23.1 | 15.1 | 14.1 | 23.4 | 37.2 | 38.5 | 14.8 | 13.1 | 0.7 | 2.4 | 6.5 | 1.5 |
| sugarbeet_furrow | 22.7 | 15.0 | 12.2 | 21.3 | 34.9 | 36.4 | 12.3 | 10.4 | 0.8 | 2.5 | 6.7 | 1.5 |
| sunflower_dry | 20.4 | 17.1 | 2.9 | 10.5 | 23.3 | 27.7 | 0.0 | 0.0 | 0.6 | 2.2 | 8.0 | 1.4 |
| corn_shallow_roots | 22.0 | 15.8 | 14.1 | 22.3 | 36.0 | 38.1 | 16.8 | 16.5 | 3.1 | 5.5 | 6.3 | 1.5 |
| alfalfa_pivot | 27.2 | 16.8 | 9.1 | 22.2 | 36.4 | 39.0 | 14.0 | 12.9 | 0.5 | 1.5 | 6.9 | 2.1 |
| alfalfa_two_cuts | 27.5 | 17.0 | 8.9 | 21.8 | 36.5 | 38.8 | 14.3 | 12.6 | 0.5 | 1.5 | 7.0 | 2.0 |
| alfalfa_spring_start | 22.8 | 13.4 | 8.9 | 20.4 | 31.7 | 33.9 | 14.2 | 13.1 | 0.2 | 0.4 | 5.1 | 1.5 |
| alfalfa_rotation | 30.5 | 16.9 | 7.0 | 24.3 | 37.6 | 41.2 | 16.5 | 16.5 | 0.6 | 2.1 | 7.4 | 2.0 |
| hay_pivot | 25.6 | 19.7 | 9.3 | 19.5 | 34.8 | 39.2 | 12.8 | 13.1 | 0.7 | 1.5 | 7.1 | 2.0 |
| pasture_pivot | 24.3 | 19.4 | 9.5 | 20.1 | 33.8 | 39.5 | 11.7 | 12.9 | 0.7 | 1.1 | 7.2 | 2.1 |
| pasture_rotation | 29.7 | 20.8 | 7.9 | 20.3 | 37.6 | 41.1 | 16.5 | 16.5 | 0.8 | 2.0 | 7.3 | 2.1 |
| range | 25.6 | 18.5 | 4.0 | 11.0 | 29.7 | 29.5 | 0.0 | 0.0 | 1.7 | 1.7 | 0.2 | 0.3 |
| range_rotation | 31.4 | 22.7 | 8.7 | 18.5 | 40.1 | 41.2 | 16.3 | 16.5 | 4.4 | 3.4 | 0.4 | 0.5 |
| turf_pivot | 26.5 | 13.4 | 6.6 | 26.5 | 33.0 | 39.9 | 10.8 | 13.8 | 0.8 | 1.3 | 7.2 | 2.1 |
| fallow | 10.8 | 23.8 | 0.0 | 0.0 | 10.8 | 23.8 | 0.0 | 0.0 | 15.5 | 5.2 | 4.6 | 2.1 |
| farmstead | 10.5 | 20.2 | 5.8 | 8.3 | 16.2 | 28.6 | 0.0 | 0.0 | 10.5 | 1.5 | 4.3 | 1.4 |
| upland_wood | 24.6 | 15.1 | 5.2 | 14.3 | 29.8 | 29.3 | 0.0 | 0.0 | 1.7 | 1.9 | 0.2 | 0.3 |

What it shows:

* **T rises and E falls** for every crop: irrigated corn from 14 to 23 in
  of T, dryland corn from 3.5 to 12 in, alfalfa from 9 to 22 in. Total ET
  moves much less (corn +2 in irrigated, +4 in dryland).
* **Irrigation demand drops a little** for most irrigated crops (corn 15.3 to
  14.3 in): less of the applied water evaporates from the surface.
* **Runoff falls** (corn 6.1 to 1.6 in): the curve number follows the
  moisture of the upper soil, which transpiration now dries more.
* **Deep percolation rises** under crops (corn 1.7 to 3.3 in) but **falls
  sharply under bare and sparse covers** (fallow 15.5 to 5.2 in, farmstead
  10.5 to 1.5 in), because a wet bare surface now evaporates at up to ETr.
  Fallow then loses about 80% of its precipitation to evaporation, which is
  in line with Great Plains fallow efficiencies (10-35% of precipitation
  stored); the original lost under 40%.

The recharge changes under fallow and farmsteads matter most for the
groundwater model and should be checked against field data before the
option is used for production runs.

## Tests

* `tests/test_dual_kc.py`: every test parcel on every soil for six years
  runs, closes its water balance, and never has E + T above
  max(KCMAX, KCB + 0.05)·ETr; fallow has no T; E on a wet bare surface starts
  at or below KCMAX·ETr and falls as the surface dries; mid-season corn E is
  under 15% of ET and unstressed T equals KCB·ETr; wet covers are unchanged;
  the default is unchanged; six-year ET of the main crops stays within 25%
  of the original split's.
* `rust/cropsim/tests/golden.rs::dual_crop_coefficient_option` and
  `rust/pbm-core/tests/example_run.rs::dual_crop_coefficient_run`: the option
  through Rust and a parcel run file.

## Not done

* KCB is the kernel's existing curve. The FAO-56 Table 17 or the 2021
  updated KCB values, converted to an alfalfa basis, could replace it per
  crop. That would be a separate curve option.
* No climate adjustment of KCB_mid (FAO-56 Eq. 70): it is for grass-based
  coefficients and is small on an alfalfa basis.
* REW, the furrow FW and KCMAX are fixed constants, not inputs.
