# Fable handoff: TR3 3.25 publisher-manifest reconciliation v1

Date: 2026-08-04

Status: design review requested

GPU authorization conveyed by this handoff: none

Review candidate commit:
`f758c1ab82557b1a44fd746d9b36d76faeaa28d8`

Current HEAD and the working tree may drift. Review the candidate in a
detached worktree and verify every pinned input at the start and finish.

Required result path:
`fable-tr3-325-publisher-manifest-reconciliation-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`tr3-325-publisher-manifest-reconciliation-v1-design-accepted`

Emit the token only if all four questions receive an unqualified **YES**.
Otherwise withhold it and identify the smallest corrective scope.

## Pinned inputs

| Path | SHA-256 |
|---|---|
| `docs/tr3-325-publisher-manifest-reconciliation-v1.md` | `804e259ed345b7c52fa4c07e0ba12eea702547728e8a0073bb04fac35fa7a14a` |
| `docs/cn4-checkpoint-runtime-identity-20260804.md` | `af5ef876669f0493c2b065954b683c1ad50c2eb131779c2c021b2879f16a5cdc` |
| `docs/cn4-tr3-source-audit-20260803.md` | `0898b6fd74001cc5606725f00c548a3ccf9d74ae484fba2003432056803bf110` |
| `crates/glm-format/src/checkpoint.rs` | `08450f0cb33e592ec76dfbe655b06580ba1743e60f3109a65218d052dbea406c` |
| `crates/glm-format/src/exl3.rs` | `f6fa1b25311d78e13e22a0c7c908da7abca636948218fef1987c89850e974edb` |
| `docs/production-rank-manifest-validation-v2.md` | `542c48d969ddebc40a14aefe269deff85656054ef937053034718650d8eb0f45` |
| `fable-production-rank-manifest-validation-v2.md` | `e3e260d262b0b4ec04a2fb74ed6055fafaf4c7b86d3b87a0f6033bb1b226dd61` |
| `docs/checkpoint-ingest.md` | `186ce5985ca1adbf280a011a7692ae07780736f842c786dccb599ed8a458d07d` |
| `scripts/local-checks.sh` | `2d1882be9afd91f4a54c1d3ff9b9f02cd5087357eeb5668d4094c2114c3003ce` |

The authoritative full-manifest evidence root is
`/home/derek/glmaxx/evidence/20260803T130218Z-tr3-manifest-r2` on cn4. Its
record hashes are pinned in the source-audit input. This handoff grants no cn4
access and does not require GPU work.

## Independent public anchors

Fetch the exact bytes from:

```text
https://huggingface.co/willfalco/GLM-5.2-EXL3-TR3-3.25bpw/resolve/e2b03576cd103e6ad322a1e091e5d0e2d0529073/MANIFEST.sha256
https://huggingface.co/willfalco/GLM-5.2-EXL3-TR3-3.25bpw/resolve/e2b03576cd103e6ad322a1e091e5d0e2d0529073/README.md
https://huggingface.co/willfalco/GLM-5.2-EXL3-TR3-3.25bpw/resolve/e2b03576cd103e6ad322a1e091e5d0e2d0529073/docker-compose.yaml
https://huggingface.co/willfalco/GLM-5.2-EXL3-TR3-3.25bpw/resolve/e2b03576cd103e6ad322a1e091e5d0e2d0529073/docker-compose.yml
```

Independently confirm manifest byte/row counts and hash, the two present-file
hashes, HTTP 404 for the `.yml` path, and the 9,263-byte canonical
present/absent JSON digest. Do not rely solely on the candidate's derivation.

## Questions

1. Does the immutable publisher revision, the full cn4 manifest audit, and the
   runtime recheck jointly prove that exactly 96 present files and one absent
   file describe the local 3.25-bpw tree, without authenticating it through
   the stale `.manifest_verified` marker?
2. Are the two complete present-replacement tuples and one complete
   absent-at-revision tuple sufficiently exact and narrow that no weight,
   runtime, tokenizer, tier, calibration, or script mutation can pass?
3. Does the canonical `absent`/`present` identity, path/fingerprint algorithm,
   byte accounting, marker handling, and four-rank receipt consensus close
   missing-file ambiguity and verify-then-reopen/rank-local fallback risks?
4. Does the separate profile identity and proposed CPU mutation proof prevent
   either the existing 3.0-bpw verifier or this new 3.25-bpw verifier from
   being widened or cross-admitting the other checkpoint, while keeping the
   implementation authorization within the stated claim boundary?

Classify findings as blocker, major, or minor and explicitly report candidate
and input-hash checks at review start and finish.
