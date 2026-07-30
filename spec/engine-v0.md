# GLM-5.2 SM120 native engine specification v0

Status: **DRAFT — FABLE V2 CONDITIONS PLUS PHASE-A IMPLEMENTATION AMENDMENT**

Specification revision: 0.2.2

Date: 2026-07-28

Intended reviewer: Fable

Implementation status: M1 CPU-proof candidate; post-manifest review pending

## 1. Normative language

The key words **MUST**, **MUST NOT**, **REQUIRED**, **SHALL**, **SHALL NOT**,
**SHOULD**, **SHOULD NOT**, **RECOMMENDED**, **MAY**, and **OPTIONAL** are to
be interpreted as normative requirements.

An item labeled **OPEN** is not part of the frozen ABI. Every blocking OPEN
item MUST be resolved and independently reviewed before implementation
passes the named gate.

This specification defines the engine contract. The binary checkpoint
contract is in [format-v0.md](format-v0.md).

## 2. Scope

Version zero SHALL execute only:

- `zai-org/GLM-5.2`;
- four NVIDIA RTX PRO 6000 Blackwell GPUs in one host;
- compute capability SM120;
- tensor parallel degree four;
- decode context parallel degree four over those same ranks;
- EXL3/Trellis and NVFP4 weight payloads described by the format spec;
- the dynamic 368-byte NVFP4/FP8-RoPE MLA KV record;
- MTP speculative depths zero through six.

The implementation sequence SHALL be NVFP4-first through CPU proof, SM120
microbenchmark, one-layer replay, and the small-checkpoint runner. EXL3 is
REQUIRED before a fit-capable full-checkpoint serving profile or M5.

DCP4 is process-immutable in v0. DCP1, DCP2, and runtime posture changes are
outside scope.

Version zero SHALL NOT implement:

- another model architecture;
- another GPU architecture;
- arbitrary TP, pipeline parallel, or expert parallel degrees;
- training, LoRA, adapters, or mutable weights;
- multimodal encoders;
- a generic tensor graph;
- quantization plugins other than the specified codecs;
- CPU inference fallback.

The engine MAY accept a future byte-identical repack of the pinned model. It
MUST reject a different model, tokenizer, tensor inventory, or semantic
configuration unless this specification is revised.

## 3. Immutable source identities

The reference model identity is:

| Item | Pin |
|---|---|
| model repository | `zai-org/GLM-5.2` |
| model revision | `b4734de4facf877f85769a911abafc5283eab3d9` |
| model architecture | `GlmMoeDsaForCausalLM` |
| model config | exact bytes and SHA-256 recorded by the converter |
| tokenizer | exact revision and file hashes recorded by the converter |
| chat template | exact bytes and SHA-256 recorded by the converter |

The first EXL3 compatibility control is:

| Item | Pin |
|---|---|
| repository | `brandonmusic/GLM-5.2-EXL3-TR3-3.0bpw` |
| revision | `9297b9f1d53af5c67cffa01e30cc071a1ff7144b` |
| repository storage | `336,543,556,886` bytes |
| manifest SHA-256 | `bfb6dc39f28da08c1cfc5b89603414046adf7003152d69e9ee350e11f7a1fa63` |
| config SHA-256 | `fcde001350291a0048318d4a1136e0732e31f829f804a57cfbb558903e54171a` |
| vLLM EXL3 source control | `00787eeabebc11cee12cff12a823011b4e1a5ebc` |
| SparkInfer EXL3 source control | `669a12ddc7cf3021e91a25f398b1a883b703fd12` |

The converter MUST record the exact EXL3 quantizer and kernel source
revisions it used to define reconstruction. The control revisions above do
not automatically become the native engine's final codec ABI.

## 4. Fixed model constants

The engine SHALL compile these values as constants and SHALL verify them
against the checkpoint manifest:

| Constant | Value |
|---|---:|
| target decoder layers | 78 |
| target sparse-MoE layers | 75 |
| hidden dimension | 6,144 |
| dense MLP intermediate | 12,288 |
| first dense MLP layers | 3 |
| routed experts per sparse layer | 256 |
| selected routed experts per token | 8 |
| shared experts per sparse layer | 1 |
| routed/shared expert intermediate | 2,048 |
| attention heads | 64 |
| local TP4 heads | 16 |
| Q LoRA rank | 2,048 |
| KV LoRA rank | 512 |
| QK no-PE dimension | 192 |
| QK RoPE dimension | 64 |
| QK head dimension | 256 |
| value head dimension | 256 |
| sparse index heads | 32 |
| sparse index head dimension | 128 |
| sparse index top-k | 2,048 |
| sparse index refresh frequency | 4 |
| MTP prediction layers | 1 |
| MTP layer contents | MLA attention plus routed/shared MoE |
| total checkpoint layer IDs | 79 (`0..77` target, `78` draft) |
| vocabulary | 154,880 |
| maximum total positions | 1,048,576 |
| RMSNorm epsilon | `1e-5` |
| RoPE theta | `8,000,000` |
| router score function | sigmoid |
| router probability normalization | enabled |
| routed scaling factor | `2.5` |
| router arithmetic | FP32 |

The exact 78-entry indexer reuse pattern, tensor list, tensor shapes, operation
order, indexer-key production/attachment mapping, and layer-78 recurrence
SHALL live in the generated model manifest
`manifests/glm52-operation-v1.json`. The Phase-A candidate is generated from
Rust; independent review of that artifact remains a blocking OPEN item for an
M1 pass.

## 5. Weight profiles

A process loads exactly one immutable weight policy.

### 5.1 `capacity-exl3`

- Routed experts SHALL use the reviewed EXL3 codec.
- Protected components SHALL use the manifest-declared source precision.
- The profile MUST admit 1,048,576 committed total positions in HBM across
  DCP4 after maximum configured scratch and graph capture.

