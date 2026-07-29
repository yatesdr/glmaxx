# Distributed sampling ABI v1

Date: 2026-07-29

Status: design candidate; adversarial review required before ABI promotion

GPU evidence: none

## Scope

This contract freezes the GLM-5.2-only sampling boundary for:

- sharded-vocabulary greedy sampling;
- exact bounded top-k then top-p sampling;
- unfiltered distributed categorical sampling;
- MTP draft proposal, acceptance, residual, and bonus draws;
- deterministic seed and counter continuation; and
- full-vocabulary quality evidence without a production logits gather.

It does not claim a GPU implementation, stochastic equivalence, or model
quality result.

## Current blocking gaps

The current CPU oracle is useful but not a production ABI:

1. the API permits noncanonical greedy tuples such as zero temperature with
   nontrivial top-k/top-p;
2. an omitted seed remains optional rather than being materialized once;
3. `RequestSpec` retains only a collective class and discards exact sampling
   parameters;
4. `StepOutput` has no RNG counter continuation or sampling provenance;
5. the existing `SamplingCounter` allocates generic sequential tickets but
   does not define the MTP control-flow schedule;
6. collective route byte fields do not define gather/reduction/broadcast
   sub-operations;
7. the fixed 154,856-token vocabulary and 24 padded LM-head rows are not
   validated by the generic shard oracle; and
8. no retained trace proves the actual filtered target/draft distributions
   used for acceptance and residual sampling.

The backend correctly rejects probabilistic requests until these gaps close.

## Vocabulary partition

The logical output vocabulary is exactly 154,856 tokens. The checkpoint head
has 154,880 physical rows, evenly partitioned over TP4:

```text
rank 0 physical/logical  [0,      38,720)
rank 1 physical/logical  [38,720, 77,440)
rank 2 physical/logical  [77,440,116,160)
rank 3 physical          [116,160,154,880)
rank 3 logical           [116,160,154,856)
padded invalid IDs       [154,856,154,880)
```

The final 24 rows are set to negative infinity before any local maximum,
candidate selection, mass, or probability operation. A padded ID in a
candidate or result is fatal. Rank intervals are contiguous, immutable, and
part of the sampling ABI hash.

## Canonical request parameters

The coordinator materializes:

```text
SamplingConfig.v1 {
    kind: GREEDY | TOP_K | MASS
    temperature_bits: u32
    top_p_bits: u32
    top_k: u16
    seed: u64
    initial_counter: u64
}
```

Valid forms are:

| Kind | Temperature | top-k | top-p |
|---|---:|---:|---:|
| greedy | canonical `+0.0` | 0 | canonical `1.0` |
| bounded | finite `0 < t <= 2` | `1..=256` | finite `0 < p <= 1` |
| mass | finite `0 < t <= 2` | 0 | canonical `1.0` |

Negative zero, NaN, infinity, explicit API `top_k=0`, and noncanonical greedy
filters are rejected. API omission maps to internal `top_k=0`.

A caller-supplied seed, including zero, is used exactly. If omitted, a
process CSPRNG materializes one seed before scheduler admission. The
effective seed is retained in the request trace and returned in a
`x-glmaxx-effective-seed` response header so an unseeded request can be
replayed. Seed generation is never rank-local.

The initial counter is zero for a new request. A resumed persisted session
must restore its exact seed and committed counter or fail closed.

## Counter-based uniform

Version one retains the existing SplitMix64-derived mapping:

```text
gamma = 0x9e3779b97f4a7c15
x = seed + (counter + 1) * gamma                 wrapping u64
x = (x ^ (x >> 30)) * 0xbf58476d1ce4e5b9       wrapping u64
x = (x ^ (x >> 27)) * 0x94d049bb133111eb       wrapping u64
x = x ^ (x >> 31)
u = f64(x >> 11) * 2^-53
```

`u` is in `[0,1)`. Integer operations are bit-exact. The integer-to-FP64
conversion and multiplication use round-to-nearest-even. No fast-math,
contraction, or rank-specific RNG library is permitted.

