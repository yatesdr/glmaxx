# cn4 EXL3 source-projection Phase B

Date: 2026-08-03

Status: accepted synthetic SM120 correctness launch; no checkpoint, TP4,
quality, capacity, serving, or performance claim

## Result

The reviewed EXL3 v1 source-order projection launched on cn4 GPU 0 for the
actual per-rank GLM-5.2 expert projection shapes. Gate, up, and down all
matched the Rust CPU oracle bit-for-bit on both repetitions:

| projection | shape | failed | max abs | max rel | deterministic |
| --- | --- | ---: | ---: | ---: | --- |
| gate | `1 x 6144 x 512` | 0 | 0.0 | 0.0 | yes, two runs |
| up | `1 x 6144 x 512` | 0 | 0.0 | 0.0 | yes, two runs |
| down | `1 x 512 x 6144` | 0 | 0.0 | 0.0 | yes, two runs |

All three launches reported zero persistent reconstructed-weight bytes and
zero runtime weight-repack bytes. The final gate verdict is:

```text
EXL3_SOURCE_PROJECTION_M1_CORRECTNESS_PASSED
```

This is a deliberately slow synthetic control. It proves the reviewed K=3
source projection can compile and execute correctly on SM120; it does not
measure a real TR3 payload or establish useful kernel speed.

## Provenance

The source is the isolated qualification commit formed from Fable's exact
accepted candidate plus the byte-identical tracked acceptance result:

```text
reviewed candidate  0edfc8d796aeaeb969668005149bcb6286aa1e85
qualification       ccf0162e236e8a8b5d4d6a308d6491759750e83e
review artifact     cac885880345fb2f02e940bcf0cd32420acf5ac8a6a3e34fc76e7971a5aa2964
CUTLASS              e05f953a5b3d38adc240df2ff928e0421c2abba3
container            sha256:2e401388dcc9c180401cb9997e3e8394c2db695c7bf4e3139ff8a9a517940719
library              0d95723eb9eb3ed625d6f4933177006faa870eca9624dd3ee1a4fc200813d43d
Rust binary          ad2fb57c7cb25588f3cea3bc9f421994f4c16e84eea9c42a530b3342dd14187f
```

The clean worktree was
`/home/derek/glmaxx/worktrees/exl3-phase-b-ccf0162-20260803T171038Z`.
The container used all four visible GPUs only to enforce the exactly-four
SM120 admission rule; the synthetic launches bound the default device,
GPU 0. Network was disabled, IPC was private, UID/GID was 1000, source and
CUTLASS were read-only, and the GLMAXX-owned offline Cargo registry was
mounted at `/cargo-home`.

The complete workspace suite passed 163 tests before kernel build. The
library contains real `sm_120f` cubins and exactly the required EXL3 ABI,
workspace, and launch symbols. The EXL3 control resources were 22 registers
and 1,536 bytes shared memory for each rotation kernel, and 38 registers with
no shared memory for the projection kernel.

Raw evidence is outside Git at:

```text
/home/derek/glmaxx/evidence/20260803T171210Z-exl3-phase-b-ccf0162
```

The SHA-256 of the sorted 30-file evidence hash stream, excluding the
separately hashed `cargo-target` and `kernel-build` trees, is
`c2325d2f12b98898abcec08f640733cf1dbf733017b67e89972c2285ebc56857`.
The source worktree remained clean. cn4 returned to 2/2/2/10 MiB used, 0%
utilization, and no compute process.

## Fail-closed preparation records

Three preparation attempts produced no device launch. The first two stopped
before evidence creation because the CUTLASS path was wrong and then because
current `main` no longer matched the accepted `exl3.rs` hash. The third is
preserved at
`/home/derek/glmaxx/evidence/20260803T171038Z-exl3-phase-b-ccf0162` and
stopped during offline Cargo resolution because the registry was not mounted.
No acceptance claim is made for those attempts.

## Next gate

The next EXL3 step is an accepted K=3/K=4 mixed-format implementation and a
real TR3 tensor replay. This result does not unblock K=4 by itself. After the
real-payload microbenchmark passes, the path remains TP4 one-layer replay,
checkpoint smoke, MTP0 quality, and only then matched serving benchmarks.
