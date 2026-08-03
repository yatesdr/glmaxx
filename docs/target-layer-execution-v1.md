# GLM-5.2 target-layer execution contract v1

Date: 2026-07-29

Status: design candidate; adversarial review required before CPU ABI or CUDA
implementation

GPU evidence: none

## Purpose and boundary

This document freezes the rank-local program and collective-visible dataflow
for checkpoint layers `0..77`. It is deliberately specific to GLM-5.2, TP4,
DCP4 page ownership, four SM120 GPUs over PCIe, and the already pinned weight,
KV, indexer, step, and sampling contracts.

It closes four gaps that currently prevent a truthful `RankExecutor`:

1. `StepPlan` names a collective schedule but the current schedule records do
   not identify a layer, phase, dependency, or graph buffer;
2. no contract says when new KV and indexer records become tentative, visible,
   committed, or reusable;
3. the sharded embedding entry, exact layer order, residuals, dense/sparse
   MLP, final head, and TP reductions are not an executable rank program; and
4. the one-layer gate does not identify a layer that exercises both a full
   sparse indexer and sparse MoE.

The selected first full replay is target layer 6. It is the first target layer
that is simultaneously:

- a sparse routed/shared MoE layer;
- a full IndexShare indexer layer; and
- downstream of both dense and earlier shared-index target layers.

Layer 7 is the required companion replay for reuse of layer 6's winner list.

This contract does not define draft layer 78 recurrence, distributed sampling
probabilities, checkpoint adoption, route implementation, or CUDA kernel
internals. It consumes those separate reviewed contracts.

## Immutable source basis

| Input | Identity |
|---|---|
| model | `zai-org/GLM-5.2` |
| model revision | `b4734de4facf877f85769a911abafc5283eab3d9` |
| operation manifest | `manifests/glm52-operation-v1.json` |
| operation-manifest SHA-256 | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| installed Transformers modeling source SHA-256 | `adb8317a21716b01273046e46c807f14f0dbaf035af59b60d52bd6bc3007cf72` |
| exact official Transformers commit | `5204b4fe36956e9214b9279f1e1be2fd5dd1d9f3` |
| exact model config SHA-256 | `185f93ee6d12548e16a847e279dc0c3c90b1524c970b0866b42fb545747d859a` |
| model-source audit | `docs/manifest-source-audit-20260729.md` |
| engine specification | `spec/engine-v0.md` |

The fixed target geometry is:

```text
layers                         78
hidden                         6144
query heads                    64, 16 per TP rank
Q LoRA                         2048
QK NoPE / RoPE                 192 / 64
KV LoRA                        512
value head                     256
index heads / dimension        32 / 128
index winners                  2048
experts / selected             256 / 8
expert intermediate            2048, 512 per TP rank
shared intermediate            2048, 512 per TP rank
dense intermediate             12288, 3072 per TP rank
```

Every runtime tensor is resolved by `(layer_id, role_id, tensor_id)` from the
already validated rank manifest. String lookup and rank-local policy choice
are forbidden inside a graph launch.

## Static target program

The engine compiles one immutable `TargetProgram.v1` at startup. It contains
one embedding entry, 78 layer entries in ascending layer order, and one
final-head entry. Each layer entry binds:

```text
layer_id
index_group_id
index_mode: FULL | SHARED
mlp_mode: DENSE | SPARSE
all required tensor IDs
attention TP rule
MLP TP rule
collective phase template
graph buffer slots
program_entry_sha256
```

The embedding entry binds role `0x0001`, the exact valid/physical vocabulary
split, and its TP reduction. The final-head entry binds roles `0x0003` and
`0x0002`, the rank-local vocabulary interval, padding mask, and sampling ABI.

The complete program is domain-separated and hashed. Every rank derives the
same hash from engine-owned layer/role constants, but may do so only after its
rank-specific semantic tensor catalog has passed the production-manifest
validator. Rank-specific payload offsets and codec metadata digests are bound
by the rank manifest and deliberately excluded from this common program hash.

The canonical program hash is:

