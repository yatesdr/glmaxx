# SM120 W4A16/NF3 fused-MoE execution design v1

Date: 2026-08-03

Status: implementation design candidate; adversarial acceptance required

GPU evidence: none

## Purpose and prerequisites

This note defines the first Rust-owned execution ABI for the real GLM-5.2
ModelOpt-NVFP4/NF3 routed experts. It consumes, but does not amend:

- `nf3-nvfp4-hybrid-source-and-kernel-v1-r2.md`;
- `nf3-nvfp4-native-rank-manifest-v1-r2.md`;
- `target-layer-execution-v1-r2.md`; and
- `resident-weight-runtime-generation-v1.md`.

No CPU or CUDA implementation is authorized until the applicable source,
manifest, target-layer, resident-weight, and this design have each received
their required adversarial acceptance. The first CUDA implementation is
ModelOpt W4A16 plus NF3. The existing W4A4 CUTLASS control remains a separate
diagnostic and cannot enter this graph.

The design has four goals:

1. consume the immutable tensor-ID-ordered arena without moving weights;
2. make MTP0 C1 and MTP3 C1 small-row execution the primary kernel shape;
3. keep prefill scratch bounded without changing FP32 reduction semantics;
4. permit compatible cubin and graph generations to change while the weight
   arena remains resident.

## Closed geometry and arithmetic

The production specialization is exact:

```text
SM target                    sm_120f only
TP                           4
target sparse layers         3..77
draft sparse layer           78
experts per layer            256
target tier counts           64 ModelOpt W4A16 + 192 NF3
draft tier counts            256 ModelOpt W4A16 + 0 NF3
top-k                        8
hidden                       6,144
rank-local intermediate      512
FC1 logical shape            [1,024,6,144]
FC2 logical shape            [6,144,512]
MMA                          m16n8k16 BF16 x BF16 -> FP32
```

Codec `0x0102` reconstructs E2M1 times E4M3FN as BF16 register fragments.
Codec `0x0300` reconstructs the pinned NF3 codebook times E4M3FN as BF16
register fragments. Activations stay BF16 and accumulation stays FP32.
Neither path creates a persistent dense weight plane. ModelOpt gate and up
outer scales are applied independently to their FP32 projections before
SwiGLU; the down outer scale is applied before route weighting. NF3 has no
invented outer scalar.

## Resident binding table

Payload descriptors retain tensor-ID order. Kernel locality is supplied by a
small address table, never by copying or reordering weight payloads.

For each layer, global expert IDs are stably partitioned into ModelOpt then
NF3 lists, preserving ascending global expert ID within each tier. A
256-entry locator maps global expert to `(tier, local_id)`. Target layers
must have list lengths 64 and 192; layer 78 must have lengths 256 and zero.

Each locator is exactly four bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 1 | tier: `0=ModelOpt W4A16`, `1=NF3` |
| 1 | 1 | tier-local expert ID |
| 2 | 2 | reserved, zero |

Each tier binding is 128 bytes and 16-byte aligned:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | FC1 value/code device address |
| 8 | 8 | FC1 scale device address |
| 16 | 8 | FC2 value/code device address |
| 24 | 8 | FC2 scale device address |
| 32 | 8 | FC1 value/code bytes |
| 40 | 8 | FC1 scale bytes |
| 48 | 8 | FC2 value/code bytes |
| 56 | 8 | FC2 scale bytes |
| 64 | 8 | FC1 codec-metadata device address |
| 72 | 8 | FC2 codec-metadata device address |
| 80 | 2 | global expert ID |
| 82 | 2 | codec ID |
| 84 | 2 | FC1 value layout ID |
| 86 | 2 | FC1 scale layout ID |
| 88 | 2 | FC2 value layout ID |
| 90 | 2 | FC2 scale layout ID |
| 92 | 4 | gate outer-scale F32 bits |
| 96 | 4 | up outer-scale F32 bits |
| 100 | 4 | down outer-scale F32 bits |
| 104 | 4 | gate input-scale F32 bits, authenticated but unused |
| 108 | 4 | up input-scale F32 bits, authenticated but unused |
| 112 | 4 | down input-scale F32 bits, authenticated but unused |
| 116 | 4 | flags |
| 120 | 8 | reserved, zero |

Flags permit only bit 0 for ModelOpt and bit 1 for NF3, exactly one set.
ModelOpt fields must match codec `0x0102`, layouts `0x1201/0x1201`, exact
plane lengths, finite positive outer/input scalars, and the fused/down scalar
arity. NF3 fields must match codec `0x0300`, layouts `0x1230/0x1231`, exact
plane lengths, and positive-zero bits in all six scalar slots.