### 5.2 `nvfp4-laboratory`

- Routed experts SHALL use the reviewed SM120 NVFP4 codec.
- Protected components SHALL use the manifest-declared precision.
- The profile SHALL contain only the tensor subset required by its declared
  CPU, microbenchmark, one-layer, or small-checkpoint experiment.
- It MUST NOT report full-model `HEALTHY`, expose the serving API, or be used
  as evidence of full-checkpoint capacity.
- The exact included tensor IDs and omitted dependencies SHALL be hashed into
  the laboratory manifest.

An all-NVFP4 full-model serving profile does not exist in v0.

### 5.3 `hybrid-serve`

- Each routed expert SHALL have one immutable physical gate/up representation
  and one immutable down representation selected by the manifest.
- Gate and up MAY be scored separately for quality, but their physical
  selection SHALL be expert-atomic: either two direct EXL3 source
  projections, or one combined NVFP4 1D projection. Mixed gate/up backends,
  split NVFP4, combined EXL3, and NVFP4 2D gate/up are forbidden.
- Down MAY independently use direct EXL3 source, NVFP4 1D, or NVFP4 2D.
- Codec selection MAY differ for the same expert index in different layers;
  the immutable selection key is `(layer_id, expert_id, tensor_role)`.
- Any NVFP4-bearing full-model serving process SHALL use this profile.
- The average routed-expert physical bytes MUST fit the reviewed profile
  budget in section 8; this requires a substantial EXL3 allocation.
- The policy MUST declare physical bytes and quality evidence.
- The policy, manifest, semantic catalog, target program, graph profile,
  load-plan domain, and cache namespace MUST bind the exact physical
  realization and distinguish NVFP4 1D from NVFP4 2D.
- Runtime routing MAY group active experts by codec.
- A request or step MUST NOT change an expert's codec.

The profile name alone is not a cache identity. The full weight policy hash,
target KV ABI hash, and draft KV ABI hash are REQUIRED.

## 6. Process and rank model

The engine SHALL use one process containing:

- one coordinator;
- one pinned worker thread per GPU;
- one CUDA primary context per worker;
- one rank number `0..3` per worker;
- bounded command and completion queues;
- separate compute, TP, DCP, and transfer streams per rank.

Only the coordinator MAY:

- admit or cancel requests;
- form a batch;
- select MTP depth;
- select a CUDA graph;
- choose TP or DCP routes;
- select packed-CKV or query transport;
- initiate a fallback;
- commit a `StepPlan`.

A worker MUST NOT independently select a route or fallback.

## 7. Startup state machine

Startup SHALL proceed through:

```text
CREATED
  -> HOST_VALIDATED
  -> CUDA_CONTEXTS_READY
  -> TOPOLOGY_VALIDATED
  -> MODULES_READY
  -> MEMORY_PLANNED
  -> WEIGHTS_LOADED
  -> GRAPHS_CAPTURED
  -> KV_READY
  -> COLLECTIVES_VOTED
  -> HEALTHY
```

Any failure SHALL transition all ranks to `FAILED`. No rank may report
healthy until every rank reaches the same state.

Before `WEIGHTS_LOADED`, the engine MUST validate:

- format major/minor;
- all source identity hashes;
- TP degree and rank file;
- complete tensor inventory;
- all tensor and codec descriptors;
- every payload hash;
- kernel ABI compatibility;
- selected serving profile.

Before `HEALTHY`, the engine MUST:

- capture or load every REQUIRED graph;
- allocate the configured active KV floor;
- execute the maximum-scratch escrow;
- validate peer access and collective resources;
- perform a four-rank adoption vote;
- run a no-model collective smoke;
- run a small deterministic kernel smoke.

No request may enter the scheduler before `HEALTHY`.

## 8. Static memory contract

The runtime SHALL use deterministic arenas. It SHALL NOT allocate device
memory in a serving step.

All production-serving weights SHALL remain HBM-resident. Runtime expert
weight paging from DRAM or NVMe is outside v0 because it would put
latency-critical weight traffic on the PCIe path. The mandatory DRAM/NVMe
offload contract applies to inactive KV and prefix pages.

The planner SHALL account for:

1. CUDA contexts and loaded modules;
2. immutable model metadata;
3. rank-local weights and codec metadata;
4. maximum prefill workspace;
5. maximum `C64 × MTP6 = 448` verifier workspace;
6. graph objects and graph argument slabs;
7. TP and DCP communication slabs;
8. HBM/DRAM tier staging rings;
9. target and draft KV pages and their page tables;
10. sparse-indexer key pages and their page table;
11. immutable model and execution-plan metadata;
12. MTP draft-layer committed and transactional KV plus indexer-key pages;
13. an emergency diagnostic escrow.

An MTP0 capacity profile SHALL reserve at least:

```text
262,144 local committed token slots per rank
```

at DCP4. Page rounding, active sequence tail slack, and MTP tentative slots
are additional and SHALL NOT consume those committed slots.

Every attention-capable profile SHALL reserve the cached sparse-indexer key
store defined in section 16.1. An MTP-enabled capacity profile SHALL
additionally reserve the one-layer draft KV plus draft-indexer sidecar defined
in section 16.

### 8.1 Normative infeasibility bound

The 75 target sparse layers contain:

```text
3 × 6,144 × 2,048 × 256 × 75
  = 724,775,731,200 routed-expert parameters
```

For the direct group-16 scale plane specified by codec `0x0100`, the target
routed experts alone require:

| Component | GiB aggregate |
|---|---:|
| packed E2M1 values at 0.5 byte/parameter | 337.500000 |
| one E4M3 byte per 16 values | 42.187500 |
| target routed-expert total | 379.687500 |

The most favorable nominal target total is 384 GiB before driver
reservations. The routed experts alone leave only 4.3125 GiB. This proves
that the profile cannot also contain the remaining 28.224B parameters,
runtime state, or KV.

