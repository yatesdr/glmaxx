# Distributed sampling ABI v1 r2 amendment

Date: 2026-08-04

Status: corrective design candidate; adversarial acceptance required before
CPU implementation

GPU evidence: none

## Scope and precedence

This amendment resolves the withheld review of
`docs/distributed-sampling-abi-v1.md`. The reviewer accepted the canonical
request tuples, vocabulary partition, SplitMix64 mapping, ticket purposes,
acceptance rule, maximum counter arithmetic, rank-zero decision structure,
and transaction boundary. This r2 retains those accepted sections and
replaces the contradictory proposal timing, incomplete CDF arithmetic, and
unspecified TOP_K residual route.

The operative design is the conjunction of:

- `docs/distributed-sampling-abi-v1.md`;
- this corrective amendment;
- `docs/step-execution-abi-v3.md` and its target-layer successors
  `StepPlan.v4`/`StepInput.v3`;
- `docs/mtp-layer-execution-v1-r2.md`; and
- `docs/target-layer-execution-v1-r2.md`.

This amendment wins over conflicting retained text. No v1 sampling object,
same-step proposal interpretation, or predecessor step ABI may coexist with
these bytes in one worker generation.

This is a design only. It does not promote probabilistic serving, implement a
CPU or CUDA path, authorize a GPU launch, or claim model quality.

## Closed identities

The coordinated implementation uses:

```text
SamplingConfig.v2            32 bytes
SamplingCounter.v2           48 bytes
SamplingTicket.v2            48 bytes
TopKSupportRecord.v2       2,048 bytes
MassProposalState.v2      154,880 bytes per rank and proposal
SamplingTrace.v2             variable, digest retained in StepOutput.v2
```

The single sampling hash stored by `TargetProgram.v1` is no longer the hash
of the retained v1 file alone. It is the composite:

```text
distributed_sampling_abi_sha256 = SHA256(
  "glmaxx.distributed-sampling-abi.v1-r2\0" ||
  SHA256(exact distributed-sampling-abi-v1.md bytes) ||
  SHA256(exact distributed-sampling-abi-v1-r2.md bytes)
)
```

The two accepted file hashes are startup-manifest fields and must be identical
on all ranks. This formula amends the final-head field in target-layer r2 so a
worker cannot bind the reviewed v1 mechanics while silently omitting the r2
proposal, draw, or residual rules. There is no prose-selected alternate
digest and no self-reference: each inner hash is computed over file bytes,
then the outer domain binds their order.

`SamplingCounter.v2` and `SamplingTicket.v2` have the exact layouts and
purpose enums in `mtp-layer-execution-v1-r2.md`. The fixed request record is:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 1 | kind: `GREEDY=1`, `TOP_K=2`, `MASS=3` |
| 1 | 1 | reserved zero |
| 2 | 2 | `top_k: u16` |
| 4 | 4 | `temperature_bits: u32` |
| 8 | 4 | `top_p_bits: u32` |
| 12 | 4 | reserved zero |
| 16 | 8 | `seed: u64` |
| 24 | 8 | `initial_counter: u64` |

All integers are little-endian. Floating fields contain IEEE binary32 bits.
Canonical tuple validation remains exactly as in v1. A new request requires
`initial_counter=0`; a resumed request separately restores the committed
`SamplingCounter.v2` and proves that its seed matches this record.

## Proposal timing and immutable input

The current verifier never generates the proposal bundle it verifies.
Proposal bundle `g` was created by a prior `MTP_BOOTSTRAP` or `MTP_VERIFY`
physical step and installed only after that step's four-rank consensus. The
later verifier's immutable `StepInput.v3` therefore carries already committed:

```text
current_bundle_generation = g
proposal_count = R
verify_row_mask = (1 << (R + 1)) - 1
proposal_token_ids[0..R]
proposal-state spans and digests
installed DRAFT ticket range
proposal trace digest
```

Those fields exist before selection and dispatch. They cannot change during
verification.

