# cn4 EXL3 Nsight Compute diagnostic

Date: 2026-08-03

Status: optimization diagnostic against an accepted correctness artifact; not
a profiler-package, real-payload, TP4, serving, or performance acceptance

## Result

Nsight Compute 2026.2 profiled only
`project_native_f16(glmaxx_exl3_descriptor)` from the exact binary and SM120
library retained by the accepted EXL3 Phase-B qualification. Each case kept
the two-run CPU/GPU correctness control, and Nsight used kernel replay with the
`basic` set (nine passes per invocation).

| projection | M | grid CTAs | duration min/avg/max | SM throughput | L2 throughput | active warps | registers/thread |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| gate | 1 | 2 | 503.520 / 517.296 / 531.072 us | 0.527% | 0.049% | 16.113% | 38 |
| gate | 8 | 16 | 564.128 / 564.960 / 565.792 us | 3.856% | 0.190% | 16.552% | 38 |
| down | 1 | 24 | 45.792 / 46.000 / 46.208 us | 5.943% | 0.552% | 15.906% | 38 |

The gate and down matrices each contain `6144 x 512` weights and therefore the
same number of scalar products per row. The retained control assigns one
output element to one thread: gate M1 exposes only two CTAs whose threads each
run a 6,144-element serial inner loop, while down M1 exposes 24 CTAs with a
512-element inner loop. Gate M8 increases useful work eightfold but latency by
only 9.2%, because its 16 CTAs use more of the device. Together with sub-0.6%
L2 throughput, this identifies under-parallelized scalar decode/accumulation,
not external-memory bandwidth, as the first-order defect in the control.

## Optimization decision

Do not tune the transparent one-output-thread control into the serving path.
Retain it as an independent SM120 correctness oracle. The optimized EXL3
projection needs, in order:

1. fragment/warp-local Trellis decode with K-parallel accumulation and a
   deterministic reduction for gate/up at M=1;
2. one grouped launch across routed experts so the aggregate decode step has
   enough CTAs even when each expert sees one row;
3. fused or producer-consumer input/output rotations that do not materialize
   dense reconstructed weights; and
4. a measured K=3/K=4 mixed-format route after its pending design is accepted.

The first two changes are coupled: split-K alone adds reduction traffic, while
expert grouping alone leaves every gate/up CTA internally serial. A candidate
must beat this control with matched source planes and output membership, then
pass the same exact CPU/GPU and repeat-determinism gates.

### Rotation priority

A separate basic-set capture measured the retained gate M1 rotations:

| kernel | grid CTAs | duration min/avg/max | SM throughput | L2 throughput |
| --- | ---: | ---: | ---: | ---: |
| input H128 rotation | 48 | 2.848 / 2.848 / 2.848 us | 3.862% | 0.465% |
| output H128 rotation | 4 | 3.136 / 3.216 / 3.296 us | 0.285% | 0.385% |

Their combined average is 6.064 us, only 1.17% of the 517.296-us scalar gate
projection. Rotation fusion therefore follows K-parallel projection and
grouped expert execution in the optimization order. It can become material
after the projection is accelerated, but it is not the current first-order
bottleneck.

## Provenance

```text
source commit       ccf0162e236e8a8b5d4d6a308d6491759750e83e
Rust binary         ad2fb57c7cb25588f3cea3bc9f421994f4c16e84eea9c42a530b3342dd14187f
SM120 library       0d95723eb9eb3ed625d6f4933177006faa870eca9624dd3ee1a4fc200813d43d
container image     sha256:2e401388dcc9c180401cb9997e3e8394c2db695c7bf4e3139ff8a9a517940719
Nsight Compute      2026.2.0.0 build 37790515
```

Raw reports, CSV exports, and profiler logs are outside Git at:

```text
/home/derek/glmaxx/evidence/20260803T175200Z-exl3-ncu-diagnostic-ccf0162
```

The sorted relative-path ten-file hash stream is
`34afb700924691c12bd4d51250141c8a017cda73b81f4bf501072826683b546c`.
The three `.ncu-rep` identities are:

```text
gate M1  f4211fcedb51573d42c915392d30e011e99a2124b44ffc39b5a6cac11feb2956
gate M8  0d2355bc94923fc2d901d38c22e7f8da681b89fe2792fba4abed6fa62a5f45a1
down M1  694fc72b78cb87dee4a8749da67397484e18abb19ac86c8cb3e10c99b3c231a9
```

The first non-root container probe failed closed with `ERR_NVGPUCTRPERM` and
produced no report; its log is retained. Successful profiling ran as root only
inside disposable, GLMAXX-named containers with `SYS_ADMIN`; no host driver
policy changed. CSV export used a no-GPU container. cn4 returned to
2/2/2/10 MiB used, 0% utilization, and no compute process.

The rotation report, CSV, and log are separately retained at:

```text
/home/derek/glmaxx/evidence/20260803T181000Z-exl3-rotation-ncu-ccf0162
```

Its sorted relative-path three-file hash stream is
`4fa04b2099a0bbebac1685f3821c4400c65feb7e919bdea11477163a74a14ae1`;
the `.ncu-rep` SHA-256 is
`f4bdb6d813f3840c5b30327d00d0325b778e2ed03871cb826f032ee16028387a`.

## Scope

Profiler replay perturbs execution and these are two-sample diagnostic values,
not production latency estimates. The projection and rotation captures are
reported separately; allocation, transfers, routing, collectives, and
framework time are deliberately excluded. The pending SM120-profiler-package
review still governs any acceptance-grade timing or comparison.
