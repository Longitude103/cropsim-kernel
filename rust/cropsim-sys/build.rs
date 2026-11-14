//! Builds or finds the CropSim compute kernel and links it statically.
//!
//! Where the kernel comes from, in order:
//!
//! 1. `CROPSIM_KERNEL_LIB_DIR`: a folder holding a precompiled
//!    `libcropsim_kernel.a` (and, if the Fortran runtime is not on the
//!    system, `libgfortran.a` / `libquadmath.a`). This is how release
//!    builds and machines without gfortran use the kernel; CI makes these
//!    folders per platform (`tools/kernel_bundle.sh`).
//! 2. `prebuilt/<target>/` inside this crate, same contents, for a
//!    packaged crate that carries its own kernel.
//! 3. Otherwise the Fortran in `fortran/kernel/` is compiled with `$FC`
//!    (default `gfortran`), with the same flags as the CMake build
//!    (`-O2 -std=f2018 -frecursive -ffp-contract=off`), so results are
//!    identical to the Python package's.
//!
//! The Fortran runtime is linked statically when its static archive can be
//! found, so programs that use this crate need no gfortran at run time.
//! Shared libraries on Linux (the Python extension module) cannot take the
//! distributions' non-PIC `libgfortran.a`; the `dynamic-runtime` feature
//! links `libgfortran.so` instead, and the wheel repair step (auditwheel,
//! run by maturin) copies it into the wheel.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

const SOURCES: [&str; 5] = [
    "csk_types.f90",
    "csk_physics.f90",
    "csk_balance.f90",
    "csk_kernel.f90",
    "csk_capi.f90",
];

fn main() {
    println!("cargo:rerun-if-env-changed=CROPSIM_KERNEL_LIB_DIR");
    println!("cargo:rerun-if-env-changed=FC");
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let target = env::var("TARGET").unwrap();

    let prebuilt = env::var_os("CROPSIM_KERNEL_LIB_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            let p = manifest.join("prebuilt").join(&target);
            p.join("libcropsim_kernel.a").exists().then_some(p)
        });

    let fc = env::var("FC").unwrap_or_else(|_| "gfortran".into());
    let lib_dir = match prebuilt {
        Some(dir) => {
            assert!(
                dir.join("libcropsim_kernel.a").exists(),
                "no libcropsim_kernel.a in {}",
                dir.display()
            );
            println!("cargo:rustc-env=CROPSIM_KERNEL_SOURCE=prebuilt");
            dir
        }
        None => {
            println!("cargo:rustc-env=CROPSIM_KERNEL_SOURCE=compiled");
            compile(&manifest, &fc)
        }
    };
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=cropsim_kernel");
    link_runtime(&lib_dir, &fc, &target);
}

fn kernel_dir(manifest: &Path) -> PathBuf {
    // In the repository the Fortran lives two levels up; a packaged crate
    // carries a copy in fortran/.
    let local = manifest.join("fortran");
    if local.join(SOURCES[0]).exists() {
        local
    } else {
        manifest.join("../../fortran/kernel")
    }
}

fn compile(manifest: &Path, fc: &str) -> PathBuf {
    let src = kernel_dir(manifest);
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let objs = out.join("obj");
    std::fs::create_dir_all(&objs).unwrap();
    let mut objects = Vec::new();
    for name in SOURCES {
        let path = src.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        let obj = objs.join(name.replace(".f90", ".o"));
        let status = Command::new(fc)
            .args([
                "-c",
                "-O2",
                "-std=f2018",
                "-frecursive",
                "-ffp-contract=off",
                "-fPIC",
            ])
            .arg("-J")
            .arg(&objs)
            .arg(&path)
            .arg("-o")
            .arg(&obj)
            .status()
            .unwrap_or_else(|e| {
                panic!(
                    "could not run the Fortran compiler `{fc}` ({e}). Install gfortran, set FC, \
                     or set CROPSIM_KERNEL_LIB_DIR to a folder with a precompiled \
                     libcropsim_kernel.a"
                )
            });
        assert!(status.success(), "{fc} failed on {}", path.display());
        objects.push(obj);
    }
    let lib = out.join("libcropsim_kernel.a");
    let _ = std::fs::remove_file(&lib);
    let ar = env::var("AR").unwrap_or_else(|_| "ar".into());
    let status = Command::new(&ar)
        .arg("crs")
        .arg(&lib)
        .args(&objects)
        .status()
        .expect("could not run ar");
    assert!(status.success(), "ar failed");
    out
}

/// Path of a runtime archive, from the kernel folder or the compiler.
fn find_archive(lib_dir: &Path, fc: &str, name: &str) -> Option<PathBuf> {
    let local = lib_dir.join(name);
    if local.exists() {
        return Some(local);
    }
    let out = Command::new(fc)
        .arg(format!("-print-file-name={name}"))
        .output()
        .ok()?;
    let p = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    (p.is_absolute() && p.exists()).then_some(p)
}

fn link_runtime(lib_dir: &Path, fc: &str, target: &str) {
    let dynamic = env::var_os("CARGO_FEATURE_DYNAMIC_RUNTIME").is_some();
    for (archive, lib) in [("libgfortran.a", "gfortran"), ("libquadmath.a", "quadmath")] {
        if dynamic {
            if lib == "gfortran" {
                if let Some(p) = find_archive(lib_dir, fc, "libgfortran.so") {
                    println!(
                        "cargo:rustc-link-search=native={}",
                        p.parent().unwrap().display()
                    );
                }
                println!("cargo:rustc-link-lib=dylib=gfortran");
            }
            continue;
        }
        match find_archive(lib_dir, fc, archive) {
            Some(p) => {
                println!(
                    "cargo:rustc-link-search=native={}",
                    p.parent().unwrap().display()
                );
                println!("cargo:rustc-link-lib=static={lib}");
            }
            // No static archive: fall back to the shared runtime for
            // gfortran; quadmath does not exist on every platform.
            None if lib == "gfortran" => println!("cargo:rustc-link-lib=dylib=gfortran"),
            None => {}
        }
    }
    if !target.contains("windows") {
        println!("cargo:rustc-link-lib=dylib=m");
    }
    if target.contains("apple") && !dynamic {
        // On macOS, GCC's libgfortran.a uses emulated thread-local storage
        // (___emutls_get_address, in libemutls_w.a) and libgcc helpers.
        for (archive, lib) in [("libemutls_w.a", "emutls_w"), ("libgcc.a", "gcc")] {
            if let Some(p) = find_archive(lib_dir, fc, archive) {
                println!(
                    "cargo:rustc-link-search=native={}",
                    p.parent().unwrap().display()
                );
                println!("cargo:rustc-link-lib=static={lib}");
            }
        }
    }
}