Even a hypothetical physical layout storing one unique scale byte per
16×16 numerical scale tile would require 340.136719 GiB for the target
routed experts. A deliberately optimistic lower-bound table is:

| Mandatory component | GiB aggregate |
|---|---:|
| hypothetical compact 2D target experts | 340.136719 |
| estimated remaining 28.224B parameters at 0.5625 byte each | 14.785677 |
| 78-layer target KV at 1M | 28.031250 |
| 21-group sparse-indexer key cache at 1M | 2.707031 |
| one-layer draft KV at 1M | 0.359375 |
| one-layer draft indexer key cache at 1M | 0.128906 |
| minimum 1-GiB/rank escrow | 4.000000 |
| lower-bound total | 390.148958 |

The remaining-parameter count is an estimate derived from the rounded 753B
model total until the generated tensor inventory replaces it. Even pricing
every remaining parameter at NVFP4's 0.5625-byte 1D physical rate, the total
exceeds 390 GiB. This already exceeds nominal HBM and excludes CUDA contexts,
modules, metadata, graph objects, workspaces, communication slabs, staging,
padding, and page slack. Therefore `nvfp4-laboratory` is subset-only and
`hybrid-serve` is the only NVFP4-bearing serving profile.

### 8.2 Profile budget artifact

Before any full 753B conversion, a reviewed `profile-budget-v0.json` SHALL
freeze, per rank and aggregate:

- the smallest measured post-context HBM bytes;
- physical bytes by tensor role and codec;
- target and draft KV committed/slack bytes;
- target and draft sparse-indexer key committed/slack bytes;
- immutable model metadata and every target/draft/indexer page table;
- graph, maximum-workspace, collective, and staging bytes;
- padding and allocator fragmentation;
- emergency escrow;
- resulting maximum active token slots.

The converter SHALL consume this spec-owned budget and reject a policy that
exceeds any rank. At startup, the smallest-rank `cuMemGetInfo` result SHALL
validate the budget; it MUST NOT expand it.

For every rank `r`, the budget SHALL prove:

```text
weight_bytes[r]
+ module_and_context_bytes[r]
+ graph_resident_bytes[r]
+ maximum_workspace_bytes[r]
+ collective_and_staging_bytes[r]
+ target_kv_committed_and_slack_bytes[r]
+ draft_kv_committed_and_slack_bytes[r]
+ indexer_key_committed_and_slack_bytes[r]
+ draft_indexer_key_committed_and_slack_bytes[r]
+ model_metadata_bytes[r]
+ target_draft_indexer_page_table_bytes[r]
+ allocator_padding_bytes[r]
+ escrow_bytes[r]
<= measured_usable_hbm_bytes[r]
```

Aggregate fit is insufficient; every rank inequality MUST pass.

The memory planner SHALL compute from physical bytes, not nominal bpw.
After graph capture and maximum-workspace initialization, the minimum free
memory over all ranks MUST exceed the configured escrow. The initial escrow
is 1 GiB per rank. A reviewed M4 measurement MAY reduce it in a later spec
revision.

## 9. Rank-invariant step plan

Every GPU step SHALL consume one immutable `StepPlan`.

The logical descriptor contains:

| Field | Type | Meaning |
|---|---|---|
| `epoch` | u64 | process scheduling epoch |
| `step_id` | u64 | monotonically increasing step |
| `mode` | u8 | PREFILL, DECODE, VERIFY, MIXED, CACHE_ONLY |
| `active_sequences` | u16 | real request count |
| `sequence_bucket` | u16 | captured metadata bucket |
| `scheduled_prompt_tokens` | u32 | tokens to compute |
| `query_rows` | u32 | real target rows |
| `verifier_row_bucket` | u32 | captured row capacity |
| `mtp_depth` | u8 | 0–6 |
| `graph_id` | u32 | captured execution graph |
| `tp_route_id` | u16 | qualified TP route |
| `dcp_route_id` | u16 | qualified DCP route |
| `attention_transport` | u8 | NONE, PREFILL_CKV, PREFILL_QUERY, DECODE_QUERY_LSE |
| `sampling_route_id` | u16 | qualified sharded-vocabulary route |
| `sequence_table_generation` | u64 | immutable table generation |
| `collective_schedule_hash` | 32 bytes | ordered collective hash |
| `plan_hash` | 32 bytes | canonical plan hash |

The concrete FFI layout SHALL be specified in the kernel ABI after this
logical contract passes review.

All ranks MUST receive the same `plan_hash` and
`collective_schedule_hash`. A mismatch SHALL fail the engine before any
collective in that step.

Every field SHALL have a canonical zero value when unused by `mode`.
`CACHE_ONLY` SHALL set all compute, attention, and sampling fields to zero
and SHALL have an empty GPU collective schedule; it may carry only
coordinator-approved asynchronous tier transfers. Noncanonical unused fields
SHALL make the plan invalid before hashing.

## 10. Continuous batching scheduler

The scheduler SHALL maintain:

- decode-ready;
- verify-ready;
- prefill-ready;
- restore-pending;
- admission-pending;
- cancelled/cleanup queues.

Each scheduling iteration SHALL enforce:

- maximum active sequences;
- maximum target query rows;
- maximum prefill tokens;
- maximum scratch for the selected graph;
- available committed and tentative KV slots;
- decode inter-token latency objective;
- per-tenant queue and KV limits.

The first fairness policy SHALL be weighted deficit round-robin with aging.
The policy MAY change without a checkpoint ABI change, but its identity and
configuration MUST be in every benchmark record.

Chunked prefill MAY share a step window with decode only after a matched
latency/throughput qualification. Until then, prefill and decode use
separate steps with decode priority at its latency deadline. A shared step
MUST use `MIXED`, encode both workloads in one rank-invariant `StepPlan`, and
use a graph qualified for their combined maximum scratch.

