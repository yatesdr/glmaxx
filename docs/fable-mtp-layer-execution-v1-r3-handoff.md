# Fable handoff: recurrent MTP execution v1 r3

Date: 2026-08-04

Status: superseding adversarial corrective-design review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4, launch CUDA, create a context, or modify a runtime
resource for this review.

Review candidate commit:
`3b709499e37c69e2e5411d382c56f21f8b3862a3`

Required result path:
`fable-mtp-layer-execution-v1-r3.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`mtp-layer-execution-v1-r3-design-accepted`

This handoff supersedes the unexecuted r2 handoff. Do not issue its token.
Review r1-r3 together, including the retained findings from the v1 review.
Current Rust does not implement r3.

## Provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Any mismatch withholds the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `spec/engine-v0.md` | `52497e022bde5278372a9bce168e87a602fca9341c4cb4e019b4a3c7ce63179b` |
| `docs/mtp-layer-execution-v1.md` | `5ad5bf01cdbd5e183b5e50aa0940344b5aabc09bf05a90c57d58e3e5b28dd3a7` |
| `docs/mtp-layer-execution-v1-r2.md` | `d75710b3b552f229cc3bef34a8977a7c30e5b03b4c4a268f27c0efb2a3d1f12c` |
| `docs/mtp-layer-execution-v1-r3.md` | `61bfa8e4f7991c1319ef98d3881df33c9bebf4ae247cb91b9ce06c201c8931d0` |
| `docs/distributed-sampling-abi-v1.md` | `383e328a527cc780ed553af0b78382cf200ad60f97afb26d96a2a1494b57c89b` |
| `docs/distributed-sampling-abi-v1-r2.md` | `061903d0a0cf2a284f35b177da5f1c3484cb61dfd9020f627db7b5632a4f2b6b` |
| `docs/target-layer-execution-v1.md` | `89c6cf7397a3dc6b0c01383e679dcc4b51e20e3c45057bef0928dc24b866a819` |
| `docs/target-layer-execution-v1-r2.md` | `3b70e5d4b74aa66c41c855b71f282e64ed726c86ce78161260d12dca596934eb` |
| `docs/target-graph-physical-memory-v1.md` | `9b7827850966e01c1b403bf14f9e717ac57e673d9fc760f1535b29a27ad85f1b` |
| `docs/sm120-rank-executor-v1.md` | `e97c54b865ed50c40ff8b15f6580d0edc18dbd0783135bc1c17d11cc19986fd4` |
| `docs/sm120-rank-executor-v1-r2.md` | `4f40ea7652858b4cebbe4093dc81149cb30aa26bedc69edef72fa627c987df89` |
| `docs/sm120-rank-executor-v1-r3.md` | `1bdceee409ec871edc4e193d967848e401f965e6f45d7a99782a7e444352cee8` |
| `docs/sm120-rank-executor-v1-r4.md` | `6397a07c5a00422b0e3a3941e880a0548fe21b1e5d7584967d5a2786d7f1e665` |
| `docs/sm120-rank-executor-v1-r5.md` | `da87b4dbcb031e4f4cd20c7db372e06434af3b14981c9d01cec1a15eb5659974` |
| `docs/sm120-rank-executor-native-abi-v1.h` | `25de8f1f2a81d3ff8f39cee71eb984bfd999abb08eaebb39e9690cbed49c71bb` |
| `docs/step-execution-abi-v3.md` | `1cde3bcabba0a0d861691b06ddb140cb64dfbefaab1129c8a04bc302c0ce609e` |
| `docs/fixed-page-transaction-v1-r2.md` | `aa5e40db3902425735e43665bd104124970179d15b8354a783c3dd7eb90ca495` |
| `docs/step-execution-io-v1.md` | `055412c022cfcf9299e95e3ad3f7b888a2d472835388c35c2a8443be71a7422c` |
| `docs/serving-page-transaction-v1.md` | `31983cce95ee01a5968213d5daf12c7a855f75f8735314700f2b4a9e55625d1a` |
| `docs/online-prefix-publication-v1.md` | `67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873` |
| `docs/hybrid-mtp3-capacity-ledger-v1-r2.md` | `6efeee90addafb4f8d645610bf617f1f4dd9b1bd630096f570193f407c49c9c6` |
| `docs/prefill-graph-profile-abi-v2.md` | `37154c9e31109acdf35a382c6be87b3a865e2b7f6ae8f801969526789dd41f91` |
| `crates/glm-engine/src/input.rs` | `c3d090429015030416f6c03ddb6fef2dfd569859ff6e0fcc05bcb2d6a163ffa2` |
| `crates/glm-engine/src/output.rs` | `1a82e9990af4e2892831e950f5cd3db256032ab5e43642faeb08229cbf2f1c2c` |
| `crates/glm-engine/src/memory.rs` | `2131c999b6762a9b7e505cfe542c957877d95af4ee04056affa9d677156e9491` |
| `crates/glm-cache/src/mtp.rs` | `1134213f9786eafab9dcb3dd0410f708e5b9addf083140676a523a586968a4b0` |
| `scripts/local-checks.sh` | `1675ca5bac9bda032ab6db206629bc0052ad6afc5cb6f59a7b6697e4e5c779d0` |

Run the full local gate and:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-mtp-layer-execution-v1-r3-handoff.md
```

