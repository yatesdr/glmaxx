# Fable review: backend runtime readiness v1

Date: 2026-07-31
Reviewer: Fable (adversarial design-gate review)
Handoff: `docs/fable-backend-runtime-readiness-v1-handoff.md`

Note: the handoff requests this result at the repository root; the operator
directed all review results into `docs/reviews/`, so it is written here.

## Reviewed candidate

Reviewed candidate commit (detached worktree, never moving `main`):

5ff3d48eef1a504bbbb0c65cfc9a0737dfcceac4

Implementation commit under review within the candidate:
`b22781c3bc548a4cf807cc05fab4b51f7c53d3d1` ("Gate backend health on runtime
readiness"); the candidate commit adds only the proof document.

## Verified input hash table

Every pinned input was hashed with `shasum -a 256` in the detached worktree
at the candidate commit at review start and again at review finish; all
hashes matched the handoff at both points, and `glmaxx review-proof`
independently returned verdict PASS for the same table.

| Input | SHA-256 (verified start and finish) |
|---|---|
| `spec/engine-v0.md` | efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a |
| `crates/glm-serving/src/backend.rs` | 175d20a7adde12f6c4f0b64a7e93b8b3004c581f7269eec735b9d7936db5fb63 |
| `docs/coordinator-api-backend-v1.md` | c3e0617c72523ac05221f67fb016cf1be8f391e1aebceeffed4c5940345225f6 |
| `docs/backend-runtime-readiness-proof-v1.md` | cadd6f5805caf9f62ccada34c2a68b4eb9e1ee244c47c549830d41df07db2846 |
| `docs/backend-admission-rollback-fatal-proof-v1.md` | fd9c2e12a09af096afcf13dafc282e847c8984776a8ea70ef285d9d791866499 |
| `docs/backend-event-cancellation-fatal-proof-v1.md` | 04794fb247b103e90d03a07e9827f13ce82d89e0a50dccb543c5e010f0f9bde5 |
| `docs/tp4-rank-startup-handshake-proof-v1.md` | 4bbdf757d288d065ddbb75aa090e5f5a757fa1f55abbbe71dd2edd5880984b0d |
| `docs/http-serving-contract.md` | 036de32f5dd515a7a01aa33ff982d52c37fcee1b37565f35fce1e4a00d197adc |
| `scripts/local-checks.sh` | 839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f |

Gates run in the worktree: `review-proof` (PASS), the required focused test
`backend_waits_for_runtime_readiness_and_cleans_failed_startup -- --exact`
(1 passed), `cargo clippy --offline -p glm-serving --all-targets --
-D warnings` (clean), and a full workspace `cargo test --offline`
(265 passed, 0 failed; the glm-serving suite is 38 tests as claimed).

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

None.

### QUESTION

1. The constructor's `startup_receiver.recv()` has no timeout by design (the
   handoff excludes a startup deadline). A runtime thread that hangs before
   the receipt — for example a coordinator whose worker pool blocks during
   unwind — would block construction indefinitely. This is accurately
   scoped out by both handoff and proof; noting it so the excluded
   startup-deadline work has a concrete hang case to cover.

## Answers to the handoff's required questions

1. Yes. The prior constructor returned `Ok(Self { health:
   production_healthy, .. })` immediately after `thread::Builder::spawn`
   succeeded, with no receipt from inside the runtime thread.
2. Yes. `mpsc::sync_channel(1)` is created in the constructor before
   `thread::Builder::spawn`.
3. Yes. Inside `runtime_loop` the receipt `startup.send(())` executes only
   after the function owns `coordinator` and `commands` by value and after
   `active` and `pending_admissions` are constructed (and after the
   injected-fault checkpoint, which precedes the send).
4. No — as required. `startup_receiver.recv()` sits before the `Ok(Self {
   .. })` expression; construction cannot complete without a received
   receipt, and a receive error returns before any backend is built.
5. Yes. A pre-ready panic unwinds out of `runtime_loop`, dropping the
   by-value `ServingCoordinator` and with it the prefix restore services
   and the retained `Tp4WorkerPool`, whose drop closes the dispatcher and
   joins the dispatcher and all four rank workers. The `startup` sender is
   destructured into the frame and is dropped by the same unwind, closing
   the channel.
