# Fable review: TP4 rank startup handshake v1

Date: 2026-07-30
Reviewer: Fable (adversarial design-gate reviewer)
Queue row: 76
Handoff: `docs/fable-tp4-rank-startup-handshake-v1-handoff.md`
Reviewed candidate commit: `1eb8e1c2f6c98a2d20b8e4f168b8e88aadeb97ac`
Review location: detached worktree at the pinned commit (never moving main).

Note: the operator directed review artifacts into `docs/reviews/` rather than
the repository root named in the handoff.

## Provenance

All input hashes verified at review start (against the pinned commit's
object store) and at review finish (worktree bytes); both sets matched the
handoff table exactly. The pinned commit is behind current main; the review
used the pinned worktree only.

| Input | SHA-256 (matched handoff exactly) | Start | Finish |
|---|---|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` | OK | OK |
| `crates/glm-engine/src/worker.rs` | `8c0742920847145e13975aae3db1b3a76054f94475b5a0b1ac4a4a9d05cba3ff` | OK | OK |
| `docs/offline-serving-spine.md` | `27b24d4cbafc8203937d3620e7bcd85d47fcb86cc4d8b89e237025e5d40a62f9` | OK | OK |
| `docs/sm120-rank-runtime.md` | `19638590ee3b42da32bfab7673986c26488da064649c635df895700838da5624` | OK | OK |
| `docs/sm120-rank-executor-v1.md` | `e97c54b865ed50c40ff8b15f6580d0edc18dbd0783135bc1c17d11cc19986fd4` | OK | OK |
| `docs/fable-sm120-rank-executor-v1-handoff.md` | `fe6fc7060d17db41901d545f4328a863b45737fd7e01be9c32a83bf013c2c031` | OK | OK |
| `docs/tp4-step-operation-quota-proof-v1.md` | `ab5e025afe5f4c236738ea6658d1bbcd9d7a3eac73fd53d4bf0b1cc4600f2d88` | OK | OK |
| `docs/tp4-rank-startup-handshake-proof-v1.md` | `4bbdf757d288d065ddbb75aa090e5f5a757fa1f55abbbe71dd2edd5880984b0d` | OK | OK |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` | OK | OK |

Commands executed in the pinned worktree, all passing:

- `cargo run --offline -p glm-cli --bin glmaxx -- review-proof docs/fable-tp4-rank-startup-handshake-v1-handoff.md` — exit 0.
- `cargo test --offline -p glm-engine worker::tests -- --nocapture` — 8 passed, 0 failed.
- `cargo clippy --offline -p glm-engine --all-targets -- -D warnings` — clean.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. `worker.rs:274-278` — a rank thread that sends its ready receipt and then
   dies before servicing its first command (e.g. an executor whose first
   `recv` is preceded by a panic in later code) is outside the handshake's
   guarantee: the pool publishes and the failure surfaces as `Closed` on the
   first submission. This is within the declared boundary (thread-start
   handshake only) and the proof does not overclaim, but the production
   startup state machine must add per-stage receipts, as the proof already
   states.
2. `worker.rs:298-330` — the three identical cleanup arms in the ready-mask
   loop are copy-paste triplication of the
   `shutdown → WorkerPanic-or-RankStartup → send → return` sequence; a small
   helper would remove the risk of the arms drifting apart under future
   edits.

### QUESTION

1. The constructor blocks with no deadline (`startup_receiver.recv()`). The
   proof declares this explicitly and defers a watchdog to the production
   executor contract. Given the receipt is sent before `rank_loop` and the
   ready channel has capacity 4 (sends cannot block), an indefinite hang
   requires an OS-level thread-start stall; accepted as documented. Confirm
   the production factory contract will carry the bounded watchdog.

## Answers to the handoff's required questions

1. YES. The parent-commit constructor (verified via `git show`) returned
   `Ok(Self{...})` immediately after spawning only the dispatcher thread,
   before any rank-thread spawn was attempted.
2. YES. In the prior `dispatch_loop`, a rank-spawn error hit
   `else { return; }`: the dispatcher exited silently, already-created
   `JoinHandle`s were dropped (detaching those threads), and the caller held
   a pool whose first failure was a later `Closed` on channel use.
3. YES. `spawn_inner` (worker.rs:166-196) blocks on `startup_receiver.recv()`
   and constructs the pool only in the `Ok(Ok(()))` arm.
4. YES. Each rank thread's closure executes `ready.send(rank)` as its first
   action, inside the successfully spawned thread, before `rank_loop`
   (worker.rs:274-278). The receipt cannot originate anywhere else.
