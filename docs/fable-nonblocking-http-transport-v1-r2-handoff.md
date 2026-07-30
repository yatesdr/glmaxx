# Fable handoff: nonblocking HTTP transport v1 r2

Date: 2026-07-30

Status: corrective adversarial design review requested

Review candidate commit:
`b7a2ac4bd45b1cb7a15c69c33d7d2248da826ad5`

Required result path:
`docs/reviews/fable-nonblocking-http-transport-v1-r2.md`

Requested acceptance token, only for an unqualified design pass:
`nonblocking-http-transport-v1-r2-design-accepted`

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch GPU, container, storage-device,
or network work for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if needed. Hash every input at review start and finish. Any mismatch
must withhold the token as a stale candidate.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/nonblocking-http-transport-v1-r2.md` | `8f553b02a8846a4effa1b0dd7f3a40dedd4392f616002f189449c2ba32796f77` |
| `docs/nonblocking-http-transport-v1.md` | `e1ee381ad46b9f277640e884380aaab11a6a5b23e4f87c7cdea05334e3ebddc5` |
| `docs/tenant-resource-quotas-v1.md` | `d779e4d6a4e4a6b5b57e4c76ab1cee504361df76ff8d2d78b174db00e4528cab` |
| `docs/serving-observability-v1.md` | `4058d01d58c0d8f4d7222803e05577a9419cfa6f5d0f20a65c41e9e2779213e6` |
| `docs/sustained-serving-load-fault-v1.md` | `3c80abd792455cbd00fb769702784c97c676bfec6e19ccba97c7c4bbe6e8bc38` |
| `docs/benchmark-contract.md` | `cd51d22a8faf2baacfb4682ff5e1dcb5986edc27d8aa3af188105842bb49a507` |
| `crates/glm-serving/src/http.rs` | `cc85c2084ac4cdcc3ba2926d1c93e3da39763db92102a0240269bb27764b098f` |
| `crates/glm-serving/src/backend.rs` | `afcca1f6f50cb7699ef24cfc32e45fa005a6a8663efb7dc36806b2305ff295d4` |
| `crates/glm-serving/src/lib.rs` | `bc7eff0297e14b73df7eec5ade3352ad0f75ceabeaca1862c4866a51efb948e3` |
| `crates/glm-serving/src/metrics.rs` | `378b7e441f8e2759ab562d61d2df05591fa40523b0effb1401b66b88ac644499` |
| `crates/glm-serving/Cargo.toml` | `d6715d8d222b99a08561bd788ca27aa678cd14f55826921b885559852279dcf0` |
| `Cargo.toml` | `863c28560b339f1fd7fb6b80c1b812e9fa7bc3f8f8c782126d2a29ceeffc06ea` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |
| `crates/glm-scheduler/src/lib.rs` | `5fd0c4506002c4da5679f1ca3bf96a880ca7b0b348d5f55ada26a2e06ae7ff4d` |
| `docs/production-punchlist.md` | `654e23710a2491de188b6e5adcd31697c004fdf50540ae2400cad1f92c595849` |
| `docs/results-index.md` | `cf9613b157f1d16ca7c2c5805772dd93dbc147f37ac1e5aefe14bf66e1c776f5` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/native-engine-plan.md` | `33552cd81e3d79b8b484856a99620420f3e2eddfdfa529a23b191353a702ed80` |
| `scripts/local-checks.sh` | `56f728cdf3f047f9633509a57341d25a977efa802f0d5b371c9716830517db59` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-nonblocking-http-transport-v1-r2-handoff.md
git diff --check b7a2ac4bd45b1cb7a15c69c33d7d2248da826ad5^ \
  b7a2ac4bd45b1cb7a15c69c33d7d2248da826ad5
