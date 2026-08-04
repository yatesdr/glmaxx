# Fable handoff: hybrid NVFP4/NF3 source admission v1 r2

Date: 2026-08-04

Status: corrective design review requested

GPU authorization conveyed by this handoff: none

Review candidate commit:
`99ab1351ba71982f69c5855a9dc2adb394f8be11`

Review the candidate in a detached worktree. Current HEAD and untracked review
drops are not authority. Verify every pinned input at review start and finish.

Required result path:
`fable-hybrid-nvfp4-nf3-source-admission-v1-r2.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`hybrid-nvfp4-nf3-source-admission-v1-r2-design-accepted`

Emit the token only if all five questions receive an unqualified **YES**.
Otherwise withhold it and identify the smallest corrective scope. The V1
handoff and token are superseded and must not be accepted.

## Pinned inputs

| Path | SHA-256 |
|---|---|
| `docs/hybrid-nvfp4-nf3-source-admission-v1-r2.md` | `8fda4fc1b8c0a1ecd1a4cf05fd5388dd7dabc5f315d09750dc4d6e9782fa0e53` |
| `docs/cn4-hybrid-publisher-source-proof-e1dfb9e-20260804.md` | `a9dfe89ed10628b81642baf51b9477874270f6b1af8b347e4c76bf8a28f4e785` |
| `docs/cn4-hybrid-source-content-hash-fc4871d-20260804.md` | `dc47043402211e04afe6fbeb39e29e4e5dea8bef008ca37987a62c7949fe1bce` |
| `manifests/glm52-hybrid-source-v1.sha256` | `a4d9cb546e8fdae5dd7e494228750ee0a2723904170a7c700e461704d5683ab7` |
| `scripts/public-hybrid-source-proof.sh` | `7433792cb18eab5bbe9cac7c4fa21a1d164fcd060dd082ef1c2131de46ba7c0e` |
| `scripts/cn4-hybrid-source-hash.sh` | `ccbda911564db1d002b0748fa989755bbbfeb630ec34174b705ea266472202d2` |
| `docs/nf3-nvfp4-hybrid-source-and-kernel-v1-r2.md` | `80a055c354971e7ff82b0eeeb54413cd6acafb7ee31611426f3154accc35b2aa` |
| `docs/nf3-nvfp4-native-rank-manifest-v1-r2.md` | `ee81b6fc50a9a948af48aaf60d992fceb4c1bcb0c1687ac8cbe6f133a15baf9a` |
| `crates/glm-format/src/safetensors.rs` | `4a7d8d4a2121a2257a5e8b7ec531c98b4b83bddb6ea140ade697088a05009594` |
| `crates/glm-format/src/checkpoint.rs` | `08450f0cb33e592ec76dfbe655b06580ba1743e60f3109a65218d052dbea406c` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `scripts/local-checks.sh` | `2d1882be9afd91f4a54c1d3ff9b9f02cd5087357eeb5668d4094c2114c3003ce` |

The two authoritative cn4 evidence roots remain outside Git:

```text
/home/derek/glmaxx/evidence/20260804T020700Z-hybrid-source-hash-fc4871d
/home/derek/glmaxx/evidence/20260804T024000Z-hybrid-publisher-e1dfb9e
```

Their concise artifact hashes are pinned in the result inputs. This review
requires no cn4 access and grants no CUDA or checkpoint-mutation authority.

## Independent public verification

Query the exact immutable revision, not `main`:

```text
https://huggingface.co/api/models/madeby561/GLM-5.2-MXFP8-NVFP4-NF3-Hybrid/revision/68babde27a97a4c980c2494e830dd424975cd5a3?blobs=true
```

Independently verify the exact 194-name sibling set, 186 LFS object SHA-256 and
size identities, and the eight Git-backed file bodies. Reconstruct the
canonical local-manifest rows and confirm its SHA-256 and byte totals rather
than relying solely on the candidate's derivation.

## Questions

1. Does the public proof independently establish that all 194 local files and
   all 366,021,385,004 bytes match the exact immutable publisher revision,
   with no exception, mutable-ref, cache-metadata, or filename-only claim?
2. Is the offline runtime identity exact and fail-closed: compiled manifest,
   repository/revision binding, optional all-or-none exact marker pair,
   retained descriptors, link rejection, exact byte accounting, and
   post-hash fingerprint revalidation?
3. Are all V1 semantic safeguards inherited without relaxation, including
   index/safetensors bounds, config, tier map, component family, scale,
   protected/shared tensors, TP ownership, layer 78, and complete inventory?
4. Does the typed receipt and four-rank consensus prevent profile,
   repository, revision, marker, source-map, tier, and fallback divergence,
   and prevent cross-admission from V1, EXL3, pure NVFP4, or another hybrid
   revision?
5. Are the corrective CPU-proof additions sufficient and correctly sequenced,
   and does the claim boundary avoid accepting conversion, residency, CUDA,
   checkpoint output, KLD, KV capacity, startup, reload, or performance?

Classify findings as blocker, major, or minor and explicitly report candidate
and input-hash checks at review start and finish.