Cancellation SHALL take effect at a step boundary. The coordinator SHALL
include the same removal in every rank's next sequence table generation.

## 11. Graph contract

Decode sequence buckets SHALL initially be:

```text
1, 2, 4, 8, 16, 32, 64
```

MTP depth SHALL be:

```text
0, 1, 2, 3, 4, 5, 6
```

Verifier row demand is:

```text
active_sequences × (mtp_depth + 1)
```

and SHALL be bounded by 448 rows.

The implementation SHOULD use row buckets, masked metadata, and graph-node
argument updates to reduce graph count. Weight policy SHALL NOT be a graph
key because it is process-immutable. Context bands SHOULD select pointer and
route tables rather than duplicate graphs when kernel structure is
unchanged.

An uncaptured execution path MAY exist for correctness testing. Production
traffic MUST NOT silently fall back to it. A missing graph SHALL reject or
queue the step with a metric.

Each serving profile SHALL have one reviewed `graph-profile-v0.json`
containing:

- every REQUIRED graph ID and complete key;
- real and bucketed shape limits;
- compatible attention and sampling route IDs;
- maximum scratch and argument bytes;
- measured graph-object and resident module bytes;
- supported admission/SLO class;
- hash of the kernel and `StepPlan` ABIs.

The graph-profile hash SHALL be process-immutable and recorded in every
benchmark. A request whose reachable steps are absent from the selected
graph profile SHALL be rejected at admission rather than queued indefinitely.
M4 SHALL validate the cost model against measured resident bytes before a
full serving profile can report `HEALTHY`.

## 12. Tensor parallel execution

Every rank SHALL hold one TP shard of every routed expert. Expert parallel
token all-to-all is forbidden in v0.

The reviewed model operation manifest SHALL mark every projection as:

- replicated;
- column parallel;
- row parallel;
- rank-local attention head;
- globally reduced.

The engine SHALL use 16 attention heads per rank.

Any fusion that changes a reduction boundary MUST include:

- the algebraic proof;
- accumulation precision;
- expected rounding difference;
- a one-layer replay result.

## 13. Collective routes

A qualified route table SHALL be keyed by:

- GPU PCI identifiers;
- PCIe ancestry/topology fingerprint;
- peer-access matrix;
- driver and NCCL versions;
- collective kind;
- payload size band;
- graph/eager mode.

Route candidates MAY include:

- NCCL;
- direct one-shot peer reduction;
- ring;
- tree;
- two-level pair hierarchy;
- byte-preserving packed-record gather.

Route selection SHALL happen before the `StepPlan` is committed. Every rank
SHALL use the route ID in the plan.

Collective resource initialization SHALL use:

```text
local initialization result
  -> four-rank MIN vote
  -> all adopt or all discard
```

Communicator and slab caches SHALL be keyed by capacity band, not exact tail
payload length.

## 14. Attention execution

### 14.1 Prefill

Prefill SHALL:

1. tokenize and resolve prefix pages on the host;
2. restore matched lower-tier pages asynchronously;
3. split remaining tokens into qualified chunks;
4. execute the fixed GLM layer graph;
5. write dynamic compressed KV records;
6. seal completed 64-token pages;
7. return the final prompt state to the decode scheduler.

The first DCP4 attention route SHALL keep 16 query heads local and gather
packed head-independent CKV records. It SHALL NOT perform an output
reduce-scatter for that route.

The engine SHALL also retain a query-transport control. A route table MAY
select CKV or query transport by prompt chunk, accumulated context band,
topology, and batch shape. The selection MUST be rank-invariant.

Cold prefill requires:

- a unique first full prefix block;
- zero HBM prefix attachments;
- zero DRAM restores;
- zero NVMe restores;
- server-computed prompt token accounting.

### 14.2 DCP4 decode attention

Production decode SHALL use `DECODE_QUERY_LSE`. Gathering top-2,048 packed
KV records to the query-head rank is forbidden for decode.

For every layer and real query row:

1. TP ranks all-gather their local query heads in rank order so each DCP page
   owner can process every head against owner-local KV.
2. At each full-indexer layer, each owner reads the paired 132-byte cached
   indexer-key record for each owned committed position, computes sparse-index
   scores, and selects up to 2,048 local candidates ordered by descending FP32
   score, then ascending logical position as the tie-break. Shared layers reuse
   the winner list exactly as declared by the operation manifest.
3. Owners all-gather `(score, logical_position)` candidates. Every rank
   performs the same fixed four-way merge and obtains one identical global
   top-2,048 position list. A nonfinite score is fatal to the request.
4. Each owner computes FP32 partial-softmax state for the winning positions
   it owns: local maximum `m_r`, local exponential sum `l_r`, and local
   weighted value vector `o_r`.
5. Partial state returns to the TP owner of each query head and merges in
   rank order `0,1,2,3`:

```text
m = max_r(m_r)
l = sum_r(exp(m_r - m) * l_r)
o = sum_r(exp(m_r - m) * o_r)
attention_output = o / l
```

Empty owner contributions use `m_r = -infinity`, `l_r = 0`, and a zero
vector. The fixed merge uses FP32 and is part of the numerical ABI. Query
transport, candidate exchange, partial-state return, and their order SHALL
appear in the `StepPlan` collective schedule.

The coordinator MAY omit a rank from a layer exchange only when its page
tables prove that rank owns zero committed pages for every sequence in the
step. The owner subset is rank-invariant and SHALL be encoded in the
`StepPlan`; a worker cannot make this decision. Layer data dependencies
prevent query-gather or partial-return batching across an IndexShare group in
v0. Candidate exchange occurs only on the group's full-indexer layer and its
winner list is reused by the following shared layers.

## 15. Decode and MTP

### 15.1 Target-only decode

