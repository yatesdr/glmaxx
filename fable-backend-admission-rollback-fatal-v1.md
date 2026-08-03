# Fable review: backend admission rollback fatal drain v1

Date: 2026-07-31
Reviewer: Fable (adversarial design-gate review)
Handoff: `docs/fable-backend-admission-rollback-fatal-v1-handoff.md`

Note: the handoff requests this result at the repository root; the operator
directed all review results into `docs/reviews/`, so it is written here.

## Reviewed candidate

Reviewed candidate commit (detached worktree, never moving `main`):

3ab31108f571c01ae4a83642c95e012d8b195123

Implementation commit under review within the candidate:
`6050d8ecfb3164ac50a3cb14e51fa26fb4e3eed8` ("Fail stop on retained admission
rollback"); the candidate commit adds only the proof document.

## Verified input hash table

Every pinned input was hashed with `shasum -a 256` in the detached worktree
at the candidate commit at review start and again at review finish; all
hashes matched the handoff at both points, and `glmaxx review-proof`
independently returned verdict PASS for the same table.

| Input | SHA-256 (verified start and finish) |
|---|---|
| `spec/engine-v0.md` | efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a |
| `crates/glm-serving/src/backend.rs` | 8b4f34d771374c9f442d69ba98c6ca29c501cd6f3644fef417923b258084d30a |
| `crates/glm-serving/src/lib.rs` | 3797647a8535b8a8ca80efd76b4d91407330e3147e5c8e3e0a728b5005043e11 |
| `crates/glm-serving/src/cache.rs` | e7686a678537b1644608f655e0a8bd40133e0772e0cdf12865c02bbefb15b54b |
| `docs/pending-admission-rollback-proof-v1.md` | cfd008dacc26f7d82c3f524ad7347da9d492168ebd9e53bb255bdc6cbcbfddfd |
| `docs/backend-admission-rollback-fatal-proof-v1.md` | fd9c2e12a09af096afcf13dafc282e847c8984776a8ea70ef285d9d791866499 |
| `scripts/local-checks.sh` | 839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f |

Gates run in the worktree: `review-proof` (PASS), `cargo test --offline -p
glm-serving` (31 passed, 0 failed), `cargo clippy --offline -p glm-serving
--all-targets -- -D warnings` (clean), and a full workspace `cargo test
--offline` (244 passed, 0 failed).

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. Transient coordinator `Backpressure` can be classified as an
   invariant-blocked rollback. `ServingCoordinator::cancel` on a pending
   admission calls `require_event_space(1)` before any mutation
   (`crates/glm-serving/src/lib.rs:445`); if the event queue were full, the
   error returns with `has_pending_admission` still true, and the backend
   would escalate a transient full-queue condition to
   `CANCELLATION_ROLLBACK_FAILED` and a whole-runtime fatal drain. This is
   unreachable in the current runtime shape: events are fully drained at the
   end of every tick, each command generates at most one event, pending
   admissions are capped at `event_capacity`, and `event_capacity` (1024 in
   all current wiring) far exceeds any configured
   `maximum_commands_per_tick`. But the invariant
   `maximum_commands_per_tick <= event_capacity` is enforced nowhere and
   documented nowhere; a future config change could make a cancel burst
   fatal. Recommend documenting or validating the coupling.

### QUESTION

1. `poll_pending_admission`'s retained-ownership test re-queries
   `coordinator.has_pending_admission(request_id)` after the error rather
   than having `poll_admission` return the retained/released distinction in
   its error type. This is correct today because the coordinator mutates
   `pending_admissions` only under exclusive `&mut` access on the same
   thread, but an error enum variant would make the contract
   compiler-checked. Non-blocking.

## Answers to the handoff's required questions

1. Yes. The prior poll loop (visible in the `6050d8e` diff) removed the
   backend pending ID and called `fail_request(..., "ADMISSION_FAILED")` on
   every `poll_admission` error, including errors for which the coordinator
   retained the admission.
2. Yes. The prior cancellation arm ran `pending_admissions.remove` before
   `coordinator.cancel`, then `fail_request(..., "CANCELLATION_FAILED")` on
   any error, discarding active/owner state on retained rollback errors.
3. Yes. In either old path the coordinator kept the pending admission, its
   prompt token buffer and retained-byte reservation, and the cache pending
   restore record, while every backend registry entry (active map, pending
   set, owner map) was gone — an unattributable retained request.
4. Yes. `has_pending_admission` is `&self` and only
   `pending_admissions.contains_key` (`lib.rs:640`); it neither mutates
   coordinator internals nor weakens cache validation.
5. Yes. The `Err(error)` arm without retained ownership removes the pending
   ID and takes `fail_request(..., "ADMISSION_FAILED")` — the ordinary
   request-local path.
6. Yes. The retained arm returns `ADMISSION_ROLLBACK_FAILED` without
   touching `active`, `pending_admissions`, `owners`, or the coordinator;
   the caller then performs the structured drain over that preserved state.
7. Yes. `pending_admissions.remove` runs only in the `Ok(())` arm of
   `coordinator.cancel` (and in the non-retained request-local error arm,
   where the coordinator no longer owns the request).
8. Yes. The retained-cancellation arm returns `CANCELLATION_ROLLBACK_FAILED`
   with no registry mutation of any kind before it.
9. Yes. Both `process_command` consumption sites (the per-tick `try_recv`
   quota loop and the idle `recv_timeout` path) and the poll site all run
   `fatal.store(true, Release)` before `fail_all` and then `return`, which
   drops the coordinator with the runtime thread; the outer thread guard
   additionally clears owners.
10. Yes. The retained state means rollback preflight found a broken
    residency/pin invariant that the API backend has no authority to
    repair; request-local continuation would run on corrupt global cache
    state and retry would spin the runtime thread. Fail-stop with preserved
    ownership until coordinator destruction is the correct policy, and it
    is correctly limited to errors with `has_pending_admission` true.
11. No. In the poll path the retained decision is the first action after the
    error; in the cancel path only the read-only `matches_owner` check on
    `active` precedes `coordinator.cancel`, and no event, owner removal,
    metric, or pending-set mutation occurs before the retained-ownership
    decision.
12. Yes. `retained_admission_rollback_forces_fatal_signal_without_losing_owner`
    publishes a real `FileTierStore` page, registers the prefix, begins a
    real pending admission via the production `process_command` helper,
    aborts the restore identity behind the coordinator, and polls until the
    corrupt worker result arrives; it then asserts the active entry,
    backend pending ID, external owner `(50 -> tenant 1)`, coordinator
    pending admission, and the exact 256-byte retained prompt reservation
    all remain.
13. Yes. The old code removed the pending ID and failed the user on the
    first poll error, so `active.contains_key(&50)`,
    `pending_admissions.contains(&50)`, and the owner assertion would all
    fail, and the loop's `Ok(true)` arm would hit the explicit
    "corrupt pending admission was forgotten" panic.
14. Yes. The same test then issues a Cancel command against the still-
    corrupt state, asserts `CANCELLATION_ROLLBACK_FAILED` with all three
    registries and coordinator ownership intact, then repairs the restore
    identity, cancels again successfully, dispatches the cancellation
    event, and asserts exact cleanup plus one structured
    `REQUEST_CANCELLED` client completion.
15. Yes. `fatal_step_fails_active_and_queued_requests_with_structured_events`
    drives four real requests through a failing rank executor and proves
    all active and queued users receive structured terminal failures,
    `glmaxx_backend_active_requests 0`, and `glmaxx_backend_fatal 1`.
16. Yes. The full workspace test run at the candidate summed to exactly 244
    passed tests; `git ls-tree` at the candidate counts 44 tracked
    `docs/fable-*-handoff.md` files, i.e. 42 review handoffs excluding the
    two historical umbrella handoffs; and the proof claims no GPU, model,
    or performance results.

## Handoff's seven separate statements

- Retained poll errors cannot become forgotten request-local failures: YES.
- Retained cancellation errors cannot lose backend ownership: YES.
- Ordinary completed-roll-back errors remain request-local: YES.
- Both runtime command paths fail-stop with a structured drain: YES.
- The coordinator is dropped after fatal drain rather than kept live with
  unattributed work: YES (runtime_loop returns, dropping the coordinator by
  value).
- The distinguishing regression fails the prior code for the claimed
  reasons: YES.
- The CPU proof and all scope exclusions are accurate: YES.

## Architecture & maintainability

The correction is small and well-shaped: `process_command` and the new
`poll_pending_admission` helper return `Result<_, ApiBackendError>` and all
three consumption sites share the identical fatal-drain idiom (`fatal`
before `fail_all` before `return`). The retained/released classification
lives in one place per path and reuses the already-proven `fail_all` drain,
so there is one terminal-drain code path to maintain. The submit-side
comment explaining why the owners lock is held across `try_send` is
accurate and load-bearing. The main long-term risk is the implicit
event-capacity coupling noted in MINOR-1 and the string-typed
`has_pending_admission` re-query noted in QUESTION-1; both are contained
and documented here.

## Token decision

All seven required statements are an unqualified YES; no blockers or
majors. Input hashes were re-verified at review finish and matched. The
acceptance token follows.

backend-admission-rollback-fatal-v1-accepted
