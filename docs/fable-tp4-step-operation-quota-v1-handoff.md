# Fable handoff: TP4 step operation quota ownership v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`da46a30a5df430e35d4a9d23aa6a449923494660`

Required result path:
`fable-tp4-step-operation-quota-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`tp4-step-operation-quota-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-engine/src/worker.rs` | `47206d2ef44fcbaef0cee3a1179605ff811ba7329e09f3493fc4f7a1333d3192` |
| `docs/offline-serving-spine.md` | `27b24d4cbafc8203937d3620e7bcd85d47fcb86cc4d8b89e237025e5d40a62f9` |
| `docs/sm120-rank-runtime.md` | `19638590ee3b42da32bfab7673986c26488da064649c635df895700838da5624` |
| `docs/sm120-rank-executor-v1.md` | `e97c54b865ed50c40ff8b15f6580d0edc18dbd0783135bc1c17d11cc19986fd4` |
| `docs/restore-operation-quota-proof-v1.md` | `6f7fc39db0a7cdc97c3ee9dd51d37b2adaeeb8dd3e087cb4c3fe85ff102a0128` |
| `docs/coordinator-api-backend-v1.md` | `ccfe6a07e5e9327822a3b9708d4119c5797172677d65dc116958f0e9b3378949` |
| `docs/tp4-step-operation-quota-proof-v1.md` | `ab5e025afe5f4c236738ea6658d1bbcd9d7a3eac73fd53d4bf0b1cc4600f2d88` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-tp4-step-operation-quota-v1-handoff.md
cargo test --offline -p glm-engine worker::tests -- --nocapture
cargo clippy --offline -p glm-engine --all-targets -- -D warnings
```

## Review boundary

This review covers only bounded outstanding-operation ownership and
abandoned response handling in the retained CPU TP4 worker pool. It does not
accept host/device cancellation, the pending production executor ABI,
owner-thread resource construction, normative startup, CUDA contexts,
weights, graphs, collectives, checkpoint execution, model output, throughput,
or performance.

## Required adversarial questions

1. Did the prior `StepHandle` decrement the outstanding count on receive,
   timeout, disconnect, or drop even though none of those events cancels a
   queued/running four-rank step?
2. Could handle abandonment therefore admit replacement work while the
   dispatcher and all four rank executors remained inside the original step?
3. Does `try_submit` now move exactly one uncloneable permit into every
   successfully queued `DispatchCommand`?
4. Does a missing dispatcher, saturation, or disconnected `try_send` drop the
   command permit exactly once without manual decrement or counter leak?
5. Does the dispatcher retain the permit through all four rank results,
   output validation, and consensus, then release it before sending the
   response?
6. Can any `StepHandle` method or `Drop` path still decrement the operation
   counter?
7. If the result receiver was abandoned, is the completed result safely
   destroyed without changing rank state or admitting work early?
8. If dispatcher startup or shutdown drops queued commands, do their permits
   release exactly once?
9. Does checked atomic decrement avoid release-build wraparound on an
   impossible double release?
10. Does the five-party barrier regression prove all four rank executors are
    physically active before dropping the handle and testing capacity?
11. Does that test release/drain the workers before asserting, so the former
    implementation fails deterministically without deadlocking teardown?
12. Does the former implementation record zero and accept the replacement,
    while the correction records one, rejects it as saturated, and admits a
    later step only after drain?
13. Does normal receive still expose a result only after the operation count
    reaches zero?
14. Are the 262-test, 56-handoff, CPU-only boundary, retained serial-dispatch
    statement, and all exclusions accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- outstanding count now measures queued/running TP4 operations;
- handle timeout/drop cannot release an active step slot;
- every queue/dispatcher shutdown path releases exactly one permit;
- abandoned results cannot admit replacement work before physical completion;
- the barrier regression distinguishes prior behavior without deadlocking;
  and
- the CPU proof and all exclusions are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, handle-owned decrement,
permit leak/double release, response-before-release ordering, early
replacement admission, nondistinguishing test, or overstated production
claim.

The token accepts only this retained CPU TP4 accounting correction. It does
not open cn4, authorize CUDA work, or accept the production rank executor.
