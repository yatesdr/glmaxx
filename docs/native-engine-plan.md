# SM120-native GLM-5.2 inference engine plan

Status: Fable v2 accepted; Phase-A manifest/ABI amendment pending review

Date: 2026-07-28

Target: 4× RTX PRO 6000 Blackwell, SM120, PCIe, no NVLink

Model: GLM-5.2 at TP=4, up to 1,048,576 total tokens

## Decision summary

Build a GLM-5.2-only engine whose control plane, memory planner, scheduler,
checkpoint tooling, and public API are written in Rust. Put the GPU programs
in a small, precompiled SM120-only CUDA/CUTLASS kernel library behind a stable
C ABI. There will be no C++ runtime or generic model abstraction.

Reserve one container and kernel ABI for two weight families:

1. **EXL3/Trellis** is the leading capacity-and-quality path. The initial
   control is the measured 3.0-bpw routed-expert checkpoint class.
2. **NVFP4** is the hardware-native speed path. It must be packed in the exact
   value and scale layout consumed by SM120 block-scaled MMA.

Implementation is deliberately single-track and NVFP4-first. Its defined
arithmetic and direct SM120 path make it the lowest-risk backend for proving
the Rust executor, kernel ABI, TP4, graphs, scheduling, MTP, and tiered KV.
EXL3 remains a required capacity backend, but native EXL3 work begins only
after the NVFP4 CPU/kernel path and after its own exact packing and
reconstruction contract has been extracted from pinned source and reviewed.
EXL3 is not on the M1–M4 NVFP4 bring-up path; it is mandatory before M5.

NVFP4-first does not mean all-NVFP4-full-checkpoint-first. At the published
753B geometry, the 724.776B target routed-expert parameters alone occupy
379.6875 GiB with packed E2M1 values plus the direct group-16 E4M3 scale
plane. Even a hypothetical compact 2D scale plane, the estimated remaining
parameters at the 0.5625-byte NVFP4 1D floor, target/indexer/draft 1M cache,
and a 1-GiB/rank escrow total 390.148958 GiB before runtime overhead. An
all-NVFP4 serving checkpoint cannot exist on this target.

NVFP4 proves the codec, kernel, one-layer executor, and hot-expert path first.
A reviewed EXL3 policy is required before the full-checkpoint gate, and
`hybrid-serve` is the only NVFP4-bearing serving profile.

The on-disk ABI permits either format per tensor or per `(layer, expert)`.
After both backends pass, the required routing-aware serving policy may keep
hot experts in NVFP4
and colder experts in EXL3, while retaining high precision for sensitive
tensors. It is accepted only if its measured quality, physical bytes, and
execution time beat the capacity control.

Multi-user serving is a core requirement, not an optional wrapper. The engine
must provide continuous batching, chunked prefill, prefix sharing, fair
admission, configurable MTP depth 0–6, and three-tier HBM/DRAM/NVMe KV
residency. These features shape the memory plan and CUDA graph families from
the beginning even though they are enabled only after the fixed runner proves
correctness.

Reserve KV capacity before assigning the weight budget. Use the already
proven dynamic per-token NVFP4 MLA record with FP8 RoPE as the first KV
format. At 368 bytes per layer per token, 78 target layers and 1,048,576
tokens require 28.03125 GiB aggregate. The attention-bearing MTP draft layer
adds 0.359375 GiB of KV and 0.12890625 GiB of indexer keys.

The sparse indexer additionally caches 21 refresh groups of 132-byte
E4M3-plus-FP32-scale keys, adding 2.70703125 GiB aggregate. The MTP-capable
target, indexer, and draft total is therefore 31.2265625 GiB aggregate or
7.806640625 GiB per rank before page slack.
The existing EXL3 control has already reported 1,101,312 physical GPU KV
tokens, so the immediate problem is to make the full request admissible and
fast, not to invent a smaller KV representation.

Develop in the repository's required order:

```text
design note
  -> adversarial review
  -> CPU format and operation proof
  -> authorized SM120 microbenchmark
  -> one-layer replay
  -> checkpoint smoke
  -> quality gates
  -> matched end-to-end benchmark
```

No GPU work is authorized by this plan.

## Specialization boundary

The advantage of this engine is the work it does not need to do. Version zero
supports:

- GLM-5.2 and its pinned MTP draft tensor inventory;
- four SM120 GPUs in one host;
- TP4 and process-immutable DCP4 over those same four ranks;
- EXL3/Trellis, NVFP4, and an explicit hybrid of those weight backends;
- the single versioned compressed MLA KV ABI;
- a deliberately small batch, chunk, context-band, and MTP-depth dispatch
  table.

Version zero does not support:

- other models or a model registry;
- non-SM120 GPUs or portable kernel fallbacks;
- arbitrary TP, pipeline parallelism, or expert parallelism;
- training, fine-tuning, LoRA, adapters, or runtime weight mutation;
- generic eager tensor execution or runtime graph construction;
- arbitrary quantization plugins;
- multimodal encoders;
- CPU inference fallback;
- the full configuration and API surface of vLLM or SGLang.

