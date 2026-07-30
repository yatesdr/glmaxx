# Fable handoff: StepInput/page-delta binding v1

Date: 2026-07-29

Status: adversarial CPU ABI review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`b351e44c2cddf94a12fb8d10f8632119670ca2a9`

Required result path:
`fable-step-input-page-delta-binding-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`step-input-page-delta-binding-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-engine/src/input.rs` | `1ab828cad88f62236aa67962c56d0934b1f0ddce031765455731fbfcbdb0ef61` |
| `crates/glm-engine/src/lib.rs` | `522686301c2f09f8671a881f537534a4f745d28bb9072efae0222a1e7a548f11` |
| `crates/glm-cache/src/delta.rs` | `71ac2da15e869a6f2470c3551a7cd6ec4ff387850a23240e9a44ad96a538ff16` |
| `docs/step-execution-io-v1.md` | `3201d2efe3f7a399acacd4c958327ef1dd03871baa584237ee4cc3a2dbe44671` |
| `docs/step-input-page-delta-binding-proof-v1.md` | `4a2ea5b444d25900b2b2c60c23e0a304ea2d8e69a9461ffaf60b159dcfee447f` |
| `docs/offline-serving-spine.md` | `326e69e038ea4a315803845c06b58bf034f915a4684376632bca6cfb33745554` |
| `docs/production-punchlist.md` | `3033c64b2e1e986fcdadc1393908221f34c60ee31620052cdd069dbde22cbece` |
| `docs/results-index.md` | `653b24e12436e21956f08a553063575e13f9070b56309fbc6f284f9252947438` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-step-input-page-delta-binding-v1-handoff.md
cargo test --offline -p glm-engine input::tests
cargo clippy --workspace --all-targets --offline -- -D warnings
scripts/local-checks.sh
```

## Review boundary

This review covers the immutable CPU input schema, exact row/prompt/sampling
hash, configured/effective MTP distinction, checked context/output bounds,
sampling-schedule binding, and exact canonical page-delta binding. It does
not accept worker or serving integration, mirror/device application,
admission/removal/commit transactions, RNG output commit, fixed-allocation
storage, CUDA, checkpoint execution, model output, quality, capacity under
live tiers, or performance.

## Required adversarial questions

1. Does construction verify the plan, schedule, and page delta before
   computing an input hash?
2. Does the input generation equal both the plan generation and exact delta
   successor, and does the input bind the delta global digest?
3. Does the delta require exactly one update for every unique input row and
   prohibit removals?
4. Does every update bind request ID, configured MTP posture, exact committed
   positions, and the mode-specific tentative reservation?
5. Can an MTP6-configured request legally execute an effective-MTP0 final
   decode while retaining draft page posture?
6. Does VERIFY require effective depth equal the plan while configured depth
   is at least that value?
7. Is every tentative reservation bounded by remaining output capacity?
8. Does checked arithmetic enforce context plus prompt plus maximum remaining
   output at or below exactly 1,048,576?
9. Are prompt offsets contiguous, complete, row ordered, and bound with every
   token ID into the hash?
10. Does the hash unambiguously bind every integer, float bit pattern,
    sampling class, seed, RNG counter, MTP depth, row count, prompt count,
    generation, and delta digest?
11. Are greedy, bounded top-k/top-p, and distributed mass forms canonical,
    with negative zero, non-finite values, and invalid filters rejected?
12. Does decode/verify require one logits collective, the exact plan route,
    and a kind matching every row?
13. Can a tampered hash, different delta, sampling-route mismatch, context
    overflow, or insufficient generation capacity pass either constructor or
    verifier?
14. Do the regressions distinguish row ordering, prompt-token changes,
    configured/effective depth, schedule mismatch, delta mismatch, and all
    sampling classes?
15. Are the 282-test, 67-handoff, formatting, Clippy, FFI, and deterministic
    proof claims reproducible?
16. Are every worker/serving/device/model/quality/capacity/performance
    exclusion and the unresolved `CACHE_ONLY` generation conflict accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- the input hash is complete and canonical;
- plan, schedule, and page delta are mutually bound;
- configured/effective MTP tail behavior is correct;
- context and output reservations fail closed;
- sampling forms and collective routes are exact;
- the regressions distinguish the claimed failures;
- all gate counts are accurate; and
- every integration/device/model/performance exclusion is accurate.

Only if all eight answers are unqualified `YES`, end with the requested
token. Withhold it for a conditional pass, stale input, ambiguous hash,
unbound field, generation reuse, omitted row/delta mutation, illegal
reservation, sampling-route ambiguity, nondistinguishing regression,
incorrect gate count, or overstated integration/device/model claim.

The token accepts only this CPU `StepInput`/page-delta binding. It does not
open cn4, authorize CUDA work, or accept production serving.
