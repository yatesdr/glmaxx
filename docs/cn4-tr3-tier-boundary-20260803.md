# cn4 TR3 target/draft tier boundary

Date: 2026-08-03

Status: read-only metadata discovery; no implementation, conversion, CUDA,
checkpoint admission, quality, capacity, or performance claim

## Result

The original mixed-K design treated all tier-map keys 3 through 78 as the
same 192-K3/64-K4 target-layer schema. The pinned checkpoint metadata
does not have that shape:

- target layers are `0..77`, with 75 sparse layers `3..77`;
- recurrent draft layer 78 is a separate sparse layer;
- every target sparse layer has a 256-entry `k` array containing exactly 192
  K=3 and 64 K=4 experts; and
- layer 78 has no `k` array. Its `keep_nvfp4` array is empty and `tail_tr3` is
  exactly experts 0 through 255.

A complete safetensors-header census independently checked every trellis
descriptor in layers 3 through 78, across gate/up/down and TP ranks 0 through
3:

| role | layers | K=3 trellis tensors | K=4 trellis tensors |
| --- | ---: | ---: | ---: |
| target sparse | 75 | 172,800 | 57,600 |
| recurrent draft | 1 | 3,072 | 0 |

Each target layer contains 2,304 K=3 plus 768 K=4 trellis tensors. Each
projection has 768/256 and each rank has 576/192. Draft layer 78 contains
3,072 K=3 trellis tensors: 1,024 per projection and 768 per rank. No other
width or malformed descriptor was present.

The corrected K4 source-plane delta is therefore:

```text
(1,572,864 - 1,179,648) bytes
  * 75 target layers * 64 K4 experts * 3 projections
= 5,662,310,400 bytes per rank
= 5.2734375 GiB per rank
```

The earlier v1 value, 5,737,807,872 bytes, incorrectly charged 64 K4 experts
in the uniform-K3 draft layer. Including rotations and markers, the complete
target-plus-draft routed source planes are 75,293,233,152 bytes
(70.122287750 GiB) per rank. Protected tensors and runtime allocations remain
separate budget terms.

## Procedure and provenance

`scripts/cn4-tr3-tier-boundary.sh` pinned the raw index and tier-map hashes,
read only the eight-byte safetensors prefixes and padded JSON headers for
`model-layer-003.safetensors` through `model-layer-078.safetensors`, and
validated every layer/projection/rank count. It did not read tensor payloads
or create a CUDA context.

```text
source commit       1d2d5997f633fa7952c01e26ecc4dd3e55076016
index SHA-256       f5dcd976a64ca70808dd4d8bd3ad07e9610c8ca6c30e3a6ed77ddefdac4c1d21
tier-map SHA-256    a287ffe816de5998fbc35a56a1ec05f69eb71087d5bbdfe631242c6b296b2a3d
verdict             TR3_TARGET_DRAFT_TIER_BOUNDARY_PASS
```

The 16-artifact evidence stream is outside Git at:

```text
/home/derek/glmaxx/evidence/20260803T191500Z-tr3-tier-boundary-1d2d599
```

Its `evidence-sha256.txt` SHA-256 is
`677eb23261b4ac25cf745be31911564ccca723e7c24f5239163a214c0e5e705c`;
every listed hash was revalidated from the evidence directory. GPU state was
unchanged at 2/2/2/10 MiB used and 0% utilization, with no compute process.

## Consequence

This metadata-only pass did not re-run publisher-manifest authentication; that
remains a separate mandatory admission gate.

`docs/exl3-mixed-k-source-and-kernel-v1.md` and its first handoff are invalid
as implementation authority. The r2 contract must distinguish target
192:64 binning from the uniform-K3 recurrent draft, correct all byte totals,
and test both schemas before mixed-K CPU or CUDA work begins.
