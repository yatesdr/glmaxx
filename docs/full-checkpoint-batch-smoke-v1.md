# Full-checkpoint four-row MTP0 batch smoke v1

Date: 2026-08-04

Status: design candidate; adversarial acceptance required before CPU/mock or
CUDA implementation

GPU evidence: none

## Purpose and gate position

This contract defines the first real checkpoint-to-text GLMAXX gate. It is
later than the M3 two-layer replay and M4 533-tensor laboratory runner. It
loads one complete production checkpoint, tokenizes four real prompts,
executes all target layers on four ranks, and produces exactly sixteen greedy
MTP0 tokens per row through the Rust-owned engine.

The gate is run and accepted separately for:

```text
1  CAPACITY_EXL3_TR3
2  HYBRID_W4A16_NF3
```

It is not a single-batch performance result, concurrent-serving result, MTP3
result, full KLD campaign, or capacity proof. It exists to prove that the
accepted loader, persistent rank owners, physical graphs, collectives,
tokenizer, target program, distributed sampler, cache transaction, and output
decoder compose over real full-model weights before tuning begins.

No implementation may skip directly from M2 kernels or M4 to this gate. Each
profile requires its matching accepted M3 device result, and both require the
accepted M4 device result as the bounded loader/executor predecessor.

## Profile-local prerequisites

Common prerequisites are exact accepted artifacts for:

- current format/header and source admission;
- four persistent rank owners and native ABI;
- target-layer r3, distributed sampling r2, target graph physical memory,
  GraphProfile v3, and the executor module/program-set identities;
- actual-shape protected operators and TP4/DCP4 collective routes;
- M4's load/adoption/graph/cache/execution/cleanup result;
- tokenizer/chat-template bytes and output decoding;
- the target-only MTP0 numerical policy and smoke thresholds; and
- one completed profile-specific rank-set resource budget and final memory
  plan.

Capacity TR3 additionally requires accepted complete 3.25-bpw source
reconciliation, strict production manifest/load plan, mixed K=3/K=4 source
and SM120 operator results, the 58,794-binding capacity target program, and
the capacity M3 result.

Hybrid additionally requires accepted source-set admission, expert-atomic
WeightPolicy v2, strict hybrid manifest/load plan, NF3/NVFP4 protected and
routed operator results, the 39,594-binding hybrid target program, and the
hybrid M3 result.

Every prerequisite is named by exact result SHA-256 and acceptance token in a
canonical predecessor-result set. A design token, compiled cubin, source
commit, checkpoint directory name, M3 result for another profile, M4 result
without its accepted token, or prior runtime output cannot substitute.

## Full production weight boundary

The capacity run adopts the complete authenticated four-rank TR3 generation,
including 59,585 rank records and the separately owned 791-record layer-78
MTP weight remainder. The target program uses only its 58,794 records under
MTP0; draft records remain immutable resident weight state and are not graph
uses.

The hybrid run adopts the complete strict hybrid generation selected by its
accepted WeightPolicy v2 and production load plan. Target and recurrent draft
weights are resident. The target program uses only its exact 39,594 target
bindings; no draft node executes.

Every file, source slice, descriptor, primary/auxiliary/metadata plane,
common relative arena span, rank-local device generation, and readback hash
is authenticated before the production weight handle exists. No weight
conversion, repack, dtype change, layout selection, or fallback occurs during
startup or execution. A native container is allowed only when its complete
derivation from the named read-only source checkpoint is already accepted.

The resulting `ProductionWeightHandle` is profile-specific and cannot convert
to the other profile or the M4 laboratory type. The run never obtains weights
from a Python runtime, vLLM process, filesystem name heuristic, or caller
pointer.

## BatchSmokeProgram v1

