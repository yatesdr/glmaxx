# Fable handoff: Rust-owned SM120 rank executor v1

Date: 2026-07-29

Status: adversarial design review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released and occupied by another workload; do not connect to cn4
or launch CUDA for this review

Review candidate commit:
`b64cb6dba506b7b5f6f2cac48c5f17b3b920d3bc`

Required result path:
`fable-sm120-rank-executor-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`sm120-rank-executor-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof` on this
handoff, then hash every input at review start and finish. If either set
differs, report a stale candidate and do not emit the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/sm120-rank-executor-v1.md` | `e97c54b865ed50c40ff8b15f6580d0edc18dbd0783135bc1c17d11cc19986fd4` |
| `docs/sm120-rank-runtime.md` | `19638590ee3b42da32bfab7673986c26488da064649c635df895700838da5624` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/target-layer-execution-v1.md` | `7f16667d55a50441dbc81b7fe18285be5a6f0fb06c68ef008a40aad8f406f478` |
| `docs/mtp-layer-execution-v1.md` | `5ad5bf01cdbd5e183b5e50aa0940344b5aabc09bf05a90c57d58e3e5b28dd3a7` |
| `docs/step-execution-io-v1.md` | `e8681e9278034b25fe6928c059ad58730818ce014fb3e0251549f678aa1621d5` |
| `docs/serving-page-transaction-v1.md` | `e3a9a1d9f2eb26dc5312d7c42297fa3d832e444f7e3f269094746a85fb3deac2` |
| `docs/checkpoint-load-transaction-v1.md` | `79d9c376201f3540f247344c24c37dd7d819d629f10459899a73c15f8b27015f` |
| `crates/glm-engine/src/worker.rs` | `400c7c22f2c74d3f386fefe2d144da3437f27e6c99ce9f2d4bbf87ffe98fe437` |
| `crates/glm-engine/src/startup.rs` | `9634f120a2e01f21aaa5778954053d9a06f1e8d2af6c5abe1f9c6e4cbbd31e87` |
| `crates/glm-engine/src/graph.rs` | `c85ca1aa52ba42294fc6a43524e8f70357523977d343e8c2f212787e7754cd22` |
| `crates/glm-engine/src/memory.rs` | `3a50581a8a60970a92ccf5a2c0e83c23d25ad975f1124c2332e9a2e646dbc837` |
| `crates/glm-cuda/src/ffi.rs` | `23b4f7636b5930d6d7ef5c936b333fbcaca3c84705f37a29bd22e3895f2213f1` |
| `crates/glm-cuda/src/ownership.rs` | `5ef1c916c356d84a55b00168fd5d69e80dc76ff5cf369d7a21a259002834e5ec` |
| `crates/glm-format/src/native_reader.rs` | `24eef8432a8dff2e830a8ec63e4e46bffcfafd94486e64fbb467945825ab0089` |
| `kernels/include/glmaxx_kernel.h` | `da233563c6bfe92885c1a3101bcafa20292365b12ab788afb4d32d44a3ed2472` |
| `scripts/local-checks.sh` | `378a717bc9d592bac68ac999c6a91d4655b633177a9005529288096a3d2a029b` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-sm120-rank-executor-v1-handoff.md
```

## Review boundary

This is a host-runtime design review. It must not connect to cn4 or treat
historical rank-binding/compile evidence as graph, collective, executor, or
performance evidence.

The central architectural decision is that Rust owns every policy and state
transition. Native CUDA/C++ contains only kernels and thin CUDA/NCCL shims.
The current microbenchmark FFI and `RankExecutor::execute(plan,schedule)` are
explicitly insufficient and are not being relabeled production-ready.

## Required adversarial questions

1. Is the process/thread model actually Rust-owned and race-free? Can any
   context, stream, graph, communicator, allocation, or module be created on
   one thread and used/dropped on another? Does the proposed factory/receipt
   handshake close silent rank-thread spawn and panic failures?
2. Does the normative startup order exactly match the engine spec and
   checkpoint load transaction? Identify resources created at the wrong
   stage, circular dependencies, or a state that could report success before
   four-rank agreement.
