# Fable handoff: target graph physical-memory ABI v1

Date: 2026-08-04

Status: adversarial design review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4, launch CUDA, inspect a checkpoint, or mutate a runtime
resource. This is a source, serialization, checked-arithmetic, lifetime, and
CPU-proof design review only.

Review candidate commit:
`f07b3e25716cd91112b48d3cc659fde51f667c50`

Required result path:
`fable-target-graph-physical-memory-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`target-graph-physical-memory-v1-design-accepted`

## Provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Any mismatch withholds the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `spec/engine-v0.md` | `52497e022bde5278372a9bce168e87a602fca9341c4cb4e019b4a3c7ce63179b` |
| `docs/target-layer-execution-v1.md` | `89c6cf7397a3dc6b0c01383e679dcc4b51e20e3c45057bef0928dc24b866a819` |
| `docs/target-layer-execution-v1-r2.md` | `3b70e5d4b74aa66c41c855b71f282e64ed726c86ce78161260d12dca596934eb` |
| `docs/target-graph-physical-memory-v1.md` | `135e7d61f5ce7cc94d200648e9691b9d76edaee13025c21e88f0ad2c07018bc9` |
| `docs/prefill-graph-profile-abi-v2.md` | `37154c9e31109acdf35a382c6be87b3a865e2b7f6ae8f801969526789dd41f91` |
| `docs/step-execution-abi-v3.md` | `1cde3bcabba0a0d861691b06ddb140cb64dfbefaab1129c8a04bc302c0ce609e` |
| `docs/sm120-rank-executor-v1.md` | `e97c54b865ed50c40ff8b15f6580d0edc18dbd0783135bc1c17d11cc19986fd4` |
| `docs/sm120-rank-executor-v1-r2.md` | `4f40ea7652858b4cebbe4093dc81149cb30aa26bedc69edef72fa627c987df89` |
| `docs/sm120-rank-executor-v1-r3.md` | `1bdceee409ec871edc4e193d967848e401f965e6f45d7a99782a7e444352cee8` |
| `docs/sm120-rank-executor-v1-r4.md` | `6397a07c5a00422b0e3a3941e880a0548fe21b1e5d7584967d5a2786d7f1e665` |
| `docs/sm120-rank-executor-v1-r5.md` | `85c1082575c4b4d9dbdf26affe499121339c8a3a3f7f914ff5957ff6bee7f565` |
| `docs/sm120-rank-executor-native-abi-v1.h` | `25de8f1f2a81d3ff8f39cee71eb984bfd999abb08eaebb39e9690cbed49c71bb` |
| `docs/mtp-layer-execution-v1-r2.md` | `d75710b3b552f229cc3bef34a8977a7c30e5b03b4c4a268f27c0efb2a3d1f12c` |
| `docs/mtp-layer-execution-v1-r3.md` | `5440eb54c41b977a1fe5716357e32d99a05b1f279289c95b8ac89f24bb6d4d27` |
| `docs/sm120-w4a16-nf3-fused-moe-v1-r2.md` | `311d1214ad57e97c7bab45069fae5507602c0e21922b1fde677ba129e734f265` |
| `docs/nvfp4-fused-routed-moe-v1-r3.md` | `f60d3adb777321ed715ae465abcf20895dbbf470c20970855636ed9b2b3a4db0` |
| `docs/tp4-layer6-replay-v1.md` | `70157b10753c7e043e48566e219cca0ca1e29f596c198ff7a153f576192bee66` |
| `docs/small-checkpoint-runner-v1-r2.md` | `3e7bb597981c86ed63190c67463911e3aeca192c6aa818b7e58132cac716993e` |
| `crates/glm-engine/src/graph.rs` | `c85ca1aa52ba42294fc6a43524e8f70357523977d343e8c2f212787e7754cd22` |
| `crates/glm-engine/src/memory.rs` | `2131c999b6762a9b7e505cfe542c957877d95af4ee04056affa9d677156e9491` |
| `scripts/local-checks.sh` | `1675ca5bac9bda032ab6db206629bc0052ad6afc5cb6f59a7b6697e4e5c779d0` |

Run:

```text
./scripts/local-checks.sh
clang -std=c11 -x c \
  -include docs/sm120-rank-executor-native-abi-v1.h \
  -fsyntax-only /dev/null