One ticket consumes exactly one counter. `counter_after = counter_before + 1`
with checked arithmetic. The purpose and logical coordinates are retained in
the trace even though version one obtains the random word from the sequential
counter:

```text
request_id
logical_position
draft_step: 0..6
purpose: TARGET | DRAFT | ACCEPTANCE | RESIDUAL | BONUS
counter_before
counter_after
```

Request ID is trace identity, not RNG entropy. Two requests with the same
seed, history, and parameters intentionally reproduce the same sampling
sequence.

## Filter order and distribution

For every target or draft row:

1. mask the 24 padded IDs and any separately reviewed forbidden-token mask;
2. divide finite FP32 logits by temperature;
3. apply exact global top-k when `top_k > 0`;
4. within the retained set, keep the smallest deterministic logit-ordered
   prefix whose cumulative unnormalized mass reaches `top_p * total_mass`;
5. normalize the retained FP32 masses.

No frequency, presence, repetition, min-p, typical, logit-bias, or other
history penalty is supported in v1. Adding one changes the ABI and applies
identically to target and draft distributions before acceptance.

Candidate order is descending FP32 `totalOrder` logit, then ascending global
token ID. Exact ties therefore select the lower ID. Negative infinity is
never retained when enough finite valid candidates exist. Any NaN, positive
infinity, all-masked row, nonpositive/nonfinite mass, duplicate token, or
partition gap fails the entire step.

## Deterministic numerical order

Top-k filtering is decided on rank zero after candidates arrive in rank order.
Weights and cumulative sums are evaluated in final candidate order using
FP32 round-to-nearest-even operations.

For MASS:

1. each rank computes its local finite maximum;
2. rank-order reduction selects the global maximum;
3. each rank computes `exp((logit-global_max)/temperature)` and accumulates
   FP32 mass in ascending local token order;
4. rank zero sums the four local masses in rank order;
5. one uniform selects a rank interval in rank order; and
6. that owner walks its local CDF in ascending token order.

No floating-point atomic or tree whose order changes with occupancy is
allowed. The qualified CUDA exponent implementation and compiler flags are
part of the sampling-kernel evidence. CPU and CUDA distributions are compared
over full vocabulary; probabilistic qualification is distributional rather
than a promise that unrelated libm implementations choose the same
boundary-adjacent token.

## Composite collective routes

`StepPlan.sampling_route_id` names a reviewed composite route with fixed
sub-ordinals. A rank may not select a sub-route locally.

Wire records are:

```text
GreedyCandidate { logit: f32, token_id: u32 }             8 bytes
TopKCandidate   { logit: f32, token_id: u32 }             8 bytes
MassState       { maximum: f32, mass: f32 }               8 bytes
ProbabilityAtToken { target: f32, draft: f32 }            8 bytes
SamplingResult  {
    token_id: u32,
    purpose: u8,
    draft_step: u8,
    reserved: u16,
    counter_after: u64,
}                                                         16 bytes
```

GREEDY sub-ordinals:

1. gather one candidate per rank and row to rank zero;
2. choose maximum/lower-ID tie; and
3. broadcast one result per row.

TOP_K sub-ordinals:

1. gather exactly `min(top_k, local_valid_tokens)` candidates per rank and
   row to rank zero in rank/local-candidate order;
2. fixed-order global merge, top-p filter, and draw; and
3. broadcast one result per row.

At `top_k=256`, rank zero receives at most 1,024 candidates or 8,192 logical
bytes per row, plus the 16-byte result. Route manifests record exclusive
wire bytes for the actual PCIe algorithm rather than calling buffer size
transport.

MASS sub-ordinals:

1. reduce/gather local maxima;
2. gather local masses after the global maximum is known;
3. choose the rank interval and send its local draw to the owner;
4. owner CDF selection; and
5. broadcast the result.

Residual and bonus routes reuse these primitives over post-filter
probabilities. Full-vocabulary logits or probabilities never cross ranks.