After verifying `g`, the current physical step may generate replacement
bundle `g'`. Its recurrent draft stops at configured depth, the first sampled
draft EOS, or a precomputed output/context limit. It returns only successor
fields:

```text
StepOutput.v2.next_bundle_generation
StepOutput.v2.next_proposal_count
StepOutput.v2.next_verify_row_mask
StepOutput.v2.next_draft_ticket_begin/end
rank-local next proposal-state bytes/digests
next proposal trace digest
```

For `R' > 0`, `next_verify_row_mask == (1 << (R' + 1)) - 1`. For `R'=0`
it is zero. Only four-rank output, state, page, and successor-generation
consensus installs `g'`; a failure installs none of it. Early draft EOS thus
changes the next immutable input, never the input of the step taking the draw.

Graph padding above `R+1` or `R'+1` has zero row validity and cannot execute a
model operation, consume a ticket, write KV, emit a token, or contribute a
collective record.

The current step ABI's blanket statement that every proposal bundle has four
nonzero proposal-state lengths/digests is amended as follows:

```text
GREEDY: proposal_state_kind=GREEDY; all state lengths/digests are zero
TOP_K:  proposal_state_kind=TOP_K; all four lengths/digests are nonzero
MASS:   proposal_state_kind=MASS;  all four lengths/digests are nonzero
```

GREEDY still has proposal tokens, a nonzero common proposal trace digest, and
a bundle generation; it needs no q-probability payload. This agrees with the
zero-byte greedy memory rule in the MTP r2 contract.

## Counter uniform and bounded draw

The v1 integer stream remains normative. The canonical known answer is:

```text
seed=0, counter=0 -> x=0xe220a8397b1dcdaf
```

For any positive finite binary32 mass `m`, `bounded_draw(seed,counter,m)` is
exactly:

```text
u64 x       = the v1 SplitMix64 word
f64 u       = RN_f64(f64(x >> 11) * 2^-53)
f64 product = RN_f64(u * exact_f64(m))
f32 d       = RN_f32(product)
draw        = d < m ? d : f32_from_bits(bits(m) - 1)
```

All comparisons are ordered and strict. There is no contraction, fast math,
FTZ, or DAZ. The final branch covers the case where conversion rounds the
product to `m`; because `m` is positive finite, decrementing its positive
binary32 encoding is the exact predecessor, possibly `+0`.

Every global CDF is accumulated in its declared order with one binary32 RNE
addition per item. Rank selection chooses the first interval satisfying
`draw < cumulative_after`. The owner computes:

```text
local = RN_f32(draw - cumulative_before)
local_draw = local < owner_mass
           ? local
           : f32_from_bits(bits(owner_mass) - 1)
```

It then chooses the first local item satisfying
`local_draw < local_cumulative_after`. There is no last-token rescue path;
failure to select after these exact clamps is engine-fatal.

## Exact filtering and normalized probabilities

Padding and forbidden-token masking occur first. NaN or positive infinity is
fatal. Negative infinity is excluded from TOP_K candidate batches and has
zero mass in MASS. An all-masked row is fatal.

For TOP_K, each rank emits only its finite candidates in descending binary32
total-order logit, then ascending token ID. Rank zero merges in rank/candidate
order and applies the same order globally. It retains
`min(top_k, global_finite_count)` candidates; it never pads with negative
infinity.

Rank zero then performs, in retained logit order:

```text
maximum       = first retained logit
delta[i]      = RN_f32(logit[i] - maximum)
scaled[i]     = RN_f32(delta[i] / temperature)
weight[i]     = qualified_exp_f32(scaled[i])
topk_mass     = left_fold_RN_f32_add(weight[0..K])
nucleus_limit = RN_f32(top_p * topk_mass)
```

`top_p` and `temperature` are the exact binary32 values from the config. The
kept nucleus is the smallest nonempty prefix whose left-fold binary32 sum is
greater than or equal to `nucleus_limit`; equality includes the crossing
candidate. Its final cumulative value is `kept_mass`.

