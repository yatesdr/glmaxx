# Fable handoff — manifest and cache ABI v0.2.2

Date: 2026-07-29

Candidate base commit:
`22d03fcce921483bbf71da5a51e80131326217b7`

Review scope: the single independent-review gate that remains before the
authorized cn4 M2 CUDA execution. This is intentionally narrower than the
offline-serving review.

GPU authorization conveyed by this handoff: none. Operator authorization is
separate and already recorded outside this review.

## Required provenance procedure

Hash every input at review start and finish. If either set differs from this
table, stop and report a stale candidate instead of reviewing inferred bytes.

| Input | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `docs/nvfp4-physical-abi.md` | `01939514efdd7f34045d64830b43b09647af600f8f5cf641e26a9a4d0cae2c23` |
| `docs/phase-a-proof.md` | `d38eea85efd96b07bbdbdb27c039a2d7848d348b499615ca21c59e0c29904a41` |
| `crates/glm-reference/src/manifest.rs` | `dc8076f90632ac556cf718053c82231ad2bd95d4871fc3ba23444a9574975403` |
| `crates/glm-reference/src/routed_fc1.rs` | `55709be01710d08f5b13de22afdb24884d233cdfab1c5de132ad22edd304f40b` |
| `crates/glm-reference/src/routed_fc2.rs` | `4f34f5b89cd542f096269a7442da5289900d1e831e32fcf83462560dc410a40d` |
| `crates/glm-cuda/src/abi.rs` | `3593a96f09319f0f1f7e2fef47f555d4fd47849790e2bdf60a4ce96a81f0996c` |
| `crates/glm-cuda/src/ffi.rs` | `b4bff008d1b262de9cf3032fbe3777e8e2bc1f62dd86b4dd1dbe11c0c1d55d4d` |
| `crates/glm-cuda/src/lib.rs` | `08eae48f2a60d30abc529ed299ba023d027b212fb958c8863cb27a217adc3073` |
| `kernels/include/glmaxx_kernel.h` | `e6a13f495362704f248a350bdfe941421bc8a2119109e3106dee3b42f2fc4470` |
| `kernels/sm120/nvfp4_routed_fc1.cu` | `a6ea3cd4cefd08ae2dcd98752e092fbbfa7d19bf799c00457052f2717a562f60` |
| `kernels/sm120/nvfp4_routed_fc2.cu` | `0c48e1dae810ab658bc2c565452f06e96026aed3c4b472e6036bd4ba3a49706d` |
| `kernels/sm120/cutlass_nvfp4_dense_control.cu` | `1ec4abf4fc307f709aa5d2576ac80c84825d75a7eb9db7b11ae036fd67a34541` |
| `kernels/sm120/cutlass_nvfp4_fc2_control.cu` | `26c74dfbcb7ea3f75cceb32021d2978271b3f36a1b3d87bab67284cd5d41ea63` |
| `kernels/CMakeLists.txt` | `872403cd1e67380476b91a01b60b612d2ef24d84f261e365c452e8a54a864416` |
| `crates/glm-cache/src/kv.rs` | `60701a0ec25dfac0345d3b088d8937a8adcc1107d8f2a3afa96c0b38379ec8b0` |
| `crates/glm-cache/src/tier.rs` | `2730d829c8538e7b10649e0fba6504ee3389adc21c2f557e474a93c6dbee4f97` |
| `crates/glm-cache/src/page.rs` | `d32d70b46f8e09c31923b6fb574db07ef6a8a7dfc7489392b39785dd563217ed` |
| `crates/glm-cache/src/budget.rs` | `14b563afbeea90fb2bc8897db1a73dab33c64f5427dacac83edd56a00e0eb8a7` |
| prior `fable-adversarial-v2.md` | `f0019b96d5b35bdca6d026691629b56fbeb0c3c4528e1ae4ff9c1aa06817953e` |
| `docs/fable-v2-disposition.md` | `fd60c89ec188fc6467507ad054f114a379625b0eec40b863cb61c5ace5b1783b` |
| `docs/cn4-preparation-result-20260729.md` | `427004e5bc1f6480bd62acbb11a5fab5146d8cd271c53b0e4b94595b7130e7f9` |
| `docs/manifest-source-audit-20260729.md` | `02d853aad455aa120efc88926c8dbe06841c621a2831067cf59fb4a5b78d4cad` |
| `profiles/profile-budget-v0.json` | `028516adc04d454317e1b76a3147be4807c3ed3ce371e1d43aead3396270400d` |
| `crates/glm-format/src/checkpoint.rs` | `eadf86769c220d42a419b2a9c5a78ff0377d98e85cff4d90c5b792894fa7f684` |
| `crates/glm-format/src/stream.rs` | `4cd4cb23d68ef4280a9a9a00270fc7dad4091ade058fd1165f353d6c95772c8f` |
| `crates/glm-format/src/safetensors.rs` | `a7f8ce1074e585106c2f44d05c2669518e5e3638732c0ba8ee0fdc882ac3a2d1` |
| `crates/glm-cli/src/main.rs` | `b0dbf5c3fcbff295fa1c685a3d82b234de9f67941ce25541f3cbbbf7d96ab93a` |
| `docs/checkpoint-ingest.md` | `b25ce1ba6d9c8406ed9570c95979ded52edb090d05d2f5770cf9eae57f62b6da` |
| `scripts/cn4-phase-b-prepare.sh` | `2e51621e6f9d8e74274ac1a4e89d53962620418c96c34f9c33a95cb6eb08ed4c` |
| `scripts/cn4-phase-b.sh` | `e96a1322f05eb0dc2f7ba5e978db2a2eafd7f8fcbec61251bea4bfc2e7d130cc` |