3. Is the topology identity sufficient for every cn4 PCIe layout and route?
   Can peer enablement partially succeed, leak across abort, or allow a rank
   to select NCCL/custom fallback locally?
4. Are module capabilities, measured context/module residency, deterministic
   arenas, load staging, KV floors, graph/scratch/collective slabs, tier
   staging, and diagnostic escrow ordered and accounted without hidden HBM or
   a serving-step allocation?
5. Does the two-phase weight upload/adoption compose correctly with rank-owned
   streams and RAII? Can any graph resolve a tentative pointer, can a late
   rank failure leak adopted state, and is abort/free exactly once?
6. Is cooperative four-rank graph capture sufficient for NCCL/custom
   collectives? Specify capture barriers and failure cleanup closely enough
   to prevent ranks capturing different ordinal sequences or exposing a
   partial graph. Is production eager fallback impossible?
7. Does the stream/event DAG preserve page/argument upload visibility,
   compute ordering, bounded D2H completion, and nonaliasing tier overlap
   without device-wide synchronization or hidden default-stream dependencies?
8. Are argument/completion ring states and generation checks sufficient
   against host mutation, slab ABA, stale graphs, stale page tables, pointer
   reuse, early output visibility, and a slow consumer holding GPU capacity?
9. Can the device-validation latch safely make downstream work inert while
   every rank still enters the exact fixed collective counts and ordinals?
   Which routes require a separate four-rank prelaunch validation graph?
   Identify any kernel that could dereference a bad pointer before checking
   the latch.
10. Does the route registry correctly separate associative hidden reductions
    from fixed-rank sampling, candidate, and LSE numerical ABIs? Can NCCL,
    direct P2P, ring, tree, or pair hierarchy violate required order,
    precision, payload bytes, graph capture, or participant semantics?
11. Does `RankStepCommand` carry every immutable identity and rank-specific
    projection required by target/MTP, sampling, page transactions, graph
    admission, and checkpoint adoption? Which pending contract must be v2
    rather than amended before this can become implementable?
12. Is four-rank prepare/launch/completion consensus atomic with page,
    token, RNG, pending-MTP, and output commit? Can `CACHE_ONLY`, cancellation,
    retry, or one emitted-but-unmaterialized MTP token create divergent host
    and device generations?
13. Are timeout and asynchronous-error semantics honest? Does the supervisor
    avoid claiming CUDA/NCCL cancellation, stop every future ordinal, retain
    usable diagnostics, and force process/generation replacement when safe
    cleanup cannot be proven?
14. Does one dependency-linked model step at a time still support continuous
    multi-user batching, argument preparation, tier I/O, and later reviewed
    MIXED graphs without accidentally serializing HTTP/tokenization/cache
    work or running overlapping TP4 collective chains?
15. Is the proposed native ABI narrow and versionable? Flag unsafe raw-handle,
    callback, pointer-lifetime, exception, allocator, C++ ownership, or ABI
    layout gaps that safe Rust cannot contain.
16. Does the CPU/mock gate cover every state, resource, fault, C1/C64 shape,
    MTP0–6 route, ABA, cleanup, and no-allocation property before CUDA? Are the
    later cn4 gates ordered and scoped without overstating success?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then state
separately whether:

- the Rust ownership/thread/resource lifecycle is accepted;
- startup, load adoption, graph capture, and shutdown are accepted;
- stream/event/argument/completion/device-validation semantics are accepted;
- collective routing and fixed-order numerical boundaries are accepted;
- command/receipt/transaction/watchdog integration is accepted; and
- the CPU/mock and later cn4 gate sequence is accepted.

Only if all six answers are unqualified `YES`, end with the requested token.
Do not emit it for a conditional pass, stale input, an ABI deferred to
implementation, or a design that silently depends on pending contradictory
contracts.

The token accepts only this design and opens coordinated CPU/mock
implementation after its prerequisite ABIs are accepted. It does not accept
the current Rust worker as production, authorize cn4, authorize CUDA, accept
any kernel/collective/graph, permit checkpoint conversion, or establish
quality, speed, capacity, concurrency, tiering, prefix, or serving claims.