## Required independent work

1. Recheck every retained r1/r2 conclusion and every v1-review finding against
   r3. Do not inherit an answer from the unexecuted r2 handoff.
2. Recompute the distributed-sampling composite from raw 32-byte hashes and
   attack predecessor, reordered, omitted, and lowercase-text inner hashes.
3. Encode and corrupt every TOP_K/MASS residual result, sampling-trace item,
   and `StepOutput.v2` fallback combination. Prove result, trace, and common
   output cannot disagree across ranks.
4. Independently derive the rank vocabulary interval and binary32 stride.
   Recompute every C64/MTP6 known answer in r3 with checked arithmetic.
5. Trace CURRENT/NEXT pending logits through prefill, MTP0, bootstrap, verify,
   pipelined zero, flush, EOS, cancellation, prelaunch failure, launched
   failure, and consensus. Attack alias, stale generation, wrong slot/rank,
   early swap, and one-byte-short allocation.
6. Determine whether two full pending vectors per live sequence are necessary
   and sufficient, including target-only MTP0, and whether class 30 plus
   recurrent-state arena role 5 gives them exactly one physical home.
7. Independently derive rank-logit row liveness for C1, C64, MTP0, MTP3, and
   MTP6. Attack any reuse of target/MTP head storage whose DAG intervals
   overlap and any double charge outside the single maximum workspace.
8. Reconcile pending, proposal, recurrent, boundary, argument/completion,
   winner, rank-logit, page, graph, collective, and escrow terms through the
   resource budget, physical plan, GraphProfile v3, and SystemMemoryPlan v3.
   Find every missing or double-counted byte.
9. Build target-only and target-plus-MTP program-set/module-set identities.
   Attack old executor roles, old graph profiles, single-program digests,
   mixed module generations, and locally selected routes.
10. Confirm the resource-order graph is acyclic and implementable without
    first-use allocation or descriptor-supplied pointers during capture.
11. Run the coordinated CPU/mock-gate thought experiment for all D0..D6 and
    sampling classes, including every pre/post-launch and hot-reload fault.
12. Trace current Rust and state every missing implementation accurately. No
    cn4, CUDA, checkpoint, quality, KV-capacity, concurrency, reload, or speed
    result may be inferred from this design.

## Required decisions

Answer each with an unqualified `YES` or `NO`:

1. Do r1-r3 retain the accepted teacher lineage, successor-slot sidecar,
   teacher/scratch separation, recurrent phases, terminal semantics, and
   prefix/publication invariants?
2. Is the sampling v1+r2 composite exact, and are TOP_K/MASS results, trace,
   and StepOutput fallback semantics complete and consensus-safe?
3. Are CURRENT/NEXT pending logits exact, nonaliasing, correctly committed,
   and fully charged for target-only MTP0 through MTP6?
4. Are rank-logit scratch and every retained MTP memory term exact, bounded,
   physically housed once, and free of hidden or double-charged bytes?
5. Do GraphProfile v3, the physical graph plan, executor r5, and the complete
   target/optional-MTP program set form one implementable fail-closed identity?
6. Is the combined r1-r3 design accepted for its coordinated CPU/mock proof?
7. Are all implementation and evidence nonclaims accurate?

Only if every answer is `YES`, end with the requested token as the only bare
acceptance line. Withhold for stale provenance, arithmetic error, state alias,
unowned memory, incomplete sampling output, lifetime-unsafe scratch reuse,
identity cycle, mixed generation, or any decision deferred to implementation.

The token opens only the coordinated CPU/reference implementation after every
named predecessor design is accepted. It does not authorize cn4 or CUDA.
