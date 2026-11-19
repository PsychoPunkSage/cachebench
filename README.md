# cachebench

Small Rust CLI that measures how memory access patterns interact with the CPU caches,
reading hardware counters directly through `perf_event_open` (no perf crate).

```
cachebench [--core p|e|<cpu id>] [--warmup N] [--reps N] <experiment>
```

Each experiment pins itself to one CPU, runs warmup passes, then times `reps` passes.
Every timed pass is wrapped by a counter group (reset, enable, run, disable, read);
the tables show the median across passes.

## Machine

| | |
|---|---|
| CPU | Intel Core Ultra 7 258V (Lunar Lake), 4 P-cores (CPU 0-3) + 4 E-cores (CPU 4-7), no SMT |
| L1d | 48 KiB per P-core, 32 KiB per E-core |
| L2 | 2.5 MiB per P-core, 4 MiB shared by the E-cores |
| L3 | 12 MiB |
| Cache line | 64 B |
| OS | Linux 6.14, governor `powersave`, turbo on |
| Toolchain | rustc 1.97.1, `--release` |

## Counters

| name | perf event | notes |
|---|---|---|
| cycles | `PERF_COUNT_HW_CPU_CYCLES` | |
| instructions | `PERF_COUNT_HW_INSTRUCTIONS` | |
| l1d-misses | `HW_CACHE` L1D / read / miss | not available on the E-core PMU, skipped there |
| llc-misses | `PERF_COUNT_HW_CACHE_MISSES` | requests that miss L3, including L1/L2 prefetches, so it can exceed 1 per access |
| branch-misses | `PERF_COUNT_HW_BRANCH_MISSES` | |

- Counting requires `kernel.perf_event_paranoid <= 1`; only user-space events are counted.
- P-cores and E-cores have separate PMUs (`cpu_core`, `cpu_atom`), so the PMU type is
  looked up for the pinned CPU and placed in the upper 32 bits of `config`.
- The generic `HW_CACHE` LL read-miss event reads 0 on the P-core PMU even when
  `perf stat` reports hundreds of millions of L3 misses, so `llc-misses` uses the generic
  hardware cache-miss event instead.

## Experiments

### `seq`: sequential sum of a 64 MiB `u32` array

| | P-core (CPU 3) | E-core (CPU 7) |
|---|---|---|
| median | 3.52 ms, 19.1 GB/s | 4.64 ms, 14.5 GB/s |
| cycles / elem | 0.58 | 0.54 |
| instructions / elem | 2.25 | 2.25 |
| l1d-misses / elem | 0.034 | n/a |
| llc-misses / elem | 0.016 | 0.17 |
| IPC | 3.88 | 4.18 |

Why:
- The loop is SSE2-vectorised (`paddq`, 4 elements per iteration), hence 2.25
  instructions per element; 64 MiB is far larger than L3, so it runs at DRAM bandwidth.
- L1d misses are ~0.034 per element, below the 1/16 expected from 64 B lines holding
  16 `u32`s: the hardware prefetchers bring most lines in before the loads reach them.
- Instruction count is identical on both cores (same binary). The E-core uses fewer
  cycles but is slower because it runs at a lower clock (cycles / time gives ~2.8 GHz on
  the P-core vs ~1.9 GHz on the E-core). The loop is memory-bound: DRAM waits are fixed in
  nanoseconds, so a faster clock spends more cycles waiting. Compare cores by time.

### `stride`: sum every k-th `u32` of a 64 MiB array, k = 1 .. 1024

Values are per access; accesses = ceil(n / k).

| k | bytes | P ns/access | P l1d-misses | P llc-misses | E ns/access |
|---|---|---|---|---|---|
| 1 | 4 | 0.56 | 0.003 | 0.06 | 0.57 |
| 4 | 16 | 0.76 | 0.043 | 0.05 | 1.10 |
| 8 | 32 | 1.42 | 0.30 | 0.20 | 2.16 |
| 16 | 64 | 2.77 | 0.92 | 0.46 | 3.84 |
| 64 | 256 | 6.74 | 0.98 | 1.38 | 7.37 |
| 1024 | 4096 | 9.00 | 1.00 | 2.41 | 12.8 |

Why:
- From k = 16 (64 B, one cache line) every access lands on a new line, so L1d misses
  per access reach ~1 and each access pays for a whole line fetch.
- Below k = 16 several accesses share one line, so the line cost is split between them.
- `step_by(k)` with a runtime `k` compiles to a scalar loop of 8 instructions per access
  (load, add, index and bound updates), so even k = 1 is ~2.7x slower than `seq`'s
  vectorised loop over the same memory.

### `pattern`: sum `data[idx[i]]` over a 64 MiB array, `idx` in order vs shuffled

`idx` is a `u32` index array, either `0..n` or shuffled with Fisher-Yates and xorshift.
Both orders run the same code and make n accesses; only the order differs.

| order | P ns/access | P l1d-misses | P llc-misses | E ns/access | E llc-misses |
|---|---|---|---|---|---|
| seq | 0.56 | 0.034 | 0.07 | 0.77 | 0.27 |
| random | 5.47 | 1.39 | 1.25 | 11.4 | 2.93 |

Why:
- Shuffled order defeats the prefetchers, so almost every access misses all caches.
- It still costs only ~5 ns, not a full DRAM latency (~100 ns): each `idx[i]` is known
  in advance, so the out-of-order core keeps many independent misses in flight at once
  (memory-level parallelism).
- L1d misses exceed 1 per access: ~1 for `data`, ~1/16 for the sequential `idx` stream,
  and the rest from page-table walks, since random access over 64 MiB misses the TLB on
  almost every access and the walker reads page-table entries through L1d.
