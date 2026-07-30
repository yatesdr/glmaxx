# Fable handoff: coordinator API backend v3

Date: 2026-07-30

Status: consolidated adversarial implementation review requested

Review candidate commit:
`10a068ba55cc0e8dbe39161f925a0dcf0a17d8ef`

Required result path:
`docs/reviews/fable-coordinator-api-backend-v3.md`

Requested acceptance token, only for an unqualified implementation pass:
`coordinator-api-backend-v3-accepted`

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch GPU, container, storage-device,
or network work for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if needed. Hash every input at review start and finish. Any mismatch
must withhold the token as a stale candidate.

| Input at candidate commit | SHA-256 |
|---|---|
| `crates/glm-serving/src/backend.rs` | `03a452977927426934e938a2ca8e8956335727a9d6fd2155bced69a364c83138` |
| `crates/glm-serving/src/metrics.rs` | `378b7e441f8e2759ab562d61d2df05591fa40523b0effb1401b66b88ac644499` |
| `crates/glm-serving/src/lib.rs` | `bc7eff0297e14b73df7eec5ade3352ad0f75ceabeaca1862c4866a51efb948e3` |
| `crates/glm-serving/src/http.rs` | `cc85c2084ac4cdcc3ba2926d1c93e3da39763db92102a0240269bb27764b098f` |
| `crates/glm-serving/Cargo.toml` | `d6715d8d222b99a08561bd788ca27aa678cd14f55826921b885559852279dcf0` |
| `docs/coordinator-api-backend-v1.md` | `632e4605d41a0deaeda3415165fcf874547e573c851ba2bb19664f7bbc6f4457` |
| `docs/coordinator-api-backend-v3-proof.md` | `0de1ca03bb82c23d42378a4d3171a9cbc5d193f4d410fe098c3c0b3fd92da3b9` |
| `docs/backend-event-cancellation-fatal-proof-v1.md` | `04794fb247b103e90d03a07e9827f13ce82d89e0a50dccb543c5e010f0f9bde5` |
| `docs/backend-admission-rollback-fatal-proof-v1.md` | `fd9c2e12a09af096afcf13dafc282e847c8984776a8ea70ef285d9d791866499` |
| `docs/fable-coordinator-api-backend-v2-handoff.md` | `1443a9af63b908394ec087372bd995c9666a11f7ad30ff9859430de69452a9f2` |
| `docs/serving-observability-v1.md` | `4058d01d58c0d8f4d7222803e05577a9419cfa6f5d0f20a65c41e9e2779213e6` |
| `docs/http-serving-contract.md` | `5f365e664063766448caefdef9a1d4e6cc7864f2f49be3f140933fef207bc248` |
| `docs/production-punchlist.md` | `cf37529de76844b0311cf092f34befedb75ed5f4a1ac3d6a2e4c9d60fe474b28` |
| `docs/results-index.md` | `9ee66936e30f5600606aacb210b488ec5e19ca35cd82773223b44ce90b715ac6` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `scripts/local-checks.sh` | `56f728cdf3f047f9633509a57341d25a977efa802f0d5b371c9716830517db59` |
| `Cargo.toml` | `863c28560b339f1fd7fb6b80c1b812e9fa7bc3f8f8c782126d2a29ceeffc06ea` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-coordinator-api-backend-v3-handoff.md
git diff --check 10a068ba55cc0e8dbe39161f925a0dcf0a17d8ef^ \
  10a068ba55cc0e8dbe39161f925a0dcf0a17d8ef
