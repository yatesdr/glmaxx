# Fable handoff: sustained serving load and fault qualification v1

Date: 2026-07-29

Status: adversarial design review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`1dbab21c636e495947f384751dafd219a995ad18`

Required result path:
`fable-sustained-serving-load-fault-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`sustained-serving-load-fault-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/sustained-serving-load-fault-v1.md` | `3c80abd792455cbd00fb769702784c97c676bfec6e19ccba97c7c4bbe6e8bc38` |
| `docs/benchmark-contract.md` | `cd51d22a8faf2baacfb4682ff5e1dcb5986edc27d8aa3af188105842bb49a507` |
| `docs/nonblocking-http-transport-v1.md` | `e1ee381ad46b9f277640e884380aaab11a6a5b23e4f87c7cdea05334e3ebddc5` |
| `docs/http-serving-contract.md` | `5f365e664063766448caefdef9a1d4e6cc7864f2f49be3f140933fef207bc248` |
| `docs/serving-observability-v1.md` | `4058d01d58c0d8f4d7222803e05577a9419cfa6f5d0f20a65c41e9e2779213e6` |
| `docs/tenant-resource-quotas-v1.md` | `d779e4d6a4e4a6b5b57e4c76ab1cee504361df76ff8d2d78b174db00e4528cab` |
| `crates/glm-serving/src/http.rs` | `cc85c2084ac4cdcc3ba2926d1c93e3da39763db92102a0240269bb27764b098f` |
| `crates/glm-serving/src/backend.rs` | `c1f9e9d06b44674a1d1d0ef3c24553a9ebe63e913805946bff7c2780233fe94b` |
| `crates/glm-serving/src/metrics.rs` | `378b7e441f8e2759ab562d61d2df05591fa40523b0effb1401b66b88ac644499` |
| `crates/glm-serving/src/lib.rs` | `362312a48e1269f09f2f3f6e090dffcf896a8b6c688b65d6060e6b505aae0bae` |
| `docs/production-punchlist.md` | `f089fe5b75f9221d6bbac6f8d772247482326f0f58f87a075ebbf6cb81c3377f` |
| `docs/results-index.md` | `b15b23c8aa8d7326176cf15a7cf901ed161606d1e42690da75e4c510d4593147` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-sustained-serving-load-fault-v1-handoff.md
```

## Review boundary

This review covers only the load/fault qualification design: driver trust
boundary, immutable inputs, arrivals, scenario coverage, fault predicates,
resource bounds, measurements, acceptance accounting, and result bundle.

It does not accept an implementation, the current retained HTTP transport,
observability, quotas, CUDA execution, model output, quality, capacity, or
performance.

## Required adversarial questions

1. Can the separate-process driver observe every claimed client clock without
   reading server-private state or expected model output?
2. Are open-loop deadlines generated independently of completion, and can
   driver lag be separated from server queue latency?
3. Does the immutable manifest bind every source/model/tokenizer/graph/cache/
   hardware/scenario input that could change a result?
4. Do per-request RNG seeds remain identical across arrival replays?
5. Does the CPU matrix actually exercise C64 continuous batching, weighted
   fairness, overload, all cancellation phases, slow readers, disconnects,
   prefix reuse, rank fatality, and every queue boundary?
6. Is 50,000 requests sufficient to expose lifecycle leaks without being
   misrepresented as the production soak?
7. Does the SM120 matrix keep concurrency, context, output, MTP, sampling,
   prefix, cache pressure, and arrival rate as separately retained fields?
8. Are cold and warm prefix definitions enforceable and resistant to
   accidental cross-run cache hits?
9. Are faults bound to request/step generations rather than racy wall-clock
   sleeps?
10. Can any test fault interface become an arbitrary-command or
    authentication bypass in the production binary?
11. Do rank/collective faults require full-generation failure and prevent
    rank-local retry or forged cleanup?
12. Are restart checks sufficient for catalog replay, orphan invisibility,
    quarantine reset, and new-generation health?
13. Is the driver itself bounded against slot ABA, event loss, unbounded
    output timestamps, error-cardinality growth, and evidence-writer stalls?
14. Are per-request timestamps and per-run counters sufficient to rederive
    TTFT, every ITL, throughput, useful-token throughput, fairness,
    cancellation, and failure rate?
15. Does the accounting identity distinguish structured rejection,
    cancellation, fatal failure, and success without double terminal events?
16. Can slow/disconnected clients be proven isolated rather than merely
    eventually completed?
17. Are memory/resource plateau and fairness criteria concrete enough to
    produce PASS/FAIL instead of discretionary interpretation?
18. Does the bundle retain all raw data needed to audit aggregate percentiles
    and exclude no bad run as an outlier?
19. Does the gate order prevent CPU mock timing, a short smoke, or a partial
    scenario matrix from being called production throughput?
20. Are all transport/device/model/quality/capacity/performance exclusions
    accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- the driver trust boundary and clocks are sound;
- immutable inputs and deterministic schedules are complete;
- CPU and SM120 scenario matrices are sufficient;
- fault injection and restart semantics are safe;
- the driver and evidence path are genuinely bounded;
- request/run accounting can be independently rederived;
- acceptance rules are objective and fail-closed; and
- the gate order and exclusions are accurate.

Only if all eight answers are unqualified `YES`, end with the requested
token. Withhold it for a conditional pass, stale input, coordinated omission,
unbound input, replay seed drift, weak cold/warm distinction, racy fault,
production backdoor, rank-local recovery, slot ABA, dropped raw record,
ambiguous accounting, subjective threshold, incomplete matrix, CPU timing
claim, or overstated device/model result.

The token accepts only this design and permits its CPU harness
implementation. It does not open cn4 or accept production serving.
