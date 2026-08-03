# Fable handoff: cn4 TP4 eager-module memory baseline

Date: 2026-08-03

Status: adversarial diagnostic review requested

GPU authorization conveyed by this handoff: none

Read-only SSH access to the named external evidence directory is authorized
for hash verification. Do not launch CUDA, start/stop a process, or write on
cn4.

Review candidate commit:
`f19e66e082ee8a2ace2b59db04c96e58295c0fb9`

Required result path:
`fable-cn4-tp4-eager-module-baseline-20260803.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`cn4-tp4-eager-module-baseline-20260803-diagnostic-accepted`

## Provenance

Review the candidate in a detached worktree. Hash every input at start and
finish and independently hash every external evidence record.

| Input | SHA-256 |
|---|---|
| `docs/cn4-tp4-eager-module-baseline-20260803.md` | `cbd9e55f9e42e33f021dd6a3356856fdad4cfd5a0f430bfff41e7201310ab189` |
| `docs/cn4-tp4-memory-baseline-20260803.md` | `2f4eed1617919427045bcc101c5a6e1a1b68d97b626ee889cd3ac4dbb71971fc` |
| `docs/nf3-nvfp4-native-rank-manifest-v1.md` | `72cb1f1aab8ab0a58663ea31080c9d23c9ac40950f2479682739388aea7e69c2` |
| `docs/hybrid-mtp3-capacity-ledger-v1.md` | `465af873bd388166beee5e84e3d2d4272f9501813bdfe95c1e5a3c05b062c2a8` |
| `crates/glm-cli/src/main.rs` | `381a74d0ef7311a95a2c5996be80b39eb76442489edcaf8a2f934beaf00cf518` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

External evidence:

```text
/home/derek/glmaxx/evidence/20260803T165800Z-eager-memory-baseline-36584b0
evidence-sha256.txt SHA-256 887a52a3a48c5eb5bde5d0db33dbc4b876c1aabfb0b1544c6e524697e1c1d09b
```

## Required independent work and decisions

1. Hash-verify the five JSON records, artifacts, posture, and before/after
   state. Are all samples byte-identical and stderr files empty?
2. Compare every rank with the pinned lazy baseline. Is the eager delta
   exactly 2,097,152 bytes per rank and the minimum exactly
   101,365,645,312 bytes?
3. Recompute the 3,019,092,032-byte residual and 468,955,200-byte provisional
   margin from the candidate arenas/cache/fixed terms. Are they exact?
4. Does the record clearly limit the measurement to the current linked module
   with no kernel launch, and avoid a fit, checkpoint, quality, or performance
   claim?
5. Is GLMAXX isolation preserved and is the post-run cn4 state clean?

Answer all five decisions `YES` or `NO`, with findings ordered by severity.
Only if all are unqualified `YES`, attest all six hashes plus the external
evidence-list hash and end with the requested token as the only bare
acceptance line.