```text
SHA256(
  "glmaxx.target-program.v1\0" ||
  embedding_entry_sha256 ||
  u16_le(78) ||
  layer_entry_sha256[0] || ... || layer_entry_sha256[77] ||
  final_head_entry_sha256
)
```

Each entry digest uses its own domain and exact scalar fields followed by the
ascending runtime tensor bindings. One binding is
`tensor_id:u32_le, role_id:u16_le, expert_id:i16_le, codec_id:u16_le`.
`expert_id=-1` means nonexpert. Layer entries additionally hash layer/group
and mode bytes, attention/MLP TP-rule bytes, the allowed collective-phase
template digest, and the logical buffer-lifetime digest. The embedding and
head entries hash their vocabulary intervals and phase templates. All four
ranks must have the same tensor IDs, roles, expert IDs, and codec policy;
rank-local offsets, source slices, and payload digests remain authenticated by
their distinct rank manifests.

`target_program_sha256` must be added to startup consensus, the graph-profile
identity, and the immutable step input before any executor promotion.

The target program requires:

- dense MLP for layers `0,1,2`;
- sparse MLP for layers `3..77`;
- full indexers at
  `0,1,2,6,10,14,18,22,26,30,34,38,42,46,50,54,58,62,66,70,74`; and
- shared index use from each full layer through the layer immediately before
  the next full layer.

No graph, eager path, or rank may alter those modes.

## Exact weight bindings

The target program binds these source and rank-local logical shapes. All
shapes are `[output,input]` unless noted:

| Role | Source shape | TP rule / rank shape |
|---|---:|---|
| token embedding | `[154880,6144]` | axis 0 / `[38720,6144]` |
| input/post-attention norm | `[6144]` | replicated |
| Q A projection | `[2048,6144]` | replicated |
| Q A norm | `[2048]` | replicated |
| Q B projection | `[16384,2048]` | axis 0 / `[4096,2048]` |
| KV A projection | `[576,6144]` | replicated |
| KV A norm | `[512]` | replicated |
| KV B projection | `[28672,512]` | axis 0 / `[7168,512]` |
| attention O projection | `[6144,16384]` | axis 1 / `[6144,4096]` |
| full-indexer WQ | `[4096,2048]` | replicated |
| full-indexer WK | `[128,6144]` | replicated |
| full-indexer head weights | `[32,6144]` | replicated |
| full-indexer K norm weight/bias | `[128]` each | replicated |
| dense gate/up | `[12288,6144]` each | axis 0 / `[3072,6144]` |
| dense down | `[6144,12288]` | axis 1 / `[6144,3072]` |
| sparse router | `[256,6144]` | replicated |
| sparse correction bias | `[256]` | replicated |
| shared gate/up | `[2048,6144]` each | axis 0 / `[512,6144]` |
| shared down | `[6144,2048]` | axis 1 / `[6144,512]` |
| one routed expert gate/up | `[2048,6144]` each | combined local `[1024,6144]` |
| one routed expert down | `[6144,2048]` | axis 1 / `[6144,512]` |
| final norm | `[6144]` | replicated |
| LM head | `[154880,6144]` | axis 0 / `[38720,6144]` |

The correction bias is source FP32. Other protected tensors above are source
BF16 and retain the compiled protected policy. Routed expert descriptors bind
one immutable EXL3 or NVFP4 codec for each
`(layer,expert,tensor-role)`; runtime routing cannot change it. The LM head's
154,880 physical rows contain 154,856 valid vocabulary rows and 24 masked
padding rows.

Source dtype is not arithmetic membership. The indexer head-weight projection
and sparse router widen their BF16 source weights and inputs to FP32 for their
linear operations, matching the pinned source. They may use an immutable
startup-expanded FP32 allocation only when its exact bytes are present in the
accepted memory plan; otherwise kernels widen BF16 values while loading. They
may not quantize those weights or widen them differently by rank.

## Row state

Rows are in immutable `StepInput` sequence-table order. Prefill expands each
sequence's contiguous prompt slice into token/position order. Decode has one
real target row per sequence. Verify uses the separately reviewed valid-row
mask and retains masked graph rows as inert padding.

