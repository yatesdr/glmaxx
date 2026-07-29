# Fable handoff: serving observability v1

Date: 2026-07-29

Status: adversarial implementation review; acceptance token withheld by Sol

GPU authorization conveyed by this handoff: none

Review candidate commit:
`9607aa0e3027dc998bc9489c7abe29320c7b7972`

Requested acceptance token, only if every blocker and major is resolved:
`serving-observability-v1-accepted`

## Provenance

| Input at candidate commit | SHA-256 |
|---|---|
| `crates/glm-serving/src/metrics.rs` | `378b7e441f8e2759ab562d61d2df05591fa40523b0effb1401b66b88ac644499` |
| `crates/glm-serving/src/backend.rs` | `fdc7fc99753111b4e1a61fe7212ddf07529356069978bbc24d41dd4a534a24e9` |
| `crates/glm-serving/src/lib.rs` | `9d011012cb103149aed5ff531f356746d50f0ed29398854f2d2516c42d82aeab` |
| `crates/glm-serving/src/http.rs` | `0cd2f66e45a1e79e14035c44b34ecaa73d7da80fc9b3ba580771937a6c9b5c41` |
| `crates/glm-engine/src/step.rs` | `4963a58da7c9c6bbed0fb57fb7ef56d90d1e0f09fe54da8cc02c35891f743359` |
| `crates/glm-engine/src/worker.rs` | `400c7c22f2c74d3f386fefe2d144da3437f27e6c99ce9f2d4bbf87ffe98fe437` |
| `crates/glm-scheduler/src/compile.rs` | `220cf549c0b5882d109ebce4ebd646e9b28ebbab80a83fa579ef5a2c591a070a` |
| `docs/serving-observability-v1.md` | `4058d01d58c0d8f4d7222803e05577a9419cfa6f5d0f20a65c41e9e2779213e6` |
| `docs/coordinator-api-backend-v1.md` | `74b145e025ce98ca7c8ded70018cd550ca499c8d77129eb9a63aa68bfd734b35` |
| `docs/http-serving-contract.md` | `036de32f5dd515a7a01aa33ff982d52c37fcee1b37565f35fce1e4a00d197adc` |
| `docs/benchmark-contract.md` | `cd51d22a8faf2baacfb4682ff5e1dcb5986edc27d8aa3af188105842bb49a507` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |

Hash every input at review start and finish. Review the exact candidate commit
in a worktree if `main` advances. Do not reinterpret host timings as GPU,
kernel, graph, TP, DCP, or transfer evidence.

## Requested adversarial questions

1. Do `worker_round_trip`, `coordinator_overhead`, and `total_step_time`
   partition host wall time without overlap, underflow, or an excluded
   scheduler/commit phase?
2. Are real/bucketed rows, graph ID, MTP depth, routes, schedule hash, and
   exclusive collective bytes derived from the exact committed plan rather
   than reconstructed from mutable state?
3. Can a failed/divergent step be mistaken for a committed observed step, or
   can a committed step be omitted after event backpressure?
4. Are queue, prefix-resolution, admission-to-first-token, TTFT, ITL, and
   request-lifetime clocks placed at defensible boundaries? Identify any
   hidden tokenizer, restore, scheduler, or stream time.
5. Are restored/computed target and draft prompt tokens counted exactly once
   across full hits, partial hits, chunked prefill, cancellation, and MTP0
   versus MTP1–6?
6. Does `draft_ordinal` preserve accepted-draft position for every verify
   output, including accepted EOS and a following correction/bonus token?
7. Do a full/disconnected output queue, decode error, cancellation, and fatal
   shutdown produce internally consistent completed/cancelled/failed,
   duration, and useful-token counters?
8. Are all counters and histogram accumulators saturating and race-safe, and
   is Prometheus exposition cumulative and syntactically valid?
9. Does metric recording allocate, lock, block, or introduce unbounded label
   cardinality in the serving hot path? Is the fixed graph-ID overflow
   behavior acceptable?
10. Which required engine-v0 observability fields remain absent, and are the
    documentation and metric names sufficiently explicit to prevent a host
    aggregate from being presented as device or benchmark evidence?
11. Do the tests prove concurrency behavior, or only single-request values?
    Name the minimum sustained/fault schedules required before production.
12. Does adding `draft_ordinal` require an event ABI/version amendment before
    any external consumer is allowed?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Withhold
the token unless every blocker and major is resolved. State separately
whether:

- this candidate may remain merged as CPU host telemetry;
- the metrics are safe to expose from the current HTTP endpoint;
- any field must change before `StepInput` or page-transaction integration;
  and
- any finding blocks independent SM120 kernel qualification.
