# GLM-5.2 step execution ABI v3

Date: 2026-07-30

Status: design candidate; adversarial review required before implementation

GPU evidence: none

## Purpose and supersession

This contract makes the target-only and recurrent-MTP execution boundary
implementable as one Rust-owned, four-rank command ABI. It coordinates:

- `StepPlan.v3`;
- `StepInput.v2`;
- `SamplingCounter.v2`;
- rank-local and process-common `StepOutput.v2`;
- `PageTableDelta.v2`; and
- the MTP one-ahead page-transaction amendment.

It supersedes the unpromoted `StepPlan.v1`, `StepInput.v1`, `StepOutput.v1`,
and the prefill-only `StepPlan.v2` design candidate. It retains the v2
prefill row-bucket correction and 3,072-row ceiling, but v2 must never be
implemented and then mixed with this MTP contract.

The separate `RankTierCommand.v1` from the SM120 executor r2 amendment owns
cache-only work. `CACHE_ONLY` and `MIXED` are not step phases in this ABI.

This is a host/device contract design. It does not implement a model graph,
accept a checkpoint, or authorize cn4/CUDA.

## Execution phases

One plan and every row in it have one common phase:

```text
PREFILL              = 1
TARGET_DECODE        = 2
MTP_BOOTSTRAP        = 3
MTP_VERIFY           = 4
MTP_PIPELINED_ZERO   = 5
MTP_FLUSH            = 6
```

`PREFILL` executes exact prompt tokens, retains authoritative target logits,
and emits nothing. For MTP-capable prompts, successor tokens after the
slot-zero sentinel also execute teacher-sidecar rows from authoritative target
hidden states.

`TARGET_DECODE` is the nonpipelined MTP0 reference for a target-only request:
sample pending target logits, execute that token through the target model,
materialize it, and emit it.

`MTP_BOOTSTRAP` starts the one-ahead pipeline after prefill or after a
target-only-to-MTP transition. It samples authoritative target token `a`,
emits it without materializing target KV, prepares `a`'s teacher sidecar, and
optionally creates up to the chosen next depth of draft proposals.

`MTP_VERIFY` materializes the already-emitted pending token, verifies the
installed proposals, synchronizes accepted sidecars, optionally emits one
residual/bonus target token, and optionally installs the next proposal bundle.

`MTP_PIPELINED_ZERO` materializes one prior pending token, samples and emits
one next authoritative token, and prepares its teacher sidecar without draft
proposals. This is the explicit pipelined MTP0 quality/control posture.

`MTP_FLUSH` materializes one prior pending token, discards proposal scratch,
emits nothing new, and leaves equal materialized/emitted ends. It is required
before nonterminal suspension, session export, or MTP-to-target-only
transition.

Phase is not selected by a rank. The scheduler/coordinator chooses one common
phase and graph, and all ranks hash and execute that decision.

## `StepPlan.v3`

Identities:

```text
STEP_PLAN_ABI        = glmaxx.step-plan.v3
PLAN_HASH_DOMAIN     = glmaxx.step-plan.v3\0
PLAN_HASH_INPUT_BYTES = 95
PLAN_RECORD_BYTES     = 127
```

The canonical hash input uses exact little-endian concatenation:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | `epoch: u64` |
| 8 | 8 | `step_id: u64` |
| 16 | 1 | `phase: u8` |
| 17 | 2 | `active_sequences: u16` |
| 19 | 2 | `sequence_bucket: u16` |
| 21 | 4 | `scheduled_prompt_tokens: u32` |
| 25 | 4 | `target_rows: u32` |
| 29 | 4 | `target_row_bucket: u32` |
| 33 | 4 | `teacher_row_bucket: u32` |
| 37 | 4 | `recurrent_row_bucket: u32` |
| 41 | 1 | `current_mtp_depth: u8` |
| 42 | 1 | `next_mtp_depth: u8` |
| 43 | 4 | `graph_id: u32` |
| 47 | 2 | `tp_route_id: u16` |
| 49 | 2 | `dcp_route_id: u16` |
| 51 | 1 | `target_attention_transport: u8` |
| 52 | 1 | `draft_attention_transport: u8` |
| 53 | 2 | `sampling_route_id: u16` |
| 55 | 8 | `sequence_table_generation: u64` |
| 63 | 32 | `CollectiveSchedule.v2` SHA-256 |

