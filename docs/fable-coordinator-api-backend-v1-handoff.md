# Fable handoff: coordinator API backend v1

Date: 2026-07-29

Status: adversarial implementation review; acceptance token withheld by Sol

GPU authorization conveyed by this handoff: none

Review candidate commit:
`5847a655c5751e3602b0abb7c322fc20cd975aed`

Requested acceptance token, only if every blocker and major is resolved:
`coordinator-api-backend-v1-accepted`

## Provenance

| Input at candidate commit | SHA-256 |
|---|---|
| `crates/glm-serving/src/backend.rs` | `a3ce65484a6a5a31a555241fc791c5322b9698fbf1805d30050cd5fb3a60b81c` |
| `crates/glm-serving/src/http.rs` | `0cd2f66e45a1e79e14035c44b34ecaa73d7da80fc9b3ba580771937a6c9b5c41` |
| `crates/glm-serving/src/lib.rs` | `16faea6d3330a766fdbe301077ae3af9f75bf571ec168c6bed7751376829e9e2` |
| `crates/glm-serving/src/cache.rs` | `786c7c7e5ce2f417749a78e8c48aa8a7d0a5cb617e0883e960a8e7c17d781720` |
| `crates/glm-tokenizer/src/lib.rs` | `64052178fab6e12b6c17227bc59e3937566660f9b8ed34f96a32cd606d7c6844` |
| `crates/glm-tokenizer/src/decode.rs` | `dbe1a902b4c04595a5e4576775f4ee3767d3a289a4589c27192b85427ac2b2f0` |
| `crates/glm-scheduler/src/lib.rs` | `5651a507ad240f19755d50336f09eb3ca97e32f8be51f90e0fe49ef304350f38` |
| `crates/glm-engine/src/worker.rs` | `400c7c22f2c74d3f386fefe2d144da3437f27e6c99ce9f2d4bbf87ffe98fe437` |
| `crates/glm-engine/src/startup.rs` | `9634f120a2e01f21aaa5778954053d9a06f1e8d2af6c5abe1f9c6e4cbbd31e87` |
| `docs/coordinator-api-backend-v1.md` | `86699b4aefe66a82cf4184ce6aa52768a18d200ce58850221d470867e2c4c0b3` |
| `docs/http-serving-contract.md` | `27bb611ecf8c9b7c6064ad7279a91524046ec1ed572032ff8d4cda84c919489e` |
| `docs/step-execution-io-v1.md` | `e8681e9278034b25fe6928c059ad58730818ce014fb3e0251549f678aa1621d5` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |

Hash every input at review start and finish. Review the exact candidate commit
in a worktree if `main` advances. Do not interpret the CPU worker tests,
`ApiHealth`, or `StartupCoordinator` mocks as SM120 device evidence.

## Candidate claims

- one runtime thread owns all mutable serving/coordinator state;
- HTTP workers render and tokenize concurrently, then use a bounded command
  channel;
- prefix admission remains asynchronous and does not block unrelated
  requests;
- per-request completion queues are bounded and a slow/disconnected receiver
  cancels only that request;
- committed token positions are contiguous and checked before decoding;
- stop, EOS, length, cancellation, and fatal paths emit consistent terminal
  events and usage;
- cancellation is authenticated to the request's tenant;
- only canonical greedy sampling is admitted until `StepInput` sampling/RNG
  is promoted;
- prompt plus output is bounded by 1,048,576 positions;
- public backend construction requires four-rank startup consensus; and
- HTTP bind and request admission require the pinned model identity, TP=4,
  SM=120, and healthy state.

The full local proof passed with 206 Rust tests, workspace Clippy warnings
denied, deterministic proof fixtures, FFI compilation checks, and the
external pinned tokenizer proof. It launched no CUDA work.

## Requested adversarial questions

1. Can command draining, cache polling, or synchronous rank execution starve
   another request, cancellation, tenant, or prefill class despite the stated
   bounds?
2. Can the concurrent owner registry disagree with runtime-owned request
   state during submit failure, terminal delivery, explicit cancellation,
   panic, channel closure, or request-ID exhaustion?
3. Is every event sequence legal and exactly-once, including EOS followed by
   coordinator `Finished`, stop inside a multi-token verifier result, explicit
   cancel during restore, and fatal rank failure after partial event output?
4. Does stop handling expose correct text and usage when already-committed
   later verifier tokens are hidden and the sequence is cancelled?
5. Can a full/disconnected completion queue block the global runtime, leak a
   prefix/prompt reservation, omit required cancellation, or retain request
   ownership indefinitely?
6. Are checked context, request-ID, position, token-count, and usage bounds
   complete? Recheck negative zero and all canonical greedy tuple cases.
7. Does accepting an optional greedy seed while `StepInput` is pending lose
   state that must be preserved now, or is rejecting probabilistic tuples
   sufficient for this phase?
8. Does the runtime correctly distinguish per-request admission errors from
   process-fatal step/worker/consensus failures?
9. Are shutdown and caught-panic paths guaranteed to make health fatal and
   wake every completion waiter without deadlock?
10. Is requiring a healthy `StartupCoordinator` plus checking the fixed
    health identity sufficient to prevent a CPU/mock coordinator from
    masquerading as production, or is a stronger non-forgeable readiness
    capability required?
11. Do the tests actually prove the concurrency and lifecycle claims, and
    which missing schedule should be added before this adapter can be a
    serving contract?
12. Does any behavior cross or prejudice the separately pending
    `StepInput.v1` and serving-page-transaction reviews?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Withhold
the token unless every blocker and major is resolved. State separately
whether:

- this adapter may remain merged as a CPU candidate;
- its greedy-only route is safe to connect after the pending input/page
  reviews pass;
- a public API or health-capability revision is required; and
- any issue blocks independent CUDA kernel work.
