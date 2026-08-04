# Fable handoff: profile-specific TP4 layer replay r2

Date: 2026-08-04

Status: adversarial corrective M3 design review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4, inspect a checkpoint, launch CUDA, create a context, or
modify a runtime resource for this review.

Review candidate commit:
`2d5721db1dfd03890e6c260bb5b9f95ba6c04266`

Required result path:
`docs/reviews/fable-tp4-layer6-replay-v1-r2.md`

Requested acceptance token, only if every blocker and major is resolved:
`tp4-layer6-replay-v1-r2-design-accepted`

This handoff supersedes the unexecuted v1 M3 handoff and token. Review the
base and r2 together. Current Rust implements neither replay program.

## Provenance

Review the exact candidate in a detached worktree. Hash every input at start
and finish. Any mismatch withholds the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `spec/engine-v0.md` | `52497e022bde5278372a9bce168e87a602fca9341c4cb4e019b4a3c7ce63179b` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `docs/tp4-layer6-replay-v1.md` | `70157b10753c7e043e48566e219cca0ca1e29f596c198ff7a153f576192bee66` |
| `docs/tp4-layer6-replay-v1-r2.md` | `1a9c1819548cb0550b3060e558991ce3b2a12844f1b853c28ae2f5d68811068a` |
| `docs/target-layer-execution-v1.md` | `89c6cf7397a3dc6b0c01383e679dcc4b51e20e3c45057bef0928dc24b866a819` |
| `docs/target-layer-execution-v1-r2.md` | `3b70e5d4b74aa66c41c855b71f282e64ed726c86ce78161260d12dca596934eb` |
| `docs/target-layer-execution-v1-r3.md` | `97c2c3615384dddc6204e910fe3c498fdd7a26554ed8aecec790d62f72c2ad87` |
| `docs/target-graph-physical-memory-v1.md` | `135e7d61f5ce7cc94d200648e9691b9d76edaee13025c21e88f0ad2c07018bc9` |
| `docs/sm120-rank-executor-v1-r4.md` | `6397a07c5a00422b0e3a3941e880a0548fe21b1e5d7584967d5a2786d7f1e665` |
| `docs/sm120-rank-executor-v1-r5.md` | `85c1082575c4b4d9dbdf26affe499121339c8a3a3f7f914ff5957ff6bee7f565` |
| `docs/sm120-rank-executor-native-abi-v1.h` | `25de8f1f2a81d3ff8f39cee71eb984bfd999abb08eaebb39e9690cbed49c71bb` |
| `docs/exl3-mixed-k-source-and-kernel-v1-r3.md` | `683bec3908a0650a4cef7d53075c5438f7d15473d631f62da1de3cd70d8e2866` |
| `docs/nvfp4-fused-routed-moe-v1-r3.md` | `f60d3adb777321ed715ae465abcf20895dbbf470c20970855636ed9b2b3a4db0` |
| `docs/sm120-w4a16-nf3-fused-moe-v1-r2.md` | `311d1214ad57e97c7bab45069fae5507602c0e21922b1fde677ba129e734f265` |
| `docs/nvfp4-laboratory-manifest-v1.md` | `8a0adb54dedfab1dba0afcf09579614ce567da92fda43134b0c404af5aafb0ee` |
| `docs/hybrid-serving-manifest-v1.md` | `934787ea37a5dbd9b6778844adbeb0b40fd365d4653991fc7cbfe77df3c685cf` |
| `docs/prefill-graph-profile-abi-v2.md` | `37154c9e31109acdf35a382c6be87b3a865e2b7f6ae8f801969526789dd41f91` |
| `docs/step-execution-abi-v3.md` | `1cde3bcabba0a0d861691b06ddb140cb64dfbefaab1129c8a04bc302c0ce609e` |
| `docs/small-checkpoint-runner-v1-r3.md` | `223042c553ce0584737590217251d841ca6bb8991c236f6167f47068fd452047` |
| `crates/glm-engine/src/graph.rs` | `c85ca1aa52ba42294fc6a43524e8f70357523977d343e8c2f212787e7754cd22` |
| `crates/glm-engine/src/checkpoint_load.rs` | `77a331e7a6ecae4e04c1677f9380007eef432ca4e21d2fd4c2bc64b42facfab3` |
| `crates/glm-engine/src/worker.rs` | `52dbb32ef45bfa652ea113b7c3db7e4fb200bfd778015abb1aebceabaddf89d6` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |

Run the full local gate and:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-tp4-layer6-replay-v1-r2-handoff.md
```

## Required independent work

1. Recheck every retained v1 fixture, target-math, indexer-reuse, cache,
   reference, fault, timing, evidence, and nonclaim decision for regression.
2. Derive the layer-6/7 closed weight membership for capacity TR3, production
   hybrid, and all-NVFP4 laboratory replay. Prove none can satisfy another.
3. Independently serialize all three target-program/record-stream preimages.
   Recompute `531 + 526 = 1,057` for the laboratory replay and prove it is not
   the 533-tensor M4 program.
4. Recompute every `ReplayProgram.v2` offset and prove 32 header bytes plus
   fourteen hashes is exactly 480 bytes with no overlap or omitted identity.
5. Attack every profile/program/catalog/graph/plan/program-set/module-set/
   budget/schedule/fixture/numerical/route/codec cross-substitution.
6. Trace the accepted acyclic resource-budget -> GraphMemoryPlan ->
   GraphProfile-v3 -> final-memory-plan order. Find any hidden hash cycle.
7. Reconstruct all ten arena roles. Determine whether the unused 256-byte
   arena-5 guard is compatible with the physical-memory/executor contracts
   and cannot mask a missing class or recurrent use.
8. Prove every weight/metadata/page-table pointer is an owner-derived use and
   the four rank-local binding tables preserve one common program/plan.
9. Prove eager/captured paths are matched but cannot create a serving eager
   fallback, and no MTP program/member can enter M3.
10. Trace each profile-specific M3 result to only its matching checkpoint
    successor; in particular, prove M4 requires the laboratory M3 lineage,
    not hybrid or capacity evidence.
11. Inspect current Rust and enumerate every missing type, compiler, record,
    physical-plan proof, fixture/oracle, owner binding, and receipt. Do not
    promote current code or cn4 evidence.

## Required decisions

Answer each with an unqualified `YES` or `NO`:

1. Does r2 close every stale v1 graph, executor, physical-memory, and
   profile-program identity without weakening the retained M3 gate?
2. Are the three replay profiles necessary, complete, and mutually
   substitution-resistant, including the separate M4 predecessor?
3. Are the 480-byte record and every target/record/program hash domain exact,
   canonical, cycle-free, and implementable?
4. Do the target-only program set, module set, resource budget,
   GraphMemoryPlan, GraphProfile v3, ten arenas, and four rank-local bindings
   identify one executable physical replay with no raw-address escape?
5. Are class 30/recurrent uses correctly absent while the arena-5 guard is
   explicit, charged, unused, and safe?
6. Does the CPU/mock gate cover all three real profile semantics, physical
   tables, reference computation, cache/winner lifetime, faults, cleanup, and
   bounded resources before CUDA?
7. Are all implementation, cn4, checkpoint, full-model, quality, capacity,
   concurrency, serving, reload, and performance nonclaims accurate?

Only if every answer is `YES`, end with the requested token as the only bare
acceptance line. Withhold for stale provenance, a profile/type escape,
laboratory/hybrid conflation, wrong count or record size, a hash cycle,
logical-only graph acceptance, missing arena/use, raw-address injection,
rank-local fallback, M3/M4 bypass, incomplete CPU proof, or runtime
overstatement.

The token opens only the coordinated M3 CPU/mock implementation after all
named predecessor designs are accepted. It authorizes no cn4 or CUDA work.