The 32-byte plan hash is appended at offset 95. There is no implicit padding
or native-struct hashing.

Constants:

```text
MAX_ACTIVE_SEQUENCES       = 64
MAX_PREFILL_TARGET_ROWS    = 3072
MAX_DECODE_TARGET_ROWS     = 448
MAX_MTP_DEPTH              = 6
MAX_PREFILL_TEACHER_ROWS   = 3072
MAX_DECODE_TEACHER_ROWS    = 448
MAX_RECURRENT_ROWS         = 320
```

Sequence buckets are `1,2,4,8,16,32,64` and must cover
`active_sequences`. All row/bucket arithmetic is checked.

### Phase shapes

For `PREFILL`:

```text
scheduled_prompt_tokens == target_rows
1 <= target_rows <= target_row_bucket <= 3072
current_mtp_depth == next_mtp_depth == 0
teacher_row_bucket == target_row_bucket if any row is MTP-capable, else 0
recurrent_row_bucket == 0
target attention in {PREFILL_CKV, PREFILL_QUERY}
draft attention matches the reviewed prefill teacher route when teacher rows
exist, otherwise NONE
sampling_route_id == 0
```

For `TARGET_DECODE`:

```text
scheduled_prompt_tokens == 0
target_rows == active_sequences
target_row_bucket == sequence_bucket
teacher_row_bucket == recurrent_row_bucket == 0
current_mtp_depth == next_mtp_depth == 0
target attention == DECODE_QUERY_LSE
draft attention == NONE
sampling_route_id != 0
```

For `MTP_BOOTSTRAP`:

```text
scheduled_prompt_tokens == target_rows == target_row_bucket == 0
teacher_row_bucket == sequence_bucket
recurrent_row_bucket == sequence_bucket * max(next_mtp_depth - 1, 0)
current_mtp_depth == 0
0 <= next_mtp_depth <= 6
target attention == NONE
draft attention == DECODE_QUERY_LSE
sampling_route_id != 0
```

For `MTP_VERIFY`:

```text
scheduled_prompt_tokens == 0
1 <= current_mtp_depth <= 6
0 <= next_mtp_depth <= 6
target_rows == sum(row.proposal_count + 1)
active_sequences <= target_rows
target_rows <= target_row_bucket
target_row_bucket == sequence_bucket * (current_mtp_depth + 1)
teacher_row_bucket == target_row_bucket
recurrent_row_bucket == sequence_bucket * max(next_mtp_depth - 1, 0)
target and draft attention == DECODE_QUERY_LSE
sampling_route_id != 0
```

For `MTP_PIPELINED_ZERO`:

```text
scheduled_prompt_tokens == 0
target_rows == active_sequences
target_row_bucket == teacher_row_bucket == sequence_bucket
recurrent_row_bucket == 0
current_mtp_depth == next_mtp_depth == 0
target and draft attention == DECODE_QUERY_LSE
sampling_route_id != 0
```

For `MTP_FLUSH`:

```text
scheduled_prompt_tokens == 0
target_rows == active_sequences
target_row_bucket == sequence_bucket
teacher_row_bucket == recurrent_row_bucket == 0
0 <= current_mtp_depth <= 6
next_mtp_depth == 0
target attention == DECODE_QUERY_LSE
draft attention == NONE
sampling_route_id == 0
```

The graph key is the exact tuple:

```text
(phase, sequence_bucket, target_row_bucket, teacher_row_bucket,
 recurrent_row_bucket, current_mtp_depth, next_mtp_depth,
 target_attention_transport, draft_attention_transport)
```

Graph admission additionally requires the exact graph ID, schedule hash, and
route-table identity. No entry aliases a different phase or row family.

## `StepInput.v2`

Identities:

```text
STEP_INPUT_SCHEMA    = glmaxx.step-input.v2
INPUT_HASH_DOMAIN    = glmaxx.step-input.v2\0
SEQUENCE_INPUT_BYTES = 480
```

