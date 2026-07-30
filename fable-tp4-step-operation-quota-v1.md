# Fable review: TP4 step operation quota ownership v1

Date: 2026-07-30
Reviewer: Fable (adversarial design-gate reviewer)
Queue row: 77
Handoff: `docs/fable-tp4-step-operation-quota-v1-handoff.md`
Reviewed candidate commit: `da46a30a5df430e35d4a9d23aa6a449923494660`
Review location: detached worktree at the pinned commit (never moving main).

Note: the operator directed review artifacts into `docs/reviews/` rather than
the repository root named in the handoff.

## Provenance

All input hashes were verified twice: once at review start (via
`git show <commit>:<path>` against the main repository object store, before
worktree creation) and once at review finish (shasum of worktree bytes).
Both sets matched the handoff table exactly.

| Input | SHA-256 (matched handoff exactly) | Start | Finish |
|---|---|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` | OK | OK |
| `crates/glm-engine/src/worker.rs` | `47206d2ef44fcbaef0cee3a1179605ff811ba7329e09f3493fc4f7a1333d3192` | OK | OK |
| `docs/offline-serving-spine.md` | `27b24d4cbafc8203937d3620e7bcd85d47fcb86cc4d8b89e237025e5d40a62f9` | OK | OK |
| `docs/sm120-rank-runtime.md` | `19638590ee3b42da32bfab7673986c26488da064649c635df895700838da5624` | OK | OK |
| `docs/sm120-rank-executor-v1.md` | `e97c54b865ed50c40ff8b15f6580d0edc18dbd0783135bc1c17d11cc19986fd4` | OK | OK |
| `docs/restore-operation-quota-proof-v1.md` | `6f7fc39db0a7cdc97c3ee9dd51d37b2adaeeb8dd3e087cb4c3fe85ff102a0128` | OK | OK |
| `docs/coordinator-api-backend-v1.md` | `ccfe6a07e5e9327822a3b9708d4119c5797172677d65dc116958f0e9b3378949` | OK | OK |
| `docs/tp4-step-operation-quota-proof-v1.md` | `ab5e025afe5f4c236738ea6658d1bbcd9d7a3eac73fd53d4bf0b1cc4600f2d88` | OK | OK |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` | OK | OK |

Note: the pinned commit is no longer at main HEAD — main has moved — so the
review was performed against the pinned detached worktree only.

Commands executed in the pinned worktree, all passing:

- `cargo run --offline -p glm-cli --bin glmaxx -- review-proof docs/fable-tp4-step-operation-quota-v1-handoff.md` — exit 0, `glmaxx.review-provenance-proof.v2` emitted.
- `cargo test --offline -p glm-engine worker::tests -- --nocapture` — 7 passed, 0 failed.
- `cargo clippy --offline -p glm-engine --all-targets -- -D warnings` — clean.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. `worker.rs:124` — the underflow detection for an impossible double
   release is `debug_assert!` only; a release build silently ignores the
   failed `fetch_update`. This is the intended fail-safe direction (checked
   decrement cannot wrap; see Q9) but a production counter-corruption signal
   is lost. Acceptable for the retained CPU pool; the production executor
   contract should surface it.
2. `worker.rs:252-268` — after any failed step the dispatcher `break`s and
   the pool is permanently closed, but `Tp4WorkerPool::outstanding()` and
   `try_submit` still allow a caller to reserve a slot whose command can only
   ever return `Closed` (permit is correctly released via command drop). Not
   a quota bug; a small ergonomics wart on the fail-stop path.

### QUESTION

1. The quota and the dispatch channel capacity are both
   `maximum_outstanding`; `reserve_slot` is the authoritative gate and
   `TrySendError::Full` should be unreachable in practice. Intentional
   double-bounding? (No safety impact; `Full` maps to `Saturated` and the
   permit is released exactly once via command drop.)

## Answers to the handoff's required questions

1. YES. The prior implementation (parent lineage `e52ce5b~`, verified via
   `git show`) had `StepHandle { outstanding, released }` with `release()`
   called from `receive`, `receive_timeout`, and `Drop` — none of which
   cancels a queued/running four-rank step.
2. YES. Under the prior code, dropping the handle decremented the counter to
   zero while the dispatcher and all four rank executors were still inside
   the step, so `try_submit` would admit replacement work.
3. YES. `try_submit` (worker.rs:179-205) creates exactly one
   `OutstandingPermit` (no `Clone` impl, private type) after `reserve_slot`,
   moved into the `DispatchCommand`.
4. YES. Missing dispatcher (`sender` is `None`): the constructed command is
   dropped on the early return, releasing via `Drop` exactly once.
   `try_send` `Full`/`Disconnected`: the command inside the error is dropped
   with the error value — one release, no manual decrement anywhere.