This permits fixed GLM operation enums, compile-time matrix dimensions,
rank-local checkpoint files, preselected fusions, static scratch offsets,
and precompiled CUDA graphs. The remaining runtime choices are only choices
that matter for the target: request mix, batch bucket, prompt chunk, context
band, weight backend, MTP depth, KV residency, and measured PCIe route.

Serving compatibility should be narrow: the required text chat/completions
and metrics behavior, pinned tokenizer/chat-template semantics, common
sampling controls, streaming, cancellation, and structured error reporting.
Do not reproduce unrelated command-line flags or extension systems.

## Fixed model contract

The first engine version recognizes exactly one pinned model identity. The
starting upstream revision is
`zai-org/GLM-5.2@b4734de4facf877f85769a911abafc5283eab3d9`.
Its relevant fixed geometry is:

| Property | Value |
|---|---:|
| target layers | 78 |
| hidden size | 6,144 |
| dense intermediate | 12,288 |
| routed experts | 256 |
| routed experts selected per token | 8 |
| shared experts | 1 |
| expert intermediate | 2,048 |
| dense front layers | 3 |
| attention heads | 64 |
| local TP4 heads | 16 |
| KV latent rank | 512 |
| RoPE head dimension | 64 |
| sparse index top-k | 2,048 |
| index heads / dimension | 32 / 128 |
| MTP layers | 1 |
| vocabulary | 154,880 |
| maximum positions | 1,048,576 |

The loader fails closed if the model revision, tensor inventory, shapes,
tokenizer/config hashes, or architecture constants differ. Supporting a
future GLM revision requires a new engine ABI or an explicitly reviewed
manifest, not permissive shape inference.

The model declares one next-token prediction layer. Configurable “MTP depth”
means applying that pinned draft layer recurrently to propose 0–6
speculative tokens and verifying up to `depth + 1` query rows per request. It
does not mean loading six independent draft layers.

## What “written in Rust” means

Rust owns:

- checkpoint manifests, packing policy, checksums, and the CPU oracle;
- CUDA Driver API resource ownership and error handling;
- one-process/four-device coordination;
- static memory planning and KV page management;
- execution plans, graph selection, request scheduling, and sampling;
- tokenizer integration, prefix-cache metadata, metrics, and serving API;
- benchmark orchestration and immutable result manifests.

CUDA C++/CUTLASS owns only device kernels that need SM120 MMA, TMA, warp
specialization, or direct peer-memory operations. Those kernels are compiled
ahead of time for the selected SM120 feature target and exposed as plain C
functions taking POD descriptors and CUDA streams. Rust never sees CUTLASS
types.

This boundary keeps the engine Rust-native without giving up the NVIDIA
toolchain that exposes SM120 tensor-core features. It also keeps unsafe code
concentrated in two auditable places: the low-level CUDA bindings and the
kernel ABI.

Do not use Python, PyTorch, Triton, a JIT compiler, or a dynamic computation
graph in the production executor. Python remains acceptable for generating
pinned reference fixtures and for independent evaluation harnesses.

## Runtime architecture

Use one Rust process with:

- one coordinator thread;
- one pinned worker thread and CUDA primary context per GPU;
- a rank-invariant `StepPlan` built by the coordinator;
- preallocated lock-free command and completion queues;
- fixed streams for compute, TP communication, DCP communication, and
  asynchronous host transfers;
- no allocator calls after graph capture begins.

The coordinator is the sole authority for batch shape, collective route,
graph choice, DCP transport, and fallback. Every `StepPlan` contains an
ordered collective schedule and a digest. All four workers verify the same
digest before entering the step.

Rank-local fallback is forbidden. Initialization follows:

```text
local capability/resource result
  -> four-rank MIN vote
  -> all ranks adopt the path or all ranks reject it
```

The initial runner is deliberately smaller than a server:

1. one request;
2. MTP disabled;
3. fixed prompt chunking;
4. greedy sampling;
5. static KV allocation;
6. deterministic logits comparison.

This is a validation stage, not the product architecture. The same executor
uses batched sequence descriptors, refcounted KV page handles, transactional
tail pages, and graph keys that extend to continuous batching and MTP. Avoid
a single-request implementation that must be replaced to become a server.

### Continuous scheduler

Use iteration-level continuous batching with separate logical queues for:

- decode-ready requests;
- MTP verification-ready requests;
- new or resumed prefills;
- KV restore from DRAM or NVMe;
- requests blocked by admission or backpressure.

The scheduler builds a global `StepPlan` under budgets for total query rows,
prefill tokens, scratch, KV pages, and latency. It may mix chunked prefill
with decode only in graph families whose interference has been measured.
Decode receives an explicit inter-token-latency budget so a large prompt
cannot monopolize the device.