Requests with the same collective class and MTP depth may batch despite
different temperature, top-k, top-p, seed, and counter values. Buffers use
the class maximum and every row carries its exact immutable parameters.

## MTP probabilistic schedule

Let the admitted depth after context/output clamping be `K`, `1..=6`. Draft
recurrence proposes `R <= K` tokens, stopping early if it samples EOS.

### Proposal

Each proposed token consumes one `DRAFT` ticket in recurrence order. All
proposal draws occur before target acceptance. If target verification later
rejects at position `i`, already generated proposal tickets after `i` remain
consumed because that draft work actually occurred.

The draft runner retains, for every proposal:

- token ID;
- its normalized `q_i(token)` probability;
- the post-filter draft support descriptor/digest; and
- its draft ticket.

### Acceptance

Target verification constructs the actual post-filter `p_i` for each proposed
position. Every examined proposal consumes one `ACCEPTANCE` ticket, including
ratios zero or one:

```text
ratio = min(1, p_i(d_i) / q_i(d_i))
accept iff uniform < ratio
```

`q_i(d_i)` must be finite and positive because `d_i` was sampled from that
distribution. `p_i(d_i)` may be zero. Ratio arithmetic and comparison are
FP64 round-to-nearest-even from the retained normalized probabilities.

Acceptance stops at the first rejection.

### Rejection and residual

At the first rejected position, one `RESIDUAL` ticket samples normalized:

```text
r_i(token) = max(p_i(token) - q_i(token), 0)
```

Both arrays are their actual post-filter full-vocabulary distributions.
Ranks compute local residual masses in ascending token order, rank zero sums
them in rank order, and the selected rank walks its local CDF.

If numerical residual mass is zero, the v1 fallback samples `p_i` with the
same residual ticket and records `residual_fallback=true`. The quality gate
counts every fallback; any nonzero production rate requires review.

The residual token is the one target token in `CommittedTokens`. Remaining
draft proposals and tentative KV are rejected.

### All accepted and bonus

If all `R` proposals are accepted, one `BONUS` ticket samples `p_R` only when:

- the final accepted proposal is not EOS;
- at least one output token remains under the request limit; and
- at least one model position remains.

An accepted draft EOS has no target token and consumes no bonus ticket. A
limit-clamped all-accepted result also consumes no bonus ticket.

### Counter bounds

For a non-EOS `R`-proposal cycle:

```text
rejection at zero-based i:  R proposal + (i+1) acceptance + 1 residual
all accepted with bonus:    R proposal + R acceptance + 1 bonus
maximum:                    2R + 1 <= 13
```

Accepted draft EOS at proposal `R-1` consumes `R` proposal plus `R`
acceptance tickets and no bonus. Before dispatch, the coordinator proves
`counter_before + (2K+1)` cannot overflow.

Greedy MTP consumes no RNG tickets. It uses authoritative target argmaxes,
accepts equal draft IDs, emits the first target mismatch, and emits a bonus
argmax only when the same EOS/output/context rules allow it.

## MTP row masking

Graph query rows remain bucketed at `depth+1` per active sequence. Each
`SequenceInput` additionally supplies the actual proposal count and valid
verify-row mask. Rows after early draft EOS or context/output clamping:

- perform no sampling;
- write no committed/tentative KV;
- consume no counter; and
- contribute no collective payload beyond fixed graph padding.

Masks and proposal counts are covered by the immutable input hash and
identical on all ranks.

## Transaction and output ABI

`StepOutput.v2` adds, per sequence:

```text
rng_counter_before: u64
rng_counter_after: u64
proposal_count: u8
accepted_draft_count: u8
target_kind: NONE | MTP0 | GREEDY_MISMATCH | RESIDUAL | BONUS
residual_fallback: bool
sampling_trace_digest: [u8; 32]
```

The existing committed token IDs and target-present bit remain.

Every rank acknowledges identical plan, input, output, and trace digests.
Only after four-rank consensus does the coordinator atomically commit:

- emitted tokens and scheduler progress;
- target/draft KV and indexer positions;
- the final RNG counter; and
- the committed token-chain entries used by prefix publication.

A worker, collective, output, or consensus failure commits none of them.
Retrying the same immutable step therefore reuses the same seed/counter and
draws.

Greedy rows require `counter_after == counter_before`. MTP0 probabilistic
rows require exactly one target draw. MTP rows validate the exact control-flow
formula above rather than only a monotonic range.

## Trace and quality evidence

The hot-path trace digest covers:

- sampling config bits and vocabulary ABI;
- each ticket;
- candidate/support digests;
- selected token and normalized probability;
- for MTP, `p(d)`, `q(d)`, ratio, acceptance uniform, residual mass,
  fallback, and target kind; and
- final counter.

Qualification mode writes external per-position records containing:

- full valid-vocabulary target logits and post-filter probabilities;
- full draft probabilities for proposed positions;
- CPU/reference and GPU selected tokens;
- top candidates, margins, KLD, total-variation error, and normalization
  error;
- MTP acceptance decision and accepted length; and
- exact seed/counter tickets.

These large records remain outside Git and are pinned by hash in the result
index. Aggregate means never replace per-position values.

Greedy MTP stable/tie-adjacent thresholds remain a separate measured quality
artifact. MTP1 cannot be enabled merely because the sampling mechanics pass.

## Failure semantics

Engine-fatal:

- vocabulary partition or padding-mask mismatch;
- rank plan/input/trace/result disagreement;
- NaN, positive infinity, invalid mass, duplicate/gapped candidate IDs;
- a padded/out-of-range selected token;
- counter reuse, regression, overflow, or impossible final counter; or
- a sampling collective timeout/failure.

Request-local before launch:

- invalid/canonicalization-failing API parameters;
- unavailable persisted seed/counter;
- unsupported filter or history-penalty request; or
- insufficient remaining output/context for the requested depth after legal
  clamping.

No rank falls back from top-k to mass, changes depth, changes filter order, or
selects a different RNG path locally.

## Required adversarial and CPU gates

Adversarial review must accept the RNG mapping, exact MTP ticket schedule,
accepted-EOS/limit rules, residual fallback, numerical order, route messages,
and `StepOutput.v2` fields before implementation.

After acceptance, CPU proof covers:

1. all canonical and invalid API tuples, including negative zero;
2. supplied/omitted seed materialization and persisted continuation;
3. every counter formula at MTP0–6, rejection positions, EOS, and limits;
4. RNG known-answer vectors at boundary seeds/counters;
5. exact 154,856/154,880 partition and padding rejection;
6. greedy ties across every rank boundary;
7. top-k `1,2,255,256`, top-p boundaries, ties, and fewer finite candidates;
8. mass sampling versus a gathered fixed-order control for every counter
   band;
9. target/draft filtering, acceptance ratios zero/one/interior, and strict
   comparison boundaries;
10. residual distributions with disjoint/equal/partially overlapping
    supports and zero-mass fallback;
11. bonus, accepted draft EOS, early draft EOS, context clamp, and output
    clamp;
12. mixed row parameters within one route class;
13. exact composite message/wire-byte arithmetic;
14. failed/retried step counter transactionality;
15. full-vocabulary retained vectors and trace-digest reproducibility; and
16. statistical frequency tests with declared confidence bounds in addition
    to deterministic known-answer tests.

CUDA qualification then compares every required row bucket and MTP depth to
the CPU/gathered controls, profiles each composite sub-route, and retains
per-position model evidence.

## Required contract amendments

Acceptance requires coordinated amendments to:

- `StepInput.v1` for actual proposal count, verify-row mask, and exact config;
- `StepOutput` for counter and sampling provenance;
- scheduler route manifests for composite sub-ordinals and maximum bytes;
- HTTP response metadata for effective seed;
- serving observability for purpose/counter/fallback/route metrics; and
- the quality contract for full-vocabulary sampling evidence.

No part of this document authorizes cn4 access, a GPU launch, or
probabilistic production serving.