Every run uses one exact 672-byte `BatchSmokeProgram.v1`. All integers are
little-endian, all hashes are raw 32-byte values, and every reserved value is
zero:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `G5BSPV1\0` |
| 8 | 2 | version, exactly `1` |
| 10 | 2 | record bytes, exactly `672` |
| 12 | 1 | production profile ID, `1` or `2` |
| 13 | 1 | rank count, exactly `4` |
| 14 | 1 | MTP depth, exactly `0` |
| 15 | 1 | flags; bit 0 `EXACT_GREEDY`, no other bit |
| 16 | 2 | prompt rows, exactly `4` |
| 18 | 2 | generated tokens per row, exactly `16` |
| 20 | 4 | prefill row bucket |
| 24 | 4 | decode row bucket, exactly `4` |
| 28 | 2 | sequence bucket, exactly `4` |
| 30 | 2 | reserved zero |
| 32 | 32 | operation-manifest SHA-256 |
| 64 | 32 | complete source-checkpoint identity SHA-256 |
| 96 | 32 | production rank-set load-plan SHA-256 |
| 128 | 32 | adopted resident-weight generation SHA-256 |
| 160 | 32 | exact profile target-program SHA-256 |
| 192 | 32 | `GraphProfile.v3` SHA-256 |
| 224 | 32 | required graph-memory-plan-set SHA-256 |
| 256 | 32 | executor target-only program-set SHA-256 |
| 288 | 32 | adopted module-set capability SHA-256 |
| 320 | 32 | accepted rank-set resource-budget SHA-256 |
| 352 | 32 | final `SystemMemoryPlan.v3` SHA-256 |
| 384 | 32 | collective-schedule-set SHA-256 |
| 416 | 32 | route-table/topology SHA-256 |
| 448 | 32 | tokenizer/chat-template SHA-256 |
| 480 | 32 | four-prompt/token fixture SHA-256 |
| 512 | 32 | numerical-policy SHA-256 |
| 544 | 32 | distributed-sampling composite SHA-256 |
| 576 | 32 | cache/page-table ABI and layout SHA-256 |
| 608 | 32 | pinned reference/runtime-output SHA-256 |
| 640 | 32 | predecessor-result-set SHA-256 |

The program digest is:

```text
SHA256("glmaxx.full-batch-smoke-program.v1\0" || exact 672-byte record)
```

The record is a 32-byte header plus twenty 32-byte identities. Hash equality
without reconstruction and validation of every typed object is insufficient.
Unknown flags, a text digest, another profile, nonzero MTP depth, a row count
other than four, fewer/more than sixteen requested tokens, a decode row bucket
other than four, or a sequence bucket other than four fails before weight
adoption.

## Exact graph and schedule sets

The smoke `GraphProfile.v3` contains exactly the required target-only prefill
and C4 decode entries. Prompt chunking may launch the same padded prefill
entry more than once; it cannot select another graph or transport. The decode
entry has four real rows, MTP depth zero, and the accepted
`DECODE_QUERY_LSE` route.

The graph-plan-set digest is:

```text
SHA256(
  "glmaxx.full-batch-smoke-graph-plan-set.v1\0" ||
  u16_le(2) || two_zero_bytes ||
  u32_le(prefill_graph_id) || prefill_GraphMemoryPlan_sha256 ||
  u32_le(decode_graph_id) || decode_GraphMemoryPlan_sha256
)
```

Records are in ascending graph ID and the two graph IDs are distinct. Each
plan names the same target program, target-only executor program set,
module-set capability, resource budget, topology route family, and
GraphProfile-v2 parent. `GraphProfile.v3` binds each graph ID to the same plan
digest displayed in the set.

The schedule-set digest uses the identical shape and ordering with domain
`glmaxx.full-batch-smoke-collective-schedule-set.v1\0` and replaces each plan
hash with the exact `CollectiveSchedule.v2` hash. Each schedule is fully
reconstructed from the target program and global route table. A rank-local
participant, empty-owner, transport, algorithm, chunk, or ordinal decision is
forbidden.

The executor program-set digest has `mtp_program_present=0` and an all-zero
MTP digest. The module set contains target and device-validation families but
no required MTP family. Resident draft weights do not imply an MTP node.

## Physical memory posture

The final memory plan charges all ten graph-visible arenas once on every
rank:

```text
1  graph arguments
2  maximum prefill/decode graph scratch, including rank-logit scratch
3  target KV
4  target indexer
5  CURRENT/NEXT target pending logits and target-only recurrent state
6  fixed collective spans
7  completion/device-validation status
8  complete resident weight payload
9  complete resident codec metadata
10 device page table
```

Arenas 8 and 9 include resident draft weights, but target graph uses cover
only target bindings. With exactly four sequence slots, class 30 in arena 5
owns the C4 MTP0 CURRENT/NEXT pending-logit double buffers, exactly
`4 * 2 * 154,880 = 1,239,040` bytes per rank. Production prefill evaluates
the head only for the last processed row of each sequence and decode has four
real rows, so `L=4`; target class 26 in arena 2 owns exactly
`4 * 154,880 = 619,520` bytes per rank of rank-logit scratch. Rank-logit
scratch is never persistent arena-5 state. Proposal, q-state,
boundary-hidden, draft-KV/indexer, and MTP bundle subranges are zero. All
primary, auxiliary, metadata, page-table, cache, argument, scratch,
collective, and status spans appear in the accepted physical use tables and
resolve only through owner-created ten-arena bindings.