Start with graph batch buckets `1, 2, 4, 8, 16, 32, 64`. Pad metadata to the
next bucket while keeping token and expert work masked. Retain an explicit
uncaptured correctness path for unusual tails during development, but do not
silently serve production traffic through it.

Avoid a combinatorial graph matrix. One engine instance loads one immutable
weight policy, so EXL3/NVFP4/hybrid is not a per-step graph key. Context bands
select pointer tables and collective plans without changing the fixed sparse
top-k kernel shape. Decode graphs are keyed primarily by active-sequence
bucket, verifier-row bucket, and MTP depth. Capture only reachable/hot
combinations; use graph-node parameter updates and masked rows where they are
cheaper than another graph.

Every serving policy has a reviewed, hashed graph-profile artifact listing
the required graph keys, route compatibility, scratch, resident bytes, and
admission/SLO class. Requests whose reachable shapes are absent are rejected
at admission. Start with a 1-GiB/rank emergency escrow and let M4 measurement
justify any reduction.

Admission is based on the HBM working set, not aggregate DRAM/NVMe capacity.
A single near-1M active request can consume most of the physical GPU KV pool;
the scheduler must queue or suspend other long active requests instead of
thrashing tens of GiB between tiers. Multi-user throughput goals primarily
apply to the measured short/medium-context concurrency bands, while paused
long sessions remain cheaply resident in lower tiers.

Fairness is defined and measured. The first policy is weighted deficit
round-robin over tenants with:

- per-request maximum prefill chunk;
- an aging term for time-to-first-token;
- a decode latency deadline;
- per-tenant queued-token and resident-KV limits;
- bounded cancellation and cleanup time.

Report useful output tokens, not scheduled or speculative tokens, for
throughput. Also report TTFT, inter-token latency, p50/p95/p99, queue time,
batch occupancy, accepted MTP tokens, and KV tier stalls.

## Tensor-parallel execution

Use TP=4 with every routed expert sharded across all four GPUs. This avoids
expert-parallel token all-to-all on PCIe and matches the fixed hardware
target.

The starting partition is:

- column-parallel Q and MLP input projections;
- row-parallel attention output and MLP down projections;
- 16 attention heads per rank;
- every rank stores one TP shard of all 256 routed experts and the shared
  expert;
- hidden-state reductions use topology-selected four-rank collectives.

The exact reduction points must be derived from the pinned GLM reference and
proven in the CPU graph. Fusion may delay or combine a reduction only when
the algebraic equivalence and accumulation order are documented.

For decode-sized payloads, test one-shot and pair-hierarchical reductions
against NCCL. For prefill-sized payloads, test rings, trees, and
pair-hierarchical schedules. NCCL is the initial correctness control and a
group-selected fallback, not an assumed final fast path.

## Weight representation

### Common rank-file ABI

Produce four rank-local files so startup performs no slicing or repacking.
Each file contains:

- magic, endian marker, format ABI, and kernel ABI;
- model, tokenizer, converter, calibration, and policy revisions;
- TP degree and rank;
- exact logical and padded tensor shapes;
- tensor, expert, and precision-tier tables;
- payload and scale offsets with explicit alignment;
- the kernel layout identifier for every payload;
- per-payload and whole-manifest cryptographic hashes;
- total physical bytes by tensor class and format.

The physical payload is the kernel input. Runtime transpose, swizzle, scale
reordering, or whole-weight reconstruction means the checkpoint ABI is
incomplete.

### EXL3/Trellis backend

Treat the current 3.0-bpw checkpoint as a control, not as an immutable format
decision. Reproduce its CPU reconstruction exactly, then isolate:

- bitstream and codebook/LUT layout;
- group size and scale representation;
- Hessian or calibration inputs;
- tensor protection rules;
- per-expert bit allocation;
- reconstruction arithmetic and accumulation type;
- cost of decode and prefill reconstruction.

The first custom EXL3 kernel should consume packed weights directly and
reconstruct fragments in registers or shared memory immediately before use.
It must not expand an expert shard into a persistent BF16/FP16 buffer.

Test separate kernels for:

- `M = 1, 2, 4, 8, 16` decode and MTP verification;
- routed prefill expert groups at the real 6,144×2,048 shapes;
- FC1/gate fusion where the physical layout permits it;
- FC1 epilogue + SiLU + FC2 input construction;
- expert-weighted reduction into the TP partial.

The capacity profile starts with EXL3 for routed experts and higher precision
for the router, indexer, attention projections, dense front layers, shared
expert, LM head, and measured-sensitive MTP tensors.

### NVFP4 backend

Pack quantized values and UE4M3 scale factors directly into the scale-vector
and `32x4x4`-style swizzle required by Blackwell block-scaled MMA. Freeze the
exact layout only after the selected CUDA and CUTLASS revision passes an
SM120 correctness matrix.

Measure activation quantization as part of the operator. Candidate fusions
include:

- BF16-to-NVFP4 activation quantization with input staging;
- grouped routed-expert FC1/gate;
- activation and second-input scaling;
- FC2 plus expert weighting;
- accumulation/reduction without writing avoidable intermediates.

An NVFP4 result is not “faster” because its GEMM subinterval is faster.
Routing, sorting, descriptor construction, activation scaling, epilogues,
launches, and TP communication are inside the comparison interval.

### Hybrid backend

The common ABI permits format selection per tensor and per expert. The first
hybrid hypothesis is:

- NVFP4 for routing-hot, quality-tolerant experts where direct MMA wins;
- EXL3 for colder experts where bytes dominate capacity;
- FP8/BF16 or source precision for sensitivity-protected tensors.

Allocation uses measured routing frequency, expert co-occurrence,
activation-weighted reconstruction error, downstream logit KLD, and physical
bytes. It may not use routing frequency alone: a rare expert can still be
quality-critical.

The scheduler groups active experts by backend so a warp or CTA does not
branch between EXL3 and NVFP4 reconstruction paths.

### Quantization program

Re-quantization is explicitly in scope. Evaluate:

1. the pinned current EXL3 3.0-bpw checkpoint;
2. a reproduced EXL3 policy from the pinned BF16 source;
3. published/pinned NVFP4 controls;
4. quality-first NVFP4 with protected tensors;
5. routing- and Hessian-aware EXL3 allocations;
6. the measured hybrid policy.

Change only one of format, calibration data, protection policy, or kernel in
the first comparison for each experiment. Every full conversion is
resumable at tensor granularity and records source/output hashes.

## KV and one-million-token design

### First cache ABI

Use the proven 368-byte record:

- dynamically scaled NVFP4 MLA latent payload;
- FP8 RoPE payload;
- per-token outer scaling;
- fixed padding/alignment suitable for byte-preserving peer transport.

The CPU oracle must prove encode/decode, RoPE, absorbed-attention, sparse
selection, padding, and page-boundary behavior. The first GPU writer is
compared at every record byte and at the downstream attention output.

### Capacity reservation

For the exact 1,048,576-token model limit:

```text
target: 368 bytes × 78 layers × 1,048,576 = 28.03125 GiB
draft:  368 bytes ×  1 layer  × 1,048,576 =  0.359375 GiB
index:  132 bytes × 21 groups × 1,048,576 =  2.70703125 GiB
draft index:
         132 bytes ×  1 group  × 1,048,576 =  0.12890625 GiB
MTP-capable total:                              31.22656250 GiB
per rank at DCP4:                                7.806640625 GiB
```

At boot, reserve in this order:

1. CUDA modules, immutable control data, and a measured driver allowance;
2. maximum decode and prefill scratch slabs;
3. CUDA graph argument and workspace slabs;
4. at least 262,144 local target, indexer-key, and optional draft token slots
   per rank;
5. a measured emergency/diagnostic escrow;
6. weights using the remaining hard budget.

In practice the converter must target a weight budget derived from the
smallest same-phase `cuMemGetInfo` result across all four ranks. Nominal bpw
is informative but never an admission proof.

The committed floor does not include page fragmentation or speculative
writes. At C64, all request tails may have the same next DCP owner. Each rank
therefore reserves one full page per active sequence (4,096 slots) for
worst-case ownership alignment and partial-tail slack and, for an
MTP6 profile, 448 target plus 448 draft tentative slots. The resulting target
and draft arenas each contain 266,688 token slots, or 4,167 complete
64-token pages per rank. Every requested arena is rounded up to a complete
page before its byte terms enter the fit inequality.

Allocate large deterministic arenas and suballocate them in Rust. Do not
depend on a framework caching allocator. After model initialization, run a
fail-closed maximum-scratch probe before reporting the 1M profile healthy.

### Page and ownership model

Start with 64-token logical pages, matching the proven control. Stripe page
ownership over four DCP ranks while preserving a rank-invariant logical token
ID. Store page-table metadata separately from packed records and keep it
small enough to capture in fixed graph shapes.

The full active 1M request stays in HBM. DRAM and NVMe are mandatory serving
tiers for paused sessions and reusable prefixes, but neither is counted as
active GPU context capacity.

Use the exact same 368-byte record at every tier. No lossy transcode occurs
during eviction or restore. Key tier content by:

- model, tokenizer, chat-template, and KV ABI hashes;
- KV scale policy and RoPE representation;
- target/draft record role and page size.

Exclude DCP ownership and byte-preserving kernel revisions from that content
key. Validate them through a separate HBM attachment ABI during restore, and
send the ownership-neutral record to the current DCP4 owner. A change that
alters record interpretation receives a new KV ABI and therefore a new
content namespace.

### Tiered KV state machine

Use these page states:

```text
FREE
  -> HBM_MUTABLE
  -> HBM_TENTATIVE
  -> HBM_SEALED
  -> DRAM_WRITING
  -> DRAM_RESIDENT
  -> NVME_WRITING
  -> NVME_RESIDENT
  -> RESTORING
  -> HBM_SEALED
  -> INVALID
```

