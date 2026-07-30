# StepInput/page-delta binding CPU proof v1

Date: 2026-07-29

Implementation and contract commit:
`d1f98257ebf83d03c5e5e56ab2d2c1ce5404ac35`

Status: CPU ABI passed; independent review pending

GPU claim: none

## Implemented boundary

`glm-engine` now defines `glmaxx.step-input.v1`, an immutable input object
that binds the exact rank-visible work for one prefill, decode, or verify
step. It contains:

- the nonzero successor page-table generation;
- the canonical `PageTableDelta.v1` global digest;
- one ordered row per active sequence;
- exact prompt token IDs in contiguous row order; and
- a domain-separated SHA-256 over every field.

Each row binds request identity, context and generated counts, maximum output
count, prompt range, configured and effective MTP depths, sampling class,
exact float bits, top-k, materialized seed, and RNG counter before the step.

Construction and verification both revalidate the `StepPlan`, collective
schedule, and page delta. The plan generation must equal the delta successor
and input generation. The delta must contain exactly one update per row and
no removals. Each update must bind the row's request ID, configured MTP
posture, exact committed count, and zero, one, or `effective_depth + 1`
tentative positions according to mode.

Decode and verify require exactly one logits collective whose kind maps to
every row's sampling kind and whose route equals the plan route. Prefill
requires no logits collective.

## Tail and context correction

The original design had one `mtp_depth`. Implementation of the reviewed
scheduler tail clamp showed that this is insufficient: an MTP6-capable
request can require an MTP0 final decode when one output token remains.
`configured_mtp_depth` therefore binds target-plus-draft page posture, while
`effective_mtp_depth` binds the selected graph and reservation for this step.

Every row uses checked arithmetic to prove:

```text
context_before + prompt_this_step + maximum_remaining_output <= 1,048,576
reservation_count <= maximum_remaining_output
```

This prevents an otherwise valid MTP graph from reserving more positions than
the response can commit.

## Sampling validation

The canonical forms are:

- greedy: temperature `+0`, top-p `1`, top-k `0`, counter `0`;
- top-k: finite temperature above zero, finite top-p in `(0,1]`, and top-k
  in `1..=256`;
- mass: finite temperature above zero, top-p exactly `1`, and top-k `0`.

Negative zero, NaN, infinity, zero/out-of-range filters, route mismatch, and
mixed decode/verify sampling classes fail closed. The exact seed and counter
are hashed even when the sampling class does not consume a counter in the
current step.

## CPU regressions

The four new tests prove:

1. deterministic multi-row prefill hashing, arbitrary batch row order,
   contiguous prompt concatenation, prompt-token sensitivity, and exact
   page-delta binding;
2. configured MTP6 with effective MTP0 and an exact one-position tentative
   tail reservation;
3. MTP5 verification with a six-position reservation, top-k schedule
   binding, hash/delta/context/route/output-capacity rejection; and
4. the valid form of all three sampling classes plus invalid counter,
   NaN, zero top-k/top-p, and noncanonical mass-filter cases.

## Gate result and exclusions

`scripts/local-checks.sh` passed:

- 282 Rust tests with zero failures;
- workspace formatting;
- workspace Clippy with warnings denied;
- CUDA FFI type checks;
- deterministic CPU proof regeneration; and
- all 67 then-present review-handoff provenance proofs.

The external tokenizer proof was skipped because `GLMAXX_TOKENIZER_DIR` was
unset. CUDA compilation was skipped because `nvcc` is not installed on this
CPU host.

Relevant hashes:

```text
crates/glm-engine/src/input.rs
1ab828cad88f62236aa67962c56d0934b1f0ddce031765455731fbfcbdb0ef61

crates/glm-engine/src/lib.rs
522686301c2f09f8671a881f537534a4f745d28bb9072efae0222a1e7a548f11

crates/glm-cache/src/delta.rs
71ac2da15e869a6f2470c3551a7cd6ec4ff387850a23240e9a44ad96a538ff16

docs/step-execution-io-v1.md
3201d2efe3f7a399acacd4c958327ef1dd03871baa584237ee4cc3a2dbe44671

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f
```

This proof does not claim worker dispatch, serving construction, rank-mirror
application, admission/removal synchronization, post-output commit deltas,
device upload acknowledgment, fixed-allocation hot-path storage, RNG output
commit, CUDA execution, checkpoint execution, model quality, capacity under
live tiers, or performance. `MIXED` and `CACHE_ONLY` remain rejected by this
object pending reviewed transaction contracts.
