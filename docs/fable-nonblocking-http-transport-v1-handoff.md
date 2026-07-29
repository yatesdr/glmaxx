# Fable handoff: nonblocking HTTP transport v1

Date: 2026-07-29

Status: adversarial design review; implementation token withheld by Sol

GPU authorization conveyed by this handoff: none

Review candidate commit:
`3608a030530d2157fedc25a1432cd769ec8e9f98`

Requested acceptance token, only if every blocker and major is resolved:
`nonblocking-http-transport-v1-accepted`

## Provenance

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/nonblocking-http-transport-v1.md` | `e1ee381ad46b9f277640e884380aaab11a6a5b23e4f87c7cdea05334e3ebddc5` |
| `docs/tenant-resource-quotas-v1.md` | `d779e4d6a4e4a6b5b57e4c76ab1cee504361df76ff8d2d78b174db00e4528cab` |
| `docs/serving-observability-v1.md` | `4058d01d58c0d8f4d7222803e05577a9419cfa6f5d0f20a65c41e9e2779213e6` |
| `docs/benchmark-contract.md` | `cd51d22a8faf2baacfb4682ff5e1dcb5986edc27d8aa3af188105842bb49a507` |
| `crates/glm-serving/src/http.rs` | `0cd2f66e45a1e79e14035c44b34ecaa73d7da80fc9b3ba580771937a6c9b5c41` |
| `crates/glm-serving/src/backend.rs` | `34396a06b459e060af0c5f6b0cfb6451522af0f72536312da24804b25fe40c6c` |
| `crates/glm-serving/src/lib.rs` | `9d011012cb103149aed5ff531f356746d50f0ed29398854f2d2516c42d82aeab` |
| `crates/glm-serving/src/metrics.rs` | `378b7e441f8e2759ab562d61d2df05591fa40523b0effb1401b66b88ac644499` |
| `crates/glm-serving/Cargo.toml` | `d6715d8d222b99a08561bd788ca27aa678cd14f55826921b885559852279dcf0` |
| `Cargo.toml` | `863c28560b339f1fd7fb6b80c1b812e9fa7bc3f8f8c782126d2a29ceeffc06ea` |
| `Cargo.lock` | `ed694e41e6f1ba1723480d1052846d14d12086d85514b628133f5c2390d69bc1` |
| `crates/glm-scheduler/src/lib.rs` | `5651a507ad240f19755d50336f09eb3ca97e32f8be51f90e0fe49ef304350f38` |
| `docs/production-punchlist.md` | `32d6d48c079130698b2d9c48d85c04388a92d8879fd4a27e54a8c93f7014cdd0` |
| `docs/results-index.md` | `de6f48efbbec7f839e7bc3be038a146d0ec477ea6335c0ee04118472ea904d7d` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/native-engine-plan.md` | `33552cd81e3d79b8b484856a99620420f3e2eddfdfa529a23b191353a702ed80` |

Hash every input at review start and finish. Review the exact candidate commit
in a separate worktree if `main` advances. The candidate deliberately
contains no transport implementation, load result, or GPU evidence.

## Requested adversarial questions

1. Is direct Linux epoll/eventfd through the existing `libc` dependency a
   sound lean production choice, or is a different reactor primitive required
   for correctness on the target host?
2. Can sharded `SO_REUSEPORT` listeners bind an ephemeral port in the stated
   reactor-zero order without an accept gap or inconsistent socket options?
3. Do edge-triggered accept/read/write loops plus explicit work budgets avoid
   both starvation and lost readiness?
4. Is `ConnectionKey(reactor,slot,generation)` sufficient against file
   descriptor and slab-slot reuse for completions, timers, admission results,
   and cancellation?
5. Must epoll user data carry the complete generation, a cookie checked
   against the slab, or another ABA defense?
6. Does the incremental parser authenticate and reserve declared body bytes
   before every body allocation/read, including body bytes coalesced with the
   final header read?
7. Is rejecting transfer encoding, duplicate security headers, folding, and
   pipelining sufficient to avoid request smuggling behind the declared
   reverse proxy?
8. Can exact-length reads plus rejection of already-buffered extra bytes
   safely implement no pipelining while retaining nonstreaming keepalive?
9. Does moving JSON/render/tokenization/backend submission to bounded workers
   remove every blocking or unbounded task from reactors?
10. Is there any stale-key ordering where backend submission succeeds without
    a cleanup registry or where cancellation misses the accepted request?
11. Is the bounded pre-accept mailbox sufficient when backend text or terminal
    events beat the successful admission result to the reactor?
12. Must completion envelopes carry a request-local sequence, and does the
    proposed gap/duplicate/after-terminal fatal rule preserve role/text/final
    wire order?
13. Re-derive the eventfd `notified` protocol under enqueue-before-clear,
    enqueue-after-clear, budget exhaustion with a nonempty queue, coalesced
    producers, eventfd `EAGAIN`, and reactor fatal. Can any entry sleep?
14. Does nonblocking `try_push` plus output-byte permits isolate a slow client
    without blocking coordinator/model progress?
15. Which thread may serialize JSON/SSE without violating the no-unbounded
    allocation hot-path requirement?
16. Is connection-close-delimited SSE after `[DONE]` correct for version one,
    and can nonstreaming keepalive remain unambiguous without HTTP pipelining?
17. Do partial writes, `EPOLLOUT` interest changes, and fixed segment cursors
    prevent duplicate, missing, or reordered bytes?
18. Are all header/body/admission/request/write/idle deadlines based on the
    correct monotonic start and protected by connection generation?
19. Does first-serialized-transition-wins cover every disconnect, timeout,
    terminal, stop-string, explicit cancel, and fatal race without double
    cleanup?
20. Should one reactor, eventfd, or admission-worker fatal make the complete
    API unhealthy, and is the drain sequence sufficient to prove zero routes,
    permits, buffers, and descriptors?
21. Is the reverse-proxy boundary safe and explicit enough for TLS, SSE
    buffering, forwarded headers, public rate limits, and evidence provenance?
22. Does the memory formula bound all user-space bytes without multiplying
    connection count by maximum body/response size?
23. Are fixed-cardinality transport metrics sufficient to separate reactor,
    queue, network, backend, and model latency?
24. Does the 28-case proof matrix exercise the relevant parser, epoll,
    eventfd, connection-reuse, backpressure, fatal, and sustained-load cases?
25. Are C64 active plus 4,096 idle/queued connections a meaningful minimum
    CPU control, and what open-loop duration/arrival criteria must be frozen
    before S05 can pass?
26. Which API backend, completion, quota, metrics, and startup types must
    version atomically?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Withhold
the token unless every blocker and major is resolved. State separately
whether:

- the epoll/eventfd architecture is accepted;
- a pure CPU parser/reactor implementation may begin;
- the `ApiCompletionHandle` replacement may begin;
- the retained blocking server must remain available as a nonproduction
  control;
- S05 and S07 must remain unpassed;
- any finding changes the tenant quota contract or backend fatal drain;
- authorized SM120 load is required only after CPU gates; and
- no cn4 access or GPU launch is authorized by the verdict.
