# GLM-5.2 target-layer execution contract v1 r2 amendment

Date: 2026-08-03

Status: design candidate; adversarial acceptance required before CPU or CUDA
implementation

GPU evidence: none

## Scope and precedence

This amendment resolves every finding in
`docs/reviews/fable-target-layer-execution-v1.md`. It is normative over the
conflicting or incomplete text in `docs/target-layer-execution-v1.md` and
amends the still-unimplemented step-execution v3 candidate. No earlier target
program, step plan/input, graph profile, or collective schedule may coexist in
one process with these bytes.

The retained model, revision, source, tensor shapes, layer/indexer modes,
attention routes, MoE rules, record arithmetic, transaction boundary, and
nonclaims remain unchanged.

## Source facts and numerical membership

The exact official config at revision
`b4734de4facf877f85769a911abafc5283eab3d9` hashes as
`185f93ee6d12548e16a847e279dc0c3c90b1524c970b0866b42fb545747d859a`.
It fixes:

```text
vocab_size                         154880
rms_norm_eps                       1e-5
rope_parameters.rope_type          default
rope_parameters.rope_theta         8000000
moe_router_dtype                   float32
```

There is no yarn factor or mscale. Input, post-attention, and final RMSNorm
use `1e-5`. Q-A and KV-A RMSNorm are constructed without a config epsilon in
the pinned source and use the class default `1e-6`. Indexer K LayerNorm also
uses `1e-6`. Those six sites are distinct fields in the CPU oracle and
kernel descriptors.

Every protected projection consumes BF16 operands, accumulates FP32, and
rounds once to BF16 at its declared output. Each TP rank's attention and MLP
projection partial is BF16. The reviewed TP4 collective route consumes and
returns BF16 and is part of the matched quality posture. Both residuals are:

```text
BF16_RN(FP32(residual_bf16) + FP32(tp4_sum_bf16))
```

Changing a partial, collective, residual input/output, or intermediate to
FP32 is a different precision posture and requires a matched control; fusion
cannot move either residual before the complete TP4 sum.

The pinned interleaved RoPE reads `(x[2i],x[2i+1])` pairs but stores its
64-value output in de-interleaved half order:

```text
rotated_first[0..32] || rotated_second[0..32]
```

The KV record's 64 E4M3 RoPE bytes use exactly that order. An indexer record
stores that 64-value de-interleaved rotated prefix followed by its 64
unrotated pass-through values. Neither wire re-interleaves the output.

## Exact target-program digest ABI

`TargetProgram.v1` uses no native-struct hashing and no implicit padding.
All integers are little-endian. Enum values not named below are invalid.

One tensor binding is exactly ten bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 4 | `tensor_id: u32` |
| 4 | 2 | `role_id: u16` |
| 6 | 2 | `expert_id: i16`; `-1` means nonexpert |
| 8 | 2 | `codec_id: u16` |

Bindings are strictly ascending by `tensor_id`, unique, and selected from the
already consensus-equal rank-invariant semantic catalog. The catalog's
layer, TP axis, shapes, roles, and codec must match the entry before hashing;
offsets, source slices, and payload digests remain rank-manifest identities.

Exact enum bytes are:

```text
index_mode       FULL=1 SHARED=2
mlp_mode         DENSE=1 SPARSE=2
tp_rule          VOCAB_AXIS0_ONE_SUM=1
                 HEAD_AXIS0_O_AXIS1_ONE_SUM=2
                 DENSE_AXIS0_DOWN_AXIS1_ONE_SUM=3
                 ROUTED_SHARED_AXIS0_DOWN_AXIS1_ONE_SUM=4
                 VOCAB_AXIS0_NO_SUM=5
```

The embedding-entry digest is:

```text
SHA256(
  "glmaxx.target-program.embedding.v1\0" ||
  u16_le(1) ||                         # entry version
  u16_le(0x0001) ||                    # embedding role
  u8(VOCAB_AXIS0_ONE_SUM) || u8(4) ||  # TP rule, rank count
  u32_le(154856) || u32_le(154880) ||
  for rank 0..3: u32_le(rank*38720) || u32_le((rank+1)*38720) ||
  u16_le(binding_count) || bindings
)
```

There is exactly one embedding binding and its expert is `-1`.

One layer-entry digest is:

```text
SHA256(
  "glmaxx.target-program.layer.v1\0" ||
  u16_le(1) ||
  u8(layer_id) || u8(index_group_id) ||
  u8(index_mode) || u8(mlp_mode) ||
  u8(HEAD_AXIS0_O_AXIS1_ONE_SUM) ||
  u8(mlp_mode == DENSE
       ? DENSE_AXIS0_DOWN_AXIS1_ONE_SUM
       : ROUTED_SHARED_AXIS0_DOWN_AXIS1_ONE_SUM) ||
  u16_le(allowed_phase_variant_mask) ||
  target_phase_template_sha256 ||
  target_buffer_lifetime_sha256 ||
  u16_le(binding_count) || bindings
)
```

Layer IDs are `0..77`. Group IDs are `0..20`. The full/shared and dense/sparse
maps are the exact lists in v1. A layer binding contains all and only that
layer's protected tensors and, for sparse layers, both routed roles for all
256 experts. FULL adds all five indexer bindings; SHARED adds none.

The final-head digest is:

```text
SHA256(
  "glmaxx.target-program.final-head.v1\0" ||
  u16_le(1) ||
  u16_le(0x0003) || u16_le(0x0002) ||   # final norm, LM head
  u8(VOCAB_AXIS0_NO_SUM) || u8(4) ||
  u32_le(154856) || u32_le(154880) ||
  u32_le(154856) || u32_le(154880) ||  # masked physical interval
  for rank 0..3: u32_le(rank*38720) || u32_le((rank+1)*38720) ||
  distributed_sampling_abi_sha256 ||
  u16_le(binding_count) || bindings
)
```

There are exactly two nonexpert bindings. The term for the head is always
`vocabulary-axis-0 sharded`; “column parallel” and “vocabulary-row parallel”
are not alternate ABI names.

`distributed_sampling_abi_sha256` is the composite identity defined by the
corrective sampling successor, not the hash of the retained v1 file alone:

```text
SHA256(
  "glmaxx.distributed-sampling-abi.v1-r2\0" ||
  SHA256(exact docs/distributed-sampling-abi-v1.md bytes) ||
  SHA256(exact docs/distributed-sampling-abi-v1-r2.md bytes)
)
```

For the exact pinned inner hashes
`383e328a527cc780ed553af0b78382cf200ad60f97afb26d96a2a1494b57c89b`
and
`f2fb8ec8c81c63e76b7a0639fddc8c74719faff2a972bafcdf0b1d5de8db3db7`,
the composite is
`8edd0d940273ee2e242b8164b611b8d997f7616f4618b0c1d894ea4dc114aa0f`.
An absent or unaccepted r2 successor, either changed inner file, a different
composite, or a rank-local sampling contract prevents target-program
construction. This dependency creates no hash cycle: sampling binds the two
raw design files, while the final-head entry consumes only their composite.

The top-level hash remains:

```text
SHA256(
  "glmaxx.target-program.v1\0" ||
  embedding_entry_sha256 || u16_le(78) ||
  layer_entry_sha256[0] || ... || layer_entry_sha256[77] ||
  final_head_entry_sha256
)
```

## Phase-template digest and dependencies

The global `TargetPhaseTemplate.v1` is serialized as:

```text
"glmaxx.target-phase-template.v1\0"
u16_le(1)                       # version
u16_le(5)                       # variant_count
variants in strictly ascending variant_id
```

Each variant begins `variant_id:u8, operation_count:u8, zero:u16`, followed
by its operations in execution order. One operation is exactly twelve bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 1 | phase ID from `CollectiveOp.v2` |
| 1 | 1 | collective predecessor count `0..3` |
| 2 | 3 | predecessor phase IDs, ascending; unused bytes zero |
| 5 | 1 | local-prerequisite mask |
| 6 | 2 | flags required |
| 8 | 2 | flags forbidden |
| 10 | 2 | graph slot class |

The SHA-256 of these bytes is `target_phase_template_sha256`. The exact
variants and phase order are:

```text
1 EMBEDDING             8
2 FULL_QUERY            1,2,3,4,5
3 SHARED_QUERY          1,3,4,5
4 FULL_CKV              7,6,4,5
5 CKV_WITH_WINNERS      6,4,5
```

Direct collective predecessors are:

```text
FULL_QUERY:       2<-1, 3<-2, 4<-3, 5<-4
SHARED_QUERY:     3<-1, 4<-3, 5<-4
FULL_CKV:         6<-7, 4<-6, 5<-4
CKV_WITH_WINNERS: 4<-6, 5<-4
```

Embedding has none. Local-prerequisite bits are:

```text
bit0 normalized/query ready
bit1 local index selection/winner ready
bit2 local attention partial ready
bit3 attention residual ready
bit4 normalized MLP input ready
bit5 local dense or routed+shared MLP partial ready
bit6 local embedding contribution ready
bit7 zero
```

Required flag, local-mask, and graph-slot-class fields are exact:

| Phase | Local mask | Required flags | Slot class |
|---:|---:|---:|---:|
| 1 query gather | `0x01` | `0x0001` | 5 |
| 2 candidate exchange | `0x02` | `0x0003` | 10 |
| 3 partial return | `0x04` | `0x0003` | 13 |
| 4 attention TP sum | `0x04` | `0x0001` | 15 |
| 5 MLP TP sum | `0x38` | `0x0001` | 24 |
| 6 packed CKV | `0x02` | `0x0002` | 31 |
| 7 indexer-key gather | `0x02` | `0x0002` | 32 |
| 8 embedding TP sum | `0x40` | `0x0001` | 1 |

`0x0001` is fixed-capacity payload and `0x0002` is zero-count records legal.
Every template operation has `flags_forbidden=0`; the separate v2 validator
still requires exactly one of CUDA_GRAPH/EAGER and forbids unknown bits. The
CPU proof emits all twelve-byte records and pins their digest before
implementation.

The single dependency ordinal in `CollectiveOp.v2` is the ordinal of the
greatest direct collective predecessor, or `65535` when none exists. The
validator independently requires every predecessor named by the template to
exist earlier in the same layer/row route. Local prerequisites are device
events or graph edges, not invented collective ordinals. This rule resolves
multi-prerequisite prose without losing a collective dependency.

Layer masks are exact:

```text
bit variant_id set means permitted
FULL layers:   FULL_QUERY | FULL_CKV | CKV_WITH_WINNERS = 0x0034
SHARED layers: SHARED_QUERY | CKV_WITH_WINNERS = 0x0028
bits for EMBEDDING or unknown variants are zero
```

Final sampling dependencies remain the distributed-sampling ABI and are
bound by its exact SHA-256 in the final-head entry.

## Buffer-lifetime digest

`TargetBufferLifetime.v1` is:

```text
"glmaxx.target-buffer-lifetime.v1\0" ||
u16_le(1) || u16_le(32) || records ordered by slot_class
```

One record is twelve bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 2 | slot class |
| 2 | 2 | applicable phase-variant mask |
| 4 | 2 | writer stage |
| 6 | 2 | last-reader stage |
| 8 | 2 | alias class; zero means never alias |
| 10 | 2 | flags; bit 0 external/tentative, other bits zero |

Stages are fixed:

```text
1 input hidden       2 attention norm    3 query/record production
4 index production   5 candidate merge   6 attention accumulation
7 attention TP sum   8 attention residual/MLP norm
9 router/compaction  10 FC1              11 FC2/shared combine
12 MLP TP sum        13 MLP residual      14 final norm/head
15 pending-state adoption
```

The route mask containing variants 2,3,4,5 is `0x003c`; variant 2 alone is
`0x0004`; full-layer variants 2,4,5 are `0x0034`. Stage `65535` means the
step's external transaction completion, not a graph node. The exact records
are:

| Class | Name | Mask | Writer | Last reader | Alias | Flags |
|---:|---|---:|---:|---:|---:|---:|
| 1 | hidden ping | `0x003c` | 1 | 13 | 1 | 0 |
| 2 | hidden pong | `0x003c` | 1 | 13 | 1 | 0 |
| 3 | attention norm | `0x003c` | 2 | 4 | 1 | 0 |
| 4 | Q LoRA | `0x003c` | 3 | 4 | 1 | 0 |
| 5 | local query heads | `0x003c` | 3 | 6 | 1 | 0 |
| 6 | absorbed query | `0x003c` | 3 | 6 | 1 | 0 |
| 7 | KV encode staging | `0x003c` | 3 | 3 | 1 | 0 |
| 8 | index encode staging | `0x0034` | 4 | 4 | 1 | 0 |
| 9 | local candidates | `0x0004` | 4 | 5 | 1 | 0 |
| 10 | exchanged candidates | `0x0004` | 4 | 5 | 1 | 0 |
| 11 | winner lists | `0x003c` | 5 | 6 | 1 | 0 |
| 12 | local partial state | `0x000c` | 6 | 6 | 1 | 0 |
| 13 | returned partial state | `0x000c` | 6 | 6 | 1 | 0 |
| 14 | local attention output | `0x003c` | 6 | 7 | 1 | 0 |
| 15 | attention reduction slab | `0x003c` | 7 | 8 | 1 | 0 |
| 16 | normalized MLP input | `0x003c` | 8 | 11 | 1 | 0 |
| 17 | router logits | `0x003c` | 9 | 9 | 1 | 0 |
| 18 | expert IDs / weights | `0x003c` | 9 | 11 | 1 | 0 |
| 19 | compaction / offsets | `0x003c` | 9 | 11 | 1 | 0 |
| 20 | routed or dense FC1 | `0x003c` | 10 | 11 | 1 | 0 |
| 21 | routed/dense activation | `0x003c` | 10 | 11 | 1 | 0 |
| 22 | routed FC2 | `0x003c` | 11 | 11 | 1 | 0 |
| 23 | shared or dense MLP | `0x003c` | 10 | 11 | 1 | 0 |
| 24 | MLP reduction slab | `0x003c` | 12 | 13 | 1 | 0 |
| 25 | final normalized hidden | `0x003c` | 14 | 14 | 1 | 0 |
| 26 | rank logits | `0x003c` | 14 | 15 | 1 | 0 |
| 27 | immutable argument tables | `0x003c` | 1 | 15 | 0 | 0 |
| 28 | target KV destinations | `0x003c` | 3 | 65535 | 0 | 1 |
| 29 | indexer destinations | `0x0034` | 4 | 65535 | 0 | 1 |
| 30 | pending-logit destinations | `0x003c` | 14 | 65535 | 0 | 1 |
| 31 | packed CKV payload | `0x0030` | 6 | 6 | 1 | 0 |
| 32 | indexer-key union payload | `0x0010` | 4 | 5 | 1 | 0 |

Dense layers validate that router/routed records have zero capacity and
sparse layers validate that dense-only spans have zero capacity; the common
records remain hash-stable. Shared index layers similarly give class 8/29
zero capacity. Winner-list class 11 remains live through stage 6 of the last
layer in its index group, represented by the graph profile's resolved layer
ordinal while retaining this logical stage ID.

An implementation first materializes this canonical record set, validates
one writer, writer-before-reader, group-dependent last readers, disjoint alias
intervals, and external nonaliasing, then hashes it. A prose-derived or
implementation-private lifetime digest is invalid.

`alias class` is a logical reuse-eligibility class, not a physical arena ID,
offset, capacity, pointer, or assertion that two slots overlap. Two records
with the same nonzero alias class may be proposed for physical reuse only when
their resolved live intervals are disjoint in every phase variant in which
both records apply. A zero alias class forbids physical reuse with every other
record. Equal or overlapping live intervals in the table therefore require
distinct storage even when both records carry alias class 1.

This r2 amendment intentionally does not serialize physical target-buffer
spans. The retained `GraphEntry.maximum_scratch_bytes` is one aggregate charge
and cannot prove a class offset, a class capacity, or a one-byte-short
rejection. Consequently this document accepts no physical alias map and makes
no undersized-graph-slot claim. The first CPU implementation must materialize
each nonzero-capacity logical class in distinct owned storage. A later graph
memory amendment must byte-specify and hash every class arena, offset, and
capacity, validate all consumer subranges, and reject every live overlap and
one-byte-short span before captured or eager CUDA execution can open. It may
reuse this lifetime digest, but it cannot infer a physical layout from the
alias-class field.

## Hash-covered row and slot tables

Every table uses an exact domain followed by `u32_le(record_count)` and fixed
records in the stated order. Duplicate keys, nonzero reserved bytes, invalid
owners, invalid row masks, overflow, or a different sort order fail before
device upload.