Only sealed full pages may be shared or evicted. A request's tail page is
copy-on-write and remains mutable until committed. Cancellation, MTP
rejection, or error rolls back uncommitted tail slots without corrupting a
shared prefix.

Treat one logical 64-token block across all 78 target layers as the primary
tiering unit.
At 368 bytes per token per layer it carries 1,837,056 bytes before metadata,
large enough for efficient batched PCIe and NVMe transfers. HBM remains
layer-major for attention kernels; GPU pack/unpack kernels and fixed pinned
staging rings convert between layer fragments and the contiguous tiering
record without changing payload bytes.

Every prefix carries a mandatory 177,408-byte target-indexer sidecar, and an
MTP-capable prefix carries a separate 32,000-byte composite sidecar containing
23,552 draft-KV bytes and 8,448 draft-indexer bytes for the same page. Target
plus target-indexer publish atomically; a valid pair without the composite
draft sidecar is attachable only as MTP0. Tentative target and draft tail
writes commit or roll back together.

The DRAM tier is bounded and process-volatile, with a small registered/pinned
transfer window; do not pin the entire cache. The NVMe tier uses aligned,
large, checksummed records, a bounded log/index, a configured rolling
bytes-per-day write cap, and batched asynchronous I/O.
The first implementation may use ordinary direct I/O through a Rust I/O
worker; GPUDirect Storage is an optional measured optimization, not a
requirement.

Each record carries a content key, sequence/block identity, logical token
range, valid-token count, advisory writer rank, payload checksum, and
generation. Publish a journal only after all required target/indexer/draft
pieces are durable. Crash recovery ignores incomplete generations and orphan
sidecars.

Eviction prefers unreferenced, cold, cheaply recomputable pages and protects
high-fanout shared prefixes. Restore is asynchronous: a request enters the
GPU-ready queue only after all pages required by its next step are resident.
Never block a device worker on filesystem I/O.

Measure tiering separately:

- HBM-to-DRAM and DRAM-to-HBM bytes, latency, and overlap;
- DRAM-to-NVMe and NVMe-to-DRAM bytes, IOPS, bandwidth, and write
  amplification;
- restore-to-first-token latency;
- hit rate and reuse distance;
- HBM residency and eviction churn;
- NVMe capacity, endurance assumptions, and error recovery.

### Prefix cache

Build a page-aligned radix/prefix index over token IDs. The content key
chains:

```text
content namespace hash + parent page hash + token IDs + valid-token count
```

Requests share sealed HBM, DRAM, or NVMe pages by reference count. Partial
page matches are recomputed into a private tail; full matched pages are
restored or attached without model execution. Use a frequency/recency score
weighted by saved prefill work and page fanout for eviction.

Prefix-cache reporting distinguishes:

- cold computed prompt tokens;
- HBM-attached tokens;
- DRAM-restored tokens;
- NVMe-restored tokens;
- recomputed partial-tail tokens.

Cold prefill benchmarks require zero hits at every tier. Warm benchmarks
report restore time and cache-effective throughput, never relabel restored
tokens as model-compute throughput.

### Sparse attention and IndexShare

Implement exact global top-2,048 selection:

1. each rank evaluates its local context shard;
2. ranks exchange local candidates;
3. a deterministic global merge produces identical logical winners;
4. winners are remapped to physical local or gathered-record slots.

Reuse an indexer result only in the layer pattern declared by the pinned
model. The shared top-k buffer is transient and recycled across the
four-layer IndexShare group; do not persist 2,048 indices per token.

Cache each full-indexer layer's 128 E4M3 key bytes and one FP32 power-of-two
scale per token. The 21-group, 132-byte record store follows target KV
ownership and is a mandatory independently namespaced tier sidecar. It is
not allocator overhead and is restored atomically with its target page.

## Prefill plan

Keep 16 query heads local to each TP rank and use packed-CKV ownership
inversion for the initial DCP4 path:

```text
local query heads
  -> byte-preserving packed-CKV gather
  -> local sparse attention
  -> local output projection
```

This avoids head-expanded query transport and its output reduce-scatter.
Existing target evidence reduced measured per-layer/chunk DCP transport from
about 18.6 ms to about 1.4 ms with this mechanism, so it is a design input,
not a speculative optimization.

At deep context, gathering the full packed context becomes expensive. Build
both packed-CKV and query-transport plans, then select through a measured
global cost table keyed by:

- prompt chunk size;
- accumulated context band;
- peer topology class;
- active batch shape;
- available overlap.

The chosen route is part of the rank-invariant `StepPlan`. No rank measures
and switches independently during execution.

Use separate graph families for a small set of prefill chunk sizes. Start
from the measured 3,072-token control and sweep neighboring shapes. Double
buffer CKV transport, remapping, and attention only after individual phase
timings reproduce wall time.

