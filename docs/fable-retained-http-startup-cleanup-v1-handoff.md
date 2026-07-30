# Fable handoff: retained HTTP startup cleanup v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`20c773c94179b2ab0913ed69eaf82a301d6b27db`

Required result path:
`fable-retained-http-startup-cleanup-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`retained-http-startup-cleanup-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-serving/src/http.rs` | `cc85c2084ac4cdcc3ba2926d1c93e3da39763db92102a0240269bb27764b098f` |
| `crates/glm-serving/src/lib.rs` | `c3ed1b80f5b0561626fc0e61426054a76f9e507b095e0f136accb29ac26a0c07` |
| `docs/http-serving-contract.md` | `036de32f5dd515a7a01aa33ff982d52c37fcee1b37565f35fce1e4a00d197adc` |
| `docs/nonblocking-http-transport-v1.md` | `e1ee381ad46b9f277640e884380aaab11a6a5b23e4f87c7cdea05334e3ebddc5` |
| `docs/retained-http-request-ownership-proof-v1.md` | `83616d0878a4177d0df2e268d36de02a58d1380361cff42bda414178ce8c0971` |
| `docs/tp4-step-operation-quota-proof-v1.md` | `ab5e025afe5f4c236738ea6658d1bbcd9d7a3eac73fd53d4bf0b1cc4600f2d88` |
| `docs/retained-http-startup-cleanup-proof-v1.md` | `74266c9985f4a22f98bd53ca4500f7fb09d255395279ef10840f8473d3196fec` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-retained-http-startup-cleanup-v1-handoff.md
cargo test --offline -p glm-serving http::tests -- --nocapture
cargo test --offline -p glm-serving \
  tests::submit_failure_fails_selected_rows_without_stranding_inflight \
  -- --nocapture
cargo clippy --offline -p glm-serving --all-targets -- -D warnings
```

## Review boundary

This review covers only synchronous cleanup after retained HTTP connection
worker or accept-thread spawn failure, plus the physical-work correction to
the serving saturation regression. It does not accept worker readiness,
startup deadlines, post-bind monitoring, the planned epoll/eventfd transport,
keep-alive, pipelining, syscall cancellation, checkpoint execution, CUDA,
concurrency capacity, or performance.

## Required adversarial questions

1. Did the prior `?` on connection-worker spawn return while dropping and
   detaching every earlier worker `JoinHandle`?
2. Did accept-thread spawn failure have the same detached-worker behavior
   after the full worker set had started?
3. Did sender destruction make eventual worker exit likely but provide no
   proof that backend clones and thread resources were released before
   `bind` returned?
4. On worker-spawn error, does the correction drop the only connection
   sender before joining every partial worker, avoiding a join-on-`recv`
   deadlock?
5. On real accept-thread spawn error, does failure to spawn drop the closure
   and its moved sender before the correction joins every worker?
6. Does the injected accept failure explicitly drop the sender before the
   same join path?
7. If any partial worker panics during cleanup, is `ThreadPanic` returned
   instead of hiding that failure behind the original spawn error?
8. Can either failure path still publish an `ApiHttpServer` or retain a
   listening endpoint?
9. Do worker-2-of-4 and accept-boundary injections exercise both partial
   startup paths?
10. Does backend `Arc` strong count exactly one after each failed bind prove
    every partial worker clone and startup-local clone was destroyed before
    return?
11. Did the former serving saturation regression depend on retaining a
    response handle after the underlying TP4 step might already have
    completed?
12. Does its replacement wait for all four barrier-blocked rank executors
    before asserting saturation, then release and receive the physical held
    step after atomic selected-request failure?
13. Is the proof explicit that successful thread spawn still has no readiness
    receipt or startup deadline?
14. Are the 264-test, 58-handoff, CPU-only boundary, and all exclusions
    accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- worker-spawn failure synchronously joins every partial worker;
- accept-spawn failure synchronously joins the full worker set;
- cleanup panic cannot be silently discarded;
- no backend/thread/listener ownership survives a failed bind;
- both startup injections and the physical saturation regression distinguish
  the prior behavior; and
- the CPU proof and all exclusions are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, sender retained during join,
detached worker, swallowed cleanup panic, surviving backend clone/listener,
nondistinguishing test, or overstated production claim.

The token accepts only this retained CPU HTTP startup correction and evidence
repair. It does not open cn4, authorize CUDA work, or accept the production
nonblocking transport.