For each real row the executor receives:

```text
request_id
logical_position
input_token_id
committed context length before this row
target KV write slot for every target layer
target indexer-key write slot for every full-indexer group
read-only committed target page table generation
tentative target page-table generation
committed pending-logit slot/generation for DECODE or VERIFY
tentative next-pending-logit slot/generation
```

All pointer and slot tables are uploaded before graph launch and covered by
the immutable input/table hashes. Device validation checks bounds,
generations, row order, owner rank, and non-aliasing before the first weight or
KV write.

Every real row owns one hidden vector of 6,144 BF16 values. Hidden state uses
two graph-resident ping/pong slabs; a layer may not expose its output as the
next layer's input until both its attention and MLP residual boundaries have
completed.

## Step entry and head exit

Before layer 0, the valid input token ID selects exactly one physical
embedding row on exactly one TP rank. That rank loads 6,144 BF16 values;
other ranks contribute exact positive zero. One `TP4_SUM` produces the same
initial hidden row on all ranks:

```text
local_embedding =
  token in rank_vocab_interval ? embed_tokens[token] : BF16_ZERO[6144]
h_layer0 = TP4_SUM(local_embedding)
```

IDs `154856..154879`, out-of-range IDs, a nonzero masked-row contribution, or
more than one owning rank are fatal. Masked verifier graph rows remain zero
and do not perform a lookup. The embedding collective is one step-level
ordinal over the graph row bucket, not one collective per row.

After layer 77:

```text
final_hidden = RMSNorm(h_out, model.norm.weight, epsilon=1e-5)
rank_logits = lm_head_rank(final_hidden)               [row, 38720]
```

The final norm is replicated. The LM head is vocabulary-row parallel and has
no TP reduction or full-logits gather. Each rank masks invalid physical rows
before the separately reviewed distributed-sampling phases. Production
prefill evaluates the head only for the last processed row of each sequence;
decode evaluates its one real row; verify evaluates every valid verifier row.
Other layer-77 rows do not enter final norm or the LM head.

The selected head output is tentative rank-local sequence state until
four-rank consensus. Prefill stores the last processed prompt position as
pending logits and emits no token. Decode first samples the prior committed
pending logits, then embeds and executes that sampled token; the new head
output becomes pending logits for the following step. Verify follows the
separately reviewed proposal/accept/residual/bonus order and publishes only
the pending state selected by its accepted-prefix result.

## Exact layer program

For input hidden `h_in`, each target layer executes the following order.

### Phase A — normalized attention input

```text
attention_residual = h_in
x = RMSNorm(h_in, input_layernorm.weight, epsilon=1e-5)
```

The stored `x` is BF16; reduction and reciprocal-square-root arithmetic are
FP32 and must match the independent source oracle before fusion.
NaN, infinity, a nonpositive normalization denominator, or a shape mismatch
is fatal to the whole step.

### Phase B — MLA query and new record production

The projections are:

```text
q_lora = q_a_proj(x)                                  [row, 2048]
q_lora = RMSNorm(q_lora, q_a_layernorm.weight, epsilon=1e-6)
q = q_b_proj(q_lora)                                  [row, 64, 256]

kv_latent_and_rope = kv_a_proj_with_mqa(x)             [row, 512+64]
kv_latent = RMSNorm(kv_latent[0..512],
                    kv_a_layernorm.weight,
                    epsilon=1e-6)
```

`q_b_proj` and `kv_b_proj` are column-parallel by head. Each rank produces
exactly 16 query heads. Its local `kv_b_proj` shard is interpreted, in the
pinned source order, as 16 pairs of:

```text
W_K[192,512]
W_V[256,512]
```

For each local query head, the 192-value NoPE query is absorbed through
`W_K` into a 512-value query over the compressed latent. This is a local
protected-weight operation and is not a collective. The accepted CPU oracle
must reproduce the pinned reshape and transpose. V1 uses BF16 inputs and
weights, FP32 dot accumulation, and one round-to-nearest BF16 store for each
absorbed-query value.