`TargetRow.v1` is 48 bytes, ordered by graph target-row ordinal. It has
exactly `target_row_bucket` records and exactly `target_rows` valid records:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | request ID |
| 8 | 4 | target row ordinal |
| 12 | 2 | sequence-table index |
| 14 | 1 | kind: ABSENT=0, PREFILL=1, DECODE=2, VERIFY=3 |
| 15 | 1 | valid-row bit |
| 16 | 4 | logical position |
| 20 | 4 | input token ID; zero for an invalid graph row |
| 24 | 4 | committed context length before row |
| 28 | 4 | reserved zero |
| 32 | 8 | committed page-table generation |
| 40 | 8 | tentative page-table generation |

An invalid graph row has request ID, token, position, context, and generations
zero, sequence index `65535`, kind ABSENT, and valid bit zero. A valid row has
a nonzero request ID, a real sequence index, a nonzero kind, and valid bit one.

GLM-5.2's fixed layer-major HBM geometry permits one unified page slot per row
instead of 99 redundant per-layer/per-group records. `TargetPageWriteSlot.v1`
is 40 bytes, ordered by row ordinal, with exactly `target_row_bucket` records:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 4 | row ordinal |
| 4 | 1 | owner rank `0..3` |
| 5 | 1 | token offset `0..63` |
| 6 | 1 | valid bit |
| 7 | 1 | reserved zero |
| 8 | 4 | owner-local page ID |
| 12 | 4 | reserved zero |
| 16 | 8 | page generation |
| 24 | 8 | target-KV arena allocation generation |
| 32 | 8 | target-indexer arena allocation generation |

For an invalid graph row, every field except row ordinal is zero. A valid row
has valid bit one, a nonzero page generation and both nonzero arena
generations. The graph profile binds immutable base pointers and the common
owner-local page capacity. Every layer/group address is checked arithmetic:

```text
target_kv_record =
  target_kv_base +
  (((layer_id * local_page_capacity + local_page_id) * 64 + token_offset)
    * 368)

target_indexer_record =
  target_indexer_base +
  (((index_group_id * local_page_capacity + local_page_id) * 64 + token_offset)
    * 132)
```

All ranks validate the complete rank-common table, but `local_page_id` is
meaningful only in `owner_rank`'s arenas. Rank `r` may form and dereference
either address above only for records with `valid=1 && owner_rank==r`.
Nonowners form no target-KV or indexer destination pointer and perform no
write for that row; a captured kernel receives the common owner mask plus its
immutable executor rank and must make the nonowner lane a neutral no-write
path. This ownership mask cannot alter a collective ordinal, participant
mask, or route. A write by a nonowner, resolving the ID against the wrong
rank's base, or using an arena generation other than the adopted owner arena
is fatal before publication.

This is the fixed layer/group-major layout in `spec/engine-v0.md`; a different
layout hash or stride is incompatible. One maximum-sized active-step table
arena is charged per rank, not one copy per graph profile. At the 3,072-row
prefill ceiling the row, page-write, and 64-entry pending tables occupy only
`147456 + 122880 + 3072 = 273408` bytes before ordinary table-arena alignment.
At C1 decode they occupy 136 bytes; at the 448-row verifier ceiling with a
64-sequence pending table they occupy 42,496 bytes.

`PendingLogitSlot.v1` is 48 bytes, ordered by sequence-table index and has
exactly `sequence_bucket` records:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 2 | sequence-table index |
| 2 | 1 | current valid bit |
| 3 | 1 | next valid bit |
| 4 | 4 | current slot ID; zero when absent |
| 8 | 4 | next slot ID; zero when absent |
| 12 | 4 | reserved zero |
| 16 | 8 | current slot generation; zero when absent |
| 24 | 8 | next slot generation; zero when absent |
| 32 | 8 | logits arena allocation generation |
| 40 | 8 | request ID |

Padding sequence records use valid bits and all fields zero except their
sequence-table index. Active sequence records reconstruct the current/next
pending-state invariants from the 480-byte sequence input.

Domains are respectively:

```text
glmaxx.target-row-table.v1\0
glmaxx.target-page-write-table.v1\0
glmaxx.pending-logit-slot-table.v1\0
```

Invalid graph rows remain present in the fixed bucket with `valid=0`, zero
token, and canonical absent slots. “Real row” everywhere in v1 means exactly
records with `valid=1` in target-row order, including verify rows selected by
the common valid-row mask.

## Step and graph identity amendments

