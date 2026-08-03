# cn4 EXL3 Nsight Systems scaling diagnostic

Date: 2026-08-03

Status: accepted-artifact synthetic K=3 diagnostic; no retained-event,
real-payload, TP4, layer, token-throughput, or performance-acceptance claim

## Result

The exact reviewed EXL3 source-projection Phase-B executable and SM120 library
were reused without rebuilding. Gate, up, and down at `M=1,2,4,8` again
produced zero failed elements, bit-identical CPU/GPU hashes, and bit-identical
outputs across two device repetitions. Nsight Systems traced both repetitions
and reported the following average kernel durations:

| projection | M1 | M2 | M4 | M8 |
| --- | ---: | ---: | ---: | ---: |
| gate | 452.426 us | 454.8735 us | 460.4095 us | 458.106 us |
| up | 452.634 us | 454.762 us | 458.601 us | 457.6735 us |
| down | 39.616 us | 40.0315 us | 39.823 us | 63.967 us |

The matching input-plus-output rotations cost 3.2795--4.320 us. Including
those rotations, the M1 gate/up/down pipelines were
455.706/455.914/42.8955 us; at M8 they were
462.170/461.7375/68.287 us.

Gate and up latency rises only 1.3% and 1.1% respectively from M1 to M8 even
though useful row work increases eightfold. This corroborates the profiler
counter diagnosis: the scalar control exposes only two CTAs per gate/up row,
so small concurrent rows fill otherwise idle SMs without materially extending
the critical path. Down exposes 24 CTAs per row and stays flat through M4;
M8 raises latency by 61.5% when 192 CTAs require more aggregate execution.

This is the correct behavior for retaining the current kernel as an
independent correctness oracle, not for promoting it to serving. The measured
optimization order remains:

1. warp-local K-parallel Trellis decode and deterministic accumulation for
   gate/up;
2. one grouped launch across all routed experts and concurrent rows;
3. mixed K=3/K=4 specialization after its source contract is accepted; and
4. rotation fusion after the dominant projection is accelerated.

The earlier Nsight Compute replay reported 517.296 us for gate M1 and
46.000 us for down M1. These trace values are not a matched speed comparison:
counter replay and systems tracing have different perturbations. Retained
CUDA-event timing remains blocked on the profiler-package review.

## Provenance

```text
kernel source commit  ccf0162e236e8a8b5d4d6a308d6491759750e83e
Rust binary           ad2fb57c7cb25588f3cea3bc9f421994f4c16e84eea9c42a530b3342dd14187f
SM120 library         0d95723eb9eb3ed625d6f4933177006faa870eca9624dd3ee1a4fc200813d43d
toolchain commit      db1e8025317c29e0a357fc79ba1bb0852d2aa2d7
summary tool commit   ad487d6637c89f2f72dcec3cd58cddd1af78c0fe
container image       sha256:4a041313a952def9eb7353f055ee4061f5d76416e090aca04529a597b0bd549a
Nsight Systems        2026.1.3.425-261338342291v0
```

The 109-artifact hash stream is outside Git at:

```text
/home/derek/glmaxx/evidence/20260803T184000Z-exl3-nsys-scaling-ccf0162
```

Its `evidence-sha256.txt` SHA-256 is
`ef0d19c8b75f5849ee91b9c0b0fb625a1af825d5ff16c5c3cd845fe89d7a82c3`.
The evidence includes all case JSON, raw `.nsys-rep`, imported SQLite,
kernel-summary CSV, exact commands, hashes, topology, and before/after device
state. cn4 returned to 2/2/2/10 MiB used, 0% utilization, with no compute
process.