The 64-value query suffix and the 64-value
`kv_latent_and_rope[512..576]` key use RoPE at the row's absolute logical
position with theta 8,000,000 and the pinned interleave convention. Rotation
reads interleaved even/odd pairs and stores the 32 rotated-first values
followed by the 32 rotated-second values; the wire does not re-interleave that
output. The resulting head-independent 512-value latent and 64-value RoPE key are encoded
into the exact 368-byte dynamic target KV record. The record is written only
to that row's tentative slot on its DCP owner rank.

No durable expanded key/value heads are materialized. The active attention
source is the compressed owner-local KV record, including every earlier
same-sequence tentative record from the immutable step and the current row
when causally visible.

### Phase C — full or shared sparse index

On a `FULL` layer:

```text
index_q = indexer.wq_b(q_lora)                         [row, 32, 128]
index_k = indexer.wk(x)                                [row, 128]
index_k = LayerNorm(index_k,
                    indexer.k_norm.weight,
                    indexer.k_norm.bias,
                    epsilon=1e-6)
index_head_weights = indexer.weights_proj(x)           [row, 32]
```

For each index head, split query and key as 64 rotary values followed by 64
pass-through values. Apply the pinned interleaved RoPE to the first 64 at
their absolute positions, concatenate rotary then pass-through values, and
compute:

```text
head_score[h,p] =
  ReLU(FP32_DOT(index_q[h], index_k[p]) * 128^(-1/2))
head_weight[h] = FP32(index_head_weights[h]) * 32^(-1/2)
index_score[p] = FP32_SUM_h(head_weight[h] * head_score[h,p])
```

Causal masking occurs before selection. The independent CPU oracle must
match the pinned source's LayerNorm, RoPE, projection, and FP32 accumulation
boundaries. The 128-value key stores the de-interleaved 64-value rotated output
followed by the 64 pass-through values and is encoded into the exact 132-byte
indexer-key record written to the row's tentative sidecar slot.

Each DCP owner scores every owner-local committed key plus every causally
visible same-step tentative key, rejects nonfinite values, and selects at most
2,048 candidates by descending FP32 score then ascending logical position.
For `PREFILL_QUERY`, decode, and verify, candidate exchange uses these exact
little-endian records:

```text
CandidateBatch.v1, per owner and real row
[0,4)       u32 real candidate count, 0..2048
[4,8)       zero
[8,32776)   2048 Candidate.v1 slots

Candidate.v1
[0,4)       finite FP32 score
[4,8)       zero
[8,16)      u64 logical position
```

Slots at or above the real count are all zero and are never interpreted as
candidates. The fixed-capacity wire exchange is in owner-rank then real-row
order. Counts, padding, duplicate positions, owner mapping, causal bounds,
scores, and tie ordering are validated before the fixed four-way merge.

The merge produces this graph-resident winner list for each real row:

```text
WinnerList.v1
[0,4)       u32 real winner count, 0..2048
[4,5)       order: 1=SCORE_DESC_POSITION_TIE, 2=ALL_POSITIONS_ASC
[5,8)       zero
[8,16392)   2048 u64 logical-position slots
```

Unused positions are zero and ignored. Normal selected positions use order 1:
descending score then ascending-position ties. The reviewed all-positions
shortcut uses order 2: ascending logical position. Real positions must be
unique. No other order byte is legal. The winner list is stored in the graph
slot assigned to the index group.

`PREFILL_CKV` instead gathers the exact 132-byte owner records in canonical
sequence then logical-position order. Because the full-indexer query and
weights are replicated, every rank computes and validates the same winner
list locally; there is no candidate exchange on that route.

On a `SHARED` layer no index projection, index-key write, local selection, or
candidate collective occurs. The layer consumes the exact winner-list slot
produced by its group's full layer for the same row. A generation, request,
position, group, or valid-row mismatch is fatal. Winner lists never persist
as prefix-cache data; only the full-layer 132-byte keys persist.

### Phase D — attention transport and partial softmax

