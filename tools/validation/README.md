# Kernel validation on Scottsbluff test parcels

Runs the CropSim compute kernel on 20 plausible Nebraska Panhandle test
parcels with real weather, checks the results, and measures what each bug
fix changes. The output is `docs/validation/kernel-validation.md`.

There are no State parcel data sets with known answers, so the parcels are
representative, not observed: soils, crop calendars, irrigation settings and
crop coefficient curves are all set in `parcels.py`, in one place, for review.

## Run

```bash
pip install .                      # the cropsim package, from the repo root
python tools/validation/validate.py WEA_DIR [OUT_DIR]
```

`WEA_DIR` holds `SCTB2001.WEA` ... `SCTB2025.WEA`, the Scottsbluff station
files from the ESC weather set (`complete_current_1953-2025_weather_data_for_esc_run_2026-09-24/results`,
not in this repository). `OUT_DIR` defaults to `docs/validation`. A run takes
about 30 seconds.

## Files

| File | What it does |
| --- | --- |
| `parcels.py` | Test parcels: BLOCK.DAT values, soils, SIM settings, rotations, initial state |
| `runner.py` | Runs a parcel year by year, carrying soil state; builds the second-pass (known applied water) parcel from another parcel's irrigation |
| `validate.py` | Part 1 checks and plausibility, Part 2 fix-by-fix change report; writes the report and CSVs |

## Outputs

| File | Contents |
| --- | --- |
| `kernel-validation.md` | The report |
| `annual.csv` | Annual results per parcel and year, all fixes and original |
| `fix_impact.csv` | 24-year mean change per fix (alone and leave-out) and parcel |
| `fix_impact_years.csv` | The same per year (not committed; about 1 MB) |
| `checks.csv` | Every check on every parcel-year (not committed; about 1 MB) |