The variable-length canonical hash preimage is:

```text
StepPlan.v3 plan_hash                         32 bytes
sequence_table_generation                     8 bytes
PageTableDelta.v2 global_digest               32 bytes
PageTableDelta.v2 rank_delta_digest[4]        128 bytes
post_apply_device_table_digest[4]             128 bytes
row_count                                      2 bytes
prompt_token_count                             4 bytes
row_count fixed 480-byte records
prompt_token_count little-endian u32 IDs
```

The canonical input hash is stored separately and is not part of its own
preimage. Row order is scheduler and device sequence-table order. Prompt IDs
are concatenated with exact row offsets and no padding.

Each fixed row record is:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | `request_id: u64` |
| 8 | 4 | `prompt_tokens_total: u32` |
| 12 | 4 | `materialized_target_end: u32` |
| 16 | 4 | `emitted_token_end: u32` |
| 20 | 4 | `generated_tokens_emitted_before: u32` |
| 24 | 4 | `maximum_new_tokens: u32` |
| 28 | 4 | `prompt_payload_offset: u32` |
| 32 | 4 | `prompt_tokens_this_step: u32` |
| 36 | 1 | `configured_mtp_depth: u8` |
| 37 | 1 | `current_mtp_depth: u8` |
| 38 | 1 | `next_mtp_depth: u8` |
| 39 | 1 | `pending_target_kind: u8` |
| 40 | 4 | `pending_target_token: u32` |
| 44 | 1 | `proposal_count: u8` |
| 45 | 1 | `verify_row_mask: u8` |
| 46 | 1 | `proposal_state_kind: u8` |
| 47 | 1 | reserved zero |
| 48 | 24 | `proposal_token_ids: [u32;6]` |
| 72 | 1 | `sampling_kind: u8` |
| 73 | 1 | reserved zero |
| 74 | 2 | `top_k: u16` |
| 76 | 4 | `temperature_bits: u32` |
| 80 | 4 | `top_p_bits: u32` |
| 84 | 4 | reserved zero |
| 88 | 8 | `seed: u64` |
| 96 | 8 | `committed_rng_counter: u64` |
| 104 | 8 | `current_bundle_generation: u64` |
| 112 | 8 | `next_bundle_generation: u64` |
| 120 | 8 | `installed_draft_ticket_begin: u64` |
| 128 | 8 | `installed_draft_ticket_end: u64` |
| 136 | 16 | `proposal_state_bytes_by_rank: [u32;4]` |
| 152 | 128 | `proposal_state_digest_by_rank: [[u8;32];4]` |
| 280 | 128 | `retained_state_digest_by_rank: [[u8;32];4]` |
| 408 | 32 | `proposal_trace_digest: [u8;32]` |
| 440 | 32 | `draft_program_digest: [u8;32]` |
| 472 | 8 | reserved zero |

Enums:

```text
pending_target_kind:
  NONE=0 INITIAL=1 MTP0=2 GREEDY_MISMATCH=3 RESIDUAL=4 BONUS=5

proposal_state_kind:
  NONE=0 GREEDY=1 TOP_K=2 MASS=3

sampling_kind:
  GREEDY=1 TOP_K=2 MASS=3
```

All zero/reserved rules are hash-covered. Unknown values fail before queue
admission.

### Common row invariants

- request IDs are nonzero and unique;
- every count/end calculation uses checked arithmetic and is at most
  1,048,576;
- `generated_tokens_emitted_before < maximum_new_tokens`;
- `emitted_token_end - materialized_target_end` is zero or one;
- when the difference is zero, pending kind/token and current bundle
  generation are canonical absent values;
- when the difference is one for a live request, pending kind is nonzero,
  pending token is in `[0,154856)`, and current bundle generation is nonzero;
- configured/current/next depths are at most six;
- next depth is at most configured depth;
- the sampling tuple follows the distributed-sampling ABI, including
  canonical `+0`, `temperature <= 2`, and exact seed materialization;
- greedy has committed counter zero only for a new request but never advances
  it; probabilistic counters are checked for the phase maximum;