MTP0 SHALL be the correctness reference. Greedy MTP0 SHALL match the pinned
reference token sequence under the sampling ABI's stable-position and
tie-adjacent classification. Stable-position mismatches fail; permitted
tie-adjacent divergences SHALL retain both tokens, the top-two logits, and
the margin.

### 15.2 Draft semantics

The model contains one next-token prediction layer. For configured depth
`K`, `0 <= K <= 6`, the engine SHALL recurrently apply that layer to produce
up to K draft tokens. It SHALL NOT load K independent draft layers.

The pinned layer-78 draft module contains:

- embedding/hidden-state normalization and projection;
- one MLA attention block with its own KV;
- one 256-routed-expert plus shared-expert MoE block;
- the shared normalized LM-head path.

The logical recurrence, frozen in `manifests/glm52-operation-v1.json`, is:

1. recurrence zero consumes the current target input token, its logical
   position, and the corresponding final target hidden state;
2. embed that token, replacing the embedding with zero only at logical
   position zero;
3. independently RMS-normalize the embedding with `enorm` and prior hidden
   state with `hnorm`, concatenate them in that order, and apply
   `eh_proj[6144,12288]`;
4. execute checkpoint layer 78's MLA attention, routed/shared MoE, residual,
   and TP reduction;
5. form `pre_final = residual + block_output`, then
   `recycled_hidden = shared_head.RMSNorm(pre_final)`;
6. compute draft logits as `shared_vocab_head(recycled_hidden)`;
7. recurrence `i+1` consumes the token sampled at recurrence `i`, position
   `position+i+1`, and that `recycled_hidden`.

There is exactly one independent draft layer. `spec_step_idx mod 1` therefore
selects checkpoint layer 78 for every recurrence. The manifest pins both the
generic and NVIDIA-fused source hashes; fused execution MUST preserve the
logical recurrence above.

At recurrence zero, layer 78 computes its exact top-2,048 sparse-attention
winner list. Later recurrences in the same speculative cycle reuse that
transient winner list. This reuse skips index scoring, not committed-state
production: every committed layer-78 position SHALL have its own 132-byte
indexer key. An implementation MAY backfill accepted tentative keys during
the synchronization pass, but it MUST do so before the page is sealed,
shared, evicted, or used by a later fresh index selection.

An MTP-capable sequence SHALL have draft KV and a draft indexer key for every
committed position. Prompt prefill for an MTP-capable cache entry SHALL
execute the draft layer over the prompt rows and populate both records in its
draft sidecar. A target-only prefix without that sidecar MAY be attached only
with MTP0; it MUST NOT enable MTP later unless the complete missing draft
prefix is recomputed and validated.

### 15.3 Greedy verification

For greedy sampling:

1. treat the verifier pass's target logits as authoritative for that step;
2. compare draft token `d_i` with verifier target argmax `t_i` in order;
3. accept equal tokens through the first mismatch;
4. at mismatch, emit `t_i` and reject remaining draft tokens;
5. if all K draft tokens match, emit the verifier target bonus token when
   available.

The emitted sequence MUST exactly equal the sequence implied by those
authoritative verifier logits. It is not required to be bit-identical to a
separate MTP0 run whose target kernels execute at a different M or bucket.
Version zero does not require batch-invariant GEMM/reduction kernels.

The MTP quality gate SHALL compare MTPK with MTP0 over a pinned corpus and
retain every position's selected tokens, top-two target logits, top-one
margin, and logit error. Agreement is required at positions classified as
numerically stable by the reviewed sampling ABI. A mismatch MAY be
classified tie-adjacent only when the competing tokens and margin satisfy
that ABI's fixed tolerance; all other mismatches fail. Aggregate match,
tie-adjacent divergence, KLD, task quality, and accepted length SHALL be
reported separately. The stable/tie thresholds are blocking before MTP1 is
enabled.

### 15.4 Probabilistic verification

For draft distribution `q_i` and target distribution `p_i`, the engine SHALL
use standard rejection sampling. Both distributions SHALL be the actual
sampling distributions after the same ordered history penalties,
temperature, top-k, top-p, forbidden-token, and normalization operations:

```text
accept d_i with probability min(1, p_i(d_i) / q_i(d_i))
```

On first rejection it SHALL sample from normalized:

```text
max(p_i - q_i, 0)
```

If every draft token is accepted, it MAY emit the target bonus sample. RNG
counter advancement, filter order, sharded-vocabulary construction,
acceptance ratio, residual sampling, bonus sampling, and floating-point
probability construction MUST be defined in a separate sampling ABI and
tested statistically against the pinned reference.

### 15.5 Sharded-vocabulary sampling

The LM head SHALL be column-parallel with one contiguous vocabulary interval
per rank. Production execution MUST NOT gather full-vocabulary logits.

For greedy sampling, each rank computes its FP32 local `(maximum, token_id)`
and a fixed-rank reduction chooses the greatest value, breaking exact ties by
the smallest global token ID.

For probabilistic sampling, v0 supports:

- `temperature > 0`;
- `top_k = 0..256`;
- `top_p = 0 < p <= 1`;
- no history penalty not explicitly defined by the sampling ABI.

The filter order SHALL be temperature, top-k, top-p, then FP32
normalization. If `top_p < 1`, `top_k` MUST be in `1..256`; an unbounded
top-p sort is outside v0 and SHALL be rejected.

For `top_k > 0`, each rank selects local candidates, all-gathers at most
`top_k` `(logit, token_id)` pairs per row, and rank zero performs the
fixed-order global merge/filter/sample before broadcasting token IDs and RNG
counters.

For `top_k = 0, top_p = 1`, ranks compute a deterministic distributed FP32
log-sum-exp and probability mass. One shared counter-based uniform draw
selects a rank interval in rank order, and that rank selects the token from
its local vocabulary-order CDF. Residual `max(p-q,0)` sampling uses the same
distributed-mass procedure over rank-local post-filter arrays. Bonus
sampling uses the target post-filter distribution.

