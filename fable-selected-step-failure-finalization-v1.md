# Fable review: selected-step failure finalization v1

Date: 2026-07-31
Reviewer: Fable (adversarial design-gate review)
Handoff: `docs/fable-selected-step-failure-finalization-v1-handoff.md`

Location note: the handoff requests this result at the repository root as
`fable-selected-step-failure-finalization-v1.md`; the operator directed all
review results into `docs/reviews/`, so it is written here instead.

## Reviewed candidate commit

2ff0ac124be63a8a8318d664d167f34dde32ed3c

Reviewed in a detached worktree pinned at that commit. `main` was never used
as review substrate.

## Verified input hash table

Every input named by the handoff was hashed with SHA-256 at review start and
again at review finish; both hash sets were identical and matched the
handoff's pinned values exactly (8/8 inputs, zero mismatches). `glmaxx
review-proof` against this handoff reports verdict PASS with all
expected/actual hashes equal.

| Input at candidate commit | SHA-256 (verified) |
|---|---|
| `spec/engine-v0.md` | efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a |
| `crates/glm-serving/src/lib.rs` | 1aee2f7fccac9c79124edaeeef0f9759faed2bc621b3e7228ce9914eb4e432d6 |
| `crates/glm-scheduler/src/lib.rs` | 5a820b2e5013f038f07b26f14ddc24d69d00e18d3d55837ab5ff3a68daee3074 |
| `crates/glm-engine/src/worker.rs` | 400c7c22f2c74d3f386fefe2d144da3437f27e6c99ce9f2d4bbf87ffe98fe437 |
| `docs/offline-serving-spine.md` | 27b24d4cbafc8203937d3620e7bcd85d47fcb86cc4d8b89e237025e5d40a62f9 |
| `docs/serving-page-transaction-v1.md` | e3a9a1d9f2eb26dc5312d7c42297fa3d832e444f7e3f269094746a85fb3deac2 |
| `docs/selected-step-failure-finalization-proof-v1.md` | 36be571d84cff086ad3058f3426fc0fee6bdd4d33b1c4317473128e4d861512e |
| `scripts/local-checks.sh` | 839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f |

Provenance anomaly (procedural, not staleness): the handoff file itself is
not present in the tree at the candidate commit (added by the later gating
commit; the same pattern holds for the sibling gates in this queue). All
hashed inputs exist at the candidate and match; `review-proof` was run in
the pinned worktree with the handoff supplied alongside the pinned tree and
passed. The proof doc's own input hash table also matches the worktree
byte-for-byte.

## Gate commands

Run in the pinned worktree: `cargo test --offline -p glm-serving` (all
passed), `cargo clippy --offline -p glm-serving --all-targets --
-D warnings` (clean), `glmaxx review-proof` (PASS).

## Answers to the 15 required adversarial questions

1. YES. `Scheduler::next_batch` installs `self.inflight` before returning a
   selected batch, so every later immediate error return is responsible for
   consuming that state; an `awk` scan of glm-serving/src/lib.rs:386-472
   confirms zero bare `?` in the post-selection window other than
   `next_batch()?` itself and the idle-branch `emit_terminal_transitions()?`
   (no batch selected on either).
2. YES (defect confirmed against the parent commit). The pre-change code
   (visible in the fix commit's diff) used immediate `?` on graph lookup,
   compile, collective count, progress, worker submit, and the successful
   completion commit — each returned with the batch still inflight.
3. YES. All eight fallible operations after selection are `match`ed into
   `fail_selected_step`: graph lookup (lib.rs:392), compile (400),
   collective count (406), starting progress (421), `try_submit` (427),
   `handle.receive()` (433), output fit (465), and success-commit failure
   (471) — worker submission and worker receive both included.
4. YES. `fail_selected_step` (lib.rs:617-625) calls
   `self.scheduler.complete_batch(false)?` before `emit_failed_rows(batch)?`
   (the fallible prefix/token cleanup and failed-event emission).
5. NO (invariant holds). The scheduler has no request-removal path at all
   (no `.remove` on `requests`), so graph lookup, compilation, observation,
   and worker submission cannot remove a selected request before failure
   completion; `UnknownRequest` is unreachable in this window.
6. YES. `complete_batch(false)` rejects completions
   (glm-scheduler/src/lib.rs:328), preflights every selected request
   (326-370), removes the exact inflight batch only at `inflight.take()`
   (465), and marks all selected requests failed in one apply-phase commit
   (377-380 staged, applied after 465).
7. YES. The failure commit at lib.rs:622 precedes the fallible cleanup at
   623; a cache or event error afterwards returns with the scheduler batch
   already no longer inflight (proven by the regressions' idle-next-tick
   assertions).
8. YES, exactly. `MAXIMUM_STEP_EVENTS = 512` (lib.rs:22) is reserved before
   selection (lib.rs:385); a legal batch is at most 64 rows
   (`MAX_ACTIVE_SEQUENCES`, glm-engine/src/step.rs:9); worst success shape
   is 64 x (7 tokens + 1 terminal) = 512 and the failure path needs at most
   64, re-checked with no events pushed since reservation — event-capacity
   failure is unreachable on this path.
9. YES. In `Tp4WorkerPool` (glm-engine/src/worker.rs:183-212), `plan.verify`
   precedes reservation; `reserve_slot` failure enqueues nothing; a
   `try_send` Full/Disconnected returns the command inside the error (never
   enqueued) and rolls back the reservation; the closed-sender branch also
   rolls back. Saturation is decided before any command is accepted, so
   finalizing the selected rows involves no unknown in-flight device
   execution.
10. YES. A worker receive error is finalized (`fail_selected_step`) and the
    dispatcher's fail-fatal break (worker.rs:259-266) yields `Closed` for
    subsequent submissions, which are also finalized — no rank-local retry
    or partial-collective continuation exists, consistent with
    collective-safety requirements.
11. YES. All errors in `complete_batch_internal` occur before
    `inflight.take()`, so a rejected worker result leaves the identical
    unchanged inflight batch, which `complete_batch(false)` then safely
    completes as failed.
12. YES. `compile_failure_fails_selected_rows_without_stranding_inflight`
    (lib.rs:1005-1037) forces `sequence_table_generation = 0` to produce a
    real post-selection `Compile(Batch)` error, proves the request and event
    are terminal, and proves the next tick is idle rather than
    `Scheduler(Inflight)`.
13. YES. `submit_failure_...` (lib.rs:1039-1073) holds the only bounded
    worker slot, forces `Worker(Saturated)` after selection, and proves the
    same terminal and next-tick properties.
14. YES. Both regressions fail on the prior code for the claimed
    stranded-batch reason: the pre-change bare-`?` paths leave the batch
    inflight, so the next tick returns `Scheduler(Inflight)` instead of
    idle. Confirmed against the parent commit's source.
15. YES (verified). Exactly 239 `#[test]` functions statically, zero
    `#[ignore]`; 41 handoff files of which 39 carry candidate labels,
    matching the "39 then-present" claim with 2 historical skips; the open
    cross-request cleanup caveat is genuinely disclosed in the proof
    (lines 149-152); GPU/model/performance non-claims are consistent with
    the pure-CPU code. Tests and clippy were run in the pinned worktree by
    this review and pass.