Normal TOP_K sampling calls `bounded_draw(..., kept_mass)` and walks raw
weights in retained logit order. The normalized probability retained for
each kept token is `RN_f32(weight/kept_mass)`. Probabilities are then reordered
by ascending token ID without recomputation for proposal state and residual
work. Every active support probability is positive finite. The binary32 sum
of normalized entries is recorded but is not forced to exactly one.

For MASS, rank-order maximum and mass phases retain the v1 order. Each rank
computes `delta`, `scaled`, and `qualified_exp_f32` with the same three
rounding points above, then its binary32 local mass in ascending physical row;
rank zero sums rank masses in rank order. Normal sampling uses that total and
the bounded-draw rule above. For DRAFT proposal state, each rank stores
`RN_f32(weight/total_mass)` in ascending local physical-row order. Rank 3's
final 24 padding entries are exact `+0.0`.

Acceptance promotes the retained binary32 `p(d)` and `q(d)` exactly to
binary64 and computes:

```text
ratio = min(1.0, RN_f64(exact_f64(p(d)) / exact_f64(q(d))))
accept iff u < ratio
```

`q(d)` must be positive finite. One ACCEPTANCE ticket and uniform are consumed
and traced even when the ratio is zero or one.

## Fixed proposal-state records

### TOP_K

`TopKSupportRecord.v2` is exactly 2,048 bytes: 256 consecutive eight-byte
entries.

| Entry offset | Bytes | Field |
|---:|---:|---|
| 0 | 4 | `token_id: u32` |
| 4 | 4 | normalized `probability_bits: u32` |

The first `S`, `1..=256`, entries are unique and strictly ascending by token
ID. Remaining entries are canonical `{token_id=0xffffffff,
probability=+0.0}`. No active logical token can equal the sentinel. The
selected proposal token occurs exactly once with positive probability.

Rank zero constructs the record and broadcasts the identical 2,048 bytes to
all ranks as part of DRAFT proposal installation. A bundle with actual depth
`R` has `R*2,048` proposal-state bytes on every rank. At C64/MTP6, allocation
is exactly:

```text
64 * 6 * 2,048 = 786,432 bytes/rank
```

### MASS

`MassProposalState.v2` is exactly 38,720 binary32 probabilities, or 154,880
bytes, per rank and proposal. Entries map one-to-one to the rank's physical
LM-head shard. A bundle with actual depth `R` has `R*154,880` bytes per rank.
The admitted maximum remains:

```text
64 * 6 * 38,720 * 4 = 59,473,920 bytes/rank
```

### State digests

For TOP_K and MASS, each rank's bundle digest is:

```text
SHA256(
  "glmaxx.proposal-state.v2\0" ||
  rank:u8 || state_kind:u8 || R:u8 || five_zero_bytes ||
  request_id:u64_le || bundle_generation:u64_le ||
  SamplingConfig.v2 ||
  installed_draft_ticket_begin:u64_le ||
  installed_draft_ticket_end:u64_le ||
  proposal_token_ids:[u32_le;6] ||
  state_bytes_length:u32_le || exact rank-local state bytes
)
```

The common proposal trace binds the four digests in rank order. For TOP_K it
also binds, per proposal, the SHA-256 of the bare 2,048-byte support record so
the four replicated records must agree rather than merely have valid
rank-domain digests. Digests never replace checked arena bounds or bytes.

## Exact composite messages

All records use little-endian integers, binary32/binary64 bit patterns, and
canonical zero reserved fields:

