# TP4 layer-6 replay and layer-7 indexer-reuse gate v1

Date: 2026-07-30

Status: design candidate; adversarial review required before CPU fixture or
CUDA implementation

GPU evidence: none

## Purpose and gate position

This contract defines M3, the first complete target-layer execution gate on
four SM120 ranks. It turns the scattered M3 requirements in
`docs/native-engine-plan.md` and `docs/target-layer-execution-v1.md` into one
reviewable fixture, execution, comparison, failure, and evidence boundary.

M3 is not an operator microbenchmark and not a checkpoint smoke. It comes
only after:

1. current-tree format, target-program, executor, graph, and collective
   contracts are accepted and CPU-proven;
2. the NVFP4 fused routed-MoE and every protected layer operator pass their
   actual-shape M2 gates;
3. DCP4 query/candidate/partial-LSE and both prefill routes are qualified on
   the exact PCIe topology; and
4. the immutable replay weight membership and all comparison thresholds are
   frozen before a device run.

M3 executes one complete layer-6 replay for decode and prefill. Layer 6 is the
first layer that is both sparse MoE and the `FULL` producer for a four-layer
indexer group (`6,7,8,9`). A mandatory layer-7 continuation proves that the
next `SHARED` layer consumes layer 6's generated winner list without
recomputing indexer keys or exchanging candidates.

Passing M3 is a prerequisite for the M4 small-checkpoint runner. M3 cannot
provide M2 evidence, open the M4 laboratory manifest, or substitute for
checkpoint loading.

## Immutable identity set

The M3 identity block contains exact SHA-256 or fixed scalar identity for:

```text
glmaxx source commit
model repository and revision
complete source checkpoint inventory
selected source tensor catalog
native routed-weight conversion and plane catalog
protected tensor catalog
model configuration
tokenizer and chat template
prompt/token sequence
reference implementation source and environment
format and engine specifications
operation manifest
target program v2
weight/layout/conversion policy
kernel and native ABI
codec capability table
graph profile v2
collective schedule v2
PCIe topology and route table
M2 operator and collective results
numerical comparison policy
fixture schema and fixture payload
```

There is one process-common identity block. A rank-local file, environment,
kernel, layout, route, tolerance, or fixture choice is forbidden.

The replay binds source and native device weights separately. For every
routed tensor it records the source slice, conversion route, decoded codec
metadata, native primary/auxiliary/metadata plane hashes, codec ID, projection
ID, and value/scale layout IDs. FC1 is exactly 1D `0x1202/0x1202`; FC2 is the
exact M2-accepted 1D or 2D `0x1201/0x1201` variant. Protected tensors retain
their source dtype/shape/TP semantics and byte hash.

M3 does not infer weight semantics from a filename, tensor shape, marketing
label, or requested kernel. The device and packed CPU oracle consume the same
native plane bytes.

## Replay weight boundary

M3 uses a dedicated non-serving `ReplayWeightSet`. It contains every protected
and routed/shared tensor required to execute layers 6 and 7, projected from
the compiled operation manifest and target program. It contains no embedding,
layers outside 6–7, final norm, or LM-head weight.

The weight set is not a `.g5n` checkpoint and does not use the M4 or
production rank-manifest schema. It is loaded through the exact bounded M2
fixture-loader path whose device plane hashes were accepted with the operator
results. This deliberate boundary lets M3 prove layer composition before
claiming checkpoint ingestion.

The CPU proof must derive the closed tensor set and prove:

- no missing, extra, duplicate, unused, or caller-named tensor;
- exact layer, role, expert, projection, codec, shape, dtype, TP axis, layout,
  source, conversion, and native-plane identity;
- identical rank-common target-program records;
- rank-local physical spans match their authenticated TP shards; and
- no conversion, repack, protected-precision demotion, or fallback occurs
  during load or execution.

Adopting a `ReplayWeightSet` yields only a `ReplayWeightHandle`. It has no
conversion to a laboratory or production weight handle and cannot enter
startup `HEALTHY`, HTTP serving, prefix publication, or a checkpoint result.

## Replay program identity

