# Fable handoff: full-checkpoint four-row MTP0 batch smoke v1

Date: 2026-08-04

Status: adversarial first-full-batch design review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4, inspect a checkpoint, launch CUDA, create a context, or
modify a runtime resource for this review.

Review candidate commit:
`2d5721db1dfd03890e6c260bb5b9f95ba6c04266`

Required result path:
`docs/reviews/fable-full-checkpoint-batch-smoke-v1.md`

Requested acceptance token, only if every blocker and major is resolved:
`full-checkpoint-batch-smoke-v1-design-accepted`

Current Rust does not implement this runner. Review only the design bytes and
their pinned dependencies.

## Provenance

Review the exact candidate in a detached worktree. Hash every input at start
and finish. Any mismatch withholds the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `spec/engine-v0.md` | `52497e022bde5278372a9bce168e87a602fca9341c4cb4e019b4a3c7ce63179b` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `docs/full-checkpoint-batch-smoke-v1.md` | `03244842531b155259d7c52760e6def81f83ff3f91e35f2bbb44b3fc8ba94870` |
| `docs/tp4-layer6-replay-v1-r2.md` | `1a9c1819548cb0550b3060e558991ce3b2a12844f1b853c28ae2f5d68811068a` |
| `docs/small-checkpoint-runner-v1-r3.md` | `223042c553ce0584737590217251d841ca6bb8991c236f6167f47068fd452047` |
| `docs/target-layer-execution-v1-r3.md` | `97c2c3615384dddc6204e910fe3c498fdd7a26554ed8aecec790d62f72c2ad87` |
| `docs/target-graph-physical-memory-v1.md` | `135e7d61f5ce7cc94d200648e9691b9d76edaee13025c21e88f0ad2c07018bc9` |
| `docs/sm120-rank-executor-v1-r5.md` | `85c1082575c4b4d9dbdf26affe499121339c8a3a3f7f914ff5957ff6bee7f565` |
| `docs/sm120-rank-executor-native-abi-v1.h` | `25de8f1f2a81d3ff8f39cee71eb984bfd999abb08eaebb39e9690cbed49c71bb` |
| `docs/mtp-layer-execution-v1-r3.md` | `cd66910cf8738042d0c5ec8c7fbee69f024db9bde543d379abc7cfba9264de96` |
| `docs/distributed-sampling-abi-v1-r2.md` | `061903d0a0cf2a284f35b177da5f1c3484cb61dfd9020f627db7b5632a4f2b6b` |
| `docs/quality-acceptance-v1-r3.md` | `44392c02bfc84b6813c90b8348033c953624bae436925acd220cf1a0ee6af0cd` |
| `docs/exl3-mixed-k-source-and-kernel-v1-r3.md` | `683bec3908a0650a4cef7d53075c5438f7d15473d631f62da1de3cd70d8e2866` |
| `docs/hybrid-serving-manifest-v1.md` | `934787ea37a5dbd9b6778844adbeb0b40fd365d4653991fc7cbfe77df3c685cf` |
| `docs/nvfp4-laboratory-manifest-v1.md` | `8a0adb54dedfab1dba0afcf09579614ce567da92fda43134b0c404af5aafb0ee` |
| `docs/prefill-graph-profile-abi-v2.md` | `37154c9e31109acdf35a382c6be87b3a865e2b7f6ae8f801969526789dd41f91` |
| `docs/step-execution-abi-v3.md` | `1cde3bcabba0a0d861691b06ddb140cb64dfbefaab1129c8a04bc302c0ce609e` |
| `docs/checkpoint-load-transaction-v1.md` | `bc3c938f488bdcbf002c788ce9c5ac493addfe81866f39875d583c4312842ccf` |
| `crates/glm-engine/src/checkpoint_load.rs` | `77a331e7a6ecae4e04c1677f9380007eef432ca4e21d2fd4c2bc64b42facfab3` |
| `crates/glm-engine/src/checkpoint_cuda.rs` | `8eed3e8302d3b41772b2cbdc74ab2bd1fac27e718510c4243d8466b2d0a10593` |
| `crates/glm-engine/src/graph.rs` | `c85ca1aa52ba42294fc6a43524e8f70357523977d343e8c2f212787e7754cd22` |
| `crates/glm-engine/src/worker.rs` | `52dbb32ef45bfa652ea113b7c3db7e4fb200bfd778015abb1aebceabaddf89d6` |
| `crates/glm-serving/src/backend.rs` | `07a4d53de6755ed8180ff90aed82f9efc71b8461070a909a10891c783a8fcf78` |
| `crates/glm-tokenizer/src/lib.rs` | `aa7a738c58df6618880c8311a8c1fa4b7f9cae46ef2b6988fe51e06ca3358b84` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |

Run the full local gate and:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-full-checkpoint-batch-smoke-v1-handoff.md
```

## Required independent work

1. Prove the gate is strictly after profile-matched M3 and accepted M4 and
   cannot bootstrap any format, loader, kernel, graph, quality, or hardware
   prerequisite.
2. Trace complete capacity and hybrid weight membership from source through
   rank-set adoption. Prove draft weights are resident but unused under MTP0
   and no laboratory/other-profile handle can substitute.
3. Recompute every `BatchSmokeProgram.v1` offset and prove 32 header bytes
   plus twenty hashes is exactly 672 bytes with no overlap or missing identity.
4. Reconstruct the two graph-plan and schedule-set encodings, GraphProfile v3,
   target-only program set, module set, resource budget, final memory plan,
   and four rank-local ten-arena tables. Find any hash cycle.
5. Independently prove class 30 owns exactly 1,239,040 bytes of arena-5 C4
   MTP0 pending-logit state and class 26 owns exactly 619,520 bytes of arena-2
   rank-logit scratch. Prove proposal/draft state and an MTP module are absent.
6. Recompute prompt/page/tentative slack for the smoke-minimal cache. Prove it
   uses 1M-safe types/arithmetic without claiming or preventing the later
   524,288-token allocation.
7. Trace token feedback exactly: prefill emits no token and commits CURRENT
   pending logits; sixteen C4 decode steps each sample token 1--16 from prior
   pending state, execute that token into KV/indexer state, and publish the
   next pending state. Prove no reference route/logit/token enters execution.
8. Prove distributed greedy never gathers the full vocabulary at runtime,
   while offline per-position evidence remains complete and correctly scoped.
9. Attack every profile, predecessor, source, catalog, program, module, graph,
   plan, schedule, route, tokenizer, prompt, numerical, cache, generation,
   and rank-local substitution.
10. Analyze early EOS, tie-adjacent output, cache state after the final token,
    fixture reset, five warm runs, and the compatible-reload/deferral boundary
    for ambiguity or overclaim.
11. Exhaust recoverable versus process-fatal faults and prove no partial row,
    stale output, forged cleanup, unsafe free, or one-rank retry can pass.
12. Inspect current Rust and enumerate every missing loader/program/compiler,
    graph/collective/cache integration, reference, tokenizer, receipt, and
    evidence component. Do not promote current CPU tokens or cn4 records.

## Required decisions

Answer each with an unqualified `YES` or `NO`:

1. Is this the minimal honest full-checkpoint-to-text MTP0 gate required after
   M3/M4, with no prerequisite bypass or full-model overclaim?
2. Are capacity TR3 and hybrid source/weight/program/result types complete,
   profile-local, and mutually substitution-resistant?
3. Is the 672-byte program and every graph/schedule/predecessor hash domain
   exact, canonical, cycle-free, and implementable?
4. Do all ten arenas, physical uses, four rank-local bindings, budgets, and
   final plans identify one bounded executable full-model generation with no
   raw-address, hidden-allocation, or rank-local fallback path?
5. Is four-row prefill pending-state publication plus sixteen C4 decode steps
   an exact sixteen-token autoregressive smoke with correct terminal pending,
   cache, and EOS semantics?
6. Are the reference ladder, distributed sampling, offline vocabulary data,
   per-position quality, reset/repetition, and fault gates sufficient to
   detect a wrong full-model execution?
7. Does the smoke-minimal KV posture preserve the later 1M/524,288 design
   without pretending to prove capacity?
8. Is the CPU/mock gate complete and correctly before any full-checkpoint CUDA
   execution?
9. Are all MTP3, service, concurrency, prefix/tier, capacity, quality-campaign,
   cold/hot reload, reliability, and performance nonclaims accurate?

Only if every answer is `YES`, end with the requested token as the only bare
acceptance line. Withhold for stale provenance, gate bypass, profile/type
escape, wrong byte/step arithmetic, hash cycle, hidden/missing memory, MTP
leakage, reference-fed execution, runtime vocabulary gather, incorrect final
cache semantics, partial output, unsafe cleanup, incomplete CPU proof, or any
quality/capacity/service/performance overstatement.

The token opens only the profile-specific CPU/mock runner implementation after
all named predecessors are accepted. It authorizes no cn4 or CUDA work.
