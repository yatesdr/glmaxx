# Fable adversarial review — distributed greedy all-masked rejection v1 (queue row 15)

Date: 2026-07-31
Handoff: `docs/fable-distributed-greedy-all-masked-v1-handoff.md`

Location note: the operator directed queue reviews into `docs/reviews/`
instead of the repository root; this artifact is the required result for the
handoff's declared `fable-distributed-greedy-all-masked-v1.md`.

## Reviewed candidate

7867ed2e3839d74aad83f8b504bf5000247838b6

Reviewed in a pre-existing detached worktree at exactly this commit
(`scratchpad/wt-lps-7867ed2`); moving `main` was not consulted for any input.

## Verified input hashes

Every pinned input was hashed with `shasum -a 256` in the detached worktree at
review START and again at review FINISH; both passes matched the handoff table
exactly. `glmaxx review-proof` on this handoff also returned `PASS`.

| Input | SHA-256 (verified start and finish) |
|---|---|
| `spec/engine-v0.md` | efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a |
| `crates/glm-reference/src/sampling.rs` | 3205f2b11d5253c51176434337be8a3e4738a1cc84a4f2d16975248d816edfb5 |
| `docs/distributed-sampling-abi-v1.md` | d717508e4d90f6ef378d486c0bd3e93e7dad522e6529b8504ccb687a0280fdce |
| `docs/fable-distributed-sampling-abi-v1-handoff.md` | 4d4bb431eabc9c48435d8ed19cfdd2532a780224d0fb79c7e69ba2bbae7058c6 |
| `docs/quality-acceptance-v1.md` | 3f87cd128b633d6812dce31fb6f3bfbd700debae587a32350e0cb46e24a6e1e9 |
| `docs/distributed-greedy-all-masked-proof-v1.md` | c6fd7efa937a1cee875621a190b8886153eddc9d323ee65bf7d406a5a8df9b46 |
| `scripts/local-checks.sh` | 839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f |

## Commands executed in the worktree

- `cargo test --offline -p glm-reference sampling::tests` — 7 passed, 0
  failed (includes `greedy_rejects_an_all_masked_row_but_accepts_masked_ranks`).
- `cargo clippy --offline -p glm-reference --all-targets -- -D warnings` —
  clean.
- Full workspace `cargo test --offline` — 266 tests passed, 0 failed,
  matching the proof document's claimed 266 exactly.
- `docs/fable-*-handoff.md` at the candidate: 62 files; minus the two
  historical umbrella handoffs that `review-proof-all` skips = the claimed 60
  provenance-proof handoffs.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

- m1: The regression exercises a 32-token synthetic vocabulary (8 tokens per
  rank), not the production partition (154,880 physical / 154,856 logical /
  24 padded rows). The padded-ID-fatal rule at production scale is contract
  text in `docs/distributed-sampling-abi-v1.md` (lines 50–61, 148–152), not a
  test here. Acceptable for a reference-route correction; noted for the later
  production sampling gate.
- m2: `SamplingError::Logit` is shared between the all-masked rejection and
  other logit-validity failures; an operator cannot distinguish "globally
  all-masked row" from other logit faults from the variant alone. The ABI
  only requires the step to fail, so this is observability polish, not a
  defect.

### QUESTION

- q1: When the pending stochastic ABI lands, should a dedicated
  `AllMasked`-style enumerant be introduced so trace records can attribute
  the engine-fatal cause? (No action required for this token.)

## Answers to the 12 required questions