The complete table contains 76 layer slabs. Each slab is 1,024 locator bytes
followed by 256 bindings in canonical tier-list order:

```text
per-layer slab                  1,024 + 256*128 = 33,792
76 layer slabs                                  = 2,568,192
256-byte header + 76*64 directory entries       =     5,120
complete device binding table                   = 2,573,312 bytes/rank
```

The 256-byte header binds ABI version, TP rank/size, active weight generation,
active module generation, target-program digest, rank load-plan digest,
logical binding-receipt digest, directory address, and slab address. Each
64-byte directory entry binds layer ID, draft/target discriminator, exact tier
counts, locator address, both tier-list addresses, and reserved zero bytes.

The header is exact; all integers are little-endian:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `G5HBT01\0` |
| 8 | 2 | version, exactly 1 |
| 10 | 2 | header bytes, exactly 256 |
| 12 | 1 | TP rank |
| 13 | 1 | TP size, exactly 4 |
| 14 | 2 | layer count, exactly 76 |
| 16 | 8 | active weight generation |
| 24 | 8 | active module generation |
| 32 | 8 | directory device address |
| 40 | 8 | first slab device address |
| 48 | 8 | directory bytes, exactly 4,864 |
| 56 | 8 | slab bytes, exactly 2,568,192 |
| 64 | 8 | total bytes, exactly 2,573,312 |
| 72 | 8 | flags, zero in v1 |
| 80 | 32 | target-program digest |
| 112 | 32 | rank load-plan digest |
| 144 | 32 | logical binding-receipt digest |
| 176 | 32 | rank-local materialization digest |
| 208 | 48 | reserved, zero |

The directory begins at `table_base+256`; the slabs begin at
`table_base+5,120`. A directory entry is exact:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 2 | layer ID |
| 2 | 2 | flags: bit 0 set only for draft layer 78 |
| 4 | 2 | ModelOpt count |
| 6 | 2 | NF3 count |
| 8 | 8 | locator device address |
| 16 | 8 | ModelOpt binding-list device address |
| 24 | 8 | NF3 binding-list device address, zero when count is zero |
| 32 | 4 | locator bytes, exactly 1,024 |
| 36 | 4 | binding stride, exactly 128 |
| 40 | 4 | complete binding bytes, exactly 32,768 |
| 44 | 20 | reserved, zero |

The materialization digest uses domain
`glmaxx.hybrid-binding-materialization.v1\0` and hashes the header with bytes
176 through 207 zero followed by the directory and slab bytes. It is rank
local because it includes CUDA addresses. It is never substituted for the
address-free logical receipt compared by all ranks.

The logical binding receipt hashes descriptor IDs, codecs, layouts, arena
offsets and lengths, scalar bits, tier order, and generation. CUDA virtual
addresses are deliberately absent from that digest. Rust materializes each
device address as checked `arena_base + offset`, proves every span lies in the
active immutable allocation, and compares the logical receipt on all ranks
before publication. CUDA does no metadata parsing and no pointer discovery.

## Dense route and work records

The protected router supplies the accepted token-major arrays:

```text
expert[rows][8] : u16
weight[rows][8] : f32
```

The upstream target phase validates all eight distinct in-range experts,
finite nonnegative weights including positive-zero spelling, and one common
route receipt across ranks. This implementation repeats bounds checks before
using the locator but does not add a rank-local fallback.

A materialized work entry is exactly 16 bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 4 | token row |
| 4 | 1 | top-k slot |
| 5 | 1 | tier |
| 6 | 2 | tier-local expert ID |
| 8 | 2 | global expert ID |
| 10 | 2 | reserved, zero |
| 12 | 4 | route-weight F32 bits |

For rows `1..8`, the direct path expands exactly `8*rows` entries in token,
then slot order. Each CTA's codec decision is uniform because a work tile
belongs to one entry. For larger row buckets, validate/count/prefix/scatter
stably orders work by codec, global expert, token, then slot. Count and prefix
are integer-only and scatter writes disjoint ranges. The compacted stream's
qualification digest is retained as evidence; production relies on the
already compared dense-route receipt, fixed algorithm, binding generation,
and status propagation.

## Kernel phases

The initial accepted path uses stream-ordered graph nodes. It does not assume
an inter-CTA barrier:

1. validate and materialize direct or compacted work;
2. FC1 decode/MMA/epilogue writes one BF16 `[512]` row per assignment;
3. FC2 decode/MMA writes disjoint FP32 slot projections; and
4. a reducer performs slot-ordered `fma.rn(weight, projection, accumulator)`
   and writes the FP32 routed partial.

FC1 applies ModelOpt gate/up outer scales independently in FP32 and evaluates
the pinned non-fast-math SwiGLU boundary before BF16 RNE output. FC2 applies
the ModelOpt down outer scale in FP32 but never pre-multiplies and rounds the
route weight. The reducer starts at positive zero and processes slots 0
through 7 exactly. Floating atomics, shared-output accumulation, and BF16
slot partials are forbidden.

The first tile candidates are the audited reference values:

```text
FC1 K tile 64, N tile 256
FC2 K tile 64, N tile 256
```

They are controls, not fixed winners. The SM120 sweep may vary warp count,
pipeline stages, N tile, K tile, register staging, and CTA scheduling while
preserving the exact arithmetic and work order. M1 and M4 are the primary
optimization points for MTP0 and MTP3; M2 and M8 are mandatory neighboring
controls.

A later persistent one-grid variant may replace multiple nodes only after it
proves cooperative residency, legal grid synchronization, identical output,
capturability, and a measured inclusive win. An occupancy assumption or
spin barrier without a cooperative launch contract is rejected.

## Bounded workspace

All offsets are 256-byte aligned and precomputed by checked Rust arithmetic.
There is no allocation, compilation, descriptor construction, or repack in a
captured graph.

For `A=8*rows`, the direct path owns:

```text
work entries               A * 16
FC1 BF16 intermediate      A * 512 * 2
FP32 slot projection       rows * 8 * 6,144 * 4
FP32 routed partial        rows * 6,144 * 4
```

Each plane is independently rounded up to 256 bytes; planes are never packed
into another plane's alignment tail.

At the largest direct bucket `rows=8`, including aligned count, offset,
active-expert, and status planes, the workspace is exactly 1,839,104 bytes.

Large-row execution bounds the dominant slot plane by FC2 N tile. For
`tile_n=256`, one tile owns `rows*8*256*4` slot bytes, reduces that tile in
slot order, writes its disjoint final columns, and reuses the slot plane only
after the stream-ordered reducer completes. At the fixed prefill maximum
`rows=3,072`:

```text
work entries                         393,216
FC1 BF16 intermediate             25,165,824
one FP32 slot tile                25,165,824
complete FP32 routed partial      75,497,472
aligned fixed route/status planes     3,072
total                            126,225,408 bytes
```

The untiled 603,979,776-byte prefill slot plane is forbidden. A graph may
serialize the 24 FC2 tiles or use a separately qualified persistent schedule;
it may not overlap reuse of the same tile scratch. Exact module, graph,
binding-table, and maximum-workspace bytes enter the startup HBM ledger and
the 524,288-token MTP3 physical allocation gate.

## Host launch ABI

`GlmaxxHybridMoePlanV1` is exactly 256 bytes and 16-byte aligned:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `G5HMOE1\0` |
| 8 | 2 | version, exactly 1 |
| 10 | 2 | descriptor bytes, exactly 256 |
| 12 | 1 | TP rank |
| 13 | 1 | TP size, exactly 4 |
| 14 | 1 | mode: `0=direct`, `1=compacted` |
| 15 | 1 | flags, zero in v1 |
| 16 | 2 | layer ID |
| 18 | 2 | row bucket |
| 20 | 2 | top-k, exactly 8 |
| 22 | 2 | FC2 tile N, exactly 256 in the first candidate |
| 24 | 4 | hidden, exactly 6,144 |
| 28 | 4 | intermediate, exactly 512 |
| 32 | 4 | maximum actual rows |
| 36 | 4 | maximum assignments, exactly `8*max_rows` |
| 40 | 8 | active weight generation |
| 48 | 8 | active module generation |
| 56 | 8 | exact layer-directory device address |
| 64 | 8 | workspace base address |
| 72 | 8 | workspace bytes |
| 80 | 8 | work-entry offset |
| 88 | 8 | FC1 intermediate offset |
| 96 | 8 | slot-projection offset |
| 104 | 8 | routed-partial offset |
| 112 | 8 | count offset |
| 120 | 8 | prefix offset |
| 128 | 8 | active-expert offset |
| 136 | 8 | status offset |
| 144 | 32 | target-program digest |
| 176 | 32 | numerical-policy digest |
| 208 | 32 | plan digest |
| 240 | 16 | reserved, zero |