M3 does not truncate a production `StepPlan` at runtime. Each fixture/path
uses one fixed 320-byte `ReplayProgram.v1` record. All integers are
little-endian and reserved bits are zero:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `G5M3RP1\0` |
| 8 | 2 | version `1` |
| 10 | 2 | record bytes `320` |
| 12 | 1 | mode: `1=DECODE`, `2=PREFILL` |
| 13 | 1 | primary layer, exactly `6` |
| 14 | 1 | continuation layer, exactly `7` |
| 15 | 1 | rank count, exactly `4` |
| 16 | 4 | real query rows |
| 20 | 4 | graph row bucket |
| 24 | 4 | sequence bucket |
| 28 | 1 | attention transport |
| 29 | 1 | execution path: `1=EAGER`, `2=CAPTURED` |
| 30 | 2 | flags; zero in v1 |
| 32 | 32 | operation-manifest SHA-256 |
| 64 | 32 | replay-weight-catalog SHA-256 |
| 96 | 32 | layer-6/7 target-record-stream SHA-256 |
| 128 | 32 | graph-profile-v2 SHA-256 |
| 160 | 32 | collective-schedule-v2 SHA-256 |
| 192 | 32 | fixture SHA-256 |
| 224 | 32 | numerical-policy SHA-256 |
| 256 | 32 | route-table/topology SHA-256 |
| 288 | 32 | codec-capability SHA-256 |

The layer target-record stream uses the accepted target-program-v2 16-byte
layout-bound record encoding, restricted to the exact layer-6/7 tensor set
and ordered by `(layer_id,role_id,expert_id,projection_id,tensor_id)`. Its
digest is domain-separated from a complete target program:

```text
SHA256(
  "glmaxx.m3-layer-records.v1\0"
  || record_count:u32_le
  || ordered 16-byte records
)
```

The replay program digest is:

```text
SHA256("glmaxx.m3-replay-program.v1\0" || 320-byte record)
```

Mode, real rows, bucket, sequence capacity, transport, and eager/captured
posture must agree with the fixture, graph entry, and every collective
record. A target record, graph, schedule, route, fixture, numerical policy,
or codec capability from another replay cannot be substituted.

`ReplayProgram` is accepted only by the M3 executor together with the exact
`ReplayWeightHandle`. It has no conversion to production `StepPlan`,
laboratory M4 execution, a full target program, or scheduler input.

## Two immutable real fixtures

M3 uses two fixture cases from the same pinned reference checkpoint and
prompt family:

### Decode case

```text
mode                 DECODE
real query rows      1
graph row bucket     exact M2-qualified bucket
MTP depth            0
attention route      DECODE_QUERY_LSE
```

The one sequence has causally visible history on all four DCP owners and a
current position whose target-KV and full-indexer-key destinations are
tentative and generation-bound.

### Prefill case

```text
mode                 PREFILL
real query rows      one M2-qualified count in 1..3072
graph row bucket     smallest accepted v2 bucket covering the real rows
MTP depth            0
attention route      PREFILL_CKV or PREFILL_QUERY
```

The chosen route, real rows, sequence bucket, context band, and graph bucket
are frozen from the accepted M2/collective result. At least one sequence has
causally visible history on every DCP owner. If both prefill transports are
qualified for that exact case, M3 runs both against byte-identical snapshots;
otherwise it runs the one globally accepted route and records the other as
unavailable rather than silently falling back.

The fixture extractor records, outside Git:

```text
complete token IDs and prompt digest
layer-6 input hidden rows
absolute positions and sequence lengths
active-sequence and valid-row masks
rank/full-device page-table bytes
committed target-KV and indexer records
tentative destination IDs and generations
DCP owner mapping
expected collective records and route payloads
reference intermediate and output tensors
reference cache successor
```

Inputs are captured from a real pinned reference execution. A synthetic,
random, zero, shape-only, or device-produced input cannot qualify.

The extractor and replay reader use strict bounded schemas, canonical
little-endian scalars, explicit dtypes/shapes/strides, per-section hashes, and
one whole-fixture hash. Paths, timestamps, credentials, and host-dependent
metadata are excluded from the semantic digest. Model data and raw evidence
remain outside Git.

## Independent reference ladder

M3 retains two independent reference branches:

1. `native_cpu`: the Rust/CPU target-layer oracle consumes the exact native
   protected and NVFP4 plane bytes used by the device;
2. `source_control`: the pinned reference runtime consumes the authenticated
   source checkpoint at its control precision.

The device is compared directly with `native_cpu` for implementation
correctness. `native_cpu` is compared with `source_control` to retain the
quantization-quality delta. A device result may not compare only with the
source control, because that would conflate kernel error with quantization
error.

Router IDs/weights, compacted assignments, indexer candidates/winners, DCP
partial states, and cache successors are outputs of both branches. Expected
values are never fed into the device computation. The only device inputs are
the immutable hidden/cache/page-table records, weight bindings, graph
arguments, and process-common schedule.

All comparison policy fields are fixed before the first device run:

```text
phase membership and dtype
rounding/accumulation contract
absolute, relative, and ULP thresholds
NaN/infinity policy
stable versus tie-adjacent classification
router and winner exact/tie rules
full-vocabulary logit/KLD policy
```

No threshold may be widened after observing a device result. Per-position
values and classifications are retained, not only maximums or means.

## Exact primary layer-6 program

For both fixtures, all four ranks execute the exact layer-6 program from
`docs/target-layer-execution-v1.md` under target-program v2:

1. input RMSNorm;
2. Q LoRA projection/norm and local Q heads;
3. KV latent/RoPE production and tentative 368-byte target-KV writes;
4. full indexer projections, LayerNorm/RoPE, and tentative 132-byte key
   writes;
5. route-appropriate top-2,048 selection and exact global winner
   construction, using owner-local candidates and fixed four-way merge only
   on query transport;
6. the globally selected CKV or query/partial-LSE attention transport;
7. V expansion, output projection, TP attention reduction, and residual;
8. post-attention RMSNorm;
9. replicated FP32 router and exact stable top-8 compaction;
10. routed FC1, SwiGLU, FC2, route-weighted deterministic scatter;
11. shared-expert gate/up/SwiGLU/down;
12. routed-plus-shared combine, one TP MLP reduction, and residual; and
13. generation-bound target-KV/indexer/page-table successor publication into
    the replay-private result state.

The layer program computes its own routes, candidates, winners, and compacted
tables. An execution that supplies reference routes to the expert kernel is
an operator replay, not M3.

All four ranks consume one byte-identical target-program digest, graph key,
step/fixture identity, and `CollectiveOp.v2` schedule. Rank-local route,
participant, codec, layout, graph, empty-owner, or fallback decisions are
forbidden.

## Mandatory layer-7 reuse continuation

After layer 6 passes, the same immutable step generation executes layer 7
from layer 6's device output and cache successor.

Layer 7 is a `SHARED` consumer of index group 3. It must:

- consume the layer-6-generated `WinnerList.v1` for the same request, row,
  position, group, and generation;
- execute no full-indexer projection;
- write no 132-byte indexer key;
- execute no candidate exchange;
- use its own layer-7 368-byte target-KV records and the same logical winner
  positions;
- execute its complete attention, routed/shared MLP, TP reductions, and both
  residuals; and
- match independent native-CPU layer-7 output and cache-successor references.

A fixture-provided expected winner list is retained for comparison but cannot
replace the graph-resident list produced by layer 6. A group/generation/row
mismatch, an extra indexer kernel or collective ordinal, or stale winner
reuse fails M3.

This continuation is an additional indexer-lifetime proof. The primary M3
claim remains one complete layer-6 replay; M3 does not claim general
correctness for layers 0–5 or 8–77.

## Cache transaction boundary

Each fixture starts from one immutable committed cache snapshot. Layer-6 and
layer-7 writes are tentative and visible only to the same replay generation.
They are unreachable from serving, prefix lookup, tier eviction, another
request, or another repetition.

The accepted replay successor contains:

```text
exact target-KV records for every new layer/row
exact layer-6 full-indexer records
no layer-7 indexer records
rank-local owner/destination generations
post-layer-6 and post-layer-7 page-table/cache digests
```

The device records are decoded and compared with `native_cpu` under the
accepted KV/indexer numerical ABI. Physical record bytes, decoded values,
finite checks, owner mapping, padding, and successor digests all pass.

