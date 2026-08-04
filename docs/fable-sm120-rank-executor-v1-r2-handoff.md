# Fable handoff: SM120 rank executor v1 corrective r2

Date: 2026-07-30

Status: focused adversarial re-review requested

Review candidate commit:
`a0f2bee3edd1754aebefe1643eecd0a63cd4d4b7`

Required result path:
`docs/reviews/fable-sm120-rank-executor-v1-r2.md`

Requested acceptance token, only for an unqualified pass:
`sm120-rank-executor-v1-accepted`

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch CUDA for this review

The original withheld review is an operator-inbox input at
`docs/reviews/fable-sm120-rank-executor-v1.md`, SHA-256
`efe697b86235ac757fcfd9123d0f28b92a37f337a75f64267d8e96862620dd36`.
Hash it before using the closure summary below. Withhold this r2 token if that
artifact is absent or differs.

One status word in the pinned r2 amendment requires explicit correction for
this review: “the accepted nonblocking transport policy” means the required
executor-local nonblocking `try_push` completion policy. It does not claim or
grant acceptance of `nonblocking-http-transport-v1.md`, whose later transport
review required a corrective successor. Judge the bounded slab-release and
cancel semantics here, while keeping any executor implementation gated on an
independently accepted transport successor.

## Provenance

Review the exact candidate commit in a detached worktree. Copy this handoff
into that worktree if necessary, run `review-proof`, and hash every listed
input at review start and finish. If any input differs from the table, report a
stale candidate and do not emit the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/sm120-rank-executor-v1.md` | `e97c54b865ed50c40ff8b15f6580d0edc18dbd0783135bc1c17d11cc19986fd4` |
| `docs/sm120-rank-executor-v1-r2.md` | `4f40ea7652858b4cebbe4093dc81149cb30aa26bedc69edef72fa627c987df89` |
| `docs/sm120-rank-executor-native-abi-v1.h` | `0d0f0357a17eba4e678d5c82da4dbff552e292fb7948931496a4382289ae4d6e` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/target-layer-execution-v1.md` | `7f16667d55a50441dbc81b7fe18285be5a6f0fb06c68ef008a40aad8f406f478` |
| `docs/mtp-layer-execution-v1.md` | `5ad5bf01cdbd5e183b5e50aa0940344b5aabc09bf05a90c57d58e3e5b28dd3a7` |
| `docs/step-execution-io-v1.md` | `055412c022cfcf9299e95e3ad3f7b888a2d472835388c35c2a8443be71a7422c` |
| `docs/serving-page-transaction-v1.md` | `31983cce95ee01a5968213d5daf12c7a855f75f8735314700f2b4a9e55625d1a` |
| `docs/checkpoint-load-transaction-v1.md` | `184199144075e3f7c511b31002ea81c1d2dbaad1087e01fe7dd4533dc36a3c21` |
| `docs/nonblocking-http-transport-v1.md` | `e1ee381ad46b9f277640e884380aaab11a6a5b23e4f87c7cdea05334e3ebddc5` |
| `crates/glm-engine/src/worker.rs` | `b8498639bb05ef84c2d06eb1e4650d8f7915eb1e3b306abdfd2cc0fb93b104fa` |
| `crates/glm-engine/src/startup.rs` | `1a5f1ac8aae94e6eb2aaf2cf4701dfc290604013103eacc1423046211609a5fc` |
| `crates/glm-engine/src/graph.rs` | `c85ca1aa52ba42294fc6a43524e8f70357523977d343e8c2f212787e7754cd22` |
| `crates/glm-engine/src/memory.rs` | `0ae657905a1b2091980c4904643e35a7a53b282ef112be44447362add89f023b` |
| `crates/glm-cuda/src/ffi.rs` | `870ef7570d8f476cdaf32cea4fc36ac63ab4619b0db63d41262a258feb7d3663` |
| `crates/glm-cuda/src/ownership.rs` | `5ef1c916c356d84a55b00168fd5d69e80dc76ff5cf369d7a21a259002834e5ec` |
| `kernels/include/glmaxx_kernel.h` | `a7ddb56de39dbd22e25184be1a2a767dd43bc3ca5ecafd3dcc771aedebbdcf13` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-sm120-rank-executor-v1-r2-handoff.md

clang++ -std=c++17 -x c++ \
  -include docs/sm120-rank-executor-native-abi-v1.h \
  -fsyntax-only /dev/null

clang -std=c11 -x c \
  -include docs/sm120-rank-executor-native-abi-v1.h \
  -fsyntax-only /dev/null