- a first prefill beginning at materialized position zero has zero retained
  state; a continued or restored-prefix prefill has four nonzero digests for
  its authoritative boundary hidden/logits, and every non-prefill phase has
  four nonzero retained-state digests;
- draft program digest is nonzero exactly for MTP phases; and
- `current_mtp_depth` and `next_mtp_depth` are bounded by their plan values.

The retained-state digest covers, as applicable, adopted program identity,
pending target logits, authoritative root hidden, pending teacher sidecar,
winner-list generation/content, recurrent scratch generation/content,
proposal-state spans, and their arena generations. A digest never replaces
the actual checked device spans; rank-local descriptors carry both.

### Bundle invariants

No current proposal bundle:

```text
proposal_count == verify_row_mask == 0
proposal tokens, proposal-state bytes/digests, trace digest == zero
installed draft ticket begin/end == 0
proposal_state_kind == NONE
```

Current bundle with `R=proposal_count`:

```text
1 <= R <= current_mtp_depth <= configured_mtp_depth
verify_row_mask == (1 << (R + 1)) - 1
proposal_token_ids[0..R] are valid; remaining IDs are zero
current_bundle_generation != 0
proposal_trace_digest != zero
proposal_state_kind matches sampling_kind
all four proposal-state lengths/digests are nonzero
```

For greedy, installed draft ticket begin/end are both zero. For probabilistic
TOP_K/MASS:

```text
installed_draft_ticket_end - installed_draft_ticket_begin == R
installed_draft_ticket_end == committed_rng_counter
```

This proves that DRAFT tickets committed when the current bundle was
installed and that verifier tickets continue from the exact successor.

`next_bundle_generation` is chosen process-common before launch. It is:

- nonzero and different from the current generation when the phase may
  install a next live pending token;
- nonzero for bootstrap even when next depth is zero, because the pending
  teacher state still has an identity; and
- zero for target-only, prefill, flush, or a row preclamped to terminal.

### Phase-specific input

`PREFILL` has equal ends, no pending/bundle, zero retained-state digests, and
positive prompt work. Configured depth controls whether prompt positions
receive draft-capable pages and teacher sidecars.

`TARGET_DECODE` has equal ends, no bundle, configured/current/next depth zero,
and zero prompt work.

`MTP_BOOTSTRAP` has equal ends, no current bundle, configured depth nonzero,
and zero prompt work. Its next bundle generation is nonzero.

`MTP_VERIFY` has one pending token, one current proposal bundle, zero prompt
work, and its actual proposal count contributes `R+1` target rows.

`MTP_PIPELINED_ZERO` has one pending token, no proposals, nonzero current and
next bundle generations, and all three depths canonical as configured-nonzero,
current zero, next zero.

`MTP_FLUSH` has one pending token, may carry a bundle that is discarded, has
next depth/generation zero, and consumes no sampling route.

## `SamplingCounter.v2`

One request owns:

```text
seed
committed_counter
current_bundle_generation
installed_draft_ticket_begin
installed_draft_ticket_end
```

Every ticket retains:

```text
request_id
logical_position
bundle_generation
draft_step
purpose: TARGET | DRAFT | ACCEPTANCE | RESIDUAL | BONUS
counter_before
counter_after
```

The SplitMix64 word and FP64 uniform mapping remain exactly as specified by
the distributed-sampling ABI. One probabilistic ticket advances the counter
by one; greedy consumes none.

Physical-step ordering is:

1. start from `committed_rng_counter`;
2. for verify, consume ACCEPTANCE tickets in proposal order and then exactly
   one RESIDUAL or BONUS ticket when required;
3. for target decode, bootstrap, or pipelined zero, consume one TARGET ticket;
4. after the authoritative next pending token and teacher state exist,
   consume DRAFT tickets for the next proposal generation in recurrence
   order; and
5. publish the final counter only with four-rank step/page/bundle consensus.

Thus proposals for bundle `g` are consumed when `g` is installed, while
acceptance/residual tickets for `g` are consumed by the later verification
step. Draft tickets for the replacement bundle follow that verification's
tickets in the same counter stream.

Before dispatch, checked maxima are:

```text
TARGET_DECODE                         1
MTP_BOOTSTRAP                         1 + next_depth
MTP_VERIFY                            current_depth + 1 + next_depth
MTP_PIPELINED_ZERO                    1
MTP_FLUSH / PREFILL                   0
```

The verify bound is conservative: at most `current_depth` acceptance tickets,
one residual/bonus ticket, and `next_depth` replacement proposals. Existing
installed DRAFT tickets are already before the input counter and are not
counted twice.

A failed step commits no counter, ticket range, or next bundle generation.
There is no same-generation retry after any native launch; prelaunch
request-local rejection may reuse the unchanged immutable request state.

## `PageTableDelta.v2` and one-ahead storage

Identities:

```text
PAGE_TABLE_DELTA_SCHEMA = glmaxx.page-table-delta.v2
GLOBAL_HASH_DOMAIN      = glmaxx.page-table-delta.v2\0
LOCAL_HASH_DOMAIN       = glmaxx.page-table-delta.local.v2\0
TABLE_HASH_DOMAIN       = glmaxx.page-table-state.local.v2\0
```

Each sequence update replaces the ambiguous single committed/tentative count
with:

```text
materialized_target_end
draft_prepared_end
reserved_end
configured_mtp_depth
page_count_after
first_changed_ordinal
changed pages
```

For target-only:

```text
draft_prepared_end == 0
materialized_target_end <= reserved_end
```

For MTP-capable private state:

```text
materialized_target_end <= draft_prepared_end <= reserved_end
draft_prepared_end - materialized_target_end in {0,1}
```

Shared, sealed, DRAM, NVMe, prefix, and session-visible state requires equal
target/draft materialized ends. Only private HBM state may have the one-ahead
teacher sidecar.

Each changed page carries separate:

```text
target_valid_tokens
draft_valid_tokens
target local page ID
draft local page ID or canonical absent
target state
draft state
```

This can represent a prepared draft slot when the paired target slot is not
yet valid. Scratch proposal KV and q state never appear in this page table.

The global delta digest hashes the complete ordered update/removal data. Each
rank-delta digest then hashes the rank ID, global digest, and that rank's exact
projection; `StepInput.v2`, not the delta preimage, carries the resulting four
digests and therefore introduces no hash cycle. The coordinator also computes
four expected post-application full device-table digests. Each rank:

1. verifies its delta projection;
2. uploads it after the exact predecessor generation;
3. runs a device hash over the complete live rank-local table;
4. compares with its expected post-state digest; and
5. acknowledges both digests and generation.

A restart begins at device generation zero with an empty-table digest. It can
accept only a full-install delta; a suffix delta structurally fails.

## Reservation bounds by phase

The page transaction reserves exact private successor slots per row:

| Phase | Maximum reserved successor slots |
|---|---:|
| PREFILL | exact prompt tokens this step |
| TARGET_DECODE | 1 |
| MTP_BOOTSTRAP | 1 pending target/draft slot |
| MTP_VERIFY with `R` proposals | `R+2` when a next pending token is allowed, otherwise `R+1` |
| MTP_PIPELINED_ZERO | 2 when a next pending token is allowed, otherwise 1 |
| MTP_FLUSH | 1 |

`MAX_VERIFY_RESERVATION_SLOTS_PER_ROW` therefore becomes eight, not seven.
The target graph still executes only `R+1` rows. The extra slot is the
possible residual/bonus token's private paired target/draft location; only
its teacher sidecar becomes prepared in the current step.

The coordinator clamps output/context and decides whether a next pending token
is allowed before reservation. It never allocates the extra slot and then
silently exceeds a limit.

At eight slots per row, decode/verify still changes at most one existing tail
and one new page per row, so the fixed 128 page-edit bound remains valid.
Prefill retains its reviewed 174-edit bound. The journal's per-row tentative
slot field widens from maximum seven to eight.

Reservation, device install, model execution, host stop filtering, commit or
rollback, and publication remain separate states. A text stop discovered
after output consensus may select a strict prefix of model-emitted tokens.
The page journal commits only the materialized prefix required by that
selected output, discards later target/teacher/scratch state, installs no next
bundle, and produces a new commit delta/digest before serving publication.

