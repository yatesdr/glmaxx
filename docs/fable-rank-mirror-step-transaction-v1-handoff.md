# Fable handoff: four-rank mirror/step transaction v1

Date: 2026-07-29

Status: adversarial integrated CPU transaction review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`414b8464a298eb749f6bb22e9f56987cc19634e3`

Required result path:
`fable-rank-mirror-step-transaction-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`rank-mirror-step-transaction-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-engine/src/input.rs` | `c3d090429015030416f6c03ddb6fef2dfd569859ff6e0fcc05bcb2d6a163ffa2` |
| `crates/glm-engine/src/worker.rs` | `39a0c0b917921149869d2afc5d652815986bf776eca5ddc9b7abee41b4892652` |
| `crates/glm-engine/src/lib.rs` | `b3ca0da8e0e61f05a92a3b15bc9dc7822395545733ebbdc270c9ff1fb21d6a54` |
| `crates/glm-cache/src/delta.rs` | `71ac2da15e869a6f2470c3551a7cd6ec4ff387850a23240e9a44ad96a538ff16` |
| `crates/glm-serving/src/lib.rs` | `b70cb901a8ef86545342771c09f285e44f9df8eb226cf728809e0aa4d7040a5b` |
| `crates/glm-serving/src/backend.rs` | `c1f9e9d06b44674a1d1d0ef3c24553a9ebe63e913805946bff7c2780233fe94b` |
| `docs/step-execution-io-v1.md` | `055412c022cfcf9299e95e3ad3f7b888a2d472835388c35c2a8443be71a7422c` |
| `docs/serving-page-transaction-v1.md` | `8e2067cc39227bc7acaef82ba71bf887718a8b5a403def32a228517506396dcb` |
| `docs/rank-mirror-step-transaction-proof-v1.md` | `fc36e7820acd6fe34ebb13ddaa62c08d9c8b0dcf2e822ec4ef47b4c098963a8b` |
| `docs/offline-serving-spine.md` | `40eb653baf7b7cf20f054f1234b86de93b4f79509312ded1db330492f9eb974d` |
| `docs/production-punchlist.md` | `d2272e0c88db849a95581019d0a78729538ac4f5457d08553315248d20f3dd0b` |
| `docs/results-index.md` | `5d038591510997577cc2f2cf79302d595f445eb56da762427168ab6eb1d82b4a` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-rank-mirror-step-transaction-v1-handoff.md
cargo test --offline -p glm-engine worker::tests
cargo test --offline -p glm-serving
cargo clippy --workspace --all-targets --offline -- -D warnings
scripts/local-checks.sh
```

## Review boundary

This review covers CPU rank-mirror initialization, admission/removal
synchronization, immutable step input/reservation delivery, exact global and
local rank receipts, post-output commit/rollback/removal, host preflight
rollback, fail-stop rank failure, prompt/sampling retention, and production
removal of the plan-only submission route.

It does not accept CUDA-visible page tables, upload stream dependencies,
fixed-allocation hot-path storage, physical-ID quarantine, cache-only
cleanup, probabilistic RNG output commit, checkpoint execution, model output,
quality, live-tier capacity, or performance.

## Required adversarial questions

1. Must coordinator construction initialize exactly four persistent mirrors
   at the authoritative nonzero generation before returning?
2. Does admission preflight scheduler state, apply one successor delta to all
   mirrors, verify every receipt, and only then publish host state?
3. Can any non-test caller submit a plan without `StepInput` and
   `PageTableDelta`?
4. Do all four commands share the same immutable input/delta allocations?
5. Does each rank independently verify plan, schedule, input, and delta before
   atomically applying the reservation?
6. Does each rank receipt bind plan, schedule, input, global delta, expected
   rank-local delta, output digest, rank, and step ID?
7. Does dispatcher consensus validate the exact rank set, all common fields,
   and each rank-specific local digest without incorrectly requiring local
   digests to equal one another?
8. Does serving construct exact prompt slices in batch-row order and retain
   exact context, configured/effective MTP, limits, sampling bits, seed, and
   RNG counter?
9. Does prefill treat reservation as final page state while decode/verify
   apply a second exact commit/rollback/removal successor?
10. Is scheduler completion preflighted on a clone before the irreversible
    post-output rank delta?
11. Does a late host publication failure issue an explicit successor rollback
    and align host generation before retryable cleanup?
12. Does a worker/consensus failure close the generation, preserve the
    original worker error, and avoid forging a cleanup receipt?
13. Do cancellation and terminal removal reach all mirrors before host page
    and prefix publication?
14. Do initialization and standalone mutation reject overlap with an
    outstanding physical step?
15. Does the bound CPU output actually depend on the input hash/request ID,
    and does the custom serving executor observe exact seed/context on all
    ranks?
16. Does explicit or materialized greedy seed survive HTTP backend admission
    without enabling probabilistic requests prematurely?
17. Are the corruption, late cleanup, 1M tail, MTP fallback, divergence, and
    multi-user regressions still meaningful under the new generation count?
18. Are the 284-test, 68-handoff, formatting, Clippy, FFI, and deterministic
    proof claims reproducible?
19. Are clone allocation, missing device receipt/quarantine/cache-only/RNG
    output, and every model/quality/capacity/performance exclusion accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- mirror initialization and every mutation are generation-exact;
- immutable compute input and reservation are identical across ranks;
- global and local receipts are complete and correctly compared;
- commit, rollback, cancellation, and terminal removal are ordered safely;
- fatal worker errors cannot become false cleanup success;
- prompt/sampling state reaches explicit bound executors exactly;
- all regressions and gate counts are accurate; and
- every device/model/performance exclusion is accurate.

Only if all eight answers are unqualified `YES`, end with the requested
token. Withhold it for a conditional pass, stale input, plan-only production
bypass, missing mirror update, unbound digest, rank-local/global confusion,
partial publication, generation reuse, forged rollback/removal, lost prompt
or seed state, nondistinguishing regression, incorrect gate count, or
overstated device/model claim.

The token accepts only this integrated CPU mirror/step transaction. It does
not open cn4, authorize CUDA work, or accept production serving.
