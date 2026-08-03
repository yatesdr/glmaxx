# Fable review: retained HTTP startup cleanup v1

Date: 2026-07-31
Reviewer: Fable (adversarial design-gate review)
Handoff: `docs/fable-retained-http-startup-cleanup-v1-handoff.md`

Note: the handoff requests this result at the repository root; the operator
directed all review results into `docs/reviews/`, so it is written here.

## Reviewed candidate

Reviewed candidate commit (detached worktree, never moving `main`):

20c773c94179b2ab0913ed69eaf82a301d6b27db

Implementation commits under review within the candidate:
`2d99fe1e5863dc2f34a0dbcd5b8d7cc8ecf8adbc` ("Clean partial HTTP server
startup") and `5066e4cc783e074482ef068022fec6264ed5fa82` ("Make serving
saturation proof physical"); the candidate commit adds only the proof
document.

## Verified input hash table

Every pinned input was hashed with `shasum -a 256` in the detached worktree
at the candidate commit at review start and again at review finish; all
hashes matched the handoff at both points, and `glmaxx review-proof`
independently returned verdict PASS for the same table.

| Input | SHA-256 (verified start and finish) |
|---|---|
| `spec/engine-v0.md` | efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a |
| `crates/glm-serving/src/http.rs` | cc85c2084ac4cdcc3ba2926d1c93e3da39763db92102a0240269bb27764b098f |
| `crates/glm-serving/src/lib.rs` | c3ed1b80f5b0561626fc0e61426054a76f9e507b095e0f136accb29ac26a0c07 |
| `docs/http-serving-contract.md` | 036de32f5dd515a7a01aa33ff982d52c37fcee1b37565f35fce1e4a00d197adc |
| `docs/nonblocking-http-transport-v1.md` | e1ee381ad46b9f277640e884380aaab11a6a5b23e4f87c7cdea05334e3ebddc5 |
| `docs/retained-http-request-ownership-proof-v1.md` | 83616d0878a4177d0df2e268d36de02a58d1380361cff42bda414178ce8c0971 |
| `docs/tp4-step-operation-quota-proof-v1.md` | ab5e025afe5f4c236738ea6658d1bbcd9d7a3eac73fd53d4bf0b1cc4600f2d88 |
| `docs/retained-http-startup-cleanup-proof-v1.md` | 74266c9985f4a22f98bd53ca4500f7fb09d255395279ef10840f8473d3196fec |
| `scripts/local-checks.sh` | 839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f |

Gates run in the worktree: `review-proof` (PASS), `cargo test --offline -p
glm-serving http::tests` (9 passed, 0 failed),
`tests::submit_failure_fails_selected_rows_without_stranding_inflight`
(1 passed), `cargo clippy --offline -p glm-serving --all-targets --
-D warnings` (clean), and a full workspace `cargo test --offline`
(264 passed, 0 failed).

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. On a cleanup panic, the original spawn error is discarded in favor of
   `ApiServerError::ThreadPanic` (both failure arms). The handoff asks for
   exactly this precedence, and hiding a panic would be worse than hiding a
   spawn errno, but the spawn error's context is lost from the returned
   value entirely (`ThreadPanic` carries no source). A wrapped
   `ThreadPanic { spawn_error }` would preserve both. Cosmetic; behavior is
   fail-closed either way.

### QUESTION

1. The real accept-spawn failure arm relies on `thread::Builder::spawn`
   dropping the not-run closure (and with it the moved sender and listener)
   before returning `Err` — which std guarantees, and which the injected
   arm makes explicit with `drop(sender)`. Worth a one-line comment at the
   real arm so a future refactor to a spawn wrapper that retains the
   closure on failure cannot silently reintroduce the join-on-`recv`
   deadlock.

## Answers to the handoff's required questions

1. Yes. The prior code pushed `thread::Builder::spawn(..)?` results
   directly; a worker-spawn error unwound `bind`, dropping (detaching)
   every earlier worker `JoinHandle` without joining.
2. Yes. Accept-thread spawn failure used the same `?` after the full
   worker set had started, detaching all of them.
3. Yes. Dropping the sender made workers exit eventually via `recv`
   disconnect, but `bind` could return before their backend `Arc` clones
   and thread resources were released, with no proof of release.
4. Yes. On a worker-spawn error the correction calls
   `shutdown_http_workers(sender, worker_threads)`, which drops the only
   connection sender first and only then joins each partial worker, so no
   join can wait on a worker still blocked in `recv`.
5. Yes. On a real accept-thread spawn error the closure (owning the moved
   sender and listener) has already been dropped by `Builder::spawn`'s
   failure path before the `Err` arm joins the workers via
   `shutdown_http_worker_handles`.
6. Yes. The injected accept fault executes `drop(sender)` explicitly
   before the same `shutdown_http_worker_handles` join path.
7. Yes. Both failure arms return `ApiServerError::ThreadPanic` when any
   join reports a panic, instead of the original spawn error (see MINOR-1
   for the trade-off, which is the direction the handoff requires).
8. No — as required. Both paths `return Err(..)` before the `Ok(Self ..)`
   constructor; the listener is a local (or died with the failed closure)
   and is closed on return, so no `ApiHttpServer` or listening endpoint
   survives.
9. Yes. `startup_failure_joins_partial_http_workers_before_returning`
   injects `Worker(2)` with 4 configured workers (workers 0 and 1 already
   started) and `Accept` after the full default worker set started.
10. Yes. After each failed bind the test asserts
    `Arc::strong_count(&backend) == 1`: the bind argument clone, every
    started worker's clone, and the failed iteration's local clone must
    all have been destroyed before the error return became observable —
    which the joins make deterministic rather than racy.
11. Yes. The former regression used `Tp4WorkerPool::spawn_cpu` and held
    only the `StepHandle` (`drop(held_slot)`), so after the reviewed
    operation-owned quota change the underlying CPU step could complete
    before the saturation assertion, making the "queue full" premise
    schedule-dependent.
12. Yes. The replacement installs four `BlockingReservationRankExecutor`s
    behind five-party entry/release barriers, waits on `entered` until all
    four ranks are physically inside the held step before asserting the
    saturated-submit behavior and the atomic selected-request failure
    (`RequestEvent::Failed { request_id: 93 }` with no stranded inflight),
    then releases and receives the physically held step
    (`held_slot.receive().unwrap()`) and proves the runtime still ticks.
13. Yes. The proof states plainly that successful OS thread creation is
    still treated as startup, with no per-worker readiness receipt and no
    startup deadline.
14. Yes. Full workspace run at the candidate: 264 passed, 0 failed;
    `git ls-tree` counts 60 tracked handoffs, i.e. 58 excluding the two
    umbrella handoffs; the CPU-only boundary and all exclusions
    (epoll/eventfd transport, readiness, post-bind monitoring, keep-alive,
    pipelining, syscall cancellation, checkpoint, capacity, performance)
    are as stated.

## Handoff's six separate statements

- Worker-spawn failure synchronously joins every partial worker: YES.
- Accept-spawn failure synchronously joins the full worker set: YES.
- Cleanup panic cannot be silently discarded: YES (surfaced as
  `ThreadPanic`).
- No backend/thread/listener ownership survives a failed bind: YES
  (strong-count-1 assertion plus join-before-return).
- Both startup injections and the physical saturation regression
  distinguish the prior behavior: YES.
- The CPU proof and all exclusions are accurate: YES.

## Architecture & maintainability

The two-function cleanup split (`shutdown_http_workers` for the
sender-still-held arm, `shutdown_http_worker_handles` for the sender-
already-gone arms) encodes the drop-before-join ordering in the call shape
rather than in comments, which is the right way to make the deadlock
impossible to reintroduce at the worker arm. The injected-fault enum is
private and mirrors the backend's readiness fault hook, keeping the
codebase's fault-injection idiom consistent. The saturation-test rewrite
converts a schedule-dependent premise into a barrier-proven physical one
and doubles as documentation of what "queue full" means under
operation-owned quota. Residual debt: `ThreadPanic` losing the spawn
error's context (MINOR-1) and the implicit std-drop reliance (QUESTION-1).

## Token decision

All six required statements are an unqualified YES; no blockers or majors.
Input hashes were re-verified at review finish and matched. The acceptance
token follows.

retained-http-startup-cleanup-v1-accepted