## Decode plan

Decode is a separate execution regime, not prefill with `M=1`.

Build graph families for:

- target-only `M = 1, 2, 4, 8, 16, 32, 64`;
- MTP depths `0, 1, 2, 3, 4, 5, 6`;
- verifier row buckets covering `batch × (depth + 1)` through 448 rows;
- short, medium, long, and near-1M context bands;
- EXL3 and NVFP4 expert backends.

Primary hypotheses:

1. an EXL3 persistent small-M expert kernel reduces launch and reconstruction
   overhead enough to make its lower traffic win;
2. a fused NVFP4 route can recover the expected 4-bit speed advantage by
   hiding activation quantization and descriptor setup;
3. topology-specialized one-shot TP reductions beat a generic collective for
   6,144-element decode payloads;
4. exact DSA selection and IndexShare reuse dominate deep-context decode
   unless explicitly fused and profiled;
5. hot-expert NVFP4 plus cold-expert EXL3 can improve decode without giving
   back the 1M KV reservation.

At DCP4, production decode uses query transport rather than gathering
top-2,048 packed KV records per layer. All ranks exchange local query heads,
compute owner-local sparse-index candidates, deterministically merge the
global winners by `(score descending, logical position ascending)`, and
compute owner-local FP32 partial softmax states. The query-head owner combines
`(max, exponential sum, weighted value)` in fixed rank order `0,1,2,3`.
This query/candidate/partial-state schedule is part of the rank-invariant
`StepPlan`. Record gather remains a prefill candidate only.

### MTP draft and verification

MTP is initially off for reference qualification, but support for the pinned
draft layer and depths 0–6 is mandatory for the serving milestone.

The draft is a complete attention-plus-MoE layer and owns one 368-byte KV plus
one 132-byte indexer-key record per committed position. An MTP-capable prompt
computes and stores that 0.48828125-GiB-at-1M composite sidecar. A restored
target prefix without the paired draft sidecar remains MTP0-only.

For each speculative cycle:

1. recurrently apply the single draft layer up to the selected depth using
   the exact manifest recurrence and step-zero IndexShare winner list;
2. retain draft probabilities, sampled tokens, and RNG state;
3. execute one target verification plan with `depth + 1` rows per request;
4. accept the valid prefix under the pinned greedy or probabilistic rule;
5. commit accepted target, draft-KV, and draft-indexer slots and roll back all
   rejected speculative tails;
6. emit only accepted useful tokens.

The memory planner reserves the worst configured verifier rows and draft KV
tail slots. MTP and continuous batching therefore share one budget:

```text
verification rows = active requests × (depth + 1)
```

A deeper draft is not automatically better. First expose a server-level
depth `0..6`. Then test a rank-invariant adaptive controller that chooses one
depth per batch from recent acceptance by depth, queue pressure, context
band, and measured verifier efficiency. The choice is encoded in the global
`StepPlan`; ranks never adapt independently.

Measure MTP0–MTP6 at C1, C2, C4, C8, C16, C32, and C64 where capacity
permits. Preserve:

- acceptance probability and accepted length at every proposed position;
- useful tokens per target pass;
- draft, verification, sampling, and rollback time;
- rejected compute and KV writes;
- TTFT and inter-token latency;
- exact verifier self-consistency;
- MTP0 agreement at numerically stable positions;
- separately classified tie-adjacent greedy divergence and probabilistic
  distribution equivalence.

Compare MTP depths with identical target and draft precision membership.
Target-only quality passes before draft quality is optimized; MTP throughput
does not excuse a target-model regression.

Keep the LM head vocabulary-sharded. Greedy sampling uses a distributed
argmax with lowest-token tie-break. Probabilistic sampling operates on
rank-local FP32 logits: bounded top-k candidates are merged, while unfiltered
categorical and speculative residual sampling use deterministic rank-mass
selection and owner-local CDFs. Never gather the 154,880-way logits for all
448 verifier rows. Version zero requires `top_k <= 256` whenever
`top_p < 1`.

## PCIe topology strategy

“Fast on all PCIe layouts” means topology-adaptive scheduling over the four
target GPUs, not a hard-coded ring.

At an operator-authorized initialization/qualification run:

- inventory peer-access support and PCIe ancestry;
- measure directed latency and bandwidth for each GPU pair and payload band;
- enumerate four-rank rings, trees, and two-level pair schedules;
- measure NCCL and custom byte-preserving paths;
- select one route per collective kind and size band;
- serialize the topology fingerprint and selected route table.

At normal boot, topology discovery is read-only. The engine loads a matching
qualified route table or uses the all-rank NCCL control. It never silently
uses performance data from a different board layout.

Important acceptance classes are the current Gen3 dual-switch layout, the
planned Gen4 layout, and any direct-root or asymmetric layout made available
for testing. Results are reported per layout; they are not extrapolated from
link generation.

## Kernel inventory