cargo test --offline -p glm-serving
cargo clippy --offline -p glm-serving --all-targets -- -D warnings
```

The handoff itself is coordination metadata added after the candidate and is
not a candidate input. The prior operator-owned review is
`docs/reviews/fable-coordinator-api-backend-v2.md`; do not modify it.

## Review purpose

The v2 review accepted the fatal/submission gate itself but withheld its token
for two ownership defects in older bytes. Current `main` contains both
separately pinned corrections. This candidate also removes the v2 review's
100 ms test schedule assumption and documents its poisoned-registry
process-fatal carve-out.

Review current source as one consolidated backend candidate. Do not merely
defer the old majors to their separate review jobs. Independently confirm
that the present bytes close them and that the new barrier test proves the
claimed active-plus-queued fatal schedule.

## Review boundary

Acceptance covers only:

- bounded API submission and owner linearization;
- queue-independent authenticated cancellation;
- retained admission/cancellation rollback attribution;
- cancellation-required event dispatch;
- recoverable active-plus-queued fatal drain;
- deterministic four-rank injected-failure test scheduling;
- owner-registry poison classification; and
- retained fixed-cardinality lifecycle metrics at this boundary.

Acceptance does not cover:

- the production nonblocking HTTP transport;
- startup liveness/deadline supervision;
- probabilistic distributed sampling;
- checkpoint-backed rank or model execution;
- MTP;
- sustained load or multi-user throughput;
- CUDA, SM120, checkpoint, quality, capacity, latency, or performance
  evidence;
- S02 as a whole; or
- cn4 access.

## Required adversarial questions

1. Do all eighteen candidate-input hashes match at review start and finish
   in a detached worktree?
2. Does `submit_chat` still hold the sole owner mutex through the terminal
   state recheck, owner insertion, and nonblocking command publication?
3. Can any accepted submission enter after recoverable fatal drain has
   completed, or escape both active and queued draining?
4. Is cancellation owner-bound, coalesced, independent of command queue
   saturation, and dispatched before later work for the cancelled request?
5. Do all seven cancellation-required event branches call the common
   `cancel_dispatch_request` helper?
6. If coordinator cancellation fails, does the helper reinsert the complete
   `ActiveRequest` before returning fatal, preserving decoder, sender,
   tenant, timing, and owner attribution for common drain?
7. Can any branch send success/failure, remove the external owner, or keep
   scheduler work alive after that rejected cancellation?
8. Does pending-admission poll failure become fatal exactly when
   `has_pending_admission(request_id)` proves coordinator ownership remains?
9. In that retained case, are pending ID, active request, owner, prompt
   bytes, and cache work retained until identity-bound repair/cancellation?
10. Can cancellation rollback failure be misclassified as an ordinary
    request-local cancellation or lose the request before fatal drain?
11. Does the first five-party barrier prove all four rank executors are
    inside one physical step before the three queued submissions begin?
12. While the four executors wait at the second barrier, can the runtime
    process a worker failure or enter fatal drain before queue construction
    finishes?
13. Does `BarrierReleaseGuard` prevent a post-entry panic from deadlocking
    backend/worker destruction without weakening the successful schedule?
14. After release, are the expected first-request and queued-request error
    codes, four terminals, fatal health, and exact lifecycle/step metrics
    correctly asserted?
15. Is 100 consecutive fresh-process passes meaningful corroboration for a
    schedule whose decisive ordering is barrier-proven rather than timed?
16. Is owner-registry poisoning correctly classified as a process-fatal Rust
    invariant breach for which continued service and exact queued terminal
    delivery are not claimed?
17. Could `fail_all` safely recover and inspect a poisoned owner map, or is
    refusing to do so the stronger fail-closed contract?
18. Does ordinary fatal drain make the published active-owner gauge zero,
    while runtime-local `pending_admissions` disappears on loop exit without
    representing a second active metric?
19. Are all mutex-held operations bounded and nonblocking, and can any lock
    ordering deadlock with runtime dispatch, metrics, shutdown, or `Drop`?
20. Do completion queue full/disconnect, decoder stop, and decoder error
    remain isolated without blocking the coordinator or losing serving
    cleanup?
21. Does the complete 42-test crate gate exercise both former majors, the
    deterministic fatal schedule, cancellation saturation, slow peers,
    tenant ownership, stop handling, and metric totals?
22. Does any current behavior silently enable probabilistic sampling,
    production transport health, checkpoint/model execution, or MTP?
23. Is a separately reviewed nonblocking transport still mandatory before
    S02 and S05 can pass?
24. Are every CPU-only, no-throughput, no-quality, no-GPU, and no-cn4
    exclusion accurate?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately:

1. current source closes both v2 MAJOR ownership findings;
2. recoverable fatal submission/drain ordering is linearizable and bounded;
3. the four-rank fatal test is deterministic and unwind-safe;
4. the poisoned-registry process-fatal boundary is accurate;
5. metrics and structured-terminal claims are not overstated;
6. the retained CPU backend may remain merged;
7. production nonblocking transport remains a separate mandatory gate; and
8. all model/performance/GPU exclusions are accurate.

Only if all twenty-four questions and all eight statements are unqualified
`YES`, end with:

```text
coordinator-api-backend-v3-accepted
```

Withhold for stale provenance, an unattributed request, cancellation result
loss, retained rollback misclassification, a timing-dependent fatal schedule,
unwind deadlock, unsafe poison recovery, blocking under the owner mutex,
metric/terminal overstatement, production-transport substitution, or any
model/performance/GPU overclaim.