This first smoke uses a bounded `SMOKE_MINIMAL` KV allocation derived from
the four fixed prompts, their page-aligned prefill lengths, sixteen generated
positions per row, one tentative decode position per row, and explicit page
slack. It makes no 524,288-token capacity claim. It nevertheless uses the
production 64-bit page IDs, 1,048,576-position arithmetic, 64-token pages,
record layouts, and arena/page-table types. A later production capacity plan
may enlarge arenas 3, 4, 5, and 10 without changing any record, pointer,
descriptor, page, or graph-memory ABI; doing so creates a cold physical
generation and must pass its own allocation gate.

The resource budget precedes physical plans and cannot contain their or
GraphProfile-v3 hashes. The final memory plan follows GraphProfile v3 and
binds the same exact ten arena charges plus context/module,
collective-library, graph-runtime, allocator-padding, load-staging, and
emergency-escrow terms. Every rank must fit independently. Aggregate HBM or
a configured byte count cannot rescue a failing allocation/readback receipt.

## Four immutable prompts

The fixture contains four bounded, nonempty prompts selected before device
execution to exercise distinct GLM-5.2 behavior:

```text
short factual continuation
multi-step reasoning
code generation
structured JSON/tool-style output
```

For each row it binds raw UTF-8, exact chat messages, template application,
token IDs, attention/position inputs, stop configuration, and the reference's
first sixteen non-EOS greedy tokens. The pinned reference must produce at
least sixteen non-EOS tokens for every row; otherwise fixture construction
fails before GLMAXX runs. No prompt is chosen or changed after observing a
GLMAXX output.

The fixture also binds initial empty sequence/page state, deterministic
prefill chunk order, target/indexer page destinations and generations,
distributed-sampling inputs, expected cache successors, and per-position
reference logit/top-two/tie evidence. Model bytes and raw logits remain
outside Git; the canonical fixture schema, hashes, prompts when license-safe,
and bounded summaries are retained.

## Exact execution

After one four-rank weight adoption and graph-ready barrier, the runner:

1. tokenizes all four prompts with the pinned Rust tokenizer/template;
2. creates four sequence transactions and allocates their exact fixed pages;
3. executes deterministic chunked prefill through embedding and all target
   layers, with real row masks and no supplied router/indexer results;
4. runs final norm/head on each sequence's final processed prompt position,
   stores the resulting rank-local logits as committed CURRENT pending state,
   and emits no token;
5. executes sixteen C4 decode steps; step `j` first distributed-greedy samples
   and emits token `j` from the prior committed CURRENT pending logits, then
   embeds and executes token `j` through all target layers and final norm/head
   to produce NEXT pending logits;
6. after each decode step, commits the generated token's target-KV/indexer/
   page-table successor and atomically swaps the accepted NEXT pending
   generation into CURRENT; and
7. detokenizes through the Rust incremental decoder and returns four bounded
   result rows only after the final common completion receipt.

The four rows remain one physical C4 batch for all sixteen decode steps. After
step 16, every generated token has been executed into target KV/indexer state;
the final CURRENT pending logits are an unused terminal successor and remain
charged until cleanup. A
row cannot finish early: the fixture's reference establishes no EOS in the
first sixteen positions, and an early GLMAXX EOS is a correctness failure.
Sampling is exact greedy under the accepted vocabulary ownership and
all-masked/tie rules. A full-vocabulary runtime gather is forbidden.

The target program computes every embedding, norm, absorbed MLA/indexer,
attention, residual, router, routed/shared expert, final-head, and sampling
input itself. Reference hidden states, routes, candidates, winners, logits,
or tokens are comparison data only and never graph inputs.

## Correctness and repetition gates

The device is compared against an independent native-plane reference and the
pinned source-control runtime under the accepted quality policy. For every
row and generated position, retain:

```text
token ID and decoded byte span
four rank-local greedy candidates and winning rank
logical-vocabulary reference/device values or authenticated external arrays
KLD contribution, max absolute/relative error, top-two margin, tie class
target-KV/indexer physical and decoded comparisons
page-table/cache successor identities
```

Stable positions require the exact greedy token. Any tie-adjacent allowance
must be explicitly permitted by the accepted numerical policy and still pass
its per-position probability/KLD gate; it cannot be invented after the run.
The final four token sequences and decoded texts must repeat exactly across
all accepted repetitions.

