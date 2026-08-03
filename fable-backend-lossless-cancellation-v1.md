# Fable review: queue-independent backend cancellation v1

Date: 2026-07-31
Reviewer: Fable (adversarial design-gate review)
Handoff: `docs/fable-backend-lossless-cancellation-v1-handoff.md`

Note: the handoff requests this result at the repository root; the operator
directed all review results into `docs/reviews/`, so it is written here.

## Reviewed candidate

Reviewed candidate commit (detached worktree, never moving `main`):

2ace56ccc9016c83422dbef1048371accd0430c8

Implementation and regression commit under review within the candidate:
`f56e0bc03dfd12fd2d5f8f03da1a57d5b66e5dcf` ("Make backend cancellation
queue-independent"); the candidate commit adds the proof and doc updates.

## Verified input hash table

Every pinned input was hashed with `shasum -a 256` in the detached worktree
at the candidate commit at review start and again at review finish; all
hashes matched the handoff at both points, and `glmaxx review-proof`
independently returned verdict PASS for the same table.

| Input | SHA-256 (verified start and finish) |
|---|---|
| `spec/engine-v0.md` | efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a |
| `crates/glm-serving/src/backend.rs` | 8c9ec8fc9ce37f6d3261940c95d8efae802fa1d95c4095b8f305c3e91dc16078 |
| `crates/glm-serving/src/lib.rs` | c3ed1b80f5b0561626fc0e61426054a76f9e507b095e0f136accb29ac26a0c07 |
| `docs/backend-lossless-cancellation-proof-v1.md` | 08fb526598d9fa2f170ced6432d579c5cc893826389f10b70a66937932b12d56 |
| `docs/coordinator-api-backend-v1.md` | 33b997380f0da1659355bca4e3d094ef21455d3dc25b1af66063239886e7a07f |
| `docs/http-serving-contract.md` | 274630837cc79abddf7d68765f806fa3672fcdf2f6b3d44f32d7659b4a148f40 |
| `docs/retained-http-request-ownership-proof-v1.md` | 83616d0878a4177d0df2e268d36de02a58d1380361cff42bda414178ce8c0971 |
| `docs/backend-admission-rollback-fatal-proof-v1.md` | fd9c2e12a09af096afcf13dafc282e847c8984776a8ea70ef285d9d791866499 |
| `docs/backend-event-cancellation-fatal-proof-v1.md` | 04794fb247b103e90d03a07e9827f13ce82d89e0a50dccb543c5e010f0f9bde5 |
| `docs/production-punchlist.md` | 7014d0da7f2a58a06d273552c4a33b3adde733dc3c326ead19ca097f521b6763 |
| `docs/results-index.md` | 27701bcccc85a45a22bb104b0e7113a5bd3ad98287e69168330436684272060f |
| `scripts/local-checks.sh` | 839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f |

Gates run in the worktree: `review-proof` (PASS), ten consecutive runs of
`backend::tests::cancellation_survives_a_saturated_submission_queue`
(10/10 passed), `cargo test --offline -p glm-serving` (39 passed, 0
failed), `cargo clippy --offline -p glm-serving --all-targets --
-D warnings` (clean), and a full workspace `cargo test --offline`
(269 passed, 0 failed).

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. The proof's sentence "its cardinality cannot exceed the existing owner
   registry" is instantaneously imprecise: a marker whose owner was just
   removed by terminal cleanup survives until the runtime's next
   cancellation turn prunes it (stale pruning is also rationed to the
   per-turn quota), so for a bounded window the cancellation registry can
   exceed the owner registry by the number of not-yet-pruned stale
   markers. The substantive claims — at most one marker per accepted
   request, insertion only under a live authenticated owner, self-draining
   prune, no unbounded queue — are all true and tested, so this is a
   wording nit, not a bound violation.
2. Dispatch-priority selection rescans the registry from its smallest key
   on every quota iteration (`find_map` from the start), making a turn's
   dispatch work O(quota x registry-size) rather than O(quota). Registry
   size is bounded by accepted requests (owners plus transient stale
   markers), so this is bounded and small in practice; noting it in case
   the accepted-request bound is ever raised substantially.

### QUESTION

1. A marker inserted while the runtime sleeps in
   `commands.recv_timeout(idle_poll_interval)` gets no wakeup and waits up
   to the idle poll interval. The proof states this exclusion explicitly
   (eventfd wakeup is future work); confirming it is understood as a
   latency property, not a loss property.

## Answers to the handoff's required questions

1. Yes. The prior public path was `command_sender.try_send(Cancel)` on the
   same bounded channel as `Submit`, mapping `TrySendError::Full` to
   `ENGINE_OVERLOADED` (visible in the `f56e0bc` diff).
2. Yes. An HTTP timeout, disconnect, or explicit cancel receiving
   `ENGINE_OVERLOADED` could abandon the completion receiver while the
   accepted request remained queued or active, leaving cancellation lost.
3. Yes. The corrected public `cancel` authenticates the owner under the
   `owners` lock (`UNKNOWN_REQUEST` / `TENANT_MISMATCH`), rechecks
   fatal/shutdown while still holding that gate, and only then inserts the
   marker.
4. Yes. The public path holds `owners` across the `cancellations` insert;
   the runtime's `process_cancellation_requests` acquires `owners` then
   `cancellations`. All other lock sites (`Dispatched` update, teardown
   guard, `fail_all`, `remove_owner`) take at most one of the two locks at
   a time; a full-file audit found no reverse-order acquisition.
5. Yes, per accepted request. `entry(request_id).or_insert(Requested)`
   coalesces duplicate public calls, and the runtime only transitions
   `Requested -> Dispatched`, never duplicates. Insertion requires a live
   owner; instantaneous cardinality can transiently include stale markers
   awaiting bounded pruning (MINOR-1), but at most one entry per accepted
   request always holds.
6. Yes. Selection dispatches only markers whose request is in the runtime
   `active` map; a queued submit's marker is not actionable, and the prune
   branch removes only markers with no owner — a queued submit still has
   its owner, so its `Requested` marker is retained.
7. Yes. The turn order is: bounded command quota, then
   `process_cancellation_requests`, then pending-admission polling, then
   the scheduler tick.
8. Yes. Dispatch iterations are capped at `maximum_commands_per_tick`;
   there is no new queue, and per-iteration work is a scan of the bounded
   marker registry (see MINOR-2), not of unbounded work.
9. Yes. Each iteration first looks for an actionable `Requested` marker
   and prunes one ownerless marker only when no actionable marker exists,
   so stale cleanup cannot starve actionable dispatch within a turn.
10. Yes. Successful dispatch sets the marker to `Dispatched`; repeat
    public calls hit `or_insert` on an existing entry and cause no second
    coordinator cancellation (selection ignores `Dispatched`).
11. Yes. Pruning requires `!owners.contains_key(request_id)`; a queued
    submit's owner is inserted at `submit_chat` and removed only on
    terminal handling, so its marker survives until then.
12. Yes. Owner/active disagreement returns `ENGINE_STATE_FAILED`, registry
    poisoning returns `ENGINE_STATE_FAILED`, and a retained coordinator
    cancellation failure propagates `CANCELLATION_ROLLBACK_FAILED` from
    `process_command` — all before the marker is touched, all routed
    through the runtime's fatal drain with active/owner state preserved
    for `fail_all`, and both registries cleared by the teardown guard.
13. Yes. The regression uses `command_capacity: 1`,
    `maximum_commands_per_tick: 1`, and four
    `FirstStepBlockingExecutor`s on five-party barriers; the target
    tenant-2 submit fills the single slot while the runtime thread is
    blocked inside the peer's physical TP4 step.
14. Yes. Both `backend.cancel(2, ..)` calls occur before `release.wait()`,
    and the test asserts exactly one entry in the registry with state
    `Requested`.
15. Yes. After release, the turn consumes the queued submit, dispatches
    the marker before admission polling and the next tick (so the target
    cannot execute a step), and the test asserts the target's terminal is
    exactly `REQUEST_CANCELLED`, the peer finishes normally at its
    one-token length limit, the marker registry drains to empty, and
    `glmaxx_backend_active_requests 0`.
16. Yes. Under the prior code the first `cancel` would `try_send` into the
    one-slot channel already holding the target submit and return
    `ENGINE_OVERLOADED`, failing the test's first `.unwrap()` for exactly
    the claimed reason.
17. Yes. The proof states it does not interrupt a physical collective in
    progress, that an idle runtime may wait up to its idle poll interval
    before observing the marker, and that a disconnected client may not
    receive its terminal event even though the model work is cancelled.
18. Yes. Ten consecutive focused runs passed here; the glm-serving suite
    is exactly 39 tests; the full workspace run passed 269 tests with zero
    failures; `git ls-tree` at the candidate counts 65 tracked handoffs,
    i.e. 63 excluding the two umbrella handoffs; and the
    GPU/model/performance exclusions are as stated.

## Handoff's eight separate statements

- The old bounded-channel cancellation loss is real and correctly
  distinguished: YES.
- The corrected public path durably records one authenticated in-process
  marker independently of submission capacity: YES.
- Queued, active, repeated, completed, and fatal cancellation ownership
  are all handled fail-closed: YES.
- Runtime ordering dispatches cancellation before the target can execute:
  YES (dispatch precedes admission polling and the tick within the same
  turn that consumes the submit).
- Registry and per-turn work remain bounded: YES (see MINOR-2 for the
  constant-factor note; the bound holds).
- Lock ordering and teardown are safe: YES (single consistent
  `owners -> cancellations` order; teardown guard clears both).
- The regression fails the prior code for the claimed reason: YES.
- The CPU proof and all latency, transport, GPU, model, and performance
  exclusions are accurate: YES (MINOR-1 is a wording nit on an invariant
  whose substantive bound is true; the latency and delivery caveats are
  stated plainly).

## Architecture & maintainability

Separating cancellation intent (a tiny owner-authenticated state map) from
the submission data path is the architecturally correct fix: intent is
idempotent and coalescible, so a two-state marker registry models it
better than a queue slot ever could. The `Requested -> Dispatched`
lifecycle plus prune-only-after-owner-removal gives each marker a single
well-defined owner-coupled lifetime, and routing dispatch through the
existing `process_command` Cancel arm means the retained-rollback fatal
semantics reviewed in the admission-rollback handoff apply unchanged. The
selection closure's nested `then/flatten` is dense and would benefit from
a named helper, and the full-rescan-per-iteration (MINOR-2) is the first
thing to revisit if accepted-request bounds grow. Teardown is symmetrical
with the owner registry, so no new leak class is introduced.

## Token decision

All eight required statements are an unqualified YES; no blockers or
majors. Input hashes were re-verified at review finish and matched. The
acceptance token follows.

backend-lossless-cancellation-v1-accepted