The engine eventually needs SM120-only implementations of:

- embedding and LM-head paths;
- RMSNorm and residual fusions;
- dense and low-rank attention projections;
- RoPE and dynamic compressed-KV writer;
- DSA indexer, exact distributed top-k, and IndexShare buffer;
- sparse MLA decode and extend/prefill;
- packed-CKV transport, remap, and staging;
- EXL3 routed-expert decode and prefill;
- NVFP4 routed-expert decode and prefill;
- shared and dense MLP paths;
- TP reductions and DCP collectives;
- logits processing and GPU sampling;
- MTP draft and verification.

Do not implement all kernels before the first proof. The first performance
kernel is the routed-expert operation selected by the accepted exclusive
phase ledger, with both EXL3 and NVFP4 controls at the same real shape.

## Rust workspace shape

Keep the initial workspace small:

```text
crates/
  glm-format/       versioned manifest, packers, hashes, CPU dequant
  glm-reference/    fixed GLM graph and CPU operation oracles
  glm-cuda/         minimal Driver/NCCL bindings, RAII, kernel ABI
  glm-engine/       memory plan, TP4 executor, graphs, step plans
  glm-cache/        HBM/DRAM/NVMe KV, prefix index, async tiering
  glm-scheduler/    continuous batching, admission, fairness, MTP policy
  glm-cli/          pack, inspect, bench, replay, run
kernels/
  include/          C ABI shared with glm-cuda
  sm120/            CUDA/CUTLASS kernels and kernel tests
spec/
  format-v0.md
fixtures/
  manifests/        small pinned metadata only
  references/       small provenance-recorded numerical fixtures
```

Split further only when compile boundaries or ownership require it. Avoid a
generic tensor library; fixed-rank descriptors and fixed GLM operation
enums are preferable here.

## Correctness and quality gates

### Format

- deterministic pack bytes;
- exact EXL3 and NVFP4 CPU reconstruction;
- scale and LUT indexing;
- swizzle and padding;
- corrupt header/payload rejection;
- manifest/model mismatch rejection;
- physical-byte accounting.

### Kernel

- real GLM dimensions;
- `M = 1, 2, 4, 8, 16, 32, 64, 128` plus prefill groups;
- empty experts, hot experts, tails, and skewed routing;
- adversarial values and realistic captured activations;
- eager and graph-captured repetition;
- max absolute/relative error and distributional error;
- no unexplained nondeterminism or illegal fallback.

### One layer

Replay captured BF16 inputs and exact routes through one complete layer.
Compare:

- indexer winners;
- attention output;
- router scores and expert IDs;
- routed and shared expert outputs;
- post-reduction residual;
- next-layer input and downstream logits;
- exclusive time and temporary bytes.

### Whole model

The initial KLD gate uses pinned BF16 reference logits, all 2,047 positions,
the full vocabulary, the exact MPFR-256 construction and final binary64
rounding in `docs/quality-acceptance-v1.md`, MTP0, and retained per-position
values. Historical means from another evaluator definition are not inputs;
every control is rerun under the accepted contract.

Qualification then includes reasoning, coding, tool use, JSON exactness,
long-generation repetition, frozen and randomized long-context retrieval,
and parser/termination behavior. Highest quality means the selected serving
profile must beat or match the best capacity-feasible compressed control
across central and tail quality metrics; a mean-only KLD win is insufficient.

### End to end

For each accessible PCIe layout, report:

- MTP0 before MTP-enabled rows;
- MTP1 through MTP6 with per-depth acceptance;
- C1, C2, C4, C8, C16, C32, and C64 decode where capacity permits;
- starting contexts 0, 16k, 64k, 128k, 480k, and near 1M;
- cold prefill at 8k, 64k, 128k, 480k, and near 1M;
- multi-user mixes of short decode, long decode, and chunked prefill;
- HBM, DRAM, and NVMe prefix reuse separately;
- physical and admitted KV capacity;
- maximum-scratch escrow result;
- kernel, framework, TP, DCP, and end-to-end time;
- TTFT, inter-token latency, useful-token throughput, and fairness;
- tier hit rate, restore latency, eviction traffic, and NVMe write
  amplification;
- power, clocks, temperature, errors, and restarts.

Compare `capacity-exl3` and `hybrid-serve` with identical context, cache,
batching, and MTP state. Retain matched NVFP4 laboratory/operator controls;
never present them as a standalone full-model profile.

## Milestones and exit gates

### M0 — design and adversarial review

Deliver this plan, a memory budget, a fixed operation graph, a risk register,
and a review that tries to falsify the EXL3, NVFP4, TP4, and packed-CKV
hypotheses.

Exit: no unresolved contradiction in model geometry, capacity accounting, or
collective order.

### M1 — CPU reference and format ABI

Implement the Rust model constants, tensor manifest, NVFP4 CPU codec,
deterministic rank packer, and packed-KV oracle. Then extract and implement
the pinned EXL3 CPU codec as a reviewed extension of the same container ABI.

