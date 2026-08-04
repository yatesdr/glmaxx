# Fable handoff: physical deterministic small-checkpoint runner r3

Date: 2026-08-04

Status: adversarial corrective M4 design review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4, inspect a checkpoint, launch CUDA, create a context, or
modify a runtime resource for this review.

Review candidate commit:
`2d5721db1dfd03890e6c260bb5b9f95ba6c04266`

Required result path:
`docs/reviews/fable-small-checkpoint-runner-v1-r3.md`

Requested acceptance token, only if every blocker and major is resolved:
`small-checkpoint-runner-v1-r3-design-accepted`

This handoff supersedes the unexecuted r1/r2 M4 handoffs and tokens. Review
r1-r3 together. Current Rust implements no M4 target program or runner.

## Provenance

Review the exact candidate in a detached worktree. Hash every input at start
and finish. Any mismatch withholds the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `spec/engine-v0.md` | `52497e022bde5278372a9bce168e87a602fca9341c4cb4e019b4a3c7ce63179b` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `docs/small-checkpoint-runner-v1.md` | `f35c71debd3d74d22cda435b60d3a7485592f96e41889481cc479f2c283b75f7` |
| `docs/small-checkpoint-runner-v1-r2.md` | `3e7bb597981c86ed63190c67463911e3aeca192c6aa818b7e58132cac716993e` |
| `docs/small-checkpoint-runner-v1-r3.md` | `223042c553ce0584737590217251d841ca6bb8991c236f6167f47068fd452047` |
| `docs/tp4-layer6-replay-v1-r2.md` | `1a9c1819548cb0550b3060e558991ce3b2a12844f1b853c28ae2f5d68811068a` |
| `docs/nvfp4-laboratory-manifest-v1.md` | `8a0adb54dedfab1dba0afcf09579614ce567da92fda43134b0c404af5aafb0ee` |
| `docs/nvfp4-fused-routed-moe-v1-r3.md` | `f60d3adb777321ed715ae465abcf20895dbbf470c20970855636ed9b2b3a4db0` |
| `docs/target-layer-execution-v1.md` | `89c6cf7397a3dc6b0c01383e679dcc4b51e20e3c45057bef0928dc24b866a819` |
| `docs/target-layer-execution-v1-r2.md` | `3b70e5d4b74aa66c41c855b71f282e64ed726c86ce78161260d12dca596934eb` |
| `docs/target-layer-execution-v1-r3.md` | `97c2c3615384dddc6204e910fe3c498fdd7a26554ed8aecec790d62f72c2ad87` |
| `docs/target-graph-physical-memory-v1.md` | `135e7d61f5ce7cc94d200648e9691b9d76edaee13025c21e88f0ad2c07018bc9` |
| `docs/sm120-rank-executor-v1-r4.md` | `6397a07c5a00422b0e3a3941e880a0548fe21b1e5d7584967d5a2786d7f1e665` |
| `docs/sm120-rank-executor-v1-r5.md` | `85c1082575c4b4d9dbdf26affe499121339c8a3a3f7f914ff5957ff6bee7f565` |
| `docs/sm120-rank-executor-native-abi-v1.h` | `25de8f1f2a81d3ff8f39cee71eb984bfd999abb08eaebb39e9690cbed49c71bb` |
| `docs/prefill-graph-profile-abi-v2.md` | `37154c9e31109acdf35a382c6be87b3a865e2b7f6ae8f801969526789dd41f91` |
| `docs/step-execution-abi-v3.md` | `1cde3bcabba0a0d861691b06ddb140cb64dfbefaab1129c8a04bc302c0ce609e` |
| `docs/distributed-sampling-abi-v1-r2.md` | `061903d0a0cf2a284f35b177da5f1c3484cb61dfd9020f627db7b5632a4f2b6b` |
| `docs/checkpoint-load-transaction-v1.md` | `bc3c938f488bdcbf002c788ce9c5ac493addfe81866f39875d583c4312842ccf` |
| `crates/glm-engine/src/graph.rs` | `c85ca1aa52ba42294fc6a43524e8f70357523977d343e8c2f212787e7754cd22` |
| `crates/glm-engine/src/checkpoint_load.rs` | `77a331e7a6ecae4e04c1677f9380007eef432ca4e21d2fd4c2bc64b42facfab3` |
| `crates/glm-engine/src/worker.rs` | `52dbb32ef45bfa652ea113b7c3db7e4fb200bfd778015abb1aebceabaddf89d6` |
| `crates/glm-engine/src/startup.rs` | `1a5f1ac8aae94e6eb2aaf2cf4701dfc290604013103eacc1423046211609a5fc` |
| `crates/glm-engine/src/memory.rs` | `2131c999b6762a9b7e505cfe542c957877d95af4ee04056affa9d677156e9491` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |

Run the full local gate and:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-small-checkpoint-runner-v1-r3-handoff.md
```

## Required independent work

1. Recheck every retained r1/r2 source, conversion, manifest, file/load,
   owner-thread, fixture, numerical, reset, fault, evidence, and nonclaim
   requirement for regression.
2. Independently compile the layer-6 and final-head program: prove 531 plus 2
   equals 533 bindings, all 16-byte records are exact, and no embedding or
   omitted layer can enter.
3. Serialize every laboratory target-program preimage. Attack production,
   M3, old-domain, layer-only, no-head, wrong sampling, catalog, projection,
   layout, and text-hash substitutions. Check the construction for cycles.
4. Recompute all `M4Program.v1` offsets and prove 32 header bytes plus sixteen
   hashes is exactly 544 bytes with no overlap or missing identity.
5. Prove M4 names the all-NVFP4 laboratory M3 result with identical
   source/control/fixture lineage and rejects capacity or hybrid M3 evidence.
6. Trace program-set, module-set, GraphMemoryPlan, GraphProfile v3, resource
   budget, final memory plan, and all ten rank-local arena bindings through
   graph readiness and each execution permit.
7. Independently rederive arena-8/9 bytes, then prove class 30 owns exactly
   309,760 bytes of arena-5 CURRENT/NEXT pending logits and class 26 owns
   exactly 154,880 bytes of arena-2 rank-logit scratch for both graph shapes.
   Prove proposal/draft state and the MTP program remain absent.
8. Enumerate every primary/auxiliary/metadata/page-table use and prove no raw
   address or descriptor-selected capacity reaches the executor.
9. Attack one-byte-short uses/classes/arenas/budgets, stale arena/module/
   program/page/cache/output generations, eager/captured drift, and rank-local
   fallback.
10. Trace the laboratory state machine and prove no 533-tensor handle,
    program, graph, permit, result, or token can reach any production type.
11. Inspect current Rust and enumerate every missing target compiler, M4
    record, physical proof, loader/executor binding, reference, graph, and
    receipt. Do not promote current code or cn4 evidence.

## Required decisions

Answer each with an unqualified `YES` or `NO`:

1. Does r3 completely close r2's unspecified target-program, logical-only
   graph, missing physical-memory, and stale module/program-generation gaps?
2. Is the 533-binding laboratory target program exact, canonical,
   cycle-free, and mutually substitution-resistant from every M3/production
   program?
3. Is the 544-byte M4 record complete, canonical, and sufficient to identify
   one source/load/program/physical-graph/sampling execution?
4. Are all ten arenas and every immutable/mutable use charged and bound once,
   including nonzero MTP0 pending logits with no proposal/draft program?
5. Are resource construction, weight adoption, graph readiness, fixture
   readiness, execution permits, reset, receipts, and cleanup implementable
   in the stated owner-thread order with no rank-local fallback?
6. Is the CPU/mock gate sufficient to implement the exact truncated
   layer-6-to-head runner, distributed greedy, repetition/fault matrix, and
   physical validation before CUDA?
7. Is the diagnostic M4 sampler correctly isolated from production
   prefill/decode token-feedback and unable to satisfy the full-batch gate?
8. Are all implementation, cn4, checkpoint-result, full-model, quality,
   capacity, concurrency, serving, reload, and performance nonclaims
   accurate?

Only if every answer is `YES`, end with the requested token as the only bare
acceptance line. Withhold for stale provenance, wrong binding/byte counts,
an ambiguous target program, a hash cycle, M3 lineage drift, logical-only
graph acceptance, missing pending-logit charge, MTP leakage, raw addresses,
rank-local fallback, production type escape, incomplete CPU proof, or runtime
overstatement.

The token opens only the corrected M4 CPU/mock implementation after all named
predecessor designs are accepted. It authorizes no cn4 or CUDA work.