The manifest records the model revision and source hashes that must be checked
against the pinned GLM-5.2 graph. If the reviewer uses the read-only
`../glm52-opt` evidence tree, record its HEAD and do not modify it.

The format hash changed after the earlier handoff because the complete pinned
checkpoint inventory, TP slicing, direct EXL3 source planes, canonical
four-rank manifests, literal output payload digest, and atomic publication
contract were added. These additions still do not qualify EXL3 GPU load
support; review the exact new bytes rather than reusing the prior spec hash.

## Decision 1: generated operation manifest

Verify from the pinned graph rather than from the prose:

1. target layers `0..77`, sparse layers `3..77`, 256 routed experts, top-8,
   hidden 6,144, expert intermediate 2,048, and TP4 local intermediate 512;
2. gate/up concatenation and TP axis, down-projection TP axis, route-weight
   placement, shared-expert combination, residual boundaries, and the exact
   TP reductions;
3. stable route-compaction order and the absence of a hidden materialization
   or collective between routed FC1 and FC2;
4. all 21 full-indexer groups, each consumer layer exactly once, key
   production on the full layer, and IndexShare reuse;
5. layer 78 as one recurrent draft layer, including attention, routed/shared
   MoE, residual, vocabulary head, recurrence-zero top-2,048 selection,
   later recurrence reuse, and per-committed-position indexer keys.

Report any manifest fact that is merely plausible but not source-derived.

## Decision 2: v0.2.2 combined draft sidecar

Re-derive independently:

1. target KV: 368 bytes per layer-position across 78 layers;
2. target indexer key: 132 bytes per full-indexer-group-position across 21
   groups;
3. draft KV plus draft indexer: `368 + 132 = 500` bytes per position;
4. 64-token draft payload: 32,000 bytes;
5. sealed draft record: 4,096-byte header plus 32,000-byte payload rounded to
   36,864 bytes;
6. aggregate 1M terms:
   30,098,325,504 target-KV bytes, 2,906,652,672 target-indexer bytes,
   385,875,968 draft-KV bytes, and 138,412,032 draft-indexer bytes.

Adversarially check that:

- target and mandatory target-indexer generations publish atomically;
- an MTP sidecar cannot attach without the paired target/indexer generation;
- incomplete draft publication can degrade only to a valid MTP0 target plus
  indexer pair;