| Record | Bytes | Fields |
|---|---:|---|
| `GreedyCandidate.v2` | 8 | `logit:f32, token_id:u32` |
| `TopKCandidateCount.v2` | 4 | `count:u16, reserved:u16` |
| `TopKCandidate.v2` | 8 | `logit:f32, token_id:u32` |
| `MaximumValue.v2` | 4 | `maximum:f32` |
| `MassValue.v2` | 4 | `mass:f32` |
| `ResidualMassValue.v2` | 8 | `target_mass:f32, residual_mass:f32` |
| `MassSelection.v2` | 16 | `total_mass:f32, global_draw:f32, cumulative_before:f32, owner_rank:u8, flags:u8, reserved:u16` |
| `ProbabilityAtToken.v2` | 16 | `token_id:u32, target:f32, draft:f32, present:u8, source_rank:u8, reserved:u16` |
| `SamplingResult.v2` | 16 | `token_id:u32, purpose:u8, draft_step:u8, reserved:u16, counter_after:u64` |
| `MassSamplingResult.v2` | 24 | `SamplingResult.v2, selected_probability:f32, reserved:u32` |
| `AcceptanceResult.v2` | 40 | `token_id:u32, accepted:u8, draft_step:u8, reserved:u16, target:f32, draft:f32, ratio:f64, uniform:f64, counter_after:u64` |
| `TopKProposalInstall.v2` | 2,064 | `SamplingResult.v2, TopKSupportRecord.v2` |

`MassSelection.flags & 1` is `residual_fallback`; every other bit is zero.
For a GREEDY rank with no finite valid token, the only legal candidate is
`{logit=-infinity, token_id=0xffffffff}`. Any other sentinel use is fatal.

### GREEDY route

Each rank sends one candidate per row to rank zero. Rank zero selects the
finite maximum/lower-token-ID tie and broadcasts `SamplingResult.v2`. Greedy
does not allocate or advance a counter.

### TOP_K route

Each rank sends one count followed by exactly that many candidates. At
`top_k=256`, maximum root ingress is:

```text
4 * 4 + 4 * 256 * 8 = 8,208 logical bytes/row
```

Rank zero validates, merges, filters, samples, and broadcasts 16 bytes for an
ordinary TARGET/BONUS/RESIDUAL result. For a DRAFT proposal it broadcasts one
2,064-byte install record. Route manifests separately retain exclusive
measured PCIe bytes for the qualified broadcast algorithm; logical payload is
never mislabeled as bus traffic.

During verification, rank zero constructs the target `p` support through the
same candidate path and already holds the installed draft `q` support. It
looks up `p(d)` and `q(d)`, takes the ACCEPTANCE ticket, and broadcasts one
`AcceptanceResult.v2`. No support scatter or full-vocabulary gather is
required.

At the first TOP_K rejection, rank zero merges the target/draft sparse-support
union, at most 512 token IDs, in ascending token order. Missing probability
is exact `+0.0`. It computes `RN_f32(max(RN_f32(p-q),+0.0))`, accumulates the
residual in ascending token order, samples with the RESIDUAL ticket, and
broadcasts `SamplingResult.v2`.

If the residual sum is zero, rank zero instead samples the target support
with the same ticket and marks `residual_fallback=1`. The event is not
engine-fatal: the original review confirmed it is the correct degenerate
limit. Every event is counted; any nonzero qualification or production rate
withholds profile promotion pending a new review.

### MASS route

For a normal MASS row:

1. every rank sends `MaximumValue.v2`; rank zero broadcasts the selected
   global maximum;
2. every rank sends `MassValue.v2`; rank zero sums in rank order, obtains the
   bounded global draw, and broadcasts `MassSelection.v2`;
3. the named owner clamps the local draw as specified, walks its ascending
   local CDF, and sends `MassSamplingResult.v2` to rank zero; and
4. rank zero broadcasts the validated 24-byte result.

All ranks receive `total_mass` and can therefore materialize normalized MASS
proposal state for a DRAFT row. `cumulative_before` and the owner's already
known local mass fully define its local draw; there is no implicit wire type.
For an ordinary row, the selected probability is the owner's retained
normalized binary32 value. For residual or fallback sampling, it is
`RN_f32(selected_weight/total_mass)` under the distribution named by the
flags. This makes the common trace independently reconstructible without
moving the owner's full probability array.

For MASS acceptance, all four ranks send `ProbabilityAtToken.v2`. Exactly the
partition owner has `present=1`; the other three use zero probabilities and
`present=0`. `source_rank` always equals the sending rank. Rank zero validates
the unique owner, consumes the ticket, and broadcasts `AcceptanceResult.v2`.

