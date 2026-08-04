# Fable handoff: target-layer execution v1 r2

Date: 2026-08-03

Status: corrective adversarial design review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4, launch CUDA, inspect model payloads, or modify any
runtime resource. This is a source, arithmetic, serialization, and CPU-design
gate only.

Review candidate commit:
`8a554af72e52bb067bf61edf43735615c6a0942a`

Required result path:
`fable-target-layer-execution-v1-r2.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`target-layer-execution-v1-accepted`

The original withheld target-layer review is an operator-inbox input at
`docs/reviews/fable-target-layer-execution-v1.md`, SHA-256
`8738a8c2c4801a9d657a292e4702f500b947fbb1015ad9a62044e54731ae9469`.
Hash it before evaluating the closure claim in decision 12.

The pinned distributed-sampling v1 dependency is not accepted. Its withheld
review is `docs/reviews/fable-distributed-sampling-abi-v1.md`, SHA-256
`901481e5b1d6b26283a7c7e8eb1a1f7af1968df2b2e5d2ddda1c54d0075aa61c`.
This target-layer review may judge the hash field and fail-closed dependency,
but it must not promote sampling v1. Target-program construction remains
closed until an independently accepted sampling successor supplies the exact
hash bound by that field.

The corrective successor candidate is
`docs/distributed-sampling-abi-v1-r2.md`. Its own handoff requests the token
`distributed-sampling-abi-v1-r2-design-accepted`; absence of that independent
token keeps target-program construction closed even if this target review
accepts the composite-field wiring.

## Provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Withhold the token for a mismatch or incomplete input.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/target-layer-execution-v1-r2.md` | `3b70e5d4b74aa66c41c855b71f282e64ed726c86ce78161260d12dca596934eb` |
| `docs/target-layer-r2-preflight-erratum-20260804.md` | `4e27b469d3e0ba2c3727b0c14ec4ef3da7f1eaeec5218c8f9208a9e22fe3412a` |
| `docs/target-layer-execution-v1.md` | `89c6cf7397a3dc6b0c01383e679dcc4b51e20e3c45057bef0928dc24b866a819` |
| `docs/manifest-source-audit-20260729.md` | `61278e9a0f85f692357ca4c193771d3d4c0487537f80cb77b2eeb956ee916ff8` |
| `spec/engine-v0.md` | `52497e022bde5278372a9bce168e87a602fca9341c4cb4e019b4a3c7ce63179b` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `docs/step-execution-abi-v3.md` | `1cde3bcabba0a0d861691b06ddb140cb64dfbefaab1129c8a04bc302c0ce609e` |
| `docs/distributed-sampling-abi-v1.md` | `383e328a527cc780ed553af0b78382cf200ad60f97afb26d96a2a1494b57c89b` |
| `docs/distributed-sampling-abi-v1-r2.md` | `061903d0a0cf2a284f35b177da5f1c3484cb61dfd9020f627db7b5632a4f2b6b` |
| `docs/serving-page-transaction-v1.md` | `31983cce95ee01a5968213d5daf12c7a855f75f8735314700f2b4a9e55625d1a` |
| `docs/sm120-rank-runtime.md` | `908b8adf0e1fc230145c009db01c71e69437ab359c76a545031fd9157c1ceea9` |
| `docs/prefill-graph-profile-abi-v2.md` | `37154c9e31109acdf35a382c6be87b3a865e2b7f6ae8f801969526789dd41f91` |
| `crates/glm-format/src/checkpoint.rs` | `12777f070e56674599ce662326552cda7c28c2b36e5155d3e8daf7718577aa18` |
| `crates/glm-format/src/rank_manifest.rs` | `cb57a81ad3643a86992c4fd9c2166e9ee238cc7e66063732355d14e485f73410` |
| `crates/glm-engine/src/step.rs` | `4963a58da7c9c6bbed0fb57fb7ef56d90d1e0f09fe54da8cc02c35891f743359` |
| `crates/glm-engine/src/graph.rs` | `c85ca1aa52ba42294fc6a43524e8f70357523977d343e8c2f212787e7754cd22` |
| `crates/glm-engine/src/input.rs` | `c3d090429015030416f6c03ddb6fef2dfd569859ff6e0fcc05bcb2d6a163ffa2` |
| `crates/glm-engine/src/output.rs` | `1a82e9990af4e2892831e950f5cd3db256032ab5e43642faeb08229cbf2f1c2c` |
| `crates/glm-engine/src/worker.rs` | `52dbb32ef45bfa652ea113b7c3db7e4fb200bfd778015abb1aebceabaddf89d6` |
| `crates/glm-cache/src/tier.rs` | `0a1541f13462bcdec92284911f96531b06869b60c7fe85fc5e9669c80fabe693` |
| `crates/glm-cache/src/attention.rs` | `662965eb0c7e9e22768ee7c95c849b403a0a0004a1c061fb98c996fdd9c4e89f` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

