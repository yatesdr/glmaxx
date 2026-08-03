# SM120 W4A16/NF3 fused-MoE execution design v1 r2

Date: 2026-08-03

Status: corrective implementation design candidate; adversarial acceptance
required

GPU evidence: none

## Purpose and supersession

This amendment supersedes the implementation authority of
`sm120-w4a16-nf3-fused-moe-v1.md`. It retains v1's numerical policy, binding
record, locator/list ordering, deterministic routes, four kernel phases,
tile candidates, workspace arithmetic, and gate sequence except where
replaced here.

R2 closes every finding in
`sm120-w4a16-nf3-integration-audit-20260803.md`:

1. physical projection and layouts enter target/MTP program identity;
2. common plans contain no rank or CUDA address;
3. persistent bindings no longer depend on replaceable modules;
4. production capture uses rank-executor spans and graph nodes;
5. layer 78 binds a distinct MTP program; and
6. every workspace and routed/shared output maps to target-layer lifetimes;
   and
7. prefill retains the target layer's canonical expert/token/slot compaction.

No CPU or CUDA implementation is authorized until the applicable source,
manifest, target-layer, MTP, resident-generation, rank-executor, and this
design have each received their required adversarial acceptance.

## Layout-bound target and MTP programs

For the hybrid profile, every target-program binding is exactly 16 bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 4 | tensor ID |
| 4 | 2 | role ID |
| 6 | 2 | expert ID as signed little-endian; `-1` is nonexpert |
| 8 | 2 | codec ID |
| 10 | 1 | projection ID |
| 11 | 1 | reserved, zero |
| 12 | 2 | value-layout ID |
| 14 | 2 | scale-layout ID |

Projection IDs remain `0=nonrouted`, `1=split gate`, `2=split up`, `3=down`,
and `4=combined gate/up`. The hybrid W4A16/NF3 profile permits only:

| Representation | Projection | Codec | Value layout | Scale layout |
|---|---:|---:|---:|---:|
| protected/nonrouted | 0 | authenticated plain codec | 0 | 0 |
| ModelOpt fused FC1 | 4 | `0x0102` | `0x1201` | `0x1201` |
| ModelOpt down | 3 | `0x0102` | `0x1201` | `0x1201` |
| NF3 fused FC1 | 4 | `0x0300` | `0x1230` | `0x1231` |
| NF3 down | 3 | `0x0300` | `0x1230` | `0x1231` |

Split routed records, combined records under any other codec, nonzero plain
layouts, a mismatched layout pair, or scalar arity inconsistent with codec and
projection fail program construction. Codec `0x0102` plus projection 4 implies
two FC1 outer/input scalars; projection 3 implies one. Codec `0x0300` implies
no outer/input scalar.

Within one entry records are ordered by
`(role_id, expert_id, projection_id, tensor_id)`. All target-layer r2 entry
preimages retain their fields and order, but replace the ten-byte binding with
this record and use new entry domains:

```text
glmaxx.target-program.embedding.v3.hybrid-layout-bound\0
glmaxx.target-program.layer.v3.hybrid-layout-bound\0
glmaxx.target-program.final-head.v3.hybrid-layout-bound\0
glmaxx.target-program.v3.hybrid-w4a16-nf3-layout-bound\0
```

The resulting digest is `hybrid_target_program_sha256`. It replaces the
target-program field in StepPlan v4 and GraphProfile v2 without changing their
record sizes. Old target-program domains cannot name a hybrid graph.

Layer 78 uses the same binding encoding. Its successor is:

```text
SHA256(
  "glmaxx.mtp-program.v2.hybrid-w4a16-nf3-layout-bound\0" ||
  mtp_program_v1_sha256 ||
  u32_le(binding_count) ||
  16-byte bindings in canonical order
)
```

The result is `hybrid_mtp_program_sha256`. Both program digests are nonzero
and consensus-equal before a production hybrid profile allocates a binding
table. MTP0 skips draft graph selection; it does not omit, rename, or
reinterpret the authenticated layer-78 resident records.

The hybrid resident identity is also upgraded rather than relying on the
target-only v1 field set:

```text
SHA256(
  "glmaxx.resident-weight-set.v2.hybrid-w4a16-nf3\0" ||
  resident_weight_set_identity_v1 ||
  hybrid_target_program_sha256 ||
  hybrid_mtp_program_sha256 ||
  w4a16_nf3_numerical_policy_sha256 ||
  common_binding_semantic_sha256
)
```

The result is `HybridResidentWeightSetIdentityV2`. Changing either program,
numerical policy, or binding semantics is not a compatible hot reload.