## `StepOutput.v2`

Each rank returns one fixed 240-byte record per sequence. Bytes `0..136` are
process-common; bytes `136..240` are rank-local:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | `request_id: u64` |
| 8 | 4 | `materialized_target_end_after: u32` |
| 12 | 4 | `emitted_token_end_after: u32` |
| 16 | 1 | `emitted_token_count: u8` |
| 17 | 1 | `materialized_token_count: u8` |
| 18 | 1 | `accepted_proposal_count: u8` |
| 19 | 1 | `target_kind: u8` |
| 20 | 1 | `residual_fallback: u8` |
| 21 | 1 | `model_terminal: u8` |
| 22 | 1 | `next_proposal_count: u8` |
| 23 | 1 | `next_verify_row_mask: u8` |
| 24 | 28 | `emitted_token_ids: [u32;7]` |
| 52 | 4 | `next_pending_target_token: u32` |
| 56 | 8 | `current_bundle_generation: u64` |
| 64 | 8 | `next_bundle_generation: u64` |
| 72 | 8 | `rng_counter_before: u64` |
| 80 | 8 | `rng_counter_after: u64` |
| 88 | 8 | `next_draft_ticket_begin: u64` |
| 96 | 8 | `next_draft_ticket_end: u64` |
| 104 | 32 | `sampling_trace_digest: [u8;32]` |
| 136 | 4 | `local_proposal_state_bytes: u32` |
| 140 | 32 | `local_proposal_state_digest: [u8;32]` |
| 172 | 32 | `local_retained_state_digest: [u8;32]` |
| 204 | 32 | `local_page_write_digest: [u8;32]` |
| 236 | 4 | reserved zero |

`target_kind` uses:

```text
NONE=0 INITIAL=1 MTP0=2 GREEDY_MISMATCH=3 RESIDUAL=4 BONUS=5
```

Unused token slots and absent next-state fields are zero. Token IDs are always
below 154,856.

The coordinator requires all ranks to return byte-identical common prefixes.
It then orders the four validated local suffixes by rank and computes:

```text
common_output_digest =
  SHA256("glmaxx.step-output.common.v2\0" || common sequence records)

rank_state_digest =
  SHA256("glmaxx.step-output.rank-state.v2\0" ||
         rank0 local suffixes || ... || rank3 local suffixes)

step_output_digest =
  SHA256("glmaxx.step-output.v2\0" ||
         plan_hash || input_hash || common_output_digest || rank_state_digest)
```

No rank pretends its local q/winner/page bytes are identical to another
rank's bytes.

### Output phase invariants

`PREFILL` returns no sequence records.

`TARGET_DECODE` materializes and emits exactly one token, has equal ends, and
installs no bundle.

`MTP_BOOTSTRAP` materializes zero and emits exactly one authoritative token.
If it is nonterminal, emitted end is one ahead and a next bundle generation,
pending token, retained state, and optional proposals are installed.

`MTP_VERIFY` materializes the prior pending token plus exactly the accepted
proposal count. It emits accepted proposal tokens plus at most one
residual/bonus target token. A nonterminal target token is the next pending
token; accepted proposal EOS has no target token or next bundle.

`MTP_PIPELINED_ZERO` materializes one prior pending token and emits zero or
one next target token.

`MTP_FLUSH` materializes one, emits zero, discards the current bundle, and
returns equal ends with no next state.

When a terminal EOS/output/context outcome emits a final authoritative target
without materializing it, `model_terminal=1`, the ends may differ by one, and
all next-bundle/local-retained fields are zero. The request is immediately
cleaned up and that final one-ahead token is never published as reusable
state.

Probabilistic counter movement must match the exact ticket trace and phase;
greedy before/after counters are equal. Next proposal state exists if and
only if a nonterminal next pending token exists and
`next_proposal_count > 0`. A depth-zero live pipeline retains the pending
teacher state with zero proposal-state bytes.

## Four-rank transaction

For one selected step:

1. scheduler selects rows and one phase;
2. coordinator preflights graph, schedule, counters, page slots, output
   capacity, and fixed journals;
