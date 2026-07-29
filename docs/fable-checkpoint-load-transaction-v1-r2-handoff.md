# Fable handoff: checkpoint load transaction v1 r2

Date: 2026-07-29

Status: superseding adversarial design review; CPU/mock and CUDA
implementation tokens withheld by Sol

GPU authorization conveyed by this handoff: none

Review candidate commit:
`4bb0708b0a3931a6018ea0e5dfcb4bf07a5ae042`

Requested acceptance token, only if every blocker and major is resolved:
`checkpoint-load-transaction-v1-r2-accepted`

The r1 handoff is superseded and its token must not be issued. Before review,
the continuation audit found that r1 incorrectly treated each manifest's
`tensor_contract_sha256` as process-common. Those digests are rank-specific:
EXL3 codec metadata contains the rank, explicit component names contain
`rankN`, and protected TP slice bounds differ. The r2 candidate binds all four
rank-specific digests and defines a separate normalized semantic catalog for
four-rank consensus.

## Provenance

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/checkpoint-load-transaction-v1.md` | `79d9c376201f3540f247344c24c37dd7d819d629f10459899a73c15f8b27015f` |
| `docs/fable-checkpoint-load-transaction-v1-handoff.md` | `8e0ed96e2a8616309b8c762ecaff7e1152942be02e1b1581531f69d512f8ff9c` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `docs/checkpoint-ingest.md` | `7d3cd28fd08df4f68f4056ef189e3367f272833cb951aed5b8744353879bef4d` |
| `docs/sm120-rank-runtime.md` | `19638590ee3b42da32bfab7673986c26488da064649c635df895700838da5624` |
| `crates/glm-format/src/checkpoint.rs` | `b9b616eae24541556753599bba670dafb4989d61f61174c9bccdfb76663b2aca` |
| `crates/glm-format/src/native_reader.rs` | `ae3579593713d35f633fadd1fe326db0ba8bae6ffe3644643e73b3321a6a0b4c` |
| `crates/glm-engine/src/startup.rs` | `9634f120a2e01f21aaa5778954053d9a06f1e8d2af6c5abe1f9c6e4cbbd31e87` |
| `crates/glm-engine/src/worker.rs` | `400c7c22f2c74d3f386fefe2d144da3437f27e6c99ce9f2d4bbf87ffe98fe437` |
| `crates/glm-engine/src/memory.rs` | `3a50581a8a60970a92ccf5a2c0e83c23d25ad975f1124c2332e9a2e646dbc837` |
| `crates/glm-engine/src/weight.rs` | `d658cefefc17757a28258bafd0e13f5309e8adcbf2b30c4d2bdc97be9899ca19` |
| `profiles/profile-budget-v0.json` | `cdbe4eaad9465181b2ba60b3656fe5207eee54467abfbf8d9bc398c3e68c23e0` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `docs/production-punchlist.md` | `e7fb909aabe0277c21c5cb59b544d0efe20f6bd259637fa7f959ea576f8307e4` |
| `crates/glm-cli/src/review.rs` | `d2c2d2756b94df8fb5555f578e7c907bef7c09b7b10fb3f310f45566f73c1c45` |
| `docs/review-provenance-verifier-v1.md` | `c4be2415ad0b13cea7fc154ce10c7aea839bd47b57af3e42fa6f329b92f3cb4e` |

Run this fail-closed check before reviewing:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-checkpoint-load-transaction-v1-r2-handoff.md
```

Hash every input at review start and finish. Review the exact candidate
commit in a separate worktree if `main` advances. The candidate contains no
load-transaction implementation, CUDA upload sink, device allocation,
checkpoint smoke, full-rank proof, performance result, or GPU evidence.

## Correction to verify

R2 changes the canonical plan as follows:

- `RankLoadEntry.v1` grows from 216 to 248 bytes;
- offset 216 binds that rank manifest's recomputed tensor-contract SHA-256;
- the plan header retains one common `tensor_catalog_sha256`;
- the common digest is derived from exact 128-byte semantic entries after
  full rank-specific validation; and
- rank-local source paths/slices, codec-metadata hashes, padded layouts,
  physical bytes, offsets, hashes, and gaps remain bound by rank-local
  contracts and are excluded only from the common semantic projection.

The remainder of the design—including normative startup order, preplanned
arenas, owned pinned-buffer lifetime, full first-load verification,
quarantined type states, prepared receipts, four-rank adoption, failure
rules, accounting, and CPU/mock proof requirements—remains subject to full
adversarial review.

## Requested adversarial questions

1. Independently confirm that rank manifests cannot share
   `tensor_contract_sha256`; enumerate every field that differs by rank.
2. Recompute the corrected 248-byte rank entry and the resulting plan
   preimage. Is the rank-specific contract placed and hashed unambiguously?
3. Is every 128-byte `TensorSemanticEntry.v1` offset and width exact?
4. Does the semantic entry retain every field that must agree across TP
   ranks—name, role, codec policy, layer/expert, TP rule, logical/global
   shapes, dtypes, flags, quantization grouping, reconstruction, collective,
   source dtype, source kind, and source axis?
5. Can excluding rank-local codec-metadata hashes, source paths/slices,
   padded shapes, physical bytes, offsets, payload hashes, or gaps conceal a
   material cross-rank policy disagreement after the mandated full
   rank-specific checks?
6. Are the source-kind, reconstruction, collective, and full safetensors
   dtype enumerations complete, stable, and collision-free? Does any current
   pinned tensor lack a defined ID?
7. Is hashing the exact UTF-8 name sufficient, or must the catalog carry the
   name bytes or their lengths separately to avoid an ambiguity?
8. Should rank logical shape be common for every fixed TP4 tensor? If a
   future legal rank has different padding or physical bytes, does the split
   between semantic and physical records remain correct?
9. Must `rank_manifest_tensor_contract_sha256` also appear directly in the
   prepared receipt, or is binding through `plan_sha256` sufficient?
10. Can all four ranks independently derive the common catalog from their
    own fully validated manifest and descriptor without reading another
    rank's rank-local source fields?
11. Re-answer all 22 r1 adversarial questions against the corrected bytes.
    In particular, decide the borrowed-buffer/pinned-ring lifetime, required
    HBM-content proof, cleanup under asynchronous failure, partial adoption
    safety, and resource accounting.
12. Does any corrected rule require an engine-v0 or format-v0 amendment
    before CPU implementation?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Withhold
the token unless every blocker and major is resolved. State separately
whether:

- the rank-specific versus rank-invariant identity split is accepted;
- the corrected binary encodings are accepted;
- the normalized semantic catalog is sufficient;
- the complete r1 design, as corrected, is accepted;
- CPU/mock implementation may begin;
- a CUDA upload implementation remains blocked;
- full-checkpoint load remains blocked; and
- no cn4 access or GPU launch is authorized by the verdict.