## Address-free binding semantics

The v1 four-byte locator, 128-byte tier binding, 64-byte layer directory,
canonical ModelOpt/NF3 list ordering, and exact 2,573,312-byte table charge
remain unchanged.

For compacted execution, counts and prefixes remain indexed by global expert.
Work entries are ordered by global expert, token, then slot exactly as required
by the target-layer contract. The single 256-entry active-expert plane is an
execution view: active ModelOpt experts in ascending global ID followed by
active NF3 experts in ascending global ID. Each active entry points to its
unchanged contiguous range in the canonical work stream. Thus codec-specialized
launches do not change compacted bytes, route receipts, or reduction order.

One address-free semantic record is exactly 88 bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 2 | layer ID |
| 2 | 2 | global expert ID |
| 4 | 1 | tier |
| 5 | 1 | program kind: `1=target`, `2=draft` |
| 6 | 2 | tier-local expert ID |
| 8 | 4 | FC1 tensor ID |
| 12 | 4 | FC2 tensor ID |
| 16 | 2 | codec ID |
| 18 | 2 | FC1 value layout |
| 20 | 2 | FC1 scale layout |
| 22 | 2 | FC2 value layout |
| 24 | 2 | FC2 scale layout |
| 26 | 2 | reserved, zero |
| 28 | 8 | FC1 value/code bytes |
| 36 | 8 | FC1 scale bytes |
| 44 | 8 | FC2 value/code bytes |
| 52 | 8 | FC2 scale bytes |
| 60 | 24 | gate/up/down outer then input scalar F32 bits |
| 84 | 4 | codec flags; exactly one tier bit set |

The common binding semantic digest is:

```text
SHA256(
  "glmaxx.hybrid-binding-semantics.v2\0" ||
  hybrid_target_program_sha256 ||
  hybrid_mtp_program_sha256 ||
  u32_le(19,456) ||
  records ordered by layer, tier, ascending global expert
)
```

It contains no rank, arena offset, payload digest, TP-rank-bearing metadata
hash, allocation generation, handle, or CUDA address. All ranks must derive
identical bytes. Each rank separately validates its tensor IDs against its
catalog and its planes/scalars against authenticated codec metadata before
materialization.

## Persistent binding-table header v2

The table header is replaced by this exact 256-byte record:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `G5HBT02\0` |
| 8 | 2 | version, exactly 2 |
| 10 | 2 | header bytes, exactly 256 |
| 12 | 1 | TP rank |
| 13 | 1 | TP size, exactly 4 |
| 14 | 2 | layer count, exactly 76 |
| 16 | 8 | resident-weight generation |
| 24 | 8 | owner allocation generation |
| 32 | 8 | directory device address |
| 40 | 8 | first slab device address |
| 48 | 8 | directory bytes, exactly 4,864 |
| 56 | 8 | slab bytes, exactly 2,568,192 |
| 64 | 8 | total bytes, exactly 2,573,312 |
| 72 | 8 | flags, zero in v2 |
| 80 | 32 | hybrid target-program digest |
| 112 | 32 | hybrid MTP-program digest |
| 144 | 32 | HybridResidentWeightSetIdentity v2 |
| 176 | 32 | common binding semantic digest |
| 208 | 32 | rank-local materialization digest |
| 240 | 16 | reserved, zero |

The rank-local digest is:

```text
SHA256(
  "glmaxx.hybrid-binding-materialization.v2\0" ||
  rank_load_plan_sha256 ||
  weight_arena_base:u64_le || weight_arena_bytes:u64_le ||
  metadata_arena_base:u64_le || metadata_arena_bytes:u64_le ||
  header with bytes 208..239 zero ||
  directory bytes || slab bytes
)
```

It includes owner-materialized addresses and is never compared as though it
were rank-common. The coordinator instead accepts one common semantic digest
plus four ordered rank-local receipts. The table contains no runtime/module
generation and is unchanged by a compatible hot reload.

## Common plan v2