The unimplemented `StepPlan.v3` becomes `StepPlan.v4`. Its first 95 hash-input
bytes are identical to v3, including `CollectiveSchedule.v2` at `63..95`.
Bytes `95..127` are `target_program_sha256`. Thus:

```text
STEP_PLAN_ABI         glmaxx.step-plan.v4
PLAN_HASH_DOMAIN      glmaxx.step-plan.v4\0
PLAN_HASH_INPUT_BYTES 127
PLAN_RECORD_BYTES     159
```

The 32-byte plan hash is stored at `127..159`. A schedule-v1 or plan-v3 hash
cannot validate under this domain.

`StepInput.v2` becomes `StepInput.v3`. Its canonical prefix is:

```text
StepPlan.v4 plan_hash                          32 bytes
sequence_table_generation                      8 bytes
PageTableDelta.v2 global_digest                32 bytes
PageTableDelta.v2 rank_delta_digest[4]        128 bytes
post_apply_device_table_digest[4]             128 bytes
TargetRow table SHA-256                        32 bytes
target page-write table SHA-256                32 bytes
pending-logit slot table SHA-256               32 bytes
sequence row_count                              2 bytes
prompt_token_count                              4 bytes
row_count fixed 480-byte sequence records
prompt_token_count little-endian u32 IDs
```

The fixed prefix before the 480-byte sequence records is exactly 430 bytes.

Its schema and hash domain are `glmaxx.step-input.v3` and
`glmaxx.step-input.v3\0`. The three tables travel as authenticated immutable
arguments and their exact counts must reconstruct the plan buckets and
sequence records. Hash equality without table validation is insufficient.

`GraphProfile.v2` is the domain-separated extension:

```text
SHA256(
  "glmaxx.graph-profile.v2\0" ||
  graph_profile_v1_sha256 ||
  target_program_sha256 ||
  SHA256("glmaxx.collective-schedule.v2\0") ||
  target_phase_template_sha256 ||
  target_buffer_lifetime_sha256 ||
  SHA256("glmaxx.target-row-table.v1\0") ||
  SHA256("glmaxx.target-page-write-table.v1\0") ||
  SHA256("glmaxx.pending-logit-slot-table.v1\0")
)
```

Startup consensus binds the graph-profile-v2 and target-program hashes.
Graph lookup requires both. No v1 profile, schedule-v1, plan-v3, input-v2, or
rank-local substitute is admitted.

This GraphProfile extension binds the logical lifetime-table identity only.
It does not add or accept the still-missing physical per-class span table.
`maximum_scratch_bytes` remains an aggregate accounting ceiling, not evidence
that any of the 32 logical classes has a sufficient or nonoverlapping device
range.

## Decoded-record controls and gates

Before any CUDA work, the CPU gate adds:

1. exact encode/decode and malformed-input proof for every 368-byte target KV
   and 132-byte indexer record field, including de-interleaved stored RoPE;
2. exhaustive enum/zero/reserved/table ordering and target-program preimage
   mutation tests;
3. a complete transition from source BF16 values through record encode and
   production decode with all rounding points retained; and
4. three matched layer-6/layer-7 controls:

```text
A source-expanded: original BF16 K/V/index keys and source operator order
B decoded-expand: production records decoded once, then dense reference math
C packed path: production record consumer and transport path
```

`A versus B` is retained as codec error and sets no implementation tolerance.
`B versus C` is the implementation-equivalence gate and uses tolerances fixed
before the run. A BF16-expanded form of B is retained separately from its FP32
decode intermediates. Per-position and per-phase errors are retained; a mean
cannot hide an outlier. The same record bytes, rows, winners, owner order,
collective route, and residual membership are used by B and C.

The first SM120 replay may begin only after adversarial acceptance, the exact
CPU preimages/tables/digests are pinned, and A/B/C passes on CPU. Layer 6 still
exercises full index plus sparse MoE; layer 7 still proves shared winner reuse.
In addition, the later physical graph-memory amendment described above must be
accepted and its class-span validator must pass before this replay can launch.

## Nonclaims

This amendment is not an implementation, accepted ABI, CPU proof, CUDA
program, graph, collective result, layer replay, checkpoint smoke, quality
result, capacity result, or performance claim. In particular, it does not
accept physical graph-buffer offsets, capacities, aliasing, or capture safety.
It authorizes no cn4 access.