Prefill uses the rank-invariant route selected by
`StepPlan.attention_transport`:

- `PREFILL_CKV` gathers exact compressed records needed by local query heads,
  computes attention for those heads, and performs no DCP output
  reduce-scatter; or
- `PREFILL_QUERY` transports queries to DCP owners and returns partial state.

For `PREFILL_CKV`, each full layer first all-gathers the union of exact
132-byte indexer records needed by the chunk. Each record is transferred once
per layer/chunk. Contributions are ordered by
`(owner_rank,local_page_id,token_offset)`, and an immutable table maps every
`(request_id,logical_position,generation)` reference to its union index.
Identical sealed-prefix physical records shared by multiple sequences are
transferred once. Every rank reconstructs canonical per-sequence order and
computes the same per-row winners. The route then all-gathers, once per
layer/chunk, the similarly ordered and deduplicated union of exact 368-byte KV
records referenced by any real row's winner list. Shared layers skip the
indexer gather and gather their own layer's 368-byte KV records for the
existing winner union.

If every real row has at most 2,048 causally visible positions, the
coordinator may select a separately qualified CKV route that omits indexer
record transport because every position wins; full layers still produce and
tentatively store their new indexer keys. It emits an
`ALL_POSITIONS_ASC` winner list. The omission is encoded in every rank's
schedule and never selected rank-locally.

The union tables and payloads are hash-covered and byte-preserving. A route
may not requantize either record, duplicate one physical record generation,
or gather a record outside the immutable causal table. Route-table
qualification compares this union-gather path with `PREFILL_QUERY` by chunk,
context band, topology, and row bucket.

Production decode and verify use only `DECODE_QUERY_LSE`. The query wire
record for one head is exactly 1,152 bytes:

```text
[0,1024)      512 BF16 absorbed NoPE query values
[1024,1152)    64 BF16 RoPE query values
```

Local heads appear in ascending global head ID. Owners all-gather the query
records in rank order, process only winner positions they own, and accumulate
FP32 partial softmax in ascending logical-position order.

For a winning position, the score is the source-ordered, source-scaled sum of
the absorbed-query/decoded-latent dot product and the RoPE-query/decoded-RoPE
dot product:

```text
score[p] =
  (FP32_DOT(q_absorbed, decoded_latent[p]) +
   FP32_DOT(q_rope, decoded_rope[p])) * 256^(-1/2)
```

The weighted numerator accumulates the decoded 512-value latent, not an
expanded 256-value V. Algebraically equivalent reassociation is not assumed
bit-equivalent.

One owner/head partial state is exactly 2,064 bytes:

```text
[0,4)       FP32 local maximum
[4,8)       FP32 local exponential sum
[8,2056)    512 FP32 weighted-latent numerator values
[2056,2060) u32 sample count
[2060,2064) zero
```

An empty state is `maximum=-infinity`, zero sum/vector/count, and zero
reserved bytes. Nonempty states require finite maximum/vector, positive finite
sum, and nonzero count.

Partial states return to each query head's fixed TP owner. Merge order is
owner rank `0,1,2,3`, using the FP32 log-sum-exp equations in engine v0.
The merged numerator is divided by the merged sum to obtain one 512-value
FP32 latent output per head, then rounds it once to BF16. The head owner
applies its local BF16 `W_V` with FP32 accumulation and a BF16 head-output
store to obtain 256 values. There is no full KV-record gather on decode.

### Phase E — attention output and residual

Each rank concatenates its 16 local attention outputs:

```text
local_head[h] = W_V[h] * merged_latent[h]               [row, 256]
local_attention = concat(head[rank*16 .. rank*16+16])  [row, 4096]
attention_partial = o_proj(local_attention)             [row, 6144]
attention_output = TP4_SUM(attention_partial)
h_attention = attention_residual + attention_output
```

`o_proj` is row-parallel. Exactly one TP4 reduction occurs over BF16 partials
and produces BF16 output under the reviewed route. The residual add is
`BF16_RN(FP32(attention_residual) + FP32(attention_output))` after that
reduction. A fusion that adds the residual before a rank-complete reduction is
forbidden.