```

The compile commands are syntax/layout checks only, not native implementation
or GPU evidence.

## First-review closure scope

The first review withheld the token on:

- MAJOR: collective resources were created after graph capture;
- MAJOR: collective/graph internal HBM and pinned mirrors escaped accounting,
  with no strict post-startup escrow check;
- MAJOR: the eight native families had no concrete ABI;
- MINOR: panic field/drop order, coordinator-attributed device operations,
  peer rollback failure, unlisted lifecycle amendment, unversioned
  `StepInput`/sampling counter, no `CACHE_ONLY` tier carrier, deferred route
  classification, blocking slow-client ambiguity, and mock coverage gaps; and
- QUESTION: emitted-but-unmaterialized MTP request ownership, next-step H2D
  overlap, and whether the deadline covers prepare.

The r2 amendment claims to close exactly those items. Findings outside that
scope are welcome if the correction introduces a new contradiction.

## Required adversarial questions

1. Trace the startup order. Do communicator and route resources now exist on
   all ranks before any graph captures them, while adoption and KAT remain at
   `COLLECTIVES_VOTED`?
2. Can communicator creation partially return and allow another rank to
   proceed? Does the one bootstrap ID and four-rank receipt protocol prevent a
   rank-local route set?
3. Independently re-derive the HBM equation. Is context/module residency
   counted exactly once against the pre-context free-HBM floor?
4. Are deterministic arenas, collective-library internal HBM,
   graph-runtime internal HBM, padding, and unallocated emergency escrow
   distinct, bounded terms?
5. Are collective and graph deltas checked without underflow, and are
   equal-to-ceiling and over-ceiling outcomes unambiguous?
6. Does the strict final `min(free_hbm) > escrow` gate occur after graph, KV,
   maximum-workspace, and collective KAT initialization and before health?
7. Independently re-derive the pinned-host formula. Can any engine-owned
   pinned allocation bypass the process cap or the native arena family?
8. Compile and independently inspect every native struct. Do all asserted
   sizes/alignments match the field layouts on a 64-bit C++ ABI, and are all
   reserved bytes and enum values frozen?
9. Is every one of the eight named native families represented by exact entry
   points and descriptors rather than deferred to implementation?
10. Can any C++ exception cross `extern "C"`? Can any native CUDA/NCCL status
    escape the frozen status taxonomy?
11. Trace every constructor and borrowed pointer through normal destruction,
    partial construction, panic unwind, and wrong-thread corruption. Can any
    handle be freed twice or service continue after cleanup cannot be proven?
12. Are module input bytes, async pinned spans, graph descriptors, device
    addresses, route spans, and native-object handles assigned lifetimes that
    safe Rust can enforce?
13. Are communicator abort, route-before-communicator destruction, abort
    failure, and process-exit containment exact enough to implement without a
    local recovery choice?
14. Does the manual Rust resource ordering actually place the context last on
    normal shutdown and panic unwind?
15. Are peer enable/undo operations and the empty-page-table upload commands
    executed only by owner threads? Is undo failure terminal?
16. Are `DRAINING`/`CLOSED`, `StepInput.v2`, `SamplingCounter.v2`, and the
    MTP successor-slot dependencies now explicit amendments rather than
    implied versions?
17. Does `RankTierCommand.v1` completely carry bounded cache-only transfers
    and the page-table transaction? Probe zero, 64, and 65 operations and
    overlapping current/admitted graph spans.
18. Is the route classification exhaustive for every v1 route? Can a
    validation latch ever change a count, pointer, participant set, or
    ordinal, or can an unclassified route enter production?
19. Does the explicit no-next-step-device-H2D rule close page-table/slab
    generation races, and is its latency cost stated honestly?
20. Does the absolute deadline cover queueing and prepare as well as launch
    and completion?
21. Does the executor-local nonblocking `try_push` slow-client policy
    guarantee slab release without waiting for the socket, without treating
    this review as acceptance of the separate transport-v1 contract?
22. Does the serving coordinator terminate, never retry, a request whose
    emitted MTP token was not materialized before generation failure?
23. Does the expanded CPU/mock matrix cover every prior omission plus ABI
    parity, memory ceilings, native unwind/abort, route latch behavior, tier
    overlap, prepare timeout, slow consumer, and pending MTP failure?
24. Did r2 introduce any new silent fallback, allocation, ownership,
    numerical-order, rank-divergence, or hot-path concurrency ambiguity?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer each statement separately:

1. Rust ownership, thread affinity, panic unwind, and cleanup are accepted.
2. Startup, collective creation, graph capture, voting, and shutdown are
   accepted.
3. HBM, pinned-host, graph-internal, collective-internal, and escrow
   accounting are accepted.
4. The native ABI layouts, status/exception rules, lifetimes, ownership, and
   NCCL abort semantics are accepted.
5. Stream, command, tier, deadline, route-validation, backpressure, and MTP
   failure semantics are accepted.
6. The corrected CPU/mock and later authorized cn4 gate sequence is accepted.

Only if all six statements are unqualified `YES`, end with:

```text
sm120-rank-executor-v1-accepted
```

Do not emit the token for a conditional pass, stale input, a struct/layout
error, an ABI choice left to implementation, a hidden allocation, or an
unclosed first-review finding.

The token accepts the combined v1+r2 design and permits only its coordinated
CPU/mock implementation after the prerequisite contract tokens are available.
It does not accept the current worker as a production executor, authorize
cn4/CUDA, accept a graph or collective, authorize checkpoint conversion, or
establish serving correctness, quality, capacity, concurrency, or speed.
