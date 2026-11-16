"""Compare two kernel benchmark output files (bench.c OUT) field by field.

    python tools/kernel_bench/compare.py before.bin after.bin

Prints how many runs differ and, for each result or state field that
changed, in how many runs and by how much at most.
"""

from __future__ import annotations

import ctypes
import math
import sys

from cropsim import _lib


def main(a_path: str, b_path: str) -> int:
    parts = ((_lib.StateT, 0), (_lib.KResultT, ctypes.sizeof(_lib.StateT)))
    rec = ctypes.sizeof(_lib.StateT) + ctypes.sizeof(_lib.KResultT)
    a, b = open(a_path, "rb").read(), open(b_path, "rb").read()
    if len(a) != len(b) or len(a) % rec:
        print("files differ in length or are not bench output")
        return 2
    n = len(a) // rec
    runs: dict[str, set] = {}
    worst: dict[str, float] = {}
    for i in range(n):
        ra, rb = a[i * rec:(i + 1) * rec], b[i * rec:(i + 1) * rec]
        if ra == rb:
            continue
        for T, off in parts:
            x = T.from_buffer_copy(ra[off:off + ctypes.sizeof(T)])
            y = T.from_buffer_copy(rb[off:off + ctypes.sizeof(T)])
            for name, _ in T._fields_:
                u, v = getattr(x, name), getattr(y, name)
                u, v = (list(u), list(v)) if hasattr(u, "__len__") else ([u], [v])
                for p, q in zip(u, v):
                    if p == q or (isinstance(p, float) and math.isnan(p) and math.isnan(q)):
                        continue
                    runs.setdefault(name, set()).add(i)
                    worst[name] = max(worst.get(name, 0.0), abs(p - q))
    changed = {i for s in runs.values() for i in s}
    print(f"{n} runs, {len(changed)} differ")
    for name in sorted(runs, key=lambda k: -len(runs[k])):
        print(f"  {name}: {len(runs[name])} runs, largest difference {worst[name]:.3g}")
    return 1 if changed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))