### Phase F — normalized MLP input

```text
mlp_residual = h_attention
y = RMSNorm(h_attention,
            post_attention_layernorm.weight,
            epsilon=1e-5)
```

### Phase G1 — dense MLP, layers 0–2

```text
gate_local = gate_proj(y)                              [row, 3072]
up_local = up_proj(y)                                  [row, 3072]
activated = SiLU(gate_local) * up_local
mlp_partial = down_proj(activated)                     [row, 6144]
mlp_output = TP4_SUM(mlp_partial)
```

Gate/up are column-parallel and down is row-parallel. Exactly one TP4
reduction follows the down projection.

### Phase G2 — sparse MLP, layers 3–77

The protected replicated FP32 router executes once per real row:

```text
scores = sigmoid(FP32(gate.weight) * FP32(y))
corrected = scores + e_score_correction_bias
expert_ids = group_limited_top8(corrected)
route_weights =
  scores[expert_ids] / (sum(scores[expert_ids]) + 1e-20) * 2.5
```

The pinned configuration has one expert group and selects that one group, so
group filtering cannot change the candidate set. The deterministic top-8
order is corrected score descending then expert ID ascending. Route weights
remain attached to those slots. An independent CPU oracle must prove the same
selected set as the pinned source away from exact boundary ties and retain
both tied alternatives at a boundary. Every rank hashes the router result;
divergence is fatal before expert kernels.

Stable compaction order is expert ID, token row, then route slot. Each rank
uses the same compacted assignment table.

For the routed path, rank-local gate/up produce 512 values each per
assignment, apply FP32 `SiLU(gate) * up`, and down-project to a 6,144-value
partial. Route weight is applied after down projection before deterministic
scatter-add into the row partial.

The shared path is:

```text
shared_gate_local = shared.gate_proj(y)                 [row, 512]
shared_up_local = shared.up_proj(y)                     [row, 512]
shared_activated = SiLU(shared_gate_local) * shared_up_local
shared_partial = shared.down_proj(shared_activated)     [row, 6144]
```

The rank combines routed and shared partials before one TP reduction:

```text
mlp_partial = routed_partial + shared_partial
mlp_output = TP4_SUM(mlp_partial)
```

Two separate TP reductions for routed and shared paths are forbidden in v1.
Empty experts contribute exact zero. Empty routed assignments still execute
the shared path and the same collective ordinal.

### Phase H — MLP residual and next layer

```text
h_out = mlp_residual + mlp_output
```

This residual is
`BF16_RN(FP32(mlp_residual) + FP32(mlp_output))` after the complete BF16 TP4
sum.

The last target layer output enters the protected final RMSNorm and
vocabulary-axis-0-sharded LM head. Production sampling consumes rank-local logical
vocabulary intervals and masks the 24 physical padding rows before any
candidate or mass operation.

## Layer-addressed collective schedule v2

The existing collective record cannot distinguish two operations with the
same kind/size at different layers. Before target execution it is replaced by
the following exact 40-byte hash record:

```text
CollectiveOp.v2
```

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 2 | contiguous ordinal |
| 2 | 1 | layer ID `0..78`, or `255` for a step-level phase |
| 3 | 1 | phase ID |
| 4 | 1 | index-group ID `0..20`, or `255` |
| 5 | 1 | participant mask |
| 6 | 2 | globally selected route ID |
| 8 | 4 | real row count |
| 12 | 4 | graph row bucket |
| 16 | 8 | exclusive logical payload bytes |
| 24 | 8 | route-manifest wire payload bytes |
| 32 | 2 | graph buffer-slot ID |
| 34 | 2 | dependency ordinal, or `65535` |
| 36 | 2 | flags |
| 38 | 2 | zero |

Phase IDs are:

```text
1   DCP_QUERY_GATHER
2   DCP_CANDIDATE_EXCHANGE
3   DCP_PARTIAL_STATE_RETURN
4   TP_ATTENTION_REDUCE
5   TP_MLP_REDUCE
6   DCP_PACKED_CKV
7   DCP_INDEXER_KEY_GATHER
8   TP_EMBED_REDUCE
16  LOGITS_GREEDY
17  LOGITS_TOP_K
18  LOGITS_MASS
19  LOGITS_RESULT_BROADCAST
```

Flags are exact:

```text
bit 0  FIXED_CAPACITY_PAYLOAD
bit 1  ZERO_COUNT_RECORDS_LEGAL
bit 2  CUDA_GRAPH
bit 3  EAGER
bits 4..15 zero
```

Exactly one of `CUDA_GRAPH` and `EAGER` is set. Data-dependent facts do not
change the flags independently on a rank.

`logical_payload_bytes` counts application records once.
`wire_payload_bytes` is the deterministic route-manifest byte count for the
selected topology and may exceed logical bytes. A benchmark reports this
declared count beside measured collective time; it must not pretend that an
opaque library exposed hardware-level PCIe byte counters. Neither field means
allocation capacity.

Dependencies are explicit:

- every layer-0 phase depends on the step-level embedding reduction;
- query-route candidate exchange depends on that layer's query gather and
  local index selection;
- CKV-route packed-record gather depends on local winner construction, which
  depends on the full-layer indexer-key gather unless the reviewed
  all-positions route is selected;
- partial return depends on query gather and, for full layers, candidate
  exchange;
- attention TP reduction depends on partial return or the selected prefill
  attention phase;
- MLP TP reduction depends on attention reduction, attention residual, and
  local MLP completion; and
- prefill has no sampling ordinal and its head result becomes pending state;
- decode sampling/result broadcast consume the previously committed pending
  logits and complete before the embedding reduction; and
- verify sampling dependencies follow the distributed-sampling ABI, while
  its newly computed target logits and pending state remain tentative.

The one encoded dependency ordinal is the greatest direct collective
predecessor ordinal, or `65535` when none exists. The target phase-template
validator separately proves that every required collective predecessor exists
earlier; local compute prerequisites are graph edges or device events rather
than synthetic collective ordinals.

Every step has exactly one embedding TP reduction. Every layer has exactly one
attention TP reduction and one MLP TP reduction.
A full-indexer query-route layer additionally has query, candidate, and
partial phases. A shared-indexer query-route layer has query and partial
phases but no candidate phase. A full-indexer CKV-route layer has
indexer-key-gather and packed-CKV phases; a shared layer has only packed CKV.

The schedule hash domain is `glmaxx.collective-schedule.v2\0`, followed by a
little-endian `u16` operation count and the exact 40-byte records.
`StepPlan` continues to carry the resulting 32-byte digest. Promotion requires
replacing the Rust v1 schedule encoder and pinning schedule-v2 identity in the
graph profile.

Route selection and participant masks are coordinator-owned. A rank-local
empty-page optimization is forbidden. If a coordinator proves an owner has no
pages for every sequence, the same participant mask is in every rank's
record.

## Graph-visible buffer contract

Every graph profile declares fixed slots for:

- hidden ping/pong;
- normalized attention and MLP input;
- Q LoRA, local query heads, and absorbed NoPE queries;
- new KV and full-indexer key encoding staging;
- local and global candidate lists;
- local and returned partial-softmax state;
- local attention output and 6,144-value reduction slab;
- router logits, expert IDs/weights, stable compaction table and offsets;
- routed FC1/activation/FC2 workspace;
- shared/dense MLP workspace and 6,144-value reduction slab;
- final normalized hidden and rank-local logits; and
- immutable row, page, pointer, mask, and collective argument tables.

Slots have one writer phase and an explicit last reader. Reuse is permitted
only when those lifetimes do not overlap for the selected graph bucket.
Raw candidate slots become reusable after the merge; winner-list slots live
through the last shared layer of their group. Tentative KV/indexer destination
slots remain outside graph scratch and cannot alias a later row.