The matrix contains:

- one fresh-process cold load/prefill/decode/destroy run;
- one second fresh-process cold run;
- five warm fixture-reset runs with weights/modules/graphs retained;
- eager versus captured controls for one prefill chunk and one C4 decode step
  under byte-identical snapshots; and
- one warm graph/config generation replacement that retains the exact
  resident-weight handle and proves zero checkpoint-read and weight-H2D bytes,
  or a fail-closed `HOT_RELOAD_DEFERRED` result if its separately reviewed
  resident-generation contract is not yet accepted.

Each repetition gets new argument, cache, pending-logit, collective, graph-
launch, sampling-result, output, and completion generations. Required
destinations are generation-poisoned and must be overwritten; unused/padded
bytes retain canonical values. Fixture reset restores the exact initial
page/cache state and never rereads or reuploads weights.

## Failure and evidence boundary

CPU/mock and device gates cover profile/program/manifest/graph/plan/module/
route mismatch, one-byte-short arenas and uses, stale generations, malformed
cache/logit/candidate/output records, missing/duplicate writes, rank
divergence, pre-collective cancellation, post-entry uncertainty,
asynchronous CUDA failure, owner loss, deadline expiry, cleanup mismatch, and
hot-generation prepare/rollback faults.

Recoverable failures require zero accepted rows and four exact synchronized
cleanup receipts. Collective/DMA/owner uncertainty is process-fatal: no
possibly referenced resource is freed, no cleanup receipt is forged, and the
isolated child terminates nonzero. Partial prompt/token output is never
reported as a successful batch.

The immutable evidence record separates:

```text
source/checkpoint validation, storage read, staging, and H2D
context/module, collective, graph, KV/page-table, and execution-ready phases
prefill kernels, collectives, framework, and end-to-end time
each of the sixteen decode physical steps and sixteen useful tokens
per-rank HBM ledger and exact arena generations
cold/warm/eager/captured/hot-deferred posture
all per-position numerical, token, cache, fault, and cleanup data
```

Timing is diagnostic. This smoke cannot claim 50 tok/s, a cold-boot win, or
any matched throughput result.

## Required CPU/mock proof after review

Before a device run, one coordinated Rust candidate must:

1. encode/decode and mutation-test all 672 program bytes and every set/hash
   domain;
2. construct both profile-local typed programs and reject every cross-profile,
   M3/M4, laboratory, old-domain, absent-predecessor, and MTP substitution;
3. derive both exact full target weight-use sets while keeping draft weights
   resident and unused;
4. construct the two graph plans, schedules, GraphProfile v3, all 32 class
   records, every buffer use, ten arenas, four rank-local bindings, and final
   memory plan with checked arithmetic;
5. prove the smoke-minimal cache fits its fixtures and preserves the exact 1M
   addressing/page ABI without claiming the later capacity floor;
6. execute all target-layer CPU references for four bounded synthetic rows,
   including prefill pending-state publication, sixteen distributed-greedy
   samples/emissions, sixteen generated-token cache commits, the unused
   terminal pending state, cache successors, and incremental decoding;
7. prove no expected route/logit/token can enter execution input and no full-
   vocabulary gather can enter the runtime sampler;
8. exhaust the generation poison/reset, one-byte-short, mismatch, rank-fault,
   cleanup, fatal-child, and bounded-time/resource matrix;
9. prove five warm runs perform zero weight read/H2D and either prove the
   reviewed compatible-generation swap or emit only the exact deferral; and
10. prove neither profile's result/token can enter MTP3, production health,
    HTTP serving, capacity, KLD-campaign, or benchmark acceptance.

Only after that implementation has its own adversarial token may a fresh cn4
occupancy check open the profile-matched device run.

## Exit tokens and nonclaims

Each device result is accepted independently:

```text
full-checkpoint-batch-smoke-v1-capacity-tr3-accepted
full-checkpoint-batch-smoke-v1-hybrid-w4a16-nf3-accepted
```

A token requires exact four-row, sixteen-token, profile-matched output plus
all correctness, repetition, cache, lifecycle, and evidence gates. It proves
the first full checkpoint-to-text MTP0 path only for that profile.

It does not prove MTP1--6, multi-user serving, prefix reuse, DRAM/NVMe
offload, 524,288-token physical KV, 1M retrieval, full quality, cold-boot
advantage, hot reload unless separately passed, sustained reliability, or
the 50/100 tok/s performance targets. This design conveys no cn4 or CUDA
authorization.
