#!/usr/bin/env bash
# Build the precompiled CropSim kernel bundle for this platform: a static
# libcropsim_kernel.a plus the static Fortran runtime it needs, so Rust
# builds (the pbm CLI, the desktop app) link the kernel without gfortran.
#
#   tools/kernel_bundle.sh OUT_DIR
#   CROPSIM_KERNEL_LIB_DIR=OUT_DIR cargo build --release
#
# Uses the same flags as CMakeLists.txt and rust/cropsim-sys/build.rs, so
# results are identical to the Python package's.
set -euo pipefail
out=${1:?usage: tools/kernel_bundle.sh OUT_DIR}
fc=${FC:-gfortran}
root=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$out"
out=$(cd "$out" && pwd)
obj=$(mktemp -d)
trap 'rm -rf "$obj"' EXIT
for f in csk_types csk_physics csk_balance csk_kernel csk_capi; do
  "$fc" -c -O2 -std=f2018 -frecursive -ffp-contract=off -fPIC -J "$obj" \
    "$root/fortran/kernel/$f.f90" -o "$obj/$f.o"
done
rm -f "$out/libcropsim_kernel.a"
ar crs "$out/libcropsim_kernel.a" "$obj"/*.o
for lib in libgfortran.a libquadmath.a libgcc.a; do
  p=$("$fc" -print-file-name=$lib)
  if [ -f "$p" ]; then
    # libgcc.a is only needed on macOS (libgfortran's unwinder symbols).
    if [ "$lib" = libgcc.a ] && [ "$(uname)" != Darwin ]; then continue; fi
    cp "$p" "$out/"
  fi
done
api=$(grep -m1 'define CSK_API_VERSION' "$root/fortran/kernel/cropsim_kernel.h" | awk '{print $3}')
{
  echo "kernel_api=$api"
  echo "commit=$(git -C "$root" rev-parse HEAD 2>/dev/null || echo unknown)"
  echo "compiler=$("$fc" --version | head -1)"
  echo "platform=$(uname -sm)"
} > "$out/BUNDLE_INFO"
ls -l "$out"