`GlmaxxHybridMoeCommonPlanV2` is 320 bytes, 16-byte aligned, and contains no
rank, native handle, allocation base, CUDA address, or module generation:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `G5HMCP2\0` |
| 8 | 2 | version, exactly 2 |
| 10 | 2 | bytes, exactly 320 |
| 12 | 1 | mode: `0=direct`, `1=compacted` |
| 13 | 1 | flags: bit 0 set only for draft layer 78 |
| 14 | 2 | layer ID |
| 16 | 4 | row bucket |
| 20 | 4 | maximum actual rows |
| 24 | 4 | maximum assignments, exactly `8*max_rows` |
| 28 | 2 | top-k, exactly 8 |
| 30 | 2 | FC2 tile N, initially 256 |
| 32 | 4 | hidden, exactly 6,144 |
| 36 | 4 | intermediate, exactly 512 |
| 40 | 2 | binding-directory ordinal, exactly `layer-3` |
| 42 | 1 | program kind: `1=target`, `2=draft` |
| 43 | 1 | argument-layout version, exactly 2 |
| 44 | 4 | reserved, zero |
| 48 | 8 | total independently aligned MoE workspace charge |
| 56 | 8 | work-entry offset in class 19 |
| 64 | 8 | FC1 intermediate offset in class 21 |
| 72 | 8 | slot-projection offset in class 22 |
| 80 | 8 | routed-partial offset in class 22 |
| 88 | 8 | count offset in class 19 |
| 96 | 8 | prefix offset in class 19 |
| 104 | 8 | active-expert offset in class 19 |
| 112 | 8 | status offset in the executor graph-status span |
| 120 | 8 | dynamic step-record offset in class 27 |
| 128 | 8 | normalized activation offset in class 16 |
| 136 | 8 | dense expert-ID offset in class 18 |
| 144 | 8 | dense route-weight offset in class 18 |
| 152 | 8 | routed FP32 output offset in class 22 |
| 160 | 32 | hybrid target-program digest |
| 192 | 32 | hybrid MTP-program digest |
| 224 | 32 | W4A16/NF3 numerical-policy digest |
| 256 | 32 | common-plan digest |
| 288 | 32 | reserved, zero |

The common-plan digest uses domain `glmaxx.hybrid-moe-common-plan.v2\0` and
hashes the complete record with bytes 256 through 287 zero. The output offset
must equal the routed-partial offset, and `row_bucket` must equal
`maximum_actual_rows`. Direct plans admit row buckets 1,2,4,8;
compacted plans admit only reviewed verify/prefill buckets above eight. Every
offset is 256-byte aligned and bounded by its exact graph-profile or status
span. Subspans within the same class are pairwise disjoint whenever their
lifetimes overlap; equal numeric offsets in distinct class bases are legal.

## Rank materialization and dynamic step

Only a persistent owner thread converts accepted executor spans into this
192-byte, 16-byte-aligned rank materialization:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `G5HMRM2\0` |
| 8 | 2 | version, exactly 2 |
| 10 | 2 | bytes, exactly 192 |
| 12 | 1 | TP rank |
| 13 | 1 | TP size, exactly 4 |
| 14 | 2 | flags, zero |
| 16 | 8 | resident-weight generation |
| 24 | 8 | runtime generation |
| 32 | 32 | common-plan digest |
| 64 | 8 | exact layer-directory device address |
| 72 | 8 | class-19 device base |
| 80 | 8 | class-21 device base |
| 88 | 8 | class-22 device base |
| 96 | 8 | class-27 device base |
| 104 | 8 | class-16 device base |
| 112 | 8 | class-18 device base |
| 120 | 8 | exact address derived from the graph-node status span |
| 128 | 32 | rank-local materialization digest |
| 160 | 32 | graph-profile-v2 digest |

The materialization digest uses domain
`glmaxx.hybrid-moe-rank-materialization.v2\0` and hashes the record with bytes
128 through 159 zero. Every address is derived from an adopted
`glmaxx_executor_span_v1`; no coordinator/request field supplies one. The
status address must equal checked `graph_status_base + status_offset` and
cannot alias a target buffer class.

`GlmaxxHybridMoeStepV2` is exactly 160 bytes and 16-byte aligned:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `G5HMST2\0` |
| 8 | 2 | version, exactly 2 |
| 10 | 2 | bytes, exactly 160 |
| 12 | 1 | mode, equal to common plan |
| 13 | 1 | flags, zero |
| 14 | 2 | reserved, zero |
| 16 | 8 | step ID |
| 24 | 4 | actual rows |
| 28 | 4 | assignments, exactly `8*rows` |
| 32 | 8 | route-buffer generation |
| 40 | 8 | input generation |
| 48 | 8 | output generation |
| 56 | 32 | common-plan digest |
| 88 | 32 | common dense-route receipt |
| 120 | 32 | logical step receipt |
| 152 | 8 | reserved, zero |

The step record contains no pointer. Its logical receipt uses domain
`glmaxx.hybrid-moe-step.v2\0` and binds the StepInput v3 hash, plan digest,
IDs, generations, rows, assignments, route receipt, and exact derived live
byte lengths. Fixed graph slots hold the dynamic arrays; their bucket tails
use the target-row valid mask and canonical zero/invalid encodings.