Exit: byte-stable NVFP4 format plus exhaustive pack/dequant equivalence.
EXL3 equivalence is a separate required exit before the capacity profile.

### M2 — standalone SM120 laboratory

With explicit operator authorization, benchmark the NVFP4 expert path at real
shapes first. Include inclusive operator time and hardware counters. After
the EXL3 CPU oracle passes, run the same laboratory gate for the existing and
new EXL3 paths before enabling the capacity profile.

Exit for each backend: at least one material kernel win with explained bytes
and operations, or a documented redesign decision. NVFP4 may proceed to the
minimal runner without waiting for the EXL3 backend exit.

### M3 — one-layer TP4 replay

Run attention, routing, experts, collectives, and residual for a captured
layer on all four GPUs.

Exit: layer output and downstream-logit agreement plus a matched speed result.

### M4 — minimal Rust target runner

Load an NVFP4 small-checkpoint slice and execute static prefill and greedy
MTP0 decode using preallocated memory, production descriptors, graph-profile,
collective, and error paths.

Exit: reference token/logit agreement and clean repeated runs for the NVFP4
slice. No full-model serving or capacity claim is permitted at M4.

### M5 — quality and 1M qualification

After the EXL3 CPU/kernel gates pass, freeze a per-rank physical budget and
load a fit-capable `capacity-exl3` or `hybrid-serve` full checkpoint. Run the
checkpoint smoke, full KLD/task gates, exact deep retrieval, capacity escrow,
near-1M prefill, and deep-context decode.

Exit: quality PASS and an admitted 1,048,576-token total sequence without
offloading active KV.

### M6 — concurrent scheduler, prefix cache, tiering, and MTP

Add continuous batching, chunked prefill, MTP0–MTP6 graphs, prefix sharing,
transactional KV tails, bounded DRAM and NVMe tiers, cancellation, and
backpressure.

Exit: concurrent correctness, distribution-preserving speculation, clean
tier transitions and crash recovery, and no collective-order divergence
under mixed traffic.

### M7 — matched multi-user performance qualification

Compare the Rust runner with the best pinned existing runtime for EXL3 and
NVFP4 across topology, concurrency, MTP depth, prefix-residency, and workload
mixes.

Exit: a material end-to-end win in at least one priority regime, no material
regression in the others without an explicit profile choice, and a complete
phase ledger. The production gate also requires bounded memory, queue
stability, cache recovery, cancellation, and long-duration reliability.

## Immediate next slice

The next work should remain CPU-only:

1. have Fable adversarially review the design and both v0 specifications;
2. incorporate Fable v2's cached-indexer-key and DCP-ledger conditions and
   freeze the pinned model operation manifest;
3. revise the specifications and record the accepted review disposition;
4. scaffold the Rust workspace through `glm-format` and `glm-reference`;
5. implement physical-byte and 1M-admission accounting;
6. freeze the batched sequence descriptor and rank-invariant `StepPlan`;
7. freeze prefix keys and the HBM/DRAM/NVMe page state machine;
8. freeze the MTP0–MTP6 transactional tail and verification contract;
9. implement one synthetic and one real-expert NVFP4 CPU pack/dequant
   fixture;
10. define the SM120 kernel C ABI without implementing a GPU kernel;
11. prepare, but do not run, the authorized microbenchmark matrix.

The first code milestone is not an HTTP server and not a full 753B
conversion. It is a deterministic Rust NVFP4 packer/oracle that can prove
exactly what bytes the first kernel will consume. The EXL3 oracle is a
separate extension before the capacity profile.

## Primary sources and local evidence

- Pinned GLM-5.2 configuration:
  <https://huggingface.co/zai-org/GLM-5.2/blob/b4734de4facf877f85769a911abafc5283eab3d9/config.json>
- GLM-5.2 model card and 1M context claim:
  <https://huggingface.co/zai-org/GLM-5.2>
- NVIDIA Blackwell tuning guide:
  <https://docs.nvidia.com/cuda/blackwell-tuning-guide/>
- NVIDIA CUTLASS Blackwell functionality:
  <https://docs.nvidia.com/cutlass/latest/media/docs/cpp/blackwell_functionality.html>
- NVIDIA block-scaled operand and scale-layout tutorial:
  <https://docs.nvidia.com/cutlass/latest/media/docs/operators/tutorials/006_block_scaled_gemm.html>
- Rust `cudarc` bindings are a candidate, not yet a pin:
  <https://docs.rs/cudarc/>
- Local production evidence is consulted read-only from
  `../glm52-opt/RESULTS.md`,
  `../glm52-opt/MEASUREMENT-LIBRARY.md`, and
  `../glm52-opt/design/cn4-tr3-qualification-20260728.md`.

Before any result, pin exact CUDA, driver, CUTLASS, Rust, binding, model,
converter, calibration, and kernel revisions in its manifest.
