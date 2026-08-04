# cn4 sealed current-tip target K=3 scalar regression

Date: 2026-08-04

Status: passed current-integration SM120 correctness regression; target sparse
projection control only

## Result

Integration commit `be825f4bd62fecc431d1a1be65b7d692cdc24ceb`
compiled the SM120 library on cn4 and passed six real, hash-pinned layer-3 K=3
projection controls from the production TR3 3.25-bpw checkpoint. Gate, up, and
down were tested for TP ranks 0 and 3 at M1/M2/M4/M8.

All 24 device cases matched the independent Rust output bit-for-bit, contained
zero failed or non-finite elements, and repeated bitwise-identically three
times. The device consumed packed EXL3 source directly with zero runtime weight
repack and zero persistent reconstructed-weight bytes. The required layer-3
K=4 expert failed closed at the K=3 trellis contract.

The run passed 436 workspace tests plus the two CUDA-FFI qualification-harness
tests before device launch.

## Scalar timing controls

These are retained CUDA-event p50 values over 1,000 samples. They are not
routed-MoE, model-layer, or serving timings.

| Rank | Projection | M1 p50 | M8 p50 |
|---:|---|---:|---:|
| 0 | gate | 457.280 us | 463.456 us |
| 0 | up | 457.312 us | 463.456 us |
| 0 | down | 45.728 us | 68.224 us |
| 3 | gate | 457.312 us | 462.752 us |
| 3 | up | 457.280 us | 462.688 us |
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
  `/home/derek/models/GLM-5.2-EXL3-TR3-3.25bpw/model-layer-003.safetensors`
- Checkpoint shard SHA-256:
  `31bc19eabf05d0782e33103672094f1d8aca2a8bb9fb5b88a502cd6caab61bd0`
- Run interval: `2026-08-04T16:21:52Z` through
  `2026-08-04T16:26:51Z`
- Evidence root:
  `/home/derek/glmaxx/evidence/20260804T162152Z-current-real-k3-target-layer3-be825f4`
- Outer evidence manifest SHA-256:
  `d1e7a3c22d3231f3403bcec3dd826756aa3ad8d24443d0d2822b9a9ca234a65c`
- Outer verifier: `COMPLETE`, 82 files
- Inner summary SHA-256:
  `8d6d927fba7a5e717dfb86442e0f7da5825ccfceec55b66bd20979c78e4f24bb`
- Inner artifact-manifest SHA-256:
  `a1401ea0115cc6d42f3c8b725cdae18935e194b32f27bb2056bbd6adc53c1469`
- Inner manifest rows verified: 36 of 36
- Exact Docker command-record SHA-256:
  `df7ceb31fab6310200c8826f6f0ea7b3d2c9e5b357a1897b2c8c4a0177e9533c`
- Executed wrapper SHA-256:
  `59f5b0903a2c99653bd0dd1101bf21d0cd55ab00c25f1a8cc7389a60c4261494`
- Release runner SHA-256:
  `ad54a6aa17e065ef2ed72e23ab9d60ac7026eb1a080614dc01cd8e9ae5e0cf51`
- Raw SM120 shared-library SHA-256:
  `97499d51c598b06af6d8a2787f016b92720bc17d30795984ad2af7bae1f45331`

Both manifest verifiers passed. The detached source stayed clean, the
container exited, compute-process inventory was empty afterward, and all four
GPUs returned to zero utilization.

## Claim boundary

This is the target-side companion to
`docs/cn4-current-tip-draft-k3-be825f4-20260804.md`. Together they prove the
current source-projection control on real target and recurrent-draft K=3
payloads. They do not prove K=4 execution, routed grouping, a complete layer,
TP4 collectives, checkpoint smoke, MTP, quality, KV capacity, or serving.
