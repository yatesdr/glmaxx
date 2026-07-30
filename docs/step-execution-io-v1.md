# Step execution I/O candidate

Date: 2026-07-29

Status: CPU worker/serving integration candidate; adversarial review required
before device promotion

GPU evidence: none

## Problem

`StepPlan` currently fixes graph shape and collective order, but the rank
executor receives no request-row data. The serving coordinator also drops the
uncached prompt-token suffix after admission and retains only a sampling
collective class. That boundary cannot truthfully execute a checkpoint:

- a prefill kernel needs the exact token IDs and absolute positions;
- decode and verify need exact temperature, top-k, top-p, seed, and RNG
  counter state;
- each row needs its request identity, context length, generation limit, and
  MTP posture;
- the coordinator must consume the final RNG counter returned by distributed
  sampling; and
- all four ranks must prove they consumed the same immutable row snapshot.

This document defines the smallest GLM-5.2-only host boundary that closes
those gaps without changing the reviewed 117-byte `StepPlan` record.

## Pipelined token convention

Prefill computes every scheduled prompt token and leaves authoritative target
logits for the final processed position in rank-local sequence state. It does
not sample or emit a token.

The next decode step performs these operations in one graph:

1. sample the pending target logits with the request's distributed sampling
   route;
2. broadcast the token ID and updated RNG counter;
3. commit that token to the step result;
4. embed and execute that token through the target model; and
5. retain its target logits as the pending logits for the following step.

This avoids reprocessing the final prompt token and avoids a separate
sample-only launch. The final requested token may produce unused pending
logits; those logits are discarded at terminal cleanup.

An MTP verify step starts from the same pending-logit state, executes the
configured recurrent draft proposal, verifies target rows transactionally,
returns accepted-draft provenance plus an optional residual/bonus target
token, and leaves the pending target state at the last committed position.

## Immutable `StepInput`

The coordinator constructs one immutable `StepInput` after selecting a
`ScheduledBatch` and before dispatch. The object is shared read-only with all
four persistent rank threads. It contains:

```text
schema                         glmaxx.step-input.v1
sequence_table_generation      u64
page_table_delta_digest        SHA-256
rows                           1..64 SequenceInput records
prompt_token_ids               0..scheduled_prompt_tokens u32 values
canonical_hash                 SHA-256
```

Each `SequenceInput` contains:

```text
request_id                     u64, nonzero
context_tokens_before          u32
generated_tokens_before        u32
maximum_new_tokens             u32
prompt_payload_offset          u32
prompt_tokens_this_step        u32
configured_mtp_depth           u8
effective_mtp_depth            u8
sampling_kind                  GREEDY, TOP_K, or MASS
temperature_bits               canonical finite f32 bits
top_p_bits                     canonical finite f32 bits
top_k                          u16
seed                           u64
rng_counter_before             u64
```

Rows are in exact `ScheduledBatch.rows` and kernel sequence-table order.
Prompt tokens are concatenated in that order with no padding. Every prompt
range must be contiguous, nonoverlapping, and collectively cover the payload.

`context_tokens_before` is the number of committed target positions before
this step. For prefill it equals the request's `prompt_done`; for decode and
verify it equals `prompt_tokens + generated_tokens_before`. The sum of
context plus scheduled/maximum output work may never exceed 1,048,576.

The two MTP fields are deliberately distinct. `configured_mtp_depth` binds
the request's immutable target-plus-draft page posture. `effective_mtp_depth`
binds the graph selected for this step after the scheduler clamps speculation
to remaining output capacity. A request configured for MTP6 can therefore
execute a final MTP0 decode without losing or silently changing its draft
attachments.

The sampling tuple is canonical:

- greedy: `temperature = 0`, `top_k = 0`, `top_p = 1`;
- bounded sampling: finite `temperature > 0`, `1 <= top_k <= 256`, and
  `0 < top_p <= 1`;
- distributed mass: finite `temperature > 0`, `top_k = 0`, and `top_p = 1`;
- unbounded top-p (`top_k = 0`, `top_p < 1`) is rejected.

Negative zero, NaN, infinity, and noncanonical float encodings are rejected.
An omitted API seed must be materialized once at admission; it cannot remain
an optional or rank-local choice.

## Mode validation

For PREFILL:

- row count equals `active_sequences`;
- the sum of row prompt counts equals `scheduled_prompt_tokens` and
  `query_rows`;
- each row's payload range contains exactly the uncached prompt slice selected
  by the scheduler;
- sampling state is carried forward but consumes no counter; and
- `StepOutput` is empty.

For DECODE:

- row count equals `active_sequences`;
- every prompt count and the prompt payload are zero;
- every row has effective MTP depth zero, while configured depth may remain
  nonzero, and every row has remaining output capacity;
- its sampling kind matches the plan's collective route; and
- exactly one target token is returned per row.

For VERIFY:

- the DECODE rules hold except every row's effective depth equals the plan's
  common depth 1–6 and configured depth is at least that value;
- query rows remain `active_sequences * (depth + 1)`; and
- result provenance contains zero through `depth` accepted draft tokens plus
  either one target residual/bonus token or a final accepted draft EOS.

