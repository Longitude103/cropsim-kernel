# Kernel benchmark

Times the compute kernel through its C interface, with no Python in the
loop, and saves every output so a change can be checked byte for byte.

```bash
pip install -e .                                  # the cropsim package
python tools/kernel_bench/dump_cases.py cases.bin # 1,944 test-parcel runs
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release && cmake --build build
cc -O2 -pthread -Ifortran/kernel tools/kernel_bench/bench.c \
   -Lbuild -lcropsim -Wl,-rpath,$PWD/build -o bench
./bench cases.bin out.bin 30        # single thread: time per run
./bench cases.bin - 10 4            # four threads: runs per second
```

To check a kernel change, build before and after, write `out.bin` from
each and compare: `cmp before.bin after.bin` must be silent for a change
meant to keep results identical. `python tools/kernel_bench/compare.py
before.bin after.bin` lists which fields moved, in how many runs, and by
how much.

The cases are the kernel test scenarios (all 22 crops, every irrigation
type and schedule, terraces, known dates) on three soils for six years,
run with every fix on, with `Fix.ORIGINAL`, and with the FAO-56 dual crop
coefficient. Results and timings are in `docs/kernel/performance.md`.