```

The handoff itself is coordination metadata added after the candidate and is
not a candidate input. The first review is operator-owned input at
`docs/reviews/fable-nonblocking-http-transport-v1.md`; do not modify or add it
to the candidate.

## Review purpose

The first review correctly withheld its token because the release-clear then
acquire-recheck protocol permitted a lost wakeup. It also requested exact
epoll cookie semantics, pre-accept overflow cleanup, listener publication
ordering, zero-timeout local runnable processing, Linux-only evidence, a
frozen S05 arrival control, and proxy identity in the configuration digest.

This candidate is a design-only correction. The reviewer must independently
decide whether the SeqCst ownership handoff closes the lost-wakeup window and
whether the proposed model proof can distinguish the original bug. Do not
accept an implementation, S05 result, Linux evidence, production health, or
GPU work.

## Review boundary

Acceptance covers only:

- the corrected completion queue/eventfd notification protocol;
- the required weak-memory model and mutations;
- the exact epoll user-data encoding and reuse boundary;
- pre-accept mailbox overflow ownership and cleanup;
- listener health/publication and runnable-list timeout rules;
- Linux CPU evidence requirements;
- the frozen S05 transport-control schedule and pass criteria; and
- reverse-proxy startup identity.

Acceptance does not cover:

- any current Rust transport implementation;
- parser, HTTP, quota, backend, scheduler, or metrics code correctness;
- a loopback, sustained-load, proxy, checkpoint, model, or GPU result;
- S05 or S07 completion;
- production transport health;
- GLM-5.2 quality or performance; or
- cn4 access.

## Required adversarial questions

1. Do all nineteen candidate-input hashes match at review start and finish in
   a detached worktree?
2. Is amendment precedence exact and narrow enough that the remaining v1
   contract stays normative without contradiction?
3. Does release-publishing the queue entry before a SeqCst false-to-true
   producer compare-exchange prevent an eventfd write from preceding entry
   visibility?
4. Does the reactor's SeqCst `swap(false)` followed immediately by an
   acquire queue recheck close the StoreLoad window identified in the first
   review?
5. Exhaustively reason through a producer that publishes and then observes
   true before the reactor clear, one that wins false-to-true after the
   clear, and one racing the post-clear local reacquisition. Can any
   published entry remain asleep with no pending eventfd and no runnable
   consumer?
6. Is it safe for the active reactor to retake `notified` without writing
   eventfd, including when a producer simultaneously wins and writes a
   redundant count?
7. Are the producer transition, reactor clear, post-clear recheck, and queue
   release/acquire orderings stated strongly and concretely enough to prevent
   an implementation from silently restoring the original bug?
8. Does the required Loom-style model cover weak-memory visibility rather
   than only thread scheduling, and do its three mandatory mutations
   distinguish the original lost-wakeup class?
9. Does retaining `notified == true`, rotating a local runnable list, and
   forcing epoll timeout zero preserve both liveness and bounded fairness
   after budget exhaustion?
10. Is the 24-bit slot plus exact 40-bit generation encoding injective over
    its admitted domain, and do startup rejection and permanent retirement
    make truncation or wrap impossible?
11. Does exact comparison against the slab's full `u64` generation protect
    local epoll actions across both slab-slot and file-descriptor reuse while
    leaving the complete `ConnectionKey` authoritative across threads?
12. Is pre-accept mailbox overflow fully request-local, nonblocking,
    idempotently cancelled, and balanced for rejected plus retained entry,
    output-byte, request, and network ownership?
13. Can overflow racing route installation or a terminal event cause double
    cancellation, double release, a leaked route, text-before-status, or an
    incorrect backend-fatal transition?
14. Does withholding every externally usable address and health signal until
    all reuseport listeners and dependencies acknowledge one digest remove
    the partial-bind accept/reset exposure for cooperating clients?
15. Is the Linux evidence boundary strong enough that macOS tests or a
    cross-compile cannot be mistaken for epoll/eventfd qualification?
16. Recompute the S05 schedule. Are there 120,000 measured requests, exactly
    30,000 streaming requests, 64 ms between assignments to one lane, and
    28 ms between its mock terminal and next assignment?
17. Is the S05 process truly open-loop, immutable under prior completions,
    sufficiently long, and paired with exact correctness, driver-lag,
    queue/pool, RSS, descriptor, shutdown, and repeatability criteria?
18. Are the streaming close/replacement descriptor counts compatible with
    the version-one close-after-`[DONE]` rule and bounded independently of
    the 4,096 idle sockets?
19. Does including deployment mode plus exact proxy binary/version/config
    and policy digests prevent evidence from silently mixing proxy
    generations?
20. Does this amendment remain compatible with the tenant quota,
    observability, sustained-load, and benchmark contracts without claiming
    their acceptance?
21. Is implementation correctly held until this design token, followed by
    model proof, Linux fault matrix, and S05 control in that order?
22. Are every implementation, evidence, S05/S07, production-health, model,
    quality, performance, GPU, and cn4 exclusion accurate?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately:

1. the corrected wake protocol has no lost-wakeup execution;
2. the weak-memory model and mutation requirements are sufficient;
3. epoll local identity and cross-thread full-key validation are complete;
4. pre-accept overflow is nonblocking and resource-balanced;
5. listener publication, runnable fairness, and Linux evidence are exact;
6. the frozen S05 control and proxy digest are coherent;
7. Linux implementation may begin only after this design pass; and
8. all non-acceptance and GPU exclusions are accurate.

Only if all twenty-two questions and all eight statements are unqualified
`YES`, end with:

```text
nonblocking-http-transport-v1-r2-design-accepted
```

Withhold for stale provenance, a remaining lost-wakeup execution, an
insufficient weak-memory proof, ambiguous cookie/reuse semantics, unbalanced
mailbox cleanup, pre-health address publication, blocking with local work,
non-Linux substitution, an unfrozen or internally inconsistent S05 control,
proxy identity drift, or any implementation/evidence/GPU overstatement.