Run the complete CPU-only gate and record its exit status:

```text
./scripts/local-checks.sh
```

## Exact external source checks

Fetch no model weights. Fetch the official GLM-5.2 config at exact revision
`b4734de4facf877f85769a911abafc5283eab3d9`:

```text
https://huggingface.co/zai-org/GLM-5.2/resolve/b4734de4facf877f85769a911abafc5283eab3d9/config.json
```

The raw response must hash to
`185f93ee6d12548e16a847e279dc0c3c90b1524c970b0866b42fb545747d859a`.
Independently extract `vocab_size`, `rms_norm_eps`, `rope_parameters`,
`moe_router_dtype`, and `transformers_version`.

From the official Transformers repository, fetch these two files at commit
`5204b4fe36956e9214b9279f1e1be2fd5dd1d9f3`:

```text
https://raw.githubusercontent.com/huggingface/transformers/5204b4fe36956e9214b9279f1e1be2fd5dd1d9f3/src/transformers/models/glm_moe_dsa/modeling_glm_moe_dsa.py
https://raw.githubusercontent.com/huggingface/transformers/5204b4fe36956e9214b9279f1e1be2fd5dd1d9f3/src/transformers/models/glm_moe_dsa/configuration_glm_moe_dsa.py
```

They must hash respectively to:

```text
adb8317a21716b01273046e46c807f14f0dbaf035af59b60d52bd6bc3007cf72
5a81164be746307431ad998f789b6b0bca20eb4c14a726552eb3730268413997
```

Do not substitute a tag, branch, mirror, or redirect body for those bytes.

## Required independent work

Do not accept the amendment by prose inspection alone. Independently:

1. fetch and hash the exact external source, then derive the Q-A/KV-A,
   input/post/final, and indexer-K normalization epsilons and RoPE mode;
2. serialize every `TargetProgramEntry.v1` hash preimage from the normative
   tensor bindings, including recomputation of the sampling v1+r2 composite
   `95fa7aa3b4b0b78a3f8313705d25e4c11682632fce6d8b8c2355b8130745f58c`
   consumed by the final-head entry. Recompute the expected 39,594 ten-byte
   binding records:
   embedding 1, final 2, three dense layers at 17 each, eighteen full-indexer
   sparse layers at 531 each, and fifty-seven shared-indexer sparse layers at
   526 each;
3. enumerate the five phase templates and independently serialize every
   twelve-byte phase record, checking dependency ordinals, participant masks,
   fixed-capacity/zero-count flags, graph/eager selection, and TP4 consensus;
4. enumerate all 32 lifetime records and calculate first/last use for each
   phase variant; prove that alias class is only reuse eligibility, that the
   candidate accepts no physical alias map, and that an undersized physical
   graph-slot claim is explicitly withheld pending a byte-specified span ABI;
5. independently encode/decode and mutation-test `TargetRow.v1`, the unified
   `TargetPageWriteSlot.v1`, and `PendingLogitSlot.v1`, including all reserved
   bits, stale generations, page mismatches, malformed modes, and overflow;
6. rederive the exact maximum table bytes: 273,408 for a 3,072-row prefill,
   136 for C1 decode, and 42,496 for a 448-row verifier plus 64 pending logits;