## Production graph integration

The common plan is an immutable target-program argument. The dynamic step is
uploaded into its class-27 slot only after four-rank preparation. Production
capture enters through `glmaxx_executor_graph_node_add_v1` with node kind
`GLMAXX_NODE_TARGET_PROGRAM` or `GLMAXX_NODE_MTP_PROGRAM`, an adopted module
handle, and an executor-owned descriptor span. The owner constructs the rank
materialization while resolving graph-profile spans.

The kernel module cannot allocate, discover a tensor, select a codec/layout,
change a graph slot, or choose a fallback. Direct launch symbols may exist for
isolated microbenchmarks under a distinct diagnostic ABI, but production Rust
cannot call them from `NativeCheckpointRankExecutor::execute_bound`.

Validation failure latches status before any data-dependent pointer use.
Later graph nodes write neutral fixed-capacity records and execute the same
collective ordinals on all ranks; publication fails after status consensus.
No rank skips a graph node or collective because its local latch is set.

## Buffer lifetimes and numerical boundary

The retained v1 workspace formulas and totals remain exact. Their ownership
is now closed:

- class 19 owns work entries, counts, prefix, and active experts;
- class 21 owns the BF16 post-SwiGLU assignment rows;
- class 22 owns the reusable FP32 slot tile and FP32 routed row partial;
- class 16 supplies normalized BF16 MLP input;
- class 18 supplies dense expert IDs and FP32 route weights;
- class 27 supplies immutable plan plus dynamic step records; and
- the executor graph-status span owns the status slice through completion.

Class 20 has zero persistent capacity for this fused FC1; gate/up accumulators
remain registers. The common plan's inclusive live charge is the independently
aligned sum of class-19, class-21, class-22, and its 256-byte status slice. Each
physical span is charged once in the graph/status ledger, never again as an
opaque kernel workspace.

The routed kernel ends with an FP32 row partial in class 22. The protected
shared-expert path remains outside this kernel. The following target-program
operation performs the one fixed rank-partial boundary:

```text
local_mlp_partial_bf16 =
  BF16_RNE(FP32(routed_partial_f32) + FP32(shared_partial_bf16))
```

It writes class 24 and enters exactly one BF16 MLP TP4 sum. Two reductions,
route-weight rounding before the slot-ordered FMA, a BF16 routed slot plane,
or adding the residual before the complete TP4 sum remain forbidden.

For M3072, class-22 slot scratch is one 256-column tile. FC2 tile production
and slot-ordered reduction are stream ordered before reuse. The complete
workspace remains 126,225,408 bytes; the 603,979,776-byte untiled slot plane
remains forbidden. Every class capacity enters GraphProfile v2, both runtime
generation slots, and the physical MTP3 HBM ledger.

## Hot reload

A compatible reload keeps the HybridResidentWeightSetIdentity, binding header,
directory, slab, common semantic digest, and every weight/metadata pointer
unchanged. It may change the runtime generation, module, tuning selection,
graph-profile materialization, and rank-materialization digest within already
reserved ceilings. Common plan semantics either remain identical or select a
different already reviewed plan digest collectively.

Prepare, canary, quiesce, commit, rollback, and retirement record unchanged
model-open, model-read, staging, and weight-H2D counters. Reuploading the
2,573,312-byte binding table during a compatible reload is forbidden as well;
only replaceable argument/materialization records may change.

## Corrected gates and nonclaims

After every prerequisite design and r2 are accepted, the CPU proof must add:

1. exact encode/decode/mutation tests for the 16-, 88-, 256-, 320-, 192-, and
   160-byte records and every digest self-exclusion;
2. four-rank proof that common semantic/plan/step bytes match while local
   materialization bytes and CUDA addresses differ;
3. target/MTP program rejection for every projection/codec/layout/scalar-arity
   cross-product outside the closed table;
4. a hot-reload proof that persistent table bytes and all weight-traffic
   counters remain unchanged;
5. graph-profile live-range simulation for every class and row bucket,
   including tiled FC2 reuse and neutral-latch collectives; and
6. FP32 routed plus protected shared to BF16 rank-partial equivalence before
   the single TP4 sum.

Implementation review then precedes any SM120 microbenchmark. Real mixed-tier
TP4 layer replay precedes checkpoint smoke; MTP0 quality precedes MTP3.

This candidate does not accept any prerequisite, target/MTP program, parser,
packer, manifest, resident arena, binding table, ABI, graph, kernel,
checkpoint, quality result, capacity result, hot reload, cold start, or
performance result. It authorizes no cn4 execution.