At rejection, every rank computes local target and residual masses from its
target row and installed q array in ascending local-token order, then sends
`ResidualMassValue.v2`. Rank zero selects residual or fallback target mass,
broadcasts `MassSelection.v2`, and the owner walks the matching local CDF and
returns `MassSamplingResult.v2`. Only masses, one owner draw, and one result cross ranks; full-vocabulary
probabilities do not.

For all accepted proposals, BONUS reuses the current target distribution's
ordinary TOP_K or MASS route. Accepted draft EOS and precomputed output or
context clamps consume no bonus ticket.

## Trace, terminal reason, and replay scope

The process-common sampling trace digest is:

```text
SHA256(
  "glmaxx.sampling-trace.v2\0" ||
  request_id:u64_le || step_id:u64_le ||
  StepInput.v3_hash || SamplingConfig.v2 || route_manifest_sha256 ||
  counter_before:u64_le || counter_after:u64_le ||
  item_count:u32_le || ordered trace items ||
  no_target_reason:u8 || seven_zero_bytes
)
```

Each item is `kind:u16_le, payload_bytes:u16_le, ordinal:u32_le, payload`.
Kinds are `TICKET=1`, `CANDIDATE_DIGEST=2`, `SUPPORT_DIGEST=3`,
`RANK_STATE_DIGEST=4`, `SAMPLING_RESULT=5`, `ACCEPTANCE_RESULT=6`,
`MASS_SELECTION=7`, and `RESIDUAL_SUMMARY=8`. Payloads are the exact records
above or 32-byte SHA-256 values. Ordinals are strictly increasing from zero.
Unknown kinds, wrong fixed lengths, duplicate/skipped ordinals, or nonzero
reserved bytes are fatal.

`no_target_reason` is:

```text
NONE=0 ACCEPTED_DRAFT_EOS=1 OUTPUT_LIMIT=2 CONTEXT_LIMIT=3
```

It resolves the retained `target_kind=NONE` ambiguity without widening the
240-byte `StepOutput.v2`. The reason is nonzero exactly when a verify cycle
emits no residual/bonus target for one of those causes.

An omitted seed is materialized once from the process CSPRNG before request
admission. Entropy failure rejects that request before scheduling. The
effective seed is returned in `x-glmaxx-effective-seed`; the response also
identifies the sampling ABI and engine build. Replay is promised only for the
same engine build digest, sampling ABI bytes, route manifest, qualified math
program, sampling tuple, token history, and initial counter. It is not a
cross-build, cross-libm, or cross-architecture promise.

## Counter schedule and terminal behavior

Installed DRAFT tickets belong to the earlier step that created the current
bundle. A verifier starts from the committed successor counter. In its own
physical step it consumes:

1. ACCEPTANCE tickets in proposal order until the first rejection or all `R`
   proposals are accepted;
2. exactly one RESIDUAL ticket after rejection, or one BONUS ticket when all
   are accepted and a target token is permitted;
3. no residual/bonus ticket after accepted draft EOS or an output/context
   clamp; and
4. DRAFT tickets for replacement bundle `g'` only after its authoritative
   pending target and teacher state exist.

The per-phase predispatch bounds in `SamplingCounter.v2` remain unchanged.
They charge `current_depth`, not unknowable in-step behavior, and include
`next_depth` replacement work. Tickets actually retained by the output must
match the exact branch, actual current `R`, and actual replacement `R'`.

The proposal EOS that shortens `R'` consumes its DRAFT ticket and appears in
the next bundle. Rows after it consume no tickets. An accepted current draft
EOS is emitted/materialized, has `target_kind=NONE`, records
`ACCEPTED_DRAFT_EOS`, installs no next bundle, and consumes no BONUS ticket.

## Quality and qualification evidence

Qualification retains both CPU and GPU post-filter supports/probabilities,
not an ambiguously owned single vector. Per scored position it records:

- input logit shard hashes and padded-row validation;
- rank candidate batches or local MASS probability arrays;
- rank-zero merged TOP_K support or four ordered MASS state digests;
- raw weights, masses, normalized binary32 probabilities, and accumulation
  order;
- every ticket, uniform, bounded draw, acceptance ratio, residual/fallback
  mass, selected interval, and token;
- CPU and GPU trace/result digests; and
- full logical-vocabulary target/draft probability arrays outside Git for the
  declared quality corpus.

This evidence distinguishes filtering, normalization, RNG, route, and CDF
errors. Aggregate KLD or frequency means never replace per-position values.

## Failure semantics

In addition to retained v1 failures, the engine fails closed on:

- current input naming an uncommitted or same-step-created proposal bundle;
- any mismatch among proposal count, mask, state length, digest, ticket range,
  or bundle generation;
- a malformed support sentinel, duplicate/out-of-order token, zero selected
  q probability, finite padded-row mass, or divergent replicated TOP_K state;
- any draw, mass, owner, CDF, result, counter, or trace disagreement; or
- any attempt to choose a collective route, fallback, or proposal depth on
  only one rank.

Invalid API parameters, unavailable persisted counter state, and CSPRNG
failure remain request-local before launch. A launched failure commits no
page, output, counter, or proposal state and retires that worker generation.

## Coordinated CPU gate

Only after adversarial acceptance, one coordinated CPU/reference proof must
cover the retained v1 matrix plus:

1. prior-step proposal installation followed by immutable later verification,
   including early EOS at every proposal depth;
2. exact binary32 draw-at-mass and owner-local subtraction boundary cases;
3. `TopKSupportRecord.v2` sentinel/order/probability corruption;
4. TOP_K target/draft support unions of sizes 1, 256, 257, and 512, with
   disjoint, equal, and partially overlapping supports;
5. MASS rank ownership at every vocabulary boundary and all 24 padded rows;
6. every fixed message byte layout, phase, logical payload maximum, and
   route-manifest exclusive-byte formula;
7. zero/fewer-than-k finite candidates without retaining negative infinity;
8. residual-zero fallback, its trace bit, and the nonzero-rate promotion gate;
9. accepted EOS versus output/context no-target trace reasons;
10. C64/MTP6 TOP_K and MASS state byte/arena bounds;
11. digest-domain separation, rank substitution, support mutation, and retry
    transactionality; and
12. the seed-zero/counter-zero known answer and CSPRNG failure before
    admission.

The proof uses an independent slow gathered oracle and adversarial boundary
vectors. Statistical frequency tests supplement but never replace bit-level
known answers.

## Withheld-review closure

| Finding | Corrective section |
|---|---|
| BLOCKER B1 current-step early EOS | Proposal timing and immutable input |
| MAJOR M1 unpinned draw | Counter uniform and bounded draw; exact mass messages |
| MAJOR M2 TOP_K residual | Fixed TOP_K support plus rank-zero sparse-union route |
| minor top-p antecedent | Exact filtering and normalized probabilities |
| minor replay scope | Trace, terminal reason, and replay scope |
| minor phase/message sizes | Exact composite messages |
| minor GPU probability ownership | Quality and qualification evidence |
| minor fewer-finite path | Exact filtering and normalized probabilities |
| minor no-target ambiguity | `no_target_reason` in the common trace |
| question residual fallback | Deliberately retained and promotion-gated |
| question known answer | Canonical SplitMix64 vector |
| question CSPRNG failure | Request-local rejection before admission |

## Gates and nonclaims

Acceptance permits only the coordinated CPU/reference proof. CUDA sampling
may begin only after that proof is accepted and the target layer, recurrent
MTP, step transaction, page transaction, memory plan, and rank executor have
their required accepted identities. Greedy-only work remains independent.

This amendment is not an accepted ABI, CPU proof, CUDA route, layer replay,
checkpoint smoke, quality result, capacity result, concurrency result, or
performance claim. It authorizes no cn4 work.