clang++ -std=c++17 -x c++ \
  -include docs/sm120-rank-executor-native-abi-v1.h \
  -fsyntax-only /dev/null
```

Green current Rust tests are only compatibility evidence. No current Rust
type implements the proposed physical-memory ABI.

## Review purpose

Determine whether the candidate closes target-layer r2's explicit launch
blocker without hiding physical capacity behind aggregate scratch, accepting
rank-local pointers, creating a profile/memory hash cycle, or treating a
logical alias class as an allocation.

## Required independent work

1. Re-add the exact 32-byte arena, 48-byte class-span, 80-byte use, 480-byte
   plan, 48-byte binding, 40-byte resolved-span, and 288-byte receipt layouts.
2. Independently serialize every table/record and prove each self-hash excludes
   exactly its digest field with no native padding.
3. Build a miniature graph with ordinary, dynamic-indexed, class-zero,
   collective, status, external, resident-weight, codec-metadata, and device
   page-table uses; reconstruct every maximum end and aligned class capacity.
4. Mutation-test count/order/enum/flag/reserved/hash fields, checked-add
   overflow, zero/invalid alignment, one-byte-short use/class/arena, and
   incorrect maximum-consumed values.
5. Model the complete DAG/lifetime rule and exhaust overlapping half-open
   intervals: dead scratch reuse may pass; live, zero-alias, argument,
   target-KV/indexer, pending-logit, recurrent, collective, and status overlap
   must fail.
6. Exercise classes 28..30 through immutable dynamic address tables and prove
   an active address cannot escape its static envelope.
7. Exercise class-zero MTP state in recurrent arena 5 and prove it cannot
   overlap live class-30 pending logits. Independently resolve every required
   tensor plane through arenas 8/9 and the device page table through arena 10;
   reject a missing plane, rank-layout drift, write access, stale generation,
   or one-byte-short range.
8. Reconstruct GraphProfile v3 and the resource-budget -> physical-plan ->
   profile -> final-memory-plan order. Prove there is no digest cycle and no
   byte can be omitted or charged twice.
9. Materialize four distinct rank-local device-address tables from one common
   plan. Prove no coordinator/request pointer can enter a native span and all
   common/rank-local receipt fields are classified correctly.
10. Trace target-only prefill/decode and target-plus-MTP verify through the r4
    program-set rule and the r5 module graph-memory capability binding.
11. Show that a compatible hot reload can prepare/rollback a new module and
    graph generation without a weight read/H2D or arena resize.
12. Identify every still-missing exact operator common plan needed to produce
    actual M1/C64/M3072 class/use bytes; do not infer those results from this
    generic ABI design.

## Required decisions

Answer each with an unqualified `YES` or `NO`:

1. Is the diagnosed target-layer launch blocker real and completely closed at
   the design/serialization boundary?
2. Are all ten logical arenas, all 32 classes, all node uses, and every
   common versus rank-local identity exact and implementable?
3. Do dynamic-indexed external writes and arena-level recurrent/collective/
   status/weight/metadata/page-table uses remain complete, read/write-correct,
   generation-bound, and in bounds?
4. Does the validator reject every undersized subrange independently of the
   aggregate scratch total?
5. Is physical reuse limited to proven dead scratch intervals, with all
   external and zero-alias storage nonoverlapping?
6. Is GraphProfile v3 bound to one plan per logical entry without a hash
   cycle or a route around the final memory charge?
7. Are program-set, module-set, graph-memory ABI, resource-budget, arena, and
   generation bindings substitution-resistant on all four ranks?
8. Is owner-thread construction/capture/receipt order compatible with the
   executor and safe under hot-reload rollback?
9. Does the required CPU/mock proof precede any M3/M4 or production launch and
   cover the full claim surface?
10. Are all CUDA, model, checkpoint, KV-capacity, quality, serving, hot-reload,
    and performance nonclaims accurate?

Only if every decision is `YES`, end with the requested token as the only bare
acceptance line. Withhold for stale provenance, an omitted graph-visible
arena or tensor plane, offset arithmetic drift, incomplete consumer
reconstruction, dynamic-address escape, a live alias, aggregate-only
capacity, a hash cycle, raw-pointer input, cross-rank fallback, an unbound
module interpretation, incomplete CPU gates, or any execution/capacity/
performance overstatement.