6. Yes. The `catch_unwind(AssertUnwindSafe(..))` boundary absorbs the
   panic; the closure then sets `fatal` (result is `Err`, shutdown unset)
   and clears the owner registry. No panic escapes the runtime thread.
7. Yes. `if startup_receiver.recv().is_err() { let _ =
   runtime_thread.join(); return Err(CoordinatorBackendError::
   RuntimeStartup); }` — join strictly precedes the error return, and the
   variant is exactly `RuntimeStartup`.
8. Yes. OS thread-spawn failure is mapped by
   `.map_err(CoordinatorBackendError::Thread)?` before the receive and
   remains a distinct variant.
9. Yes. `if startup.send(()).is_err() { return; }` — a dropped
   constructor-side receiver makes the runtime return through
   `runtime_loop`'s normal exit, dropping the coordinator ownership tree
   rather than running detached (the send cannot spuriously fail while the
   receiver is alive: capacity-one buffered send only errors on
   disconnect).
10. Yes. The test calls the same private
    `spawn_with_tokenizer_inner` used by the public constructors, with
    `Some(RuntimeStartupFault::BeforeReady)` as the only difference; the
    fault point panics inside `runtime_loop` immediately before the
    receipt, exercising the production unwind path.
11. Yes. The four `DropCountingExecutor`s (one per rank in the retained
    `Tp4WorkerPool`) increment an atomic on drop, and the test asserts the
    count is exactly 4 on the same statement boundary where
    `RuntimeStartup` is observed. That count can only be 4 there if the
    constructor joined the runtime thread and the pool's rank threads were
    themselves joined and their executors destroyed — eventual or detached
    cleanup would leave the count racing below 4.
12. Yes. `bounded_backend_runs_greedy_request_to_exact_length`,
    `concurrent_tenants_complete_with_exact_lifecycle_totals`, the
    slow-consumer pair, cancellation, and fatal-step tests all construct
    backends through the normal receipt route and pass at the candidate.
13. Yes. The proof's exclusions state no startup deadline, no post-start
    liveness watchdog, no device-health capability, no production SM120
    rank factory, and no production-health qualification beyond host-state
    ownership.
14. Yes. Full workspace run at the candidate: 265 passed, 0 failed;
    glm-serving suite is 38 tests; `git ls-tree` counts 61 tracked
    handoffs, i.e. 59 excluding the two umbrella handoffs; CPU-only
    boundary and exclusions are as stated.

## Handoff's six separate statements

- Backend health cannot publish before the runtime readiness receipt: YES.
- Pre-ready failure synchronously joins the runtime and complete
  coordinator ownership tree: YES.
- No backend, command sender, runtime thread, or rank executor survives
  the failed constructor: YES (the constructor-held `command_sender` is a
  local of the failed constructor and is dropped on return; the runtime
  thread is joined; executor drops are counted).
- The fault injection distinguishes the former behavior: YES (the former
  constructor returned a healthy backend regardless of runtime fate and
  could not assert the synchronous drop count).
- Success behavior and strict local gates remain intact: YES.
- The CPU proof and all exclusions are accurate: YES.

## Architecture & maintainability

The receipt protocol is minimal and hard to misuse: one capacity-one
channel, one send site placed after ownership initialization, one receive
site placed before publication, and failure funneled into a dedicated
error variant. Reusing the existing unwind boundary instead of adding a
second supervision mechanism keeps a single cleanup path, and the
`RuntimeControl` struct tidies what was becoming a six-argument runtime
signature. The fault-injection enum is private and single-variant, so the
test hook cannot leak into public API. The deliberate absence of a receive
timeout (QUESTION-1) is the one piece of future work this design leaves
open, and both documents say so plainly.

## Token decision

All six required statements are an unqualified YES; no blockers or majors.
Input hashes were re-verified at review finish and matched. The acceptance
token follows.

backend-runtime-readiness-v1-accepted
