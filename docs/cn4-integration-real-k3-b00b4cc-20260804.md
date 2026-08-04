# cn4 current-integration real TR3 K=3 regression

Date: 2026-08-04

Status: passed as a current-integration real-payload scalar correctness and
timing regression; no K=4, optimized-route, TP4-layer, checkpoint-smoke, or
token-throughput claim

## Provenance and isolation

The exact pushed integration commit
`b00b4cc306d5cf7eb6092b4c87f3d179278eedba` was fetched into the fresh
detached worktree:

```text
/home/derek/glmaxx/worktrees/integration-b00b4cc-20260804T102500Z
```

The run used the dedicated network-disabled GLMAXX image and only GLMAXX
paths. The checkpoint mount was read-only. No vLLM worktree, image, container,
cache, process, port, or evidence path was used or modified.

| Input | Identity |
|---|---|
| Container | `sha256:2e401388dcc9c180401cb9997e3e8394c2db695c7bf4e3139ff8a9a517940719` |
| CUTLASS | `e05f953a5b3d38adc240df2ff928e0421c2abba3` |
| Layer-3 TR3 shard | `31bc19eabf05d0782e33103672094f1d8aca2a8bb9fb5b88a502cd6caab61bd0` |
| Executable | `bfdddc1b158e9591298e98a7e8e24486000db86777dcef5e036d56152178f794` |
| SM120 library | `a66a6c4bd91b7628b33f89659510f7bfc62ffe01370f99c4d0b4d51add3d9c16` |

Before launch, cn4 exposed exactly four compute-capability 12.0 devices at
2/2/2/10 MiB used, zero utilization, and no compute process. The run used
physical GPU 0. After the container exited, all four devices returned to the
same 2/2/2/10 MiB state with zero utilization and no compute process.

The first attempt is retained separately at:

```text
/home/derek/glmaxx/evidence/20260804T102500Z-integration-real-k3-b00b4cc-r1
```

It stopped before compilation or CUDA launch because the image's default
offline Cargo home lacked `sha2`; its `cargo-test.txt` hashes to
`bdc47942813db7bff8249e857dfcb3b56530fb4bdeaee0ba2a966eb91b4a8b1e`.
The successful r2 changed only the container-local `CARGO_HOME` binding to the
dedicated `/home/derek/glmaxx/cache/cargo` cache.

## Result

The r2 run passed all 427 workspace tests, built a real `sm_120f` library,
and directly loaded the source-ordered K=3 trellis and rotations for layer 3,
expert 0, TP ranks 0 and 3. Gate, up, and down projections each ran at
M=1,2,4,8. Across all six reports and 24 cases:

- CPU and GPU FP16 output SHA-256 values matched exactly;
- maximum absolute and relative error were zero;
- failed elements were zero;
- all three device repetitions were bitwise identical; and
- runtime repack and persistent reconstructed-weight bytes were zero.

The required K=4 negative control failed closed at the incompatible K=3
trellis boundary. It is not K=4 execution evidence.

Retained CUDA-event p50 projection time, in microseconds:

| TP rank | Projection | M1 | M2 | M4 | M8 |
|---:|---|---:|---:|---:|---:|
| 0 | gate | 461.408 | 461.440 | 465.536 | 467.296 |
| 0 | up | 461.408 | 461.568 | 465.536 | 467.488 |
| 0 | down | 47.712 | 47.744 | 47.744 | 68.224 |
| 3 | gate | 461.376 | 461.440 | 465.536 | 467.264 |
| 3 | up | 461.344 | 461.504 | 465.536 | 467.552 |
| 3 | down | 47.712 | 47.744 | 47.744 | 68.224 |

Each cell retains 1,000 measured CUDA-event samples through its exact JSON
report and includes p50/p95/p99/min/max/mean. Gate/up remain effectively flat
from M1 through M8, corroborating that the accepted scalar route is
under-parallelized and cannot be a serving kernel. These numbers are a
correctness control, not an end-to-end throughput estimate.

## Evidence integrity and next gate

The successful immutable evidence directory is:

```text
/home/derek/glmaxx/evidence/20260804T102500Z-integration-real-k3-b00b4cc-r2
```

All 36 entries passed a fresh `sha256sum --check`. The artifact manifest and
compact summary hashes are:

```text
artifact-manifest.txt  49d5b664fa5b5e833b82804e4c01df1efdf568553cde1f618fc38dd1592a08ba
summary.json           00807558a0b162b69cfbeea96b145637e19c66c32dad8ee45bd78b378c12f556
```

This proves the accepted scalar K=3 route remains correct at the current
integration tip. It does not advance the mixed 192:64 target layer by itself.
The critical next path remains acceptance of the already prepared warp-staged
CPU/implementation reviews and mixed-K r2 contract, followed by real K=3/K=4
optimized projection qualification and TP4 sparse-layer replay.
