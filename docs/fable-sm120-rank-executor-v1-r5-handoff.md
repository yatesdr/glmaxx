# Fable handoff: SM120 rank executor v1 corrective r5

Date: 2026-08-04

Status: superseding adversarial design and native-ABI review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4, launch CUDA, create a context, or modify a runtime
resource for this review.

Review candidate commit:
`f07b3e25716cd91112b48d3cc659fde51f667c50`

Required result path:
`fable-sm120-rank-executor-v1-r5.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`sm120-rank-executor-v1-r5-design-accepted`

This handoff supersedes the unexecuted r2-r4 handoffs. Do not issue their
tokens or authorize implementation from their header bytes. Review r1-r5
together using the corrected header at this candidate.

The original withheld review remains a required operator-inbox input at
`docs/reviews/fable-sm120-rank-executor-v1.md`, SHA-256
`efe697b86235ac757fcfd9123d0f28b92a37f337a75f64267d8e96862620dd36`.
Hash it before using any closure claim and again at review finish. Withhold
the r5 token if it is absent or differs.

## Provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Any mismatch withholds the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `spec/engine-v0.md` | `52497e022bde5278372a9bce168e87a602fca9341c4cb4e019b4a3c7ce63179b` |
| `docs/sm120-rank-executor-v1.md` | `e97c54b865ed50c40ff8b15f6580d0edc18dbd0783135bc1c17d11cc19986fd4` |
| `docs/sm120-rank-executor-v1-r2.md` | `4f40ea7652858b4cebbe4093dc81149cb30aa26bedc69edef72fa627c987df89` |
| `docs/sm120-rank-executor-v1-r3.md` | `1bdceee409ec871edc4e193d967848e401f965e6f45d7a99782a7e444352cee8` |
| `docs/sm120-rank-executor-v1-r4.md` | `6397a07c5a00422b0e3a3941e880a0548fe21b1e5d7584967d5a2786d7f1e665` |
| `docs/sm120-rank-executor-v1-r5.md` | `85c1082575c4b4d9dbdf26affe499121339c8a3a3f7f914ff5957ff6bee7f565` |
| `docs/sm120-rank-executor-native-abi-v1.h` | `25de8f1f2a81d3ff8f39cee71eb984bfd999abb08eaebb39e9690cbed49c71bb` |
| `docs/target-graph-physical-memory-v1.md` | `135e7d61f5ce7cc94d200648e9691b9d76edaee13025c21e88f0ad2c07018bc9` |
| `docs/target-layer-execution-v1.md` | `89c6cf7397a3dc6b0c01383e679dcc4b51e20e3c45057bef0928dc24b866a819` |
| `docs/target-layer-execution-v1-r2.md` | `3b70e5d4b74aa66c41c855b71f282e64ed726c86ce78161260d12dca596934eb` |
| `docs/mtp-layer-execution-v1-r2.md` | `d75710b3b552f229cc3bef34a8977a7c30e5b03b4c4a268f27c0efb2a3d1f12c` |
| `docs/mtp-layer-execution-v1-r3.md` | `5440eb54c41b977a1fe5716357e32d99a05b1f279289c95b8ac89f24bb6d4d27` |
| `docs/step-execution-abi-v3.md` | `1cde3bcabba0a0d861691b06ddb140cb64dfbefaab1129c8a04bc302c0ce609e` |
| `crates/glm-engine/src/worker.rs` | `52dbb32ef45bfa652ea113b7c3db7e4fb200bfd778015abb1aebceabaddf89d6` |
| `crates/glm-engine/src/startup.rs` | `1a5f1ac8aae94e6eb2aaf2cf4701dfc290604013103eacc1423046211609a5fc` |
| `crates/glm-engine/src/graph.rs` | `c85ca1aa52ba42294fc6a43524e8f70357523977d343e8c2f212787e7754cd22` |
| `crates/glm-engine/src/memory.rs` | `2131c999b6762a9b7e505cfe542c957877d95af4ee04056affa9d677156e9491` |
| `scripts/local-checks.sh` | `1675ca5bac9bda032ab6db206629bc0052ad6afc5cb6f59a7b6697e4e5c779d0` |

Run the full local gate plus C11/C++17 header checks. Independently emit all
field offsets, sizes, alignments, enum values, and 35 function signatures
against a separate Rust mirror. Current Rust does not implement r5.

## Required retained review

Repeat every retained r2-r4 closure question against the conjunction of r1-r5.
Do not inherit an answer from an unexecuted prior handoff. In particular,
repeat the explicit validation-module, complete target/optional-MTP program
set, owner-thread, startup/memory, route, synchronization, fatal cleanup, and
hot-reload decisions.

## R5 independent work

1. Confirm the role-5 defect is real: target-only MTP0 has persistent pending
   logits even when proposal and draft-KV state are absent.
2. Prove the broader recurrent-state role covers each named cross-step state
   without merging lifetime or accounting identities.
3. Compile the renamed enum and capability record in C11/C++17/Rust. Confirm
   role value 5, capability size 192, alignment 16, graph-memory field
   `128..160`, and four zero reserved `u64` values at `160..192`.
4. Independently hash the NUL-terminated graph-memory ABI domain. The required
   value is
   `68ac6d6113973e61f863980f1b42a7479466164fc795f91560f0a15a4614d3b8`.
5. Independently implement the family-capability and ordered module-set digest
   formulas; mutate every field and prove the appropriate digest changes.
6. Attack zero, old, mixed, unknown, and rank-divergent graph-memory ABI
   identities in target, MTP, and validation capabilities.
7. Keep old and candidate modules resident together and prove the explicit
   r4 validation-module handle plus r5 identities prevent cross-generation
   parser/program/arena mixing.
8. Trace GraphProfile v3, physical plan, executor program set, module set,
   resource budget, all ten rank-local bindings, every immutable tensor
   plane, the device page table, and validation input through graph
   construction. No identity may be selected locally or after capture.

## Required decisions

Answer each with an unqualified `YES` or `NO`:

1. Are every retained r2-r4 correction and the complete native header still
   accepted under r5?
2. Does recurrent-state role 5 give pending logits and all MTP state one exact
   executor home without double charging or a draft-only MTP0 contradiction?
3. Are the revised C11/C++17/Rust enum and 192-byte capability layouts exact?
4. Are the graph-memory ABI, family-capability, and module-set digest formulas
   complete and substitution-resistant?
5. Do explicit validation-module and program-set bindings compose with the
   complete ten-arena physical plan on one immutable
   weight/page/module/graph/resource generation?
6. Are owner-thread construction, graph capture, hot reload, synchronization,
   failure, and destruction rules implementable and fail-closed?
7. Is the combined r1-r5 design accepted for its coordinated CPU/mock proof?
8. Are all current-Rust, native-library, cn4, graph, checkpoint, capacity,
   quality, concurrency, hot-reload, and performance nonclaims accurate?

Only if every answer is `YES`, end with the requested token as the only bare
acceptance line. Withhold for stale provenance, an ambiguous role-5 home,
layout drift, a digest that omits a module/table generation, validation-module
fallback, cross-generation mixing, incomplete retained review, or any runtime
overstatement.
