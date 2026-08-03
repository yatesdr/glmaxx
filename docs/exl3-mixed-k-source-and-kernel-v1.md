# EXL3 mixed-K source and kernel contract v1

Date: 2026-08-03

Status: superseded; do not review or implement this v1 contract

This candidate incorrectly requires a `k` array for recurrent draft layer 78
and charges that layer as another 192:64 target mixture. The pinned TR3
tier map and every layer-78 safetensors header instead prove a uniform K=3
draft layer with no `k` field. The corrective contract is
`docs/exl3-mixed-k-source-and-kernel-v1-r2.md`; only its separate r2 review may
open implementation.

## Problem

The real GLM-5.2 TR3 3.25-bpw checkpoint is not a uniform 3-bit checkpoint.
For each sparse layer its tier map selects 192 K=3 experts and 64 K=4 experts.
GLMAXX currently hard-codes `bits=3` in the source proof, checkpoint plan,
native metadata validator, Rust CUDA ABI, and CUDA control kernel. A real K=4
tensor therefore fails structural validation even though its source shape is
correct.

Mixed-K support must not introduce a per-weight branch, a dense reconstructed
weight cache, rank-local decisions, or a weak tier-map hint. It must retain
the source-order packed representation and keep the extra 0.25 bpw visible to
memory planning.

## Source contract

The only admitted EXL3 bit widths are 3 and 4. For every routed expert and TP
rank, the reader derives the physical bit width from the already-validated
trellis descriptor:

```text
shape = [logical_k / 16, logical_n / 16, 16 * bits]
bits in {3, 4}
```

The value is never supplied by a caller and never inferred from byte length
alone. `mcg`, `suh`, and `svh` retain their existing exact dtype, shape, and
byte contracts. Checked arithmetic binds the trellis byte count to the full
descriptor shape.

Production checkpoint admission also parses the content-authenticated
`tier_bitmap.json`. It requires exactly layers 3 through 78, exactly 256 `k`
entries per layer, and only integer values 3 or 4. For a given layer/expert,
the tier-map value must equal the derived trellis width for gate, up, and down
on all four TP ranks. Any missing, extra, non-integral, inconsistent, or
unsupported membership fails the complete checkpoint before conversion or
device allocation. The complete tier-map digest remains part of the source
profile and rank-manifest provenance.

A directory-only source proof may diagnose tensors before publisher
authentication, but it reports `production_admitted=false` and cannot publish
a native rank set. Production conversion continues to require a passing
content manifest and immutable source identity; mixed-K support creates no
checkpoint-name, digest, metadata, or weight exception.

## Native representation and accounting

`Exl3Metadata` already serializes `bits`, `trellis_words`, and a CRC in its
96-byte wire representation. Wire version 1 remains unchanged because the
field and its byte accounting are already explicit. The validator expands
the admitted value set from only 3 to exactly `{3,4}`. Old readers continue
to reject K=4; new readers accept existing K=3 records without reinterpretation.

The rank plan is constructed from the validated tier map, not a global bits
constant. Every tensor descriptor and operation-manifest row binds its own
bit width, primary bytes, auxiliary bytes, and metadata digest. Native rank
readers recompute and validate those values before exposing a tensor.

For the GLM-5.2 routed shape, each projection/rank has:

| Width | Trellis bytes | Rotation + marker bytes | Total source-plane bytes |
|---|---:|---:|---:|
| K=3 | 1,179,648 | 13,316 | 1,192,964 |
| K=4 | 1,572,864 | 13,316 | 1,586,180 |

Replacing 64 of 256 experts with K=4 across 76 sparse layers and three
projections adds exactly 5,737,807,872 bytes (5.34375 GiB) per TP rank versus
the uniform K=3 routed plan. The load planner must use the exact admitted map
and checked tensor sums; it may not use the 3.25 average as allocation
authority. Packed trellis planes remain resident and are never expanded into
persistent dense weights, preserving room for the target NVFP4 KV arena.

## CPU proof and CLI

The source proof derives and reports `bits`, `tier_map_sha256`, tier-map
membership, trellis shape, exact component hashes, native metadata hash, and
reconstructed FP16 digest. A production-strength invocation requires the
tier-map path explicitly:

```text
glmaxx exl3-safetensors-proof-v2 \
  checkpoint-or-index tier_bitmap.json layer expert rank gate|up|down
```

The proof matrix covers:

1. K=3 and K=4 synthetic tensors for gate/up/down and ranks 0 through 3;
2. an independent forward-scatter reconstruction for both widths, including
   off-diagonal tiles and boundary bit windows;
3. real layer-3 K=3 expert 0 and K=4 expert 6 for all three projections on
   ranks 0 and 3;
4. deterministic repeat digests for every real projection;
5. trellis third dimensions 47, 49, 63, 65, zero, overflow, wrong dtype, and
   byte-accounting mutations;
6. missing/extra layers or experts, values other than integer 3 or 4,
   projection disagreement, rank disagreement, and tier-map/tensor mismatch;
7. native metadata encode/decode and rank-manifest round trips containing
   both widths; and
8. exact aggregate rank bytes independent of JSON or tensor iteration order.

## SM120 kernel ABI and execution

The EXL3 kernel ABI advances to
`glmaxx.sm120.exl3.source_projection.v2` and ABI version 2 while preserving
the 144-byte, 16-byte-aligned descriptor. `descriptor.bits` must be exactly 3
or 4. Rust and CUDA perform identical checked geometry, pointer, workspace,
reserved-field, and SM120 device validation.

CUDA provides compile-time-specialized K=3 and K=4 decode implementations.
The launcher validates once and dispatches on `descriptor.bits` outside the
weight loop; no inner-loop width branch is permitted. The control kernel must
match the CPU oracle bit-for-bit for both widths before an optimized kernel is
benchmarked.

The optimized routed path partitions active experts into deterministic K=3
and K=4 bins using the admitted tier map, then launches the corresponding
specialization. Partitioning preserves canonical token/slot ordering and the
same accumulation and collective contract. All four ranks derive and hash the
same partition plan before launch. A rank-local fallback, silent width
substitution, or collective route change is forbidden.

K=3 and K=4 are benchmarked separately at actual GLM-5.2 decode and prefill
shapes, followed by a matched 192:64 layer replay. Evidence separates source
decode/projection kernel time, routing/binning overhead, collectives, and
end-to-end time. Performance acceptance cannot change precision membership,
batch, context, cache posture, or routing relative to its control.

## Gate sequence

Implementation proceeds only after adversarial design acceptance:

1. CPU parser, tier-map consensus, metadata, accounting, and real-source
   proof;
2. adversarial review of the implementation and pinned real-source evidence;
3. SM120 K=4 control launch and CPU-oracle comparison;
4. template-specialized K=3/K=4 microbenchmarks;
5. mixed 192:64 one-layer replay;
6. authenticated checkpoint smoke; and
7. matched quality and end-to-end decode benchmarks.

This design does not accept the current checkpoint manifest, a CUDA
implementation, conversion, quality, or performance result.