The sampling ABI SHALL define counter allocation per request, position,
draft step, accept/reject decision, residual sample, and bonus sample. Every
sampled token and final counter SHALL be broadcast before the next
`StepPlan`.

### 15.6 Transactional KV

Target draft-position writes, draft-layer KV/indexer writes, and verifier
writes SHALL target tentative slots in their respective page tables.

- Accepted target, draft-KV, and draft-indexer slots SHALL commit in the same
  order.
- Rejected target, draft-KV, and draft-indexer slots SHALL become unreachable
  before reuse.
- A shared sealed prefix SHALL never be modified.
- Target/draft sidecar page hashes and tier journals SHALL include only
  committed tokens.
- A request within K tokens of the model limit SHALL clamp depth so committed
  positions never exceed 1,048,576.

The scheduler MAY select a process-configured fixed depth. An adaptive depth
controller is OPTIONAL, but if enabled it SHALL choose one rank-invariant
depth per batch and record the choice.

## 16. KV record ABI

Each layer/token record SHALL be exactly 368 bytes:

| Byte range | Meaning |
|---|---|
| `[0,256)` | 512 packed E2M1 NoPE values |
| `[256,288)` | 32 E4M3 group-16 scale bytes |
| `[288,292)` | FP32 little-endian RoPE scale |
| `[292,296)` | FP32 little-endian per-token NoPE outer scale `s_t` |
| `[296,304)` | zero padding |
| `[304,368)` | 64 E4M3 RoPE values |

For NoPE input `x[0..512)`:

```text
amax_t = max(abs(x))
s_t = amax_t / (6 × 448)
```

If `amax_t == 0`, the writer SHALL store FP32 `s_t = 1.0`, all E4M3 group
scale bytes SHALL be positive zero `0x00`, and every packed E2M1 nibble SHALL
be positive zero `0x0`.

Each 16-value group SHALL be divided by `s_t`, assigned an E4M3 group scale,
and encoded to E2M1 using the reviewed hardware-equivalent rounding and
saturation behavior. Readers SHALL restore both the group scale and `s_t`.

For RoPE input `r[0..64)`:

```text
rope_scale = max(abs(r)) / 448
```

The writer SHALL store saturated finite E4M3 values of
`r / rope_scale`. If the RoPE input is all zero, it SHALL store FP32
`rope_scale = 1.0` and 64 positive-zero E4M3 bytes. Exact nonzero conversion
behavior SHALL match the CPU oracle.

Static records with zero bytes at `[292,296)` are a different ABI and MUST
be rejected. The ABI string is:

```text
nvfp4_ds_mla:fp8-rope-368:dynamic-token-v1
```

Target and draft layers use the same numerical KV record ABI but different
content namespaces and page tables. At 1,048,576 positions:

```text
target: 368 × 78 × 1,048,576 = 28.03125 GiB aggregate
draft:  368 ×  1 × 1,048,576 =  0.359375 GiB aggregate
MTP-capable total:                         28.390625 GiB aggregate
```

### 16.1 Sparse-indexer key record ABI

The production indexer path SHALL cache, rather than recompute from MLA
latents, one key record for each position at every full-indexer layer. The
21 full-indexer layer IDs are:

```text
0, 1, 2, 6, 10, 14, 18, 22, 26, 30, 34,
38, 42, 46, 50, 54, 58, 62, 66, 70, 74
```

Each record is exactly 132 bytes:

| Byte range | Meaning |
|---|---|
| `[0,128)` | 128 saturated-finite E4M3 key values |
| `[128,132)` | one FP32 little-endian scale |

For key vector `k`, the stored scale is the smallest power of two greater
than or equal to `max(max(abs(k)), 1e-4) / 448`, matching the pinned
`ue8m0` scale policy, and the value bytes encode `k / scale`. The record ABI
string is:

```text
glm52_dsa_index_k:e4m3-128:fp32-ue8m0-scale:v1
```

The HBM store is full-indexer-group-major and uses the same 64-token logical
page ordinal, owner, and generation as target KV:

```text
indexer_k[full_group=0..20][local_page_id][token=0..63][record=132]
```

At 1,048,576 positions this consumes:

```text
132 × 21 × 1,048,576 = 2,906,652,672 bytes
                         = 2.70703125 GiB aggregate
                         = 0.6767578125 GiB per DCP4 rank
```

A committed target page and its indexer-key sidecar SHALL publish atomically.
The indexer sidecar is mandatory for an attention-capable target prefix and
SHALL be evicted, restored, integrity-checked, and reattached with that target
page. An orphan or generation mismatch is invalid.

Layer 78 is an additional full indexer and SHALL use the same 132-byte
record. Its HBM store is separate from the 21 target groups:

```text
draft_indexer_k[draft_layer=0][local_page_id][token=0..63][record=132]
```

At 1,048,576 positions the draft indexer consumes 138,412,032 bytes,
0.12890625 GiB aggregate, or 0.0322265625 GiB per DCP4 rank. The complete
MTP-capable committed cache is therefore:

```text
target KV       28.03125000 GiB
target indexer   2.70703125 GiB
draft KV         0.35937500 GiB
draft indexer    0.12890625 GiB
total           31.22656250 GiB aggregate
per DCP4 rank    7.806640625 GiB
```

The optional MTP draft sidecar contains both its KV and indexer records and
remains a third, separately namespaced attachment.

## 17. HBM page layout and DCP ownership

The logical page size SHALL be 64 tokens.

At DCP4, committed page ordinal `j` within a sequence SHALL initially use:

```text
owner_rank = j mod 4
```

The page table SHALL map logical `(sequence, page_ordinal)` to
`(owner_rank, generation, local_page_id)`.