5. YES. The dispatcher performs exactly four `ready_receiver.recv()`s,
   rejects `rank >= TP_RANKS`, rejects duplicates via `ready_mask & bit`,
   and requires the final mask `0b1111` before `startup.send(Ok(()))`
   (worker.rs:296-334).
6. YES. `shutdown_rank_workers` (worker.rs:366-376) consumes and drops all
   rank senders first, then joins each worker. Started rank threads are
   either blocked in `rank_loop`'s `recv` (returns `Err` when its sender
   drops) or in `ready.send` (impossible to block: channel capacity is
   `TP_RANKS` and at most four sends occur), so no join can deadlock.
7. YES. On a rank-`r` spawn error the faulted rank's executor (never moved
   into a thread) drops at the end of that loop iteration, and executors for
   ranks `r+1..3` drop with the loop's `zip` iterator on `return`; both
   happen in the dispatcher before `startup.send(Err)`, and the constructor
   additionally joins the dispatcher before returning, so all destruction
   strictly precedes the failed constructor's return. The drop-count test
   proves the total of four.
8. YES. `shutdown_rank_workers` returns `panicked = true` if any join
   errs; every startup-failure arm upgrades its error to
   `WorkerError::WorkerPanic` in that case.
9. YES. `Err(_)` from `startup_receiver.recv()` joins the dispatcher and
   returns `WorkerPanic` (join failed) or `Closed` (worker.rs:188-194). No
   arm constructs a pool.
10. YES. `pool_spawn_waits_for_all_four_ranks_and_cleans_partial_startup`
    calls `Tp4WorkerPool::spawn_inner(1, executors, Some(2))` — the same
    private constructor the public `spawn`/`spawn_cpu` delegate to with
    `rank_spawn_fault = None` — and asserts a synchronous
    `Err(WorkerError::Thread(_))`. The injected fault substitutes the
    `builder.spawn` result in place, exercising the identical error-handling
    match arm.
11. YES. `assert_eq!(drops.load(...), 4)` immediately after the constructor
    returns proves ranks 0-1 executors (dropped by their joined threads),
    the faulted rank-2 executor, and the unstarted rank-3 executor were all
    destroyed before return (per Q7 ordering).
12. YES. `custom_rank_executors_are_mutable_persistent_and_thread_affine`
    still proves four persistent thread-affine executors across two steps,
    and `four_workers_acknowledge_one_identical_plan` still proves the exact
    `[0,1,2,3]` ack set, both passing through the added handshake.
13. YES. The proof states "The startup receive intentionally has no
    wall-clock deadline" and that factory construction on owner threads,
    normative stage barriers, and structured stage receipts remain pending;
    executors are still supplied from outside via the fixed array parameter.
14. ACCURATE within reproduced scope. The mandated subset reproduced: 8
    worker tests passed, clippy clean, review-proof passed. The 263-test
    figure is the full local gate at the implementation commit; the file is
    exactly the 262-test quota candidate plus one added test, consistent.
    The "57 then-present review-handoff provenance proofs" follows the
    configured-subset counting convention used by `review-proof-all`
    (59 handoff files are tracked at the implementation commit; a
    neighboring proof's "0 of 57 configured review results" phrasing
    confirms the convention). CPU-only boundary and exclusions match the
    code: no owner-thread construction, no watchdog, no CUDA.

## Acceptance statements

- The constructor cannot publish before exact four-rank readiness: YES.
- Rank-thread spawn failure is synchronously visible: YES.
- Partial workers and all executor objects are cleaned before failure
  returns: YES.
- Dispatcher disconnect/panic cannot masquerade as startup success: YES.
- The injected regression distinguishes the prior silent-failure path: YES.
- The CPU proof and all exclusions are accurate: YES.

## Architecture & maintainability

The bounded startup channel plus in-thread receipts is the correct minimal
synchronous handshake: success is defined by evidence originating inside
each rank thread, not by the absence of an error, and every failure arm
funnels through one join-then-classify cleanup helper whose panic bit
upgrades the result. Keeping the fault injection as a private `spawn_inner`
parameter (public constructors pin it to `None`) gives a deterministic
regression without exposing test surface. Two maintenance notes: deduplicate
the triplicated ready-mask cleanup arms (MINOR 2), and when the production
factory lands, the receipt payload should grow from a bare rank id to a
structured stage receipt so this handshake can merge into the normative
startup state machine rather than living beside it. The proof document is
candid about the no-deadline choice and remaining scope, which is the right
posture for an incremental correction.

## Token decision

All six acceptance statements are unqualified YES; zero BLOCKER and zero
MAJOR findings; provenance verified at start and finish with no drift.

tp4-rank-startup-handshake-v1-accepted