The plan digest uses domain `glmaxx.hybrid-moe-plan.v1\0` and hashes the
complete descriptor with bytes 208 through 239 zero. Workspace offsets are
strictly ascending, 256-byte aligned, and their bucket-derived extents end at
or before `workspace_bytes`; unused gaps and tails are zero at prepare time.

`GlmaxxHybridMoeStepV1` is exactly 192 bytes and 16-byte aligned:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `G5HMST1\0` |
| 8 | 2 | version, exactly 1 |
| 10 | 2 | descriptor bytes, exactly 192 |
| 12 | 1 | mode, equal to plan mode |
| 13 | 1 | flags, zero in v1 |
| 14 | 2 | reserved, zero |
| 16 | 8 | step ID |
| 24 | 4 | actual rows |
| 28 | 4 | assignments, exactly `8*rows` |
| 32 | 8 | route-buffer generation |
| 40 | 8 | input generation |
| 48 | 8 | output generation |
| 56 | 8 | BF16 activation address |
| 64 | 8 | dense expert-ID address |
| 72 | 8 | F32 route-weight address |
| 80 | 8 | FP32 routed-output address |
| 88 | 8 | activation bytes |
| 96 | 8 | expert-ID bytes |
| 104 | 8 | route-weight bytes |
| 112 | 8 | output bytes |
| 120 | 32 | common dense-route receipt |
| 152 | 32 | logical step receipt |
| 184 | 8 | reserved, zero |

The logical step receipt uses domain `glmaxx.hybrid-moe-step.v1\0` and binds
the plan digest, IDs, generations, rows, assignments, logical byte lengths,
route receipt, and upstream activation receipt. CUDA virtual addresses are
excluded so all ranks can compare one receipt before execution.

All reserved bytes are zero. Actual rows must be nonzero and no larger than
the plan bucket; assignments must equal `8*rows`. Activation, expert, weight,
and output lengths must equal `rows*6,144*2`, `rows*8*2`, `rows*8*4`, and
`rows*6,144*4` bytes respectively. Their addresses are 256-byte aligned.
Dynamic ranges are bounded, pairwise disjoint, and disjoint from immutable
weights, binding metadata, and every live workspace plane. The wrapper
validates SM 120, ABI sizes, generations, digests, codec/layout membership,
bucket, workspace receipt, and a clear status word before launch. Any rank's
failure is promoted to one collective step failure before publication.

The C ABI exposes separate prepare/execute symbols for direct and compacted
plans. Rust owns their lifetime and status. The kernel library cannot select a
different variant, allocate fallback scratch, compile, move weights, or change
precision. A compatible hot reload prepares a new module/plan generation,
validates it on all ranks against the existing logical binding receipt,
quiesces at a physical-step boundary, and publishes or rolls back atomically.
Weight-read and weight-H2D counters must remain unchanged.

## Required gates

After all prerequisite designs are accepted, the CPU proof must:

1. reconstruct every NF3 and ModelOpt value at the actual FC1/FC2 shapes;
2. prove separate gate/up/down scalar application and reject scalar collapse;
3. derive all 19,456 expert bindings and every locator on all four ranks;
4. prove address ranges, tier counts, table bytes, reserved bytes, and receipt
   mutation behavior with checked arithmetic;
5. prove direct and compacted work order for every M1/M2/M4/M8 route pattern,
   empty/skewed prefill experts, and malformed routes;
6. preserve per-position FP32 projection, BF16 intermediate, and ordered
   reduction values against source-expanded controls; and
7. reproduce the exact workspace layouts for every graph bucket.

The SM120 microbenchmark then requires real `sm_120f` cubins, the intended
BF16 MMA instruction family, no local-memory spills, recorded registers and
shared memory, guard-zone-clean actual-shape buffers, two-run determinism,
and separate route/FC1/FC2/reducer/launch/inclusive timings. It sweeps M1/2/4/8
and representative prefill buckets with all-ModelOpt, all-NF3, real 64/192,
and adversarial tier-skew routes. The audited SparkInfer source is a pinned
control only; its claims are not GLMAXX evidence.

Only a passing mixed-codec TP4 real layer replay may open checkpoint smoke.
MTP0 checkpoint quality precedes MTP3. End-to-end speed claims require the
same precision membership, route bytes, context, KV posture, batch, and
quality gates as their controls.

## Nonclaims

This candidate does not accept any prerequisite design, parser, packer,
manifest, resident arena, binding table, ABI, kernel, checkpoint, layer,
quality result, capacity result, hot reload, cold start, or performance
result. It does not authorize implementation or cn4 execution.