Each owner rank SHALL store all 78 target-layer fragments for its owned
logical page. The target kernel-facing HBM layout SHALL be layer-major:

```text
target_kv[target_layer=0..77][local_page_id][64][368]
```

One full logical page payload across 78 layers is:

```text
78 × 64 × 368 = 1,837,056 bytes
```

An MTP-capable sequence SHALL additionally map the same logical page ordinal
and owner to:

```text
draft_kv[draft_layer=0][local_page_id][64][368]
draft_indexer_k[draft_layer=0][local_page_id][64][132]
```

The committed draft sidecar fragment is 32,000 bytes per logical page. At
DCP4 it adds 0.1220703125 GiB per rank at the model limit before tail slack.
Target and indexer page generations SHALL commit atomically. The draft page
generation SHALL join that commit for an MTP-capable sequence.

The proposed modulo ownership is a blocking OPEN item until a CPU ownership
proof and comparison with the pinned control are reviewed. The final mapping
MUST keep all ranks balanced and preserve exact logical token ordering.

## 18. KV tiering

HBM is the only active attention tier. DRAM and NVMe are mandatory storage
tiers for sealed paused-session and prefix pages.

The page state machine SHALL include:

```text
FREE
HBM_MUTABLE
HBM_TENTATIVE
HBM_SEALED
DRAM_WRITING
DRAM_RESIDENT
NVME_WRITING
NVME_RESIDENT
RESTORING
INVALID
```

Legal transitions SHALL be listed in the format/cache specification and
tested exhaustively. A worker SHALL never block on host or filesystem I/O.

Tier transfer SHALL preserve the exact 368-byte record payload. Lossy
transcoding is forbidden.

The DRAM tier SHALL:

- be bounded;
- be process-volatile in v0 and discarded after a process crash or restart;
- use bounded pinned staging rather than pinning the entire tier;
- expose bytes, page count, hit rate, and transfer latency.

The NVMe tier SHALL:

- be bounded by configured bytes;
- use aligned large records;
- checksum headers and payloads;
- publish a generation only after all REQUIRED pieces are durable;
- ignore incomplete generations after a crash;
- enforce a configured rolling bytes-per-day write cap;
- stop admitting new NVMe writes, without corrupting resident records, when
  that cap is reached;
- expose read/write bytes, write amplification, restore latency, and errors.

Publication SHALL make the target record and mandatory target-indexer sidecar
visible in one index generation. For an MTP-capable page, that generation
SHALL also publish its combined draft-KV/draft-indexer sidecar. Recovery SHALL
expose either all three tier records or the independently valid
target/target-indexer pair as MTP0-only; it SHALL never expose an orphan
sidecar.

GPUDirect Storage is OPTIONAL and MUST be a matched optimization, not a
correctness dependency.

## 19. Prefix cache

Only sealed full pages MAY be shared.

The content namespace hash SHALL cover:

- model and weight policy hashes;
- tokenizer and chat-template hashes;
- target, indexer, or draft cache ABI and record role;
- page size;

It SHALL NOT include DCP ownership, writer rank, HBM layout, or a kernel
revision that leaves the record bytes and arithmetic unchanged. Any change
to record interpretation SHALL change the corresponding KV ABI.

The logical page key SHALL be a cryptographic hash of:

```text
content_namespace_hash
parent_page_hash
valid_token_count
token_ids
```

HBM attachment SHALL separately validate an attachment ABI covering:

- fixed DCP4 ownership mapping;
- target/indexer/draft page-table layout;
- kernel ABI and alignment;
- current destination owner.

Tier records are ownership-neutral. Restore SHALL validate content first,
select the current owner from the process-immutable DCP4 mapping, and scatter
the exact bytes into that owner's HBM layout. The tier header's writer owner
is advisory only.

Full target page hits MAY attach an HBM page or restore an exact DRAM/NVMe
page only with the paired target-indexer sidecar. MTP additionally requires a
paired draft hit containing both draft KV and draft-indexer keys. Partial page
matches SHALL be recomputed into private mutable target/indexer/draft tails as
applicable.

Reference counts and state changes SHALL be atomic with respect to scheduler
admission. Eviction SHALL NOT invalidate a page referenced by an active or
restore-pending request.

Metrics SHALL distinguish:

- computed prompt tokens;
- HBM attached tokens;
- DRAM restored tokens;
- NVMe restored tokens;
- recomputed tail tokens.

## 20. Admission and capacity semantics

The service SHALL publish:

- total physical HBM KV slots;
- committed free target, indexer, and draft HBM slots;
- tentative/slack target, indexer, and draft HBM slots;
- DRAM resident and free bytes;
- NVMe resident and free bytes;
- maximum admissible single sequence;
- current active working set.

DRAM/NVMe bytes MUST NOT be advertised as active context.

A request SHALL be admitted only if its next scheduling horizon can fit the
required HBM working set or a bounded restore plan. The engine SHOULD queue
or suspend near-1M sessions rather than repeatedly swap an active context.

The capacity profile MUST admit one sequence whose committed prompt plus
generated tokens can reach 1,048,576. Tentative MTP tokens beyond the
remaining model positions SHALL not be scheduled.

## 21. Failure semantics

The following are fatal to the engine process:

- rank plan hash mismatch;
- rank collective schedule mismatch;
- unexpected tensor or codec;
- payload checksum failure;
- CUDA illegal access or launch failure;
- collective timeout or asynchronous error;
- page ownership conflict;
- committed prefix checksum mismatch;
- memory escrow failure after health was claimed.

The following SHALL fail only the affected request when containment is
proven:

- invalid user parameters;
- context limit exceeded;
- cancelled request;
- unavailable lower-tier prefix with a valid recompute path;
- sampling constraint failure.

Any rank-level error SHALL propagate to the coordinator and all workers. A
rank MUST NOT continue through a collective after its peer has failed.

## 22. Minimal serving API