- rollback makes rejected draft KV and indexer writes unreachable before
  slot reuse;
- DRAM/NVMe headers, hashes, replay, and prefix attachment preserve the same
  rules;
- DCP posture does not contaminate posture-neutral durable bytes.

## Decision 3: conversion and profile-budget boundary

Adversarially verify that:

1. all 92 immutable source-manifest files are hashed before conversion and
   checkpoint shards are hashed through the already-open validated
   descriptors;
2. every rank has exactly 59,585 deterministic tensor contracts and
   81,590,319,104 source-plane bytes;
3. protected TP slicing, explicit-rank EXL3 components, source bindings,
   physical byte counts, role IDs, and collective boundaries are complete;
4. deferred tensor groups preserve payload-before-descriptor durability;
5. the aggregate output payload SHA-256 is sealed into the canonical manifest
   without changing layout and survives resume;
6. all rank headers share the derived conversion identity and publication
   cannot replace an existing destination;
7. the converter rejects an unmeasured or unreviewed profile budget; and
8. `profiles/profile-budget-v0.json` currently validates only as a blocked
   arithmetic candidate. It MUST remain `conversion_allowed=false` until the
   listed post-context and high-water measurements replace every assumption.

Do not call the current profile budget complete or authorize the 326-GB
conversion. This review should either accept its arithmetic and fail-closed
status as a candidate, or report exact defects for correction.

## Decision 4: SM120 routed-MoE physical ABI

Adversarially verify the `glmaxx.sm120.nvfp4.routed_moe.v2` development
boundary before any device launch:

1. Rust and C independently freeze both FC1 and FC2 descriptors at 224-byte
   size and 16-byte alignment, with matching offsets and version checks;
2. FC1 consumes packed expert-major NVFP4 weights, dynamically quantizes BF16
   assignments once, and applies SwiGLU after independent gate/up
   accumulation;
3. FC2 consumes `[assignments,512]`, produces assignment-major FP32
   `[assignments,6144]`, applies route weights after projection, then reduces
   token slots in fixed `0..7` order without floating-point atomics;
4. route validation rejects unsorted experts, malformed offsets, duplicate
   `(token,slot)` ownership, out-of-range values, and non-finite/negative
   weights before a successful result is observable;
5. dense and expert-grouped CUTLASS controls consume the exact value/SFA/SFB
   planes with no runtime weight repack or persistent dequantization;
6. grouped expert-local SFA slab arithmetic is complete for every accepted
   routing posture and scratch reuse cannot overlap live token output;
7. the Phase-B script cannot launch either smoke without the committed review
   artifact, exact token, clean source, idle device inventory, and explicit
   operator authorization; and
8. compile/SASS evidence remains preparation only. The materialized BF16
   development boundaries are not represented as fused production kernels or
   performance evidence.

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
answer these four questions separately:

1. Is the generated GLM-5.2 operation manifest accepted for M2?
2. Is the v0.2.2 combined draft-KV/draft-indexer cache ABI accepted for M2?
3. Is the conversion path and blocked profile-budget candidate accepted
   exactly in its stated, non-conversion-authorizing posture?
4. Is the routed-MoE v2 FC1/FC2 physical ABI and gated SM120 correctness
   procedure accepted for its development-control posture?

Only if all four answers are unqualified `YES`, include these exact hash
lines:

```text
engine-v0-sha256=efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
format-v0-sha256=619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a
operation-manifest-sha256=8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6
profile-budget-v0-sha256=028516adc04d454317e1b76a3147be4807c3ed3ce371e1d43aead3396270400d
```

Then end with the exact gate token:

```text
manifest-abi-v0.2.2-accepted
```

Do not emit that token for a conditional pass or a stale hash set. A valid
review closes the independent-review half of the M2 gate; it does not itself
authorize or launch GPU work, and it does not turn the blocked profile budget
into a conversion-approved artifact.
