# Fable handoff — manifest and cache ABI v0.2.2

Date: 2026-07-29

Candidate base commit:
`ee3f1f3507ba67b886796126b1cc1e91c5d099e4`

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
| `spec/format-v0.md` | `9f78f09b1d82f61a0110303b9921d59d614339bcacbec66c7735655a9c5ed01d` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `docs/nvfp4-physical-abi.md` | `8936c8a60a1d6a7a2038fcd7f3f4a352b80477c359a6f3f2f89ea3903d2a9e99` |
| `docs/phase-a-proof.md` | `d38eea85efd96b07bbdb27c039a2d7848d348b499615ca21c59e0c29904a41` |
| `crates/glm-reference/src/manifest.rs` | `dc8076f90632ac556cf718053c82231ad2bd95d4871fc3ba23444a9574975403` |
| `crates/glm-cache/src/kv.rs` | `60701a0ec25dfac0345d3b088d8937a8adcc1107d8f2a3afa96c0b38379ec8b0` |
| `crates/glm-cache/src/tier.rs` | `2730d829c8538e7b10649e0fba6504ee3389adc21c2f557e474a93c6dbee4f97` |
| `crates/glm-cache/src/page.rs` | `d32d70b46f8e09c31923b6fb574db07ef6a8a7dfc7489392b39785dd563217ed` |
| `crates/glm-cache/src/budget.rs` | `14b563afbeea90fb2bc8897db1a73dab33c64f5427dacac83edd56a00e0eb8a7` |
| prior `fable-adversarial-v2.md` | `f0019b96d5b35bdca6d026691629b56fbeb0c3c4528e1ae4ff9c1aa06817953e` |
| `docs/fable-v2-disposition.md` | `fd60c89ec188fc6467507ad054f114a379625b0eec40b863cb61c5ace5b1783b` |
| `docs/cn4-preparation-result-20260729.md` | `427004e5bc1f6480bd62acbb11a5fab5146d8cd271c53b0e4b94595b7130e7f9` |
| `docs/manifest-source-audit-20260729.md` | `02d853aad455aa120efc88926c8dbe06841c621a2831067cf59fb4a5b78d4cad` |

The manifest records the model revision and source hashes that must be checked
against the pinned GLM-5.2 graph. If the reviewer uses the read-only
`../glm52-opt` evidence tree, record its HEAD and do not modify it.

The format hash changed after the earlier handoff because codec `0x0200`
source-plane placement was made explicit. That change does not alter either
decision below and does not qualify EXL3 GPU load support; review the exact
new bytes rather than reusing the prior spec hash.

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

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
answer these two questions separately:

1. Is the generated GLM-5.2 operation manifest accepted for M2?
2. Is the v0.2.2 combined draft-KV/draft-indexer cache ABI accepted for M2?

Only if both answers are unqualified `YES`, end with the exact gate token:

```text
manifest-abi-v0.2.2-accepted
```

Do not emit that token for a conditional pass or a stale hash set. A valid
review closes the independent-review half of the M2 gate; it does not itself
authorize or launch GPU work.