1. Yes. `docs/distributed-sampling-abi-v1.md` sets the final 24 padded rows
   to `-∞` before any local maximum/candidate/mass operation and declares an
   all-masked row a whole-step failure ("Any NaN, positive infinity,
   all-masked row, … fails the entire step").
2. Yes. `validate_logit_shards` (sampling.rs:343–364) rejects NaN and
   `+∞` (`value.is_nan() || *value == f32::INFINITY`) while permitting `-∞`,
   plus rank-count/order, `first_token` contiguity, non-empty shards, and
   checked u32 width accumulation.
3. Yes. The former route merged per-rank winners and returned `best.token`
   with no finiteness check on the merged winner (per the proof document and
   the shape of the current diff; the added lines 122–125 are the only
   finiteness gate on the greedy path).
4. Yes. With every logit `-∞`, all four local winners are `-∞`; ties resolve
   to the lowest token (`candidate_order` line 405–409: `total_cmp` then
   reversed token compare), and cross-shard replacement requires strictly
   `Greater`, so shard 0's token 0 survived — the old route returned token 0.
5. Yes. Any per-shard or pre-merge finiteness requirement would reject the
   legal case where one rank's whole partition is masked but another rank
   owns a finite winner; only the post-merge global winner check preserves
   it. Verified by the first regression assertion (three fully masked ranks,
   rank 2 owns finite token 17, result 17).
6. Yes. sampling.rs:122–125: after the merge, `if !best.logit.is_finite()`
   returns exactly `Err(SamplingError::Logit)`. Since NaN and `+∞` are
   pre-rejected as `Shard`, the branch fires iff the global winner is `-∞`,
   i.e. iff every logit in the row is `-∞`; a single finite logit anywhere
   wins under `total_cmp` and returns `Ok`.
7. Yes. Test lines 490–493: `vec![f32::NEG_INFINITY; 32]` with only
   `logits[17] = -3.0`; token 17 lies in rank 2's `[16,24)` slice, ranks 0,
   1, 3 fully masked; asserts `Ok(17)`.
8. Yes. Lines 495–499 re-mask token 17 and require exact
   `Err(SamplingError::Logit)`; the former implementation returned `Ok(0)`
   for the identical input, so the assertion is distinguishing, not
   tautological.
9. Yes. `greedy_ties_choose_the_lowest_global_token` (equal 9.0 at tokens 7
   and 24 across ranks) still passes and `candidate_order` is byte-identical
   to the ABI rule (descending FP32 totalOrder, then ascending token ID).
10. Yes. Bounded top-k fails all-masked via `sample_candidates`
    (nonfinite weight → `Logit`, sampling.rs:304–306; nonpositive/nonfinite
    total → `Probability`, 310–311); the unbounded mass route rejects a
    nonfinite global maximum (`!maximum.is_finite()` → `Logit`, 254–255) and
    nonpositive mass (267–268). The greedy correction aligns greedy with the
    already-fail-closed routes rather than introducing a new policy.
11. Yes. `distributed_greedy` takes no seed, counter, or ticket — greedy
    consumes no RNG state — and no stochastic, MTP, residual, or bonus code
    path changed. Cross-reference: queue row 16
    (`docs/reviews/fable-distributed-sampling-abi-v1.md`) was withheld with
    one BLOCKER (B1, early-draft-EOS mask vs. immutable input hash) and two
    MAJORs (M1 draw representation, M2 TOP_K residual); all three live in
    the stochastic/dispatch domain and are untouched and unprejudiced by
    this correction. That artifact explicitly permits greedy-only work to
    proceed independently.
12. Yes. Recounted: 266 workspace tests pass at the candidate; 62
    `fable-*-handoff.md` files minus the two skipped historical umbrellas =
    60 provenance handoffs; no CUDA, GPU, collective, checkpoint, model, or
    HTTP execution occurs in any exercised path; the proof document's
    exclusions match the repository state.

## Separate statements required by the handoff

- Globally all-masked greedy rows now fail closed: YES.
- Rank-local masked partitions remain legal: YES.
- The regression distinguishes the former token-0 behavior: YES.
- Finite cross-rank tie behavior remains unchanged: YES.
- No pending stochastic or MTP sampling gate is opened: YES.
- The CPU proof and all exclusions are accurate: YES.

## Architecture & maintainability

The correction is minimal and in the right place: one post-merge finiteness
gate rather than per-shard policy, which keeps the legal
fully-masked-partition case structural instead of special-cased. The route
now matches the top-k and mass routes' fail-closed posture, so all three
sampling paths share one invariant (a nonfinite global decision is fatal).
The shared `Logit` variant and the synthetic 32-token test vocabulary are the
only debts, both cosmetic at this layer. The reference remains a pure
function of its shards — no RNG, no I/O — which is exactly what the pending
stochastic ABI review needs to stay independent of this token.

## Token decision

All six required answers are unqualified YES; no blocker or major exists.
The token accepts only this CPU greedy fail-closed correction; it does not
open cn4, authorize CUDA work, or accept distributed sampling for production.

distributed-greedy-all-masked-v1-accepted
