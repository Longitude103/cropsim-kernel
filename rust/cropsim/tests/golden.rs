//! The Rust interface against the pinned original results
//! (tests/golden/original.csv, made by the Python package from the same
//! kernel with every fix reverted). Every scenario that is a fixture SIM
//! file as-is, plus known irrigation dates, on all three soils for six
//! years: every value must match bit for bit on x86-64, which checks the struct
//! layout, the readers and the state carryover together.

use std::collections::HashMap;
use std::path::PathBuf;

use cropsim::{legacy, Fix, Kernel, RunOptions};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Exact on x86-64, where the golden file was made; elsewhere the math
/// library's sin/exp can differ in the last bit, so within float rounding
/// (the same rule as the Python test, tests/test_kernel.py).
fn same(golden: &str, value: f64) -> bool {
    let g: f64 = golden.parse().unwrap();
    if g.is_nan() || value.is_nan() {
        return g.is_nan() && value.is_nan();
    }
    if cfg!(target_arch = "x86_64") {
        g == value
    } else {
        (g - value).abs() <= 1e-4 + 1e-4 * g.abs()
    }
}

#[test]
fn matches_golden_original() {
    let fx = root().join("tests/fixtures");
    let kernel = Kernel::new(
        &legacy::read_block(fx.join("BLOCK.DAT")).unwrap(),
        &legacy::read_tillage(fx.join("TILLAGE.DAT")).unwrap(),
    )
    .unwrap();
    let soils = legacy::read_soils(fx.join("NESOILS.DAT")).unwrap();
    let initial = legacy::read_initial(fx.join("INITIAL.DAT")).unwrap();
    let weather: HashMap<i32, _> = (2009..=2014)
        .map(|y| {
            (
                y,
                legacy::read_weather(fx.join(format!("wea/TEST{y}.WEA")), None).unwrap(),
            )
        })
        .collect();
    let sim = |n: &str| legacy::read_sim(fx.join(format!("sim/{n}.SIM"))).unwrap();

    let mut scenarios = vec![
        ("corn_pivot", sim("CRNIRR01")),
        ("corn_dry", sim("CRNDRY01")),
        ("soy_dry", sim("SOYDRY01")),
        ("soy_rotation", sim("SOYIRR03")),
        ("sorghum_dry", sim("SORDRY01")),
        ("wheat_fallow", sim("WWTDRY01")),
        ("alfalfa_pivot", sim("ALFIRR01")),
        ("range", sim("NRANGE01")),
    ];
    let mut known = sim("CRNIRR01");
    known.irrtyp = 2;
    known.irrigation_events = vec![
        (180, 1.2, 0.85),
        (192, 1.0, 0.85),
        (205, 1.2, 0.85),
        (218, 1.0, 0.85),
        (231, 1.1, 0.85),
        (244, 0.9, 0.85),
    ];
    known.alpha1 = 0.0;
    known.sim_airdry = [0.04, 0.07];
    known.sim_fieldc = [
        0.305, 0.315, 0.325, 0.305, 0.315, 0.325, 0.305, 0.315, 0.325, 0.305,
    ];
    known.sim_pwp = [0.18, 0.19, 0.2, 0.18, 0.19, 0.2, 0.18, 0.19, 0.2, 0.18];
    known.sim_bulkd = 1.4;
    scenarios.push(("corn_known_dates", known));

    let mut rdr = csv::Reader::from_path(root().join("tests/golden/original.csv")).unwrap();
    let headers = rdr.headers().unwrap().clone();
    let golden: HashMap<(String, i32, i32), csv::StringRecord> = rdr
        .records()
        .map(|r| r.unwrap())
        .map(|r| {
            (
                (
                    r[0].to_string(),
                    r[1].parse().unwrap(),
                    r[2].parse().unwrap(),
                ),
                r,
            )
        })
        .collect();

    let opts = RunOptions {
        fixes: Fix::ORIGINAL,
        ..RunOptions::default()
    };
    let mut checked = 0;
    for (name, sim) in &scenarios {
        for soil in [421, 622, 832] {
            let mut state = initial.clone();
            for year in 2009..=2014 {
                let r = kernel
                    .run_year_status(sim, &soils[&soil], &weather[&year], &state, &opts)
                    .unwrap();
                let g = &golden[&(name.to_string(), soil, year)];
                let a = r.annual();
                let s = &r.state;
                let mut values: HashMap<&str, f64> = HashMap::from([
                    ("status", r.status as f64),
                    ("crop_yr", a.crop_yr as f64),
                    ("crop_mon", a.crop_mon as f64),
                    ("jdyplt", a.jdyplt as f64),
                    ("jfirst", a.jfirst as f64),
                    ("jdyefc", a.jdyefc as f64),
                    ("jdymat", a.jdymat as f64),
                    ("jdyfrz", a.jdyfrz as f64),
                    ("irigno", a.irigno as f64),
                    ("itflag", a.itflag as f64),
                    ("yield", a.yield_ as f64),
                    ("yldratio", a.yldratio as f64),
                    ("etmax", a.etmax as f64),
                    ("etyld", a.etyld as f64),
                    ("ets", a.ets as f64),
                    ("etrs", a.etrs as f64),
                    ("es", a.es as f64),
                    ("ts", a.ts as f64),
                    ("sgrirr", a.sgrirr as f64),
                    ("snirr", a.snirr as f64),
                    ("precips", a.precips as f64),
                    ("eprecips", a.eprecips as f64),
                    ("drains", a.drains as f64),
                    ("dplbg", a.dplbg as f64),
                    ("totrof", a.totrof as f64),
                    ("oldcrp", s.oldcrp as f64),
                    ("livecrop", s.livecrop as f64),
                    ("oldres", s.oldres as f64),
                    ("snotmp", s.snotmp as f64),
                    ("snoh2o", s.snoh2o as f64),
                ]);
                let names: Vec<String> = (1..=10)
                    .flat_map(|k| [format!("theta{k}"), format!("soilt{k}")])
                    .collect();
                for k in 0..10 {
                    values.insert(&names[2 * k], s.theta[k] as f64);
                    values.insert(&names[2 * k + 1], s.soilt[k] as f64);
                }
                for (i, h) in headers.iter().enumerate().skip(3) {
                    let v = values
                        .get(h)
                        .unwrap_or_else(|| panic!("golden column {h} not checked"));
                    assert!(
                        same(&g[i], *v),
                        "{name} soil {soil} {year}: {h} golden {} rust {v}",
                        &g[i]
                    );
                }
                checked += 1;
                state = r.state.clone();
            }
        }
    }
    assert_eq!(checked, 9 * 3 * 6);
}

