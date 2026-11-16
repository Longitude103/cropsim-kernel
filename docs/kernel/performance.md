# Kernel review: modern Fortran and speed

A review of `fortran/kernel/` (November 2026) for modern Fortran practice
and for work that slows it down, with the changes made. Every change keeps
results **bit for bit identical**: all 1,944 benchmark runs (every fix on,
`Fix.ORIGINAL` and FAO-56 dual Kc), every annual, monthly, weekly and daily
result and the end-of-year state, compared byte for byte; the 20-parcel
Scottsbluff validation (`tools/validation`, 2001-2025) gives identical
output files; and the parity, golden and fix tests all pass.

## Speed

Measured with `tools/kernel_bench` (C interface, no Python), gfortran 13,
the release flags (`-O2 -ffp-contract=off`), one x86-64 core:

| | Before | After | |
| --- | --- | --- | --- |
| Time per run (one parcel soil, one year) | 107-113 µs | 77-88 µs | about 1.3x faster |
| Instructions, all 1,944 runs (callgrind) | 2.49 billion | 1.97 billion | 21% fewer |
| Throughput, 4 threads | 28-31 thousand runs/s | 35-38 thousand runs/s | about 1.25x |

The machine timings vary by a few percent from run to run; the ranges are
the best of 30 passes over three repeats. The instruction counts are exact.

At about 80 µs a run, one core does roughly 12,000 parcel-soil-years a
second, and runs scale with threads (the kernel has no shared state). A
statewide run of, say, 35 million parcel-soil-years is about 50 core
minutes of kernel time.

## What was changed

| Where | Inefficiency | Change |
| --- | --- | --- |
| `run_year` | The 33 KB result was built in a temporary and copied (twice) | Zeroed in place (`clear_result`) |
| `run_year` | The weather was copied into the work type and then copied again into the day-of-year arrays | `prepare_weather` reads the weather struct directly; the copy and four 366-day arrays are gone from `csk_work_t` |
| `soil_temperature` | Damping depth, water scale, B and bare soil albedo depend only on the soil but were recomputed (two `exp`, a `log`) every day | Computed once per run (`soil_temperature_setup`) |
| `soil_temperature` | Cover terms (two `exp` of the cover) recomputed daily though the cover rarely changes | Kept and reused while the cover is unchanged (93% of days) |
| `soil_temperature` | Snow cover `exp` evaluated on snow-free days, where it gives exactly 0 | Skipped when there is no snow |
| `soil_temperature` | Each layer's depth factor (one `exp` per layer) recomputed when the damping depth had not changed | Reused when it has not |
| `root_zone` | Root distribution called `z**3` twice per layer (`powf`), including for layers below the roots where the result is exactly 0 | Each cube is computed once and carried to the next layer; layers below the roots get 0 directly |
| `water_balance` | Bug-fix switches looked up (a function call) up to a dozen times a day, and inside the per-layer uptake loop | Looked up once per run |
| `water_balance` | `calday` (a 12-month search) called every day | Called at month changes only; the day counts on in between |
| `water_balance` | Start-of-day storage recomputed though it equals the previous day's end-of-day storage | Carried over |
| `water_balance` | Three end-of-day passes over the layers (depletion, soil water, storage) | One pass, each sum in the same order |
| `prepare_weather` | Loop-invariant terms (crop base and ceiling temperature, clear-sky radiation coefficients) evaluated every day | Hoisted out of the loops |

Every hoisted or reused value is the same expression on the same inputs,
so it rounds the same way; that is why results do not move.

## Modern Fortran checklist

| Practice | State |
| --- | --- |
| Modules, `implicit none` | All five files; no implicit typing |
| `intent` on every dummy argument | Yes, including internal procedures |
| No `GOTO`, `COMMON`, `EQUIVALENCE`, `ENTRY`, `SAVE`d or module variables | None. `exit`/`cycle` and named `block`s replace the original's jumps |
| Thread safety | No global state; every value lives in the run's `csk_work_t`; `-frecursive` keeps locals on the stack |
| `pure` procedures | Now every physics routine (weather prep, crop coefficients, snow, runoff, evaporation, infiltration, roots, depletion, soil temperature, yield) and the setup helpers. The compiler checks they have no side effects. `water_balance` and `irrigate` stay impure only because their internal procedures update the day's local totals |
| Internal procedures that wrote to the host's variables | The crop coefficient curve (`fao_kc`) and alfalfa routine (`alfalfa_kc`) are now module procedures with explicit arguments |
| Heap allocation | None: all arrays have fixed sizes, so a run allocates nothing |
| File I/O | None, apart from the optional `legacy_round` (an internal write/read that copies INITIAL.DAT rounding) |
| Array order | Every loop walks 1-D arrays in order; the 3-D `cc` table is read one element at a time, so there is no column-order issue |
| Implicit real-to-integer conversion | The one left (`ipct = w%pct / 0.10`) is now an explicit `int()`, same value |
| `gfortran -Wall -Wextra` | Clean apart from deliberate exact real comparisons (as in the original) |
| `-fcheck=all` and floating point traps | All 1,944 benchmark runs complete with bounds checking and traps on invalid, divide-by-zero and overflow |

## What was not done, and why

* **`-O3`.** It vectorises the soil temperature layer loop with glibc's
  vector `exp`, which rounds differently: soil temperature (state only)
  moves by up to 0.00001 °F, on Linux only. Without vectorisation `-O3`
  is no faster than `-O2`. The build stays at `-O2`.
* **`-march=native`, FMA, `-ffast-math`, multiplying by reciprocals,
  reordering sums.** All change rounding, and the first two also make a
  precompiled library CPU-specific. Results would no longer match the
  original under `Fix.ORIGINAL` or across platforms.
* **Link-time optimisation.** About 1% faster, not worth the build risk on
  Windows and macOS.

## A larger gain, if wanted

The soil temperature routine (SOILTEMP) is now about a quarter of the
kernel's time, mostly ten `exp` calls a day. It never feeds back to the
water balance: it only updates the soil temperature carried in the state
(`soilt`). An option to skip it, leaving `soilt` as passed in, would make
runs that do not need soil temperature about a third faster again. That
changes the interface (a new `csk_options_t` field) and the state output
when used, so it is left as a decision.
