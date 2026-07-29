# Fable handoff: coordinator API backend v2

Date: 2026-07-29

Status: adversarial implementation review; acceptance token withheld by Sol

GPU authorization conveyed by this handoff: none

Review candidate commit:
`8aaef8e50a69ed6fecdc01c6405dd6a2ff14ebc7`

Requested acceptance token, only if every blocker and major is resolved:
`coordinator-api-backend-v2-accepted`

This handoff supersedes
`docs/fable-coordinator-api-backend-v1-handoff.md`. It also supersedes the
backend lifecycle/concurrency questions in
`docs/fable-serving-observability-v1-handoff.md`; the metric definitions and
step-observation implementation remain byte-identical to that candidate.

## Provenance

| Input at candidate commit | SHA-256 |
|---|---|
| `crates/glm-serving/src/backend.rs` | `34396a06b459e060af0c5f6b0cfb6451522af0f72536312da24804b25fe40c6c` |
| `crates/glm-serving/src/metrics.rs` | `378b7e441f8e2759ab562d61d2df05591fa40523b0effb1401b66b88ac644499` |
| `crates/glm-serving/src/lib.rs` | `9d011012cb103149aed5ff531f356746d50f0ed29398854f2d2516c42d82aeab` |
| `crates/glm-serving/src/http.rs` | `0cd2f66e45a1e79e14035c44b34ecaa73d7da80fc9b3ba580771937a6c9b5c41` |
| `docs/coordinator-api-backend-v1.md` | `ccfe6a07e5e9327822a3b9708d4119c5797172677d65dc116958f0e9b3378949` |
| `docs/serving-observability-v1.md` | `4058d01d58c0d8f4d7222803e05577a9419cfa6f5d0f20a65c41e9e2779213e6` |
| `docs/http-serving-contract.md` | `036de32f5dd515a7a01aa33ff982d52c37fcee1b37565f35fce1e4a00d197adc` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `scripts/local-checks.sh` | `c3456173f504372c2ae2cd7dc391a8886ea838c2703a6ac38bfece47b426ebef` |

Hash every input at review start and finish. Review the exact candidate commit
in a worktree if `main` advances. Do not interpret CPU worker schedules as
CUDA, model-quality, sustained-load, or throughput evidence.

## Candidate delta

The prior backend could accept a submission just before a fatal rank/step
transition, leave it only in the command queue, exit the runtime, and give a
connected, non-backpressured receiver a disconnected completion channel
rather than a structured terminal event.

The candidate:

1. rechecks fatal/shutdown state while holding the request-registry gate;
2. holds that gate through the nonblocking submit or cancel `try_send`;
3. marks fatal before step-failure event dispatch and terminal draining;
4. fails all active requests;
5. takes the registry gate, drains accepted queued submissions, records their
   queue/request durations, sends a terminal error, and clears ownership; and
6. uses the same active-plus-queued drain on orderly shutdown.

The full local gate passed with 211 Rust tests, workspace formatting, Clippy
with warnings denied, FFI checks, deterministic fixtures, and the external
pinned-tokenizer proof. The three new concurrency/fault schedules passed ten
consecutive targeted runs.

## Requested adversarial questions

1. Is the fatal/submit/cancel ordering linearizable for every interleaving, or
   can any accepted command still enter after the terminal drain?
2. Can holding the registry mutex through `try_send` deadlock with runtime
   command processing, event dispatch, shutdown, `Drop`, metrics rendering,
   or another API thread?
3. Is every operation under that mutex bounded and nonblocking? Verify that no
   blocking channel send, tokenizer call, decoder call, worker wait, or output
   send occurs under it.
4. Can active and queued failure paths double-send a terminal event,
   double-count failed/request-duration metrics, omit an accepted request, or
   clear ownership for a later request?
5. Are queued cancellation commands safely ignored by the terminal drain
   after their associated active/queued submission has already been failed?
6. Can an owner-registry poison, runtime panic, sender disconnect, or receiver
   disconnect violate the documented exactly-one terminal claim? Classify
   any distinction between recoverable engine failures and process-fatal Rust
   invariant failures.
7. Does a failed step remain absent from successful step histograms, graph
   selections, row counts, and collective-byte counters?
8. Do queue and request-duration observations for drained queued submissions
   use defensible boundaries without pretending the requests were admitted?
9. Do the multi-tenant, slow-peer, and injected-failure tests make their
   interleavings deterministic enough to prove the stated properties?
10. Do ten repeated targeted runs add evidence beyond the deterministic
    assertions, and is any stronger concurrency method required before this
    code may remain merged?
11. Re-review the v1 questions for stop/usage, tenant cancellation,
    backpressure, context arithmetic, greedy fail-closed behavior, and bounded
    event delivery against the exact v2 bytes.
12. Re-review the observability handoff questions against the new concurrency
    tests. Which production load/fault schedules still remain unproved?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Withhold
the token unless every blocker and major is resolved. State separately
whether:

- this candidate may remain merged as a CPU backend;
- the current `/metrics` endpoint remains safe to expose;
- a request acknowledged by `submit_chat` with a connected,
  non-backpressured completion receiver is guaranteed a structured terminal
  event for recoverable runtime failures;
- any finding changes the pending `StepInput` or page-transaction contracts;
  and
- any finding blocks independent SM120 kernel qualification.