Version zero SHALL provide:

- text chat completions;
- text completions if needed by the quality harness;
- streaming output;
- temperature, top-p, top-k, maximum output tokens, stop tokens/strings, and
  deterministic seed;
- cancellation;
- health and metrics endpoints.

If `top_p < 1`, clients MUST also provide `top_k` in `1..256`. The service
SHALL return a structured `UNBOUNDED_TOP_P_UNSUPPORTED` error otherwise.
Clients that want conventional nucleus sampling SHOULD send `top_k=256`;
the service SHALL NOT silently substitute it.

The pinned GLM chat template SHALL define message and tool-schema rendering.
The service is not required to reproduce unrelated vLLM/SGLang flags or
extension APIs.

## 23. Required observability

Per request:

- queue time;
- prefix resolution time;
- restore time by tier;
- computed and restored prompt tokens;
- draft-KV prompt tokens computed or restored;
- TTFT;
- accepted output tokens;
- inter-token latency;
- selected MTP depth and acceptance by position;
- termination reason.

Per step:

- real and bucketed sequence count;
- real and bucketed query rows;
- graph ID;
- TP/DCP/attention/sampling route IDs;
- collective schedule hash;
- kernel, TP, DCP, and host time;
- query, sparse-candidate, partial-softmax, and sampling collective bytes;
- a separate DCP query-gather/candidate/partial-LSE latency line, including
  owner-subset size;
- HBM allocation and KV page deltas.

Per service:

- useful accepted tokens/s;
- p50/p95/p99 TTFT and inter-token latency;
- scheduler occupancy and fairness;
- cache hits and restore bytes by tier;
- NVMe write amplification, rolling write-cap usage, and errors;
- clocks, power, temperature, and throttling during benchmarks.

Speculative proposals and restored prefix tokens SHALL NOT be counted as
useful generated or computed tokens.

## 24. Gate requirements

### M1 CPU gate

- complete tensor/operation manifest;
- deterministic checkpoint container;
- exact NVFP4 CPU reconstruction;
- exact KV record oracle;
- page ownership and tier state-machine tests;
- MTP0–MTP6 CPU transition tests;
- physical memory budget calculator.

The EXL3 CPU reconstruction is a separate REQUIRED gate before
`capacity-exl3` can load or proceed to a GPU kernel.

### M2 authorized SM120 gate

- real GLM expert shapes;
- inclusive NVFP4 operator timing first;
- correctness across decode and prefill M;
- hardware counters and bytes;
- qualified collective route table.

The NVFP4 M2 exit MAY proceed through M3 and the M4 small-checkpoint runner
without EXL3. EXL3 SHALL repeat the same M1 and M2 gates before any
full-checkpoint `capacity-exl3` or `hybrid-serve` load.

### M3 one-layer gate

- exact routes and index winners;
- DCP4 decode query transport and fixed-order FP32 LSE merge;
- an exclusive DCP collective-chain ledger line, separated from indexer
  compute, attention compute, and framework overhead;
- layer output and downstream logit comparison;
- four-rank collective agreement;
- exclusive phase ledger.

### M4 small-checkpoint runner gate

- `nvfp4-laboratory` tensor subset only;
- static prefill and greedy MTP0 decode through production descriptors;
- deterministic error, graph, collective, and memory-plan paths;
- reference logits within the declared numerical contract;
- no claim of full-model service or capacity.

### M5 full-checkpoint, quality, and capacity gate

- full fit-capable checkpoint smoke;
- reviewed `profile-budget-v0.json`;
- reviewed EXL3 CPU and M2 kernel gates;
- `capacity-exl3` or `hybrid-serve`; never `nvfp4-laboratory`;
- per-position KLD and task gates;
- target-only MTP0 before speculation;
- target-only versus MTP-capable prefill cost reported separately;
- one admitted 1,048,576-token total sequence;
- cold/warm cache accounting.

NVFP4-only CPU, kernel, one-layer, and small-checkpoint success does not
satisfy M5. M5 SHALL retain matched NVFP4 laboratory/operator controls.

### M6/M7 serving gate

- continuous mixed traffic through C64 where capacity permits;
- MTP0–MTP6 acceptance and useful throughput;
- target/draft prefix restore and rollback;
- sharded-vocabulary greedy, top-k/top-p, residual, and bonus sampling;
- prefix hits from HBM, DRAM, and NVMe;
- cancellation and fault injection;
- fairness and latency goodput;
- long-duration bounded-resource run.

## 25. Blocking OPEN items for Fable review

1. Independently review the generated GLM tensor/operation manifest,
   including the 21-group indexer-key production and IndexShare mapping.
2. Exact EXL3 trellis payload/reconstruction ABI from pinned source; this
   does not block M1–M4 NVFP4 bring-up but blocks M5–M7.
3. Final SM120 NVFP4 weight layout and CUDA/CUTLASS pin.
4. Independently review the Phase-A MTP recurrence and the combined
   draft-KV/draft-indexer residency, tiering, attach, and rollback amendment.
5. Probabilistic sampling/RNG ABI, including post-filter distributions,
   sharded-vocabulary execution, and residual/bonus sampling.
6. Greedy batch-shape numerical equivalence thresholds and tie-adjacent
   classification; batch-invariant kernels are not required by v0.
7. Final DCP4 page ownership mapping and deterministic decode attention
   candidate/LSE merge.
8. Exact Rust/C `StepPlan` and kernel descriptor layout.
9. Reviewed graph capture manifest, graph-resident memory, maximum workspace,
   and profile budget.
10. Crash-consistent multi-rank NVMe publication protocol.
11. M5 per-position KLD and task acceptance thresholds.
12. Incremental UTF-8 detokenization and cross-chunk stop-string semantics
    before the serving gate.

No blocking OPEN item may be silently chosen during implementation. Its
resolution requires a specification edit and review record.
