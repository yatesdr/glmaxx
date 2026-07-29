# Fable handoff: selected-step failure finalization v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`2ff0ac124be63a8a8318d664d167f34dde32ed3c`

Required result path:
`fable-selected-step-failure-finalization-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`selected-step-failure-finalization-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-serving/src/lib.rs` | `1aee2f7fccac9c79124edaeeef0f9759faed2bc621b3e7228ce9914eb4e432d6` |
| `crates/glm-scheduler/src/lib.rs` | `5a820b2e5013f038f07b26f14ddc24d69d00e18d3d55837ab5ff3a68daee3074` |
| `crates/glm-engine/src/worker.rs` | `400c7c22f2c74d3f386fefe2d144da3437f27e6c99ce9f2d4bbf87ffe98fe437` |
| `docs/offline-serving-spine.md` | `27b24d4cbafc8203937d3620e7bcd85d47fcb86cc4d8b89e237025e5d40a62f9` |
| `docs/serving-page-transaction-v1.md` | `e3a9a1d9f2eb26dc5312d7c42297fa3d832e444f7e3f269094746a85fb3deac2` |
| `docs/selected-step-failure-finalization-proof-v1.md` | `36be571d84cff086ad3058f3426fc0fee6bdd4d33b1c4317473128e4d861512e` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-selected-step-failure-finalization-v1-handoff.md
cargo test --offline -p glm-serving
cargo clippy --offline -p glm-serving --all-targets -- -D warnings
```

## Review boundary

This review covers only CPU failure finalization after scheduler selection
and before successful step completion, including compile, observation,
worker submit/receive, output, and completion-validation errors. It does not
accept post-success multi-request cleanup atomicity, the wider serving-page
transaction, rank execution, CUDA, direct tier I/O, checkpoint execution,
model output, or performance.

## Required adversarial questions

1. Does `Scheduler::next_batch` install `self.inflight` before returning a
   selected batch, making every later immediate error return responsible for
   consuming that state?
2. Did the prior graph, compiler, collective-count, progress, worker-submit,
   and successful-completion `?` paths return with the batch still inflight?
3. Does every reachable error after selection and before successful
   completion now call `fail_selected_step`, including both worker
   submission and worker receive?
4. Does `fail_selected_step` call atomic `complete_batch(false)` before any
   fallible prefix/token cleanup or failed-event emission?
5. Can graph lookup, compilation, observation, or worker submission remove a
   selected scheduler request before failure completion? If so, the claimed
   invariant is false.
6. Does `complete_batch(false)` reject completions, preflight every selected
   request, remove the exact inflight batch, and mark all selected requests
   failed in one scheduler commit?
7. If cache cleanup or failed-event publication later errors, is the
   scheduler batch nevertheless no longer inflight?
8. Is event capacity reserved before selection by 512 entries while a legal
   batch has at most 64 rows, making event-capacity failure unreachable on
   this path?
9. Does worker saturation happen before a command is accepted, making it
   correct to finalize the selected rows without an unknown in-flight device
   execution?
10. On a worker receive error, is process-fatal selected-row failure still
    consistent with collective-safety requirements?
11. If atomic successful-completion validation rejects the worker result,
    can the same unchanged inflight batch be safely completed as failed?
12. Does the compile regression force a real post-selection compiler error,
    prove the request and event are terminal, and prove the next tick is idle
    rather than `Inflight`?
13. Does the submit regression hold the only bounded worker slot, force
    `Worker(Saturated)` after selection, and prove the same terminal and
    next-tick properties?
14. Do both regressions fail on the prior code for the claimed stranded-batch
    reason?
15. Are the 239-test claim, 39-handoff claim, open cross-request cleanup
    caveat, and every GPU/model/performance non-claim accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- every ordinary pre-completion error consumes the selected scheduler batch;
- selected rows become failed before fallible resource cleanup;
- no worker-submit error can coexist with an accepted device command;
- cache/event cleanup failure cannot strand scheduler inflight state;
- both regressions distinguish the prior defect; and
- the CPU proof and all scope exclusions are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, any selected-step error that
retains inflight state, a post-submit ambiguity, partial scheduler failure,
a regression that cannot distinguish the defect, or an overstated proof.

The token accepts only this CPU correction. It does not open cn4, authorize
the pending prefill ABI implementation, or accept real model execution.