7. prove the unified page formula is exactly the existing layer-major target
   KV/indexer HBM layout, cannot overflow accepted dimensions, and binds both
   tentative records to the same page identity and generation;
8. independently serialize the 127-byte `StepPlan.v4` hash input and 159-byte
   record, rederive the 430-byte fixed `StepInput.v3` prefix, and test its
   stale-program/table rejections; confirm that `GraphProfile.v2` binds only
   the logical lifetime digest and cannot be mistaken for a physical span or
   anti-alias proof;
9. construct controls A, B, and C and show that A versus B isolates source
   codec error while B versus C isolates packed-kernel implementation error;
   and
10. trace the current code and every pinned dependency to identify anything
    this design would incorrectly claim is already implemented or accepted.

## Required decisions

Answer every decision with an unqualified `YES` or `NO`:

1. Are all source facts exact-revision-derived, including Q-A/KV-A epsilon
   `1e-6`, other RMSNorm epsilon `1e-5`, default RoPE with theta 8,000,000,
   FP32 router arithmetic, vocabulary-axis-zero language head, and stored
   deinterleaved RoPE output?
2. Is every target tensor binding complete and uniquely serialized, with
   exact dense, full-indexer sparse, shared-indexer sparse, embedding, final,
   and top-level program hash preimages?
3. Are the phase templates total for dense, full/shared sparse, prefill CKV,
   prefill query, decode, and verify without rank-local route choice?
4. Are phase dependencies, ordinals, participant masks, fixed capacities,
   zero-count behavior, and graph/eager flags exact and fail-closed?
5. Are all 32 logical buffer lifetimes complete, is reuse forbidden while
   live, and does the candidate accurately withhold physical span, capacity,
   capture, and eager-execution acceptance until a later byte-specified ABI?
6. Are all three table records byte-exact, bounded, mutation-tested, and
   sufficient to bind row/page/pending-logit state without a hidden pointer?
7. Does one unified page slot per row preserve the existing fixed GLM-5.2
   layer-major HBM layout, transaction semantics, and target/index generation
   equality while avoiding a per-layer page-table explosion?
8. Are the three maximum table-byte ceilings exact and small enough for the
   required C1, 448-row verifier, and 3,072-row prefill profiles?
9. Are `StepPlan.v4`, `StepInput.v3`, and the logical identity portion of
   `GraphProfile.v2` exact extensions that reject every stale target program,
   phase table, lifetime table, page table, or pending-logit table without
   claiming an absent physical graph-buffer map?
10. Are the target operator order, absorbed-MLA shapes, DCP routes, sparse-MoE
    arithmetic, residual precision, and distributed-sampling v1+r2 composite
    binding complete, cycle-free, and fail-closed on absent sampling-r2
    acceptance?
11. Do controls A, B, and C isolate source codec error from packed-kernel
    error without changing precision membership, model math, or batch shape?
12. Does the r2 amendment resolve every blocker, major, minor, and question in
    the hash-pinned operator-inbox target-layer v1 review without weakening a
    correctness gate?
13. Does the design remain compatible with high-capacity paged KV, prefix
    caching, DRAM/NVMe tiering, concurrency, TP4 PCIe operation, and MTP0-6
    without claiming any of those paths are already implemented?
14. Are all implementation, CUDA, real-checkpoint, quality, capacity,
    cold-start, latency, throughput, and serving nonclaims accurate?
15. Does the gate sequence require accepted source designs and an independent
    CPU proof before CUDA, SM120 timing, layer replay, or checkpoint smoke?
16. Is the review entirely reproducible from exact immutable inputs without
    using cn4, vLLM resources, moving branches, or unrecorded artifacts?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, followed
by the independent derivations and all sixteen decisions. Only if every
decision is `YES`, attest the candidate commit and all twenty-three exact input
hashes, then end with the requested token as the only bare acceptance line.

Acceptance opens only a CPU target-program, phase/logical-lifetime/table ABI,
and independent reference-proof implementation using distinct owned storage
for nonzero-capacity logical classes. It does not accept a physical graph
memory map, implementation, cn4 or CUDA use, full-checkpoint conversion, a
kernel, quality, KV capacity, cold-start, latency, throughput, or serving.