MIXED remains rejected. CACHE_ONLY has zero rows and no prompt payload.

## Binding and consensus

The canonical input hash covers the schema, generation, the exact canonical
`PageTableDelta` global digest, every row field, and every prompt token in
explicit little-endian order. `StepInput` must be verified before queue
admission and again on each rank thread.

The existing `sequence_table_generation` in `StepPlan` must equal the input
generation and the delta's successor generation. The delta must contain
exactly one update for every input row and no removals. Each update binds the
request ID, configured MTP posture, committed count after prefill or before
decode/verify, and an exact tentative reservation of zero, one, or
`effective_mtp_depth + 1`. A generation is immutable and cannot be reused
with a different input or delta hash.

For decode and verify, the unique logits collective in the verified schedule
must map to every row's sampling kind and use the plan's sampling route.
Prefill contains no logits collective.

The dispatcher will send the same `Arc<StepInput>` and
`Arc<PageTableDelta>` to all ranks. Each rank acknowledgment will include the
input hash, global delta digest, and its rank-local delta digest alongside
plan, schedule, and output hashes. Any mismatch kills the entire worker
generation.

This does not claim that a host hash proves a CUDA upload. The SM120 executor
must additionally make its sequence-table upload completion part of the rank
stream dependency before graph launch. Device-side descriptor validation and
the rank acknowledgment occur only after that dependency.

## Output and RNG continuation

Each sequence result carries:

- committed token IDs;
- accepted-draft count;
- target-token-present bit; and
- final RNG counter.

Greedy execution does not advance the counter. Probabilistic decode advances
it exactly once. Probabilistic MTP counter allocation must follow the separate
sampling ABI for proposal, acceptance, residual, and bonus draws. Until that
ABI fixes the exact accepted-EOS rule, the runtime may validate monotonic and
bounded advancement but must retain every pre/post counter in quality
evidence.

After four-rank output consensus, the coordinator atomically commits scheduler
progress, KV state, and the final counter. A failed or divergent step advances
none of them.

## Prompt ownership and memory bounds

The coordinator retains the complete tokenized prompt until its final prefill
chunk commits. It may then release prompt bytes not required by an external
session record. At one million tokens this is 4 MiB of token IDs per request,
so admission must budget aggregate host prompt bytes and reject rather than
grow without bound.

Prefix restoration changes `prompt_done`; it never changes or splices the
retained canonical token vector. The first uncached payload token is always
`tokens[prompt_done]`. This prevents a cache hit from changing request text or
position identity.

## Required CPU proof

After adversarial review, the CPU gate must prove:

1. byte-stable hashes and generation reuse rejection;
2. prompt concatenation for multiple rows and partial chunks;
3. exact restored-prefix suffix selection;
4. all sampling classes and invalid float/filter combinations;
5. row/order/hash divergence kills TP4 consensus;
6. output counters cannot regress or exceed the reviewed per-step bound;
7. a failed step leaves prompt progress, generation count, and RNG state
   unchanged; and
8. the one-million-token prompt-vector bound uses checked arithmetic.

## Current implementation boundary

`glm-engine::StepInput` implements the canonical row/prompt/sampling hash,
checked 1,048,576-token arithmetic, configured-versus-effective MTP
distinction, schedule-to-sampling validation, and exact
`PageTableDelta.v1` binding. `ServingCoordinator` now retains the exact
sampling tuple, prompt tokens, and context progress and constructs this
object for every selected batch.

`Tp4WorkerPool` initializes one persistent `PageTableMirror` on each rank
thread. Admission and terminal removal advance all four mirrors before host
publication. A compute command shares one `Arc<StepInput>` and
`Arc<PageTableDelta>` with every rank; the rank independently verifies the
plan, schedule, input, and delta, atomically applies the reservation, and
acknowledges input hash, global delta digest, and its expected local digest.
Decode and verify apply a second acknowledged commit/rollback/removal delta
before scheduler and host publication. A post-execution host preflight error
advances an explicit rollback delta; a worker/consensus failure closes the
worker generation and cannot be disguised as a rank cleanup receipt.

The production backend materializes and retains exact greedy seeds. It
remains fail-closed on probabilistic requests because `StepOutput` does not
yet return the reviewed final RNG counter.

The CPU regressions cover deterministic multi-row prompt hashing, an MTP6
request clamped to an MTP0 tail step, MTP5 verification, all three sampling
forms, invalid floats/filters, schedule mismatch, context/output bounds,
hash/delta tampering, persistent four-rank mirror receipts, reservation plus
commit generations, uninitialized/stale rejection, rollback after late
publication failure, and exact serving-to-rank sampling/context delivery.

`CACHE_ONLY` remains outside this object because the reviewed `StepPlan`
contract requires generation zero for that mode while a real page delta
necessarily advances a nonzero generation. Device upload receipts,
physical-ID reuse quarantine, RNG-counter output/commit, and fixed-capacity
hot-path storage remain required before device promotion.

No CUDA launch or serving claim follows from this CPU implementation.