## Six summary determinations

- Every ordinary pre-completion error consumes the selected scheduler
  batch: YES.
- Selected rows become failed before fallible resource cleanup: YES.
- No worker-submit error can coexist with an accepted device command: YES.
- Cache/event cleanup failure cannot strand scheduler inflight state: YES.
- Both regressions distinguish the prior defect: YES.
- The CPU proof and all scope exclusions are accurate: YES.

## Findings

### BLOCKER

None.

### MAJOR

None within the review boundary.

### MINOR

1. Finalization-failure error masking: `fail_selected_step`
   (lib.rs:622-623) replaces the root-cause step error with the
   scheduler/cache error when finalization itself fails. Fail-closed but
   loses the diagnosis.
2. If `release_request_prefix` fails mid `emit_failed_rows`
   (lib.rs:608-613), rows after the failure point retain leases and never
   receive a Failed event, with no retry path (failed rows never re-enter
   batches and `emit_terminal_transitions` handles only Cancelled). This is
   the proof's explicitly disclosed open cross-request cleanup boundary; it
   is reachable only behind an already-corrupt cache invariant, and the
   composed backend treats the returned error as fatal. A follow-up handoff
   giving Failed requests the same terminal-marker/retry treatment as
   cancellation is recommended.
3. The 512-event budget is an exact fit (64 x 8); any future per-row event
   type silently converts legal batches into runtime `Backpressure` at
   lib.rs:385. Deserves a compile-time assertion.
4. Failure completion still advances `decode_burst` for Decode/Verify
   batches (scheduler lib.rs:361-364, 483) — harmless but unstated.
5. Terminal requests are never removed from `Scheduler::requests`
   (pre-existing, outside the boundary; identical finding recorded in the
   sibling scheduler-batch-atomicity review): repeated fault injection grows
   the map and per-tick scans without bound. Punchlist tracking recommended.

### QUESTION

1. `complete_batch(false)` shares the duplicate-row/oversize preflight with
   the success path; if batch construction ever produced duplicate rows,
   both completion paths would reject and the inflight would strand
   permanently. Currently unreachable (rows drawn from BTreeMap keys) — is
   fail-closed-with-strand the intended posture versus a panic?
2. The submit-saturation test's out-of-band held plan uses `step_id: 99`,
   advancing the dispatcher's `last_step_id` past the coordinator's counter;
   benign here only because the sole request is already Failed. Worth a
   comment.
3. The handoff is absent at the pinned commit (gating-commit ordering);
   recommend future gating commits include the handoff in the pinned
   candidate.

## Architecture & maintainability

The correction is small, surgical, and structurally right: six scattered
`?` returns become one named finalizer with a strict order (scheduler
commit, then resource release, then event emission, then original-error
return), leaning on the previously-gated preflight-then-apply scheduler
completion so the failure commit is genuinely atomic under `&mut`
exclusivity. The `?`-free window between selection and completion is easy
to audit and keep clean; the two regressions encode the exact observable
that distinguishes the defect (next tick idle vs `Scheduler(Inflight)`).
Costs: any future fallible step inside the finalizer is a stranding hazard
mitigated only by unreachability; the 512 = 64 x 8 budget arithmetic should
live in a `const` assertion; and cross-request terminal cleanup remains
explicitly non-transactional (disclosed, follow-up recommended). The worker
pool's reserve-then-try-send pattern composes correctly with the finalizer.

## Token decision

All six summary determinations are unqualified YES; there are no BLOCKER or
MAJOR findings; the input hash set matched at review start and finish;
`review-proof`, the glm-serving test suite, and clippy pass in the pinned
worktree; the proof's counts and disclosed caveat verify. The minors are
diagnostics, hardening, or pre-existing out-of-boundary items.

selected-step-failure-finalization-v1-accepted
