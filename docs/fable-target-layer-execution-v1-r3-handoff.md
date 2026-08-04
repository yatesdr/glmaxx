# Fable handoff: target-layer profile-program amendment r3

Date: 2026-08-04

Status: superseding adversarial design review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4, inspect a checkpoint, launch CUDA, create a context, or
modify a runtime resource for this review.

Review candidate commit:
`f94a272cfefdc76e974ebecb449ec6abf66d50ad`

Required result path:
`fable-target-layer-execution-v1-r3.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`target-layer-execution-v1-r3-design-accepted`

This handoff supersedes the unexecuted target-layer r2 handoff and its
39,594-record universal-program premise. Do not issue the r2 token. Review
target layer v1-r3 together; current Rust implements none of these program
families.

## Provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Any mismatch withholds the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `spec/engine-v0.md` | `52497e022bde5278372a9bce168e87a602fca9341c4cb4e019b4a3c7ce63179b` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `docs/target-layer-execution-v1.md` | `89c6cf7397a3dc6b0c01383e679dcc4b51e20e3c45057bef0928dc24b866a819` |
| `docs/target-layer-execution-v1-r2.md` | `3b70e5d4b74aa66c41c855b71f282e64ed726c86ce78161260d12dca596934eb` |
| `docs/target-layer-execution-v1-r3.md` | `97c2c3615384dddc6204e910fe3c498fdd7a26554ed8aecec790d62f72c2ad87` |
| `docs/distributed-sampling-abi-v1.md` | `383e328a527cc780ed553af0b78382cf200ad60f97afb26d96a2a1494b57c89b` |
| `docs/distributed-sampling-abi-v1-r2.md` | `061903d0a0cf2a284f35b177da5f1c3484cb61dfd9020f627db7b5632a4f2b6b` |
| `docs/exl3-mixed-k-source-and-kernel-v1-r3.md` | `683bec3908a0650a4cef7d53075c5438f7d15473d631f62da1de3cd70d8e2866` |
| `docs/nvfp4-fused-routed-moe-v1-r3.md` | `f60d3adb777321ed715ae465abcf20895dbbf470c20970855636ed9b2b3a4db0` |
| `docs/sm120-w4a16-nf3-fused-moe-v1-r2.md` | `311d1214ad57e97c7bab45069fae5507602c0e21922b1fde677ba129e734f265` |
| `docs/nvfp4-laboratory-manifest-v1.md` | `8a0adb54dedfab1dba0afcf09579614ce567da92fda43134b0c404af5aafb0ee` |
| `docs/hybrid-serving-manifest-v1.md` | `934787ea37a5dbd9b6778844adbeb0b40fd365d4653991fc7cbfe77df3c685cf` |
| `docs/target-graph-physical-memory-v1.md` | `135e7d61f5ce7cc94d200648e9691b9d76edaee13025c21e88f0ad2c07018bc9` |
| `docs/sm120-rank-executor-v1-r5.md` | `85c1082575c4b4d9dbdf26affe499121339c8a3a3f7f914ff5957ff6bee7f565` |
| `docs/mtp-layer-execution-v1-r2.md` | `d75710b3b552f229cc3bef34a8977a7c30e5b03b4c4a268f27c0efb2a3d1f12c` |
| `docs/mtp-layer-execution-v1-r3.md` | `cd66910cf8738042d0c5ec8c7fbee69f024db9bde543d379abc7cfba9264de96` |
| `crates/glm-format/src/checkpoint.rs` | `12777f070e56674599ce662326552cda7c28c2b36e5155d3e8daf7718577aa18` |
| `crates/glm-format/src/rank_manifest.rs` | `cb57a81ad3643a86992c4fd9c2166e9ee238cc7e66063732355d14e485f73410` |
| `crates/glm-engine/src/checkpoint_load.rs` | `77a331e7a6ecae4e04c1677f9380007eef432ca4e21d2fd4c2bc64b42facfab3` |
| `crates/glm-engine/src/checkpoint_cuda.rs` | `8eed3e8302d3b41772b2cbdc74ab2bd1fac27e718510c4243d8466b2d0a10593` |
| `crates/glm-engine/src/graph.rs` | `c85ca1aa52ba42294fc6a43524e8f70357523977d343e8c2f212787e7754cd22` |
| `scripts/local-checks.sh` | `1675ca5bac9bda032ab6db206629bc0052ad6afc5cb6f59a7b6697e4e5c779d0` |

Run the full local gate and:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-target-layer-execution-v1-r3-handoff.md
```

## Required independent work

1. Recheck every retained v1/r2 mathematical, phase, row, cache, collective,
   lifetime, and nonclaim decision for regression under r3.
2. Independently inspect the capacity plan: prove every target sparse expert
   has distinct gate, up, and down descriptors/tensor IDs while gate and up
   share the old r2 role/expert/codec key.
3. Encode and corrupt every field of the 16-byte binding. Prove projection and
   layouts close that semantic ambiguity and reserved/enum/order rules are
   canonical.
4. Derive the protected counts for dense, FULL, and SHARED layers; recompute
   `1 + 3*17 + 18*787 + 57*782 + 2 = 58,794` independently.
5. Recompute the complete capacity inventory remainder: 59,585 minus 58,794
   equals the separately owned 791 layer-78 MTP records. Find any overlap or
   omission.
6. Compile representative K=3 and K=4 capacity experts. Prove K changes source
   metadata/planes but never projection, codec, layout, ordering, or request
   policy.
7. Independently derive the retained hybrid count
   `1 + 3*17 + 18*531 + 57*526 + 2 = 39,594` and verify every legal
   ModelOpt/NVFP4/NF3 codec-layout pair against its pinned contract.
8. Serialize every capacity v2 and hybrid v3 entry/top-level preimage. Attack
   old domains, ten-byte records, swapped profile domains, wrong sampling
   composite, omitted entries, and text-encoded inner hashes.
9. Trace target-program identity through StepPlan/Input, GraphProfile v3,
   ten-arena plan, executor program/module set, resident bindings, completion,
   and compatible/cold reload decisions.
10. Prove the 533-tensor all-NVFP4 laboratory subset remains a distinct type
    and digest and cannot satisfy a full target program.
11. Trace current Rust and identify every missing compiler/type/receipt. No
    code or existing cn4 record may be promoted by this design review.

## Required decisions

Answer each with an unqualified `YES` or `NO`:

1. Is r2's universal 39,594-record premise genuinely wrong for TR3 and fully
   corrected without changing target math?
2. Is the common 16-byte binding complete, canonical, profile-safe, and
   derivable only from authenticated startup state?
3. Are the capacity v2 domains, legal records, layer counts, 58,794 total,
   and separate 791-record MTP remainder exact?
4. Are the hybrid v3 domains, legal records, layer counts, and 39,594 total
   exact for the required NVFP4/NF3 family?
5. Are capacity, hybrid, and M4 laboratory programs mutually
   substitution-resistant through every downstream identity and type state?
6. Is the coordinated CPU proof sufficient to implement both production
   compilers without runtime names, metadata parsing, repack, raw addresses,
   or rank-local choices?
7. Are all implementation, CUDA, checkpoint, quality, capacity, concurrency,
   reload, and performance nonclaims accurate?

Only if every answer is `YES`, end with the requested token as the only bare
acceptance line. Withhold for stale provenance, wrong counts, an ambiguous
gate/up binding, mixed profile domains, layout drift, a laboratory type
escape, an unbound downstream identity, incomplete CPU gates, or any runtime
overstatement.

The token opens only the profile-specific target-program CPU implementation
after all named predecessor designs are accepted. It does not authorize cn4
or CUDA.