After comparison, the replay-private successor is discarded. M3 never
publishes it into a prefix namespace or lower tier.

## Downstream full-vocabulary sensitivity evidence

M3 produces two offline continuation comparisons:

1. inject `native_cpu` layer-6 output into the pinned source-control runtime
   at the layer-7 boundary and continue layers 7–77 plus final norm/head;
2. inject the device layer-6 output at the same boundary and run the
   byte-identical continuation.

The layer-7 reuse branch separately injects native-CPU and device layer-7
outputs at the layer-8 boundary.

For every real row, the evidence retains all logical vocabulary logits,
per-token absolute/relative error, stable/tie classification, top candidates,
and the reviewed KLD calculation. Vocabulary padding is excluded by the
logical interval contract and explicitly checked.

This is an offline evidence gather after device synchronization. It may copy
the four rank-logit reference shards into the evidence process. It is not the
production sampling path and cannot be used to justify a runtime
full-vocabulary gather. M3 itself does not execute an on-device LM head or
distributed sampler.

The continuation is a sensitivity measurement, not a full-model device
execution or quality pass. Only layers 6–7 came from the replay device path.

## Eager, captured, cold, and warm matrix

For each decode/prefill fixture and each qualified prefill transport, M3 runs:

- one fresh-process cold eager execution;
- one fresh-process cold graph capture/instantiate/execute path;
- 100 warm eager repetitions;
- 100 warm captured repetitions; and
- five fresh create/load/run/destroy child-process cycles.

Eager and captured paths use identical weights, fixture snapshots, routes,
row counts, capacities, and numerical policy. Each repetition has new
argument, cache, output, graph-launch, collective, and completion generations.
Required output slots are generation-poisoned; padding/unused slots use their
canonical empty value. Every current-generation output must overwrite its
poison, and stale receipts are rejected.

Cold/warm labels are exact:

- `cold_process`: no replay context or weight generation exists;
- `cold_graph`: first graph instantiation and launch;
- `cold_fixture`: first input/cache upload;
- `warm`: contexts, modules, weights, collectives, graphs, and immutable
  fixture residency retained, but all logical input/output/cache generations
  are new.

Durations and native addresses are not deterministic values. Plans, schedules,
routes, inputs, outputs, comparison results, and receipts are.

## Correctness and failure matrix

The CPU/mock proof and later SM120 gate cover:

- empty and nonempty DCP owners under one process-common participant mask;
- full 2,048-winner and all-positions winner boundaries;
- score ties, position ties, router ties, empty experts, one hot expert, and
  maximally skewed legal assignments;
- zero routed rows with the shared path and unchanged TP ordinal;
- malformed/nonfinite query, candidate, winner, partial-LSE, router,
  compaction, KV, and indexer records;
- wrong owner, generation, causal position, group, graph slot, codec, layout,
  weight span, route, participant mask, payload bytes, dependency ordinal,
  and collective schedule;
- one missing or duplicate target-KV/indexer write;
- a layer-7 indexer recomputation or candidate exchange;
- stale winner/cache/output/completion generation;
- eager/captured disagreement;
- one rank failing before each collective ordinal;
- one rank failing after collective entry or an asynchronous CUDA error;
- absolute deadline expiry; and
- cleanup/abort receipt disagreement.

Failures detected before collective entry and with provably synchronized
owner resources require zero accepted output and four exact cleanup receipts.
Collective uncertainty, asynchronous CUDA failure, owner-thread loss,
ambiguous DMA, failed synchronization/abort, or lost exclusive ownership is
process-fatal in an isolated child. The fatal path emits no accepted output,
frees no possibly referenced resource, forges no cleanup receipt, and does
not continue or retry one rank.

## Timing and matched controls

M3 records disjoint or explicitly nested timing boundaries for:

```text
fixture/storage read and validation
host-to-device transfer and drain
graph capture and instantiation
RMSNorm and protected projections
KV/indexer encode
candidate selection/merge
query, CKV, and partial-LSE transport
attention local kernels
TP attention reduction
router and compaction
routed FC1/SwiGLU/FC2
shared MLP
TP MLP reduction
launch/runtime overhead
device layer end-to-end
reference continuation
whole replay transaction
```

