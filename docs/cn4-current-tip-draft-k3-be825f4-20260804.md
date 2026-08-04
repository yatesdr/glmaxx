# cn4 current-tip recurrent-draft K=3 scalar regression

Date: 2026-08-04

Status: passed current-integration SM120 correctness regression; recurrent
draft projection control only

## Result

Integration commit `be825f4bd62fecc431d1a1be65b7d692cdc24ceb`
compiled the SM120 library on cn4 and passed all six hash-pinned layer-78 K=3
projection controls from the production TR3 3.25-bpw checkpoint. Gate, up, and
down were tested for TP ranks 0 and 3 at M1/M2/M4/M8.

All 24 device cases matched the independent Rust output bit-for-bit, contained
zero failed or non-finite elements, and repeated bitwise-identically three
times. The device consumed packed EXL3 source bytes directly with zero runtime
weight repack and zero persistent reconstructed-weight bytes. Layer 78 is an
all-K3 recurrent-draft layer, so the target-layer K=4 negative control is not
applicable.

The run passed 436 workspace tests plus the two CUDA-FFI qualification-harness
tests before device launch.

## Scalar timing controls

These are retained CUDA-event p50 values over 1,000 samples. They measure only
the scalar source-projection correctness control.

| Rank | Projection | M1 p50 | M8 p50 |
|---:|---|---:|---:|
| 0 | gate | 457.248 us | 463.456 us |
| 0 | up | 457.184 us | 463.392 us |
| 0 | down | 45.696 us | 68.224 us |
| 3 | gate | 457.248 us | 463.456 us |
| 3 | up | 457.280 us | 463.456 us |
| 3 | down | 45.696 us | 68.224 us |

## Sealed provenance

- Host: `cn4`
- GPU: physical GPU 0, NVIDIA RTX PRO 6000 Blackwell, compute capability 12.0
- Source commit:
  `be825f4bd62fecc431d1a1be65b7d692cdc24ceb`
- Container:
  `sha256:2e401388dcc9c180401cb9997e3e8394c2db695c7bf4e3139ff8a9a517940719`
- CUTLASS:
  `e05f953a5b3d38adc240df2ff928e0421c2abba3`
- Checkpoint shard:
  `/home/derek/models/GLM-5.2-EXL3-TR3-3.25bpw/model-layer-078.safetensors`
- Checkpoint shard SHA-256:
  `5448b63a32e394e8cbff5a4737fb50b40fc53c6c3c41305f1a7ee540c4d9a6e3`
- Run interval: `2026-08-04T16:12:12Z` through
  `2026-08-04T16:16:50Z`
- Evidence root:
  `/home/derek/glmaxx/evidence/20260804T161212Z-current-real-k3-draft-layer78-be825f4`
- Outer evidence manifest SHA-256:
  `09c5f9d548ef338e5ba8fceb480b66f74e85edb66ec129064f3518cf88292a36`
- Outer verifier: `COMPLETE`, 80 files
- Inner summary SHA-256:
  `84cc9b18429072db24b8d4144ba69d1199c457e4d9475ff9de3affbc0ae6f182`
- Inner artifact-manifest SHA-256:
  `e115b32900522bb2cfa1623f87c2c72bd05e2d3bb7fe94d9981b4fbe169120b5`
- Inner manifest rows verified: 34 of 34
- Exact Docker command-record SHA-256:
  `fb8f0c71e88da4f7bfc4eec97608ab8ed78b32ee737917696b240a2c5b311b39`
- Executed wrapper SHA-256:
  `59f5b0903a2c99653bd0dd1101bf21d0cd55ab00c25f1a8cc7389a60c4261494`
- Release runner SHA-256:
  `ad54a6aa17e065ef2ed72e23ab9d60ac7026eb1a080614dc01cd8e9ae5e0cf51`
- SM120 shared-library SHA-256:
  `ad771c7d4de151efa1527166149740a999204c0c0ef7bfcc35320d2306f4b073`

The wrapper used `--pull never`, `--network none`, private IPC, the isolated
GLMAXX worktree/build/evidence/cache roots, a read-only exact checkpoint-file
mount, and ephemeral Git safe-directory settings. Both manifest verifiers
passed. The detached source remained clean, the container exited, compute
process inventory was empty afterward, and all GPUs returned to zero
utilization.

## Failed-wrapper evidence

The first use of wrapper commit `45c56e9` completed all draft kernel cases but
verified the relative inner-manifest paths from the wrong directory. Its outer
generation therefore terminated `FAILED`, as intended:

```text
/home/derek/glmaxx/evidence/20260804T160618Z-current-real-k3-draft-layer78-a9151d6
```

The nested qualification in that record independently verifies 34 of 34 rows,
but it is not used as the accepted result. The fresh `be825f4` generation above
is the authoritative sealed run.

## Claim boundary and next gate

This proves a current-tree real recurrent-draft K=3 SM120 projection, not an
MTP layer or speculative decode. It does not prove K=4, routed grouping,
complete layer execution, proposal/verification semantics, TP4 collectives,
checkpoint smoke, logits, quality, KV capacity, or serving throughput.

MTP remains ordered behind target-only MTP0 correctness. The immediate source
path still requires the staged EXL3, mixed-K, executor, target-layer,
small-checkpoint, and TP4 replay reviews before composition.
