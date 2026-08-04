# cn4 current-tip real TR3 K=3 scalar regression

Date: 2026-08-04

Status: passed current-integration SM120 correctness regression; scalar
projection control only

## Result

Integration commit `02c45b06210cf9da4a2dd0ff5d9874a9199444c2`
compiled a real `sm_120f` shared library on cn4 and passed all six hash-pinned
layer-3 K=3 projection controls from the production TR3 3.25-bpw checkpoint.
Gate, up, and down were tested for TP ranks 0 and 3 at M1/M2/M4/M8.

All 24 device cases:

- matched the independent Rust output bit-for-bit;
- had zero failed or non-finite elements;
- repeated bitwise-identically three times; and
- consumed the packed EXL3 source directly with zero runtime weight repack and
  zero persistent reconstructed-weight bytes.

The required layer-3 K=4 expert negative control failed closed at the K=3
trellis contract. The current tree also passed 436 workspace tests plus the
two CUDA-FFI qualification-harness tests before launch.

The linked library contains these native cubins:

```text
nvfp4_routed_fc1.sm_120.cubin
nvfp4_routed_fc2.sm_120.cubin
exl3_projection_control.sm_120.cubin
cutlass_nvfp4_dense_control.sm_120.cubin
cutlass_nvfp4_fc2_control.sm_120.cubin
```

## Scalar timing controls

The table contains retained CUDA-event p50 values over 1,000 samples. These
are correctness-control timings, not optimized routed-MoE or model throughput.

| Rank | Projection | M1 p50 | M8 p50 |
|---:|---|---:|---:|
| 0 | gate | 457.312 us | 463.456 us |
| 0 | up | 457.312 us | 463.456 us |
| 0 | down | 45.696 us | 68.224 us |
| 3 | gate | 457.312 us | 463.456 us |
| 3 | up | 457.280 us | 463.456 us |
| 3 | down | 45.728 us | 68.224 us |

The gate/up control remains dominated by its 32-CTA grid. The reviewed
warp-staged candidate and proposed grouped-expert successor address this
known under-occupancy, but neither is promoted by this regression.

## Provenance

- Host: `cn4`
- GPU: physical GPU 0, NVIDIA RTX PRO 6000 Blackwell, compute capability 12.0
- Driver: `595.71.05`
- Source commit:
  `02c45b06210cf9da4a2dd0ff5d9874a9199444c2`
- Container:
  `sha256:2e401388dcc9c180401cb9997e3e8394c2db695c7bf4e3139ff8a9a517940719`
- CUTLASS:
  `e05f953a5b3d38adc240df2ff928e0421c2abba3`
- Checkpoint shard:
  `/home/derek/models/GLM-5.2-EXL3-TR3-3.25bpw/model-layer-003.safetensors`
- Checkpoint shard SHA-256:
  `31bc19eabf05d0782e33103672094f1d8aca2a8bb9fb5b88a502cd6caab61bd0`
- Successful evidence:
  `/home/derek/glmaxx/evidence/20260804T160000Z-current-real-k3-02c45b0-r2`
- Summary SHA-256:
  `284a68422ff0c17ec82b8332bbb5033f6817ddc42615cae018218dd754845ee5`
- Inner artifact-manifest SHA-256:
  `c28bc6877e4c0632705fd2d449e565611164aeef09f1be140dc76c2388f663f1`
- Manifest rows verified: 36 of 36
- Release runner SHA-256:
  `ad54a6aa17e065ef2ed72e23ab9d60ac7026eb1a080614dc01cd8e9ae5e0cf51`
- SM120 shared-library SHA-256:
  `9dd0829bc4b32420aeba789773077b2098444c41b9654899e0062bb659aa48ac`

The detached source worktree remained clean. The container exited, compute
process inventory was empty afterward, and all four GPUs returned to zero
utilization with 2/2/2/10 MiB reported use.

## Operational correction

Two setup attempts failed before CUDA launch: the first on Git safe-directory
ownership and the second on a missing offline Cargo-registry mount. The second
attempt remains preserved at
`/home/derek/glmaxx/evidence/20260804T160000Z-current-real-k3-02c45b0`.

Commit `45c56e90529f39a255c8698f6ecd6b43eb71ea18` adds
`scripts/cn4-current-real-k3.sh` (SHA-256
`6092ef112b78ae7a8fc5fbb77365f8758528adb9d2f3cbe7825ecdc7f36f196a`).
The wrapper encodes the corrected read-only Cargo mount and ephemeral Git
ownership settings, records the exact Docker argument vector and container
inspection, preserves failed runs, seals the inner and outer manifests, and
rechecks global occupancy before delegating to the existing fail-closed
qualification harness.

## Claim boundary and next gate

This proves that the current combined Rust tree builds and launches a correct
real-checkpoint SM120 EXL3 K=3 projection. It does not prove K=4 execution,
routed expert grouping, one complete GLM layer, TP4 collectives, checkpoint
smoke, logits, KLD, KV capacity, MTP, concurrency, or serving throughput.

The shortest path forward remains acceptance of the staged EXL3 CPU and
implementation reviews, followed by grouped gate/up, mixed-K, rank-executor,
target-layer, small-checkpoint, and TP4 layer-replay gates.