3. page reservation produces `PageTableDelta.v2` generation `R`;
4. `StepPlan.v3` and `StepInput.v2` bind `R`, all delta digests, and all
   current/next bundle identities;
5. every rank installs and device-hash-verifies `R`;
6. the exact graph executes and returns one rank-local output;
7. coordinator validates common consensus and rank-local state;
8. tokenizer/stop handling selects the externally committed output prefix;
9. page/bundle/RNG/scheduler state is finalized into successor generation
   `C=R+1`;
10. every rank installs and device-hash-verifies `C`; and
11. only then are output events published and tentative IDs made reusable.

Any native entry, rank divergence, invalid output, device-table digest
mismatch, stop-finalization failure, or missing successor receipt retires the
worker generation. A failure after a previously emitted pending token cannot
rewind client output; the serving coordinator terminates that request.

## Fixed-capacity and concurrency boundary

All plan/input/output rows, prompt references, rank digests, page deltas,
bundle descriptors, proposal tokens, and output records live in preallocated
argument/completion slabs. Rank-local TOP_K/MASS q data lives in the
memory-plan-charged proposal-state arena:

```text
TOP_K <= reviewed fixed support record for 256 global candidates
MASS  <= 6 * 38,720 * 4 bytes per rank per live sequence before alignment
```

Admission charges configured maximum depth and sampling class. A row cannot
switch to a larger state class locally.

Continuous batching may mix row sampling parameters and actual proposal
counts only when phase, graph key, route class, and plan depth buckets are
common. Fixed masks preserve collective counts. No model graph overlaps
another dependency-linked model graph in v3; host filling, tokenization, HTTP,
and nonaliasing tier I/O retain their executor-defined concurrency.

## Required adversarial review

Review must independently falsify or accept:

1. all six phase transitions and target/teacher/recurrent lineage;
2. the 95/127-byte plan serialization and 480-byte row offsets;
3. phase row/bucket formulas at C1/C64 and MTP0–6;
4. materialized/emitted/pending/bundle invariants at context/output/EOS
   boundaries;
5. physical counter ordering across proposal install and later verify;
6. proposal/retained state completeness for greedy, TOP_K, and MASS;
7. PageTableDelta v2's ability to represent a private one-ahead draft record;
8. global, rank-delta, and full device-table digest non-equivocation;
9. the eight-slot reservation bound and unchanged 128-edit decode bound;
10. the 240-byte rank output and common-versus-local consensus;
11. post-output stop-prefix finalization without publishing uncommitted state;
12. fixed-capacity memory and concurrency implications; and
13. every superseded v1/v2 identity failing closed.

## Required CPU/reference proof

Only after adversarial acceptance:

1. byte/offset/enum known answers for plan, input, counter, delta, and output;
2. v1/v2 rejection and reserved/unused-byte mutation tests;
3. every phase at C1/C64, every current/next depth pair, and mixed actual
   proposal counts;
4. prompt chunks at 1, 448, 449, 3,071, 3,072, and 3,073 rows;
5. bootstrap, zero-depth pipeline, flush, mismatch at every proposal, all
   accepted, early/accepted/correction/bonus EOS, and no-next-token clamps;
6. greedy output equality to target-only MTP0;
7. TOP_K/MASS ticket known answers across adjacent physical steps;
8. q-state byte/digest/span corruption and rank substitution;
9. slot-zero, page 63/64, maximum context, and every tail occupancy with
   reservations one through eight;
10. wrong-offset, partial-upload, stale-restart, and re-signed
    device-table-digest attacks;
11. every common-output divergence and rank-local suffix substitution;
12. stop at every byte/token boundary within a seven-token verifier result;
13. failure before/after reservation, upload, target, teacher, recurrence,
    sampling, output, stop finalization, successor upload, and publication;
14. allocation counters proving zero hot-path heap or device allocation; and
15. byte-equivalent final logical state against an independent slow oracle.

This CPU proof opens no GPU gate. Authorized SM120 work begins only after the
rank executor, target/MTP program, sampling, page transaction, and this ABI
all have accepted tokens.