Every pointer consumed by a captured node comes from a preallocated arena.
Graph update may change row counts, table pointers, positions, and slot
generations within the reviewed bounds. It may not change a weight pointer,
codec, layer program, route class, or buffer capacity.

## Transaction and failure rules

All new target KV and full-indexer keys are tentative for the entire step.
They may be read causally by that same immutable step but are unreachable from
prefix lookup, another request, lower-tier eviction, or a later step until
four-rank output consensus and the serving-page transaction commit.
Prefill and MTP0 commit every successful real row. Verification commits only
the accepted contiguous prefix selected by the consensus output; rejected and
masked rows are invalidated in the same transaction.

On any descriptor, kernel, nonfinite, collective, graph, output, or consensus
failure:

1. no target KV/indexer generation commits;
2. no scheduler/token/RNG progress commits;
3. all tentative slots are invalidated before reuse; and
4. the rank-worker generation terminates unless all ranks can prove the same
   last-completed collective ordinal and cancel every future ordinal before
   entry.

CUDA asynchronous errors are attributed to the last checked phase/ordinal and
are process-fatal for that worker generation. Continuing after one rank has
abandoned an ordinal is forbidden.

## Required CPU/reference gate

After adversarial acceptance, CPU implementation must prove:

1. embedding, all 78 immutable layer entries, final head, and exact
   full/shared index groups;
2. tensor-role completeness and no unused or duplicate required tensor;
3. byte-stable collective-v2 records and schedule hashes;
4. exact embedding/layer/head operation counts and dependency order for
   PREFILL, DECODE, and VERIFY;
5. actual row/bucket payload arithmetic for every C64/MTP0–6 bucket;
6. full-indexer key production, candidate tie order, fixed four-way merge,
   and shared-layer generation-safe reuse;
7. query and 2,064-byte partial-state wire encode/decode, including empty
   owners and malformed/nonfinite records;
8. dense and sparse residual/reduction boundaries;
9. identical router/compaction results on four ranks, including empty and
   skewed experts;
10. tentative KV/indexer visibility, commit, and rollback under faults at
    every phase; and
11. exact graph slot lifetimes with no alias between live values; and
12. one-owner embedding reduction, vocabulary padding rejection, final norm,
    rank-logit intervals, and no full-logits gather.
13. exact 368-byte KV and 132-byte indexer record encode/decode, stored RoPE
    order, and malformed inputs; and
14. source-expanded, decoded-expand, and packed-path layer controls that
    separate codec error from implementation error.

CPU proof does not open CUDA execution.

## First SM120 one-layer gate

After operator microbenchmarks pass, the first complete replay uses pinned real
reference activations and cache state for layer 6:

- one M=1 decode row and one qualified prefill row bucket;
- context positions distributed across all four DCP owners;
- actual layer-6 protected and routed/shared expert weights;
- full indexer key write, local candidates, global winner list, query
  transport, partial LSE return, attention output, both residuals, router,
  stable compaction, routed/shared MLP, and both TP reductions;
- per-phase CPU/reference and SM120 hashes with tolerances fixed before the
  run; and
- separate indexer, attention, expert, collective, launch/framework, and
  end-to-end timings.

Layer 7 then consumes layer 6's exact winner-list fixture and proves there is
no indexer compute or candidate exchange while producing correct layer output.

The replay retains:

```text
source/checkpoint/input/cache/prompt hashes
route table and topology hashes
every collective-v2 record
router IDs and weights per row
winner positions per row
per-phase output hashes/errors
layer output error
full-vocabulary downstream logit comparison
cold and warm measurements separately
```

The downstream logit comparison resumes the pinned reference from the replayed
layer output through final norm/head and retains per-position full-vocabulary
evidence. A synthetic random input or a projection-only output does not pass
this gate.

## Explicit non-claims

This candidate is not a CPU proof, CUDA implementation, graph capture,
collective qualification, one-layer result, checkpoint smoke, quality result,
or performance claim. It does not authorize cn4. It cannot be implemented as
a production ABI until the adversarial questions resolve the wire precision,
indexer math, prefill route semantics, collective-v2 encoding, and transaction
dependencies.