#[test]
fn runs_on_many_threads() {
    let fx = root().join("tests/fixtures");
    let kernel = Kernel::new(
        &legacy::read_block(fx.join("BLOCK.DAT")).unwrap(),
        &legacy::read_tillage(fx.join("TILLAGE.DAT")).unwrap(),
    )
    .unwrap();
    let soil = legacy::read_soils(fx.join("NESOILS.DAT")).unwrap()[&622].clone();
    let sim = legacy::read_sim(fx.join("sim/CRNIRR01.SIM")).unwrap();
    let state = legacy::read_initial(fx.join("INITIAL.DAT")).unwrap();
    let w = legacy::read_weather(fx.join("wea/TEST2012.WEA"), None).unwrap();
    let one = kernel
        .run_year(&sim, &soil, &w, &state, &RunOptions::default())
        .unwrap();
    std::thread::scope(|sc| {
        let hs: Vec<_> = (0..8)
            .map(|_| {
                sc.spawn(|| {
                    kernel
                        .run_year(&sim, &soil, &w, &state, &RunOptions::default())
                        .unwrap()
                })
            })
            .collect();
        for h in hs {
            let r = h.join().unwrap();
            assert_eq!(r.annual(), one.annual());
            assert_eq!(r.state, one.state);
        }
    });
}

#[test]
fn dual_crop_coefficient_option() {
    use cropsim::EtMethod;
    let fx = root().join("tests/fixtures");
    let kernel = Kernel::new(
        &legacy::read_block(fx.join("BLOCK.DAT")).unwrap(),
        &legacy::read_tillage(fx.join("TILLAGE.DAT")).unwrap(),
    )
    .unwrap();
    let soil = legacy::read_soils(fx.join("NESOILS.DAT")).unwrap()[&622].clone();
    let sim = legacy::read_sim(fx.join("sim/CRNIRR01.SIM")).unwrap();
    let state = legacy::read_initial(fx.join("INITIAL.DAT")).unwrap();
    let w = legacy::read_weather(fx.join("wea/TEST2012.WEA"), None).unwrap();
    let base = kernel
        .run_year(&sim, &soil, &w, &state, &RunOptions::default())
        .unwrap();
    let dual = kernel
        .run_year(
            &sim,
            &soil,
            &w,
            &state,
            &RunOptions {
                et_method: EtMethod::Fao56Dual,
                ..RunOptions::default()
            },
        )
        .unwrap();
    let (b, d) = (base.annual(), dual.annual());
    assert!(d.balerr.abs() < 2e-3);
    // More of the water goes to transpiration than with the original split.
    assert!(d.ts > b.ts && d.es < b.es, "{b:?} {d:?}");
    // The option is in run options as "fao56_dual".
    let o: RunOptions = serde_json::from_str(r#"{"et_method": "fao56_dual"}"#).unwrap();
    assert_eq!(o.et_method, EtMethod::Fao56Dual);
}