CUDA events measure device intervals; one pinned monotonic host clock measures
host/framework/end-to-end intervals. Collective records report logical and
route-manifest wire bytes separately. Opaque libraries are not claimed to
expose measured physical PCIe bytes.

Required matched controls are:

1. eager versus captured execution of the same M3 program;
2. fused NVFP4 expert path versus its qualified component control with the
   same native weight membership and output contract; and
3. each qualified prefill transport on byte-identical input/cache snapshots.

Protected/source-precision timing is recorded as a quality/control point but
cannot support an unqualified speedup claim because precision membership
differs. A general-purpose runtime comparison belongs to the later matched
end-to-end gate.

Before the run, the M2 result freezes an allowed inclusive layer-regression
envelope relative to the sum of matched qualified phases. An unexplained
regression outside that envelope yields `CORRECT_BUT_REDESIGN_REQUIRED` and
does not open M4. The envelope cannot be invented or widened from M3
measurements.

## Evidence record

The immutable M3 result retains:

```text
all source/build/toolchain/container/device/topology identities
every weight/source/conversion/native-plane identity
fixture and reference-ladder identities
target-program, graph, route, and collective-schedule identities
rank-local device/resource generations
every layer-6 and layer-7 collective record and receipt
router IDs, scores, original weights, and compaction records per row
candidate and winner records per row/owner
target-KV/indexer physical and decoded comparisons per position
phase tensor hashes and per-position error metrics
layer-6 and layer-7 output comparisons
full-vocabulary continuation values and KLD/classification per row
eager/captured and cold/warm generation receipts
fault injection, abort, cleanup, fatal, and parent-supervisor receipts
all timing boundaries and declared logical/wire bytes
matched-control identities and performance disposition
```

Raw tensors, model data, cache pages, and benchmark traces stay outside Git.
Git stores only the canonical result record, hashes, commands, and bounded
summaries required by the results index.

## Required CPU proof after review

Before any M3 CUDA run, one Rust CPU/mock candidate must:

1. derive the exact layer-6/layer-7 replay weight set and target programs;
2. encode and mutation-test every byte of `ReplayProgram.v1` and its separate
   layer-record/program hash domains;
3. parse and validate both strict external fixtures with bounded memory;
4. execute independent native-CPU layer 6 and layer 7 for both shapes;
5. reproduce every target-KV/indexer, route, candidate, winner, compaction,
   residual, and collective record;
6. run both downstream reference-continuation branches and retain
   per-position full-vocabulary evidence;
7. prove layer 7 consumes the generated layer-6 winner generation and emits
   no indexer work;
8. prove all four rank views have one schedule/route/program identity;
9. mutation-test every identity, shape, span, generation, owner, causal,
   record, graph, and collective boundary;
10. exhaust recoverable faults with exact cleanup;
11. execute process-fatal cases in bounded child processes with no forged
    cleanup or accepted output;
12. prove stale output/cache/receipt generations cannot pass warm controls;
13. prove all queues, records, buffers, fixtures, and deadlines are bounded;
    and
14. regenerate only small synthetic schema fixtures in Git.

CPU acceptance opens only the fixture/oracle/mock replay implementation.
Actual M3 remains behind M2, collective qualification, current operator
authorization, and a fresh environment/occupancy record.

## Exit criteria and nonclaims

M3 passes only if:

- all device phases, cache records, layer outputs, routes, and collectives
  match the accepted native-CPU contract;
- the source-control delta remains within the predeclared quality policy;
- eager and captured results agree;
- layer 7 proves generation-safe winner reuse with no indexer work;
- downstream full-vocabulary sensitivity evidence passes;
- the complete fault/lifecycle matrix passes; and
- the matched performance disposition is `ADVANCE`.

M3 does not prove:

- checkpoint-format loading or M4;
- embedding, layers 0–5 or 8–77 on the device;
- on-device final norm/head or production sampling;
- full-model device logits or task quality;
- EXL3 unless a separately identified EXL3 replay repeats this gate;
- production service, concurrency, MTP1–6, prefix reuse, DRAM/NVMe tiers,
  1M context, capacity, or an end-to-end speed advantage.

No cn4 access or GPU authorization is conveyed by this design.