5. YES. `dispatch_loop` destructures the command, retains `permit` across
   `dispatch_one` (all four rank results, per-rank `output.validate`, rank-set
   and consensus checks), then `drop(permit)` at worker.rs:261 strictly
   before `response.send(result)` at line 263.
6. NO (correct). `StepHandle` now holds only the response receiver; it has
   no counter reference and no `Drop` impl (worker.rs:94-111).
7. YES. An abandoned receiver makes `response.send` fail; the result is
   discarded (`let _ =`), rank state is untouched, and the permit was
   already released only after physical completion — no early admission.
8. YES. Dispatcher startup failure (`return` before the recv loop) drops the
   receiver; queued commands drop with it, each releasing its permit once
   via `Drop`. Shutdown: pool `Drop` closes the sender; `recv` drains
   remaining commands through the normal release path; commands still queued
   after a fail-stop `break` release on receiver drop. No path both
   explicitly releases and re-releases via `Drop` (the permit is moved out
   by destructuring exactly once).
9. YES. `OutstandingPermit::drop` uses
   `fetch_update(..., |c| c.checked_sub(1))`; on an impossible zero count the
   update fails and the counter is left unchanged (debug_assert flags it in
   debug builds). No release-build wraparound.
10. YES. `step_quota_is_owned_by_operation_after_handle_abandonment`
    (worker.rs:562-605) uses two five-party barriers (four
    `FirstStepBlockingRankExecutor`s + test thread); `entered.wait()` proves
    all four rank executors are physically inside `execute` before the
    handle is dropped and capacity is probed.
11. YES. `release.wait()` runs before any assertion, and the replacement
    handle (if the former implementation admitted it) is received before
    the drain loop, so the old code fails on
    `assert_eq!(outstanding_after_abandonment, 1)` /
    `assert!(replacement_was_saturated)` deterministically without a
    deadlocked teardown (2-second bounded drain loop).
12. YES. Former code: drop decrements to 0, `reserve_slot` succeeds and the
    empty dispatch channel accepts the replacement. Corrected code: count
    stays 1, replacement is `Saturated`, and the final `step(2)` submission
    succeeds only after the drain loop observes zero.
13. YES. `drop(permit)` precedes `response.send` in program order on the
    dispatcher thread, so a successful `receive()` can only observe a count
    that already reached zero for that operation
    (`four_workers_acknowledge_one_identical_plan` asserts 1 then 0).
14. ACCURATE within reproduced scope. Handoff-provenance count: 58
    `fable-*-handoff.md` files are tracked at the implementation commit,
    and the proof's "56 then-present review-handoff provenance proofs"
    refers to the configured `review-proof-all` subset (a later proof at
    another commit states "0 of 57 configured review results", confirming
    the configured-subset counting convention; the `review-proof` run here
    emits the v2 provenance schema and passes). The 262-test figure is the
    full local gate at the implementation commit; I reproduced the mandated
    subset (7 worker tests, clippy clean) and the successor commit's
    identical file adds exactly one test (263 claimed there), consistent.
    CPU-only boundary, retained serial-dispatch statement, and exclusions
    match the code: no cancellation, no executor ABI, no CUDA.

## Acceptance statements

- Outstanding count now measures queued/running TP4 operations: YES.
- Handle timeout/drop cannot release an active step slot: YES.
- Every queue/dispatcher shutdown path releases exactly one permit: YES.
- Abandoned results cannot admit replacement work before physical
  completion: YES.
- The barrier regression distinguishes prior behavior without deadlocking:
  YES.
- The CPU proof and all exclusions are accurate: YES.

## Architecture & maintainability

Moving quota ownership from the response handle into an uncloneable RAII
permit that travels with the command is the right shape: the invariant
("count == queued or running physical operations") is now enforced by move
semantics rather than by call-site discipline, and every drop path is a
release path by construction. The permit type is private and minimal.
`dispatch_loop`'s fail-stop `break` matches the four-rank collective-safety
rationale and is clearly commented. Two maintainability notes: the
double-bounding of quota vs. channel capacity (QUESTION 1) deserves a
comment tying them together, and the release-build silence on counter
underflow (MINOR 1) should become an explicit poisoned-state signal when the
production executor lands. Test architecture (barrier-instrumented executors
injected through the same public `spawn`) is a good template for the coming
startup/factory work.

## Token decision

All six acceptance statements are unqualified YES; zero BLOCKER and zero
MAJOR findings; provenance verified at start and finish with no drift.

tp4-step-operation-quota-v1-accepted
