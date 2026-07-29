# Fable handoff: cache arena budget v2

Date: 2026-07-29

Status: adversarial review request; token withheld by Sol

GPU or conversion authorization conveyed by this handoff: none

Review candidate commit:
`c33648aa80ddfbcf3f40eaaec23d6d584a7fd543`

## Provenance

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/native-engine-plan.md` | `33552cd81e3d79b8b484856a99620420f3e2eddfdfa529a23b191353a702ed80` |
| `docs/offline-engine-contract.md` | `b5a51b15a0a600031fcddb7d840d4d499cf915d4c22f26099cdb1dc188d74fe1` |
| `docs/offline-serving-spine.md` | `e60c37dde218198dd201635adc1239bbece045ec0e0d708e220ca1c3e27c1eb3` |
| `docs/checkpoint-ingest.md` | `466b547948d8788a39231c487042f66b1ac5692d5fff279b09859feb6cab820c` |
| `crates/glm-engine/src/memory.rs` | `3a50581a8a60970a92ccf5a2c0e83c23d25ad975f1124c2332e9a2e646dbc837` |
| `crates/glm-engine/src/lib.rs` | `4d92f2de943454af5a2443b9b37c581544cf6f90351b92dce0c77be07b68af93` |
| `crates/glm-cache/src/sequence.rs` | `fe42a717a42b53f0c739b87f84303715a2a7b0c79c2efdf4af8691fe02e16b08` |
| `crates/glm-cli/src/main.rs` | `4792e45ef7417c6cc3c6c8fd7d780ce72aeab5cf554a28d3e1771de8db6d4863` |
| `profiles/profile-budget-v0.json` | `cdbe4eaad9465181b2ba60b3656fe5207eee54467abfbf8d9bc398c3e68c23e0` |
| `fixtures/engine-contract-proof-v1.json` | `a28686829ae46d62ab449eacae3a1b64bf965c43c22699bb4c9130ecedc9c1a2` |
| `scripts/cn4-phase-b.sh` | `9ace5b4d4b0e8d2d1ee048bc32295cf86d7393b8420c5653b7d2f9faca23dd6d` |

Hash every input at review start and finish. Review the exact candidate commit
in a worktree if `main` advances.

## Defect and candidate resolution

The prior blocked profile carried seven draft tentative pages but zero target
slack. Fable reported this as `fable-manifest-abi-v022.md` finding `m4`.
The executable planner also charged requested token slots directly without
rounding to complete 64-token pages.

The candidate now reserves on every rank:

```text
committed floor                         262,144 slots
C64 page/owner-alignment slack            4,096 slots
C64 x MTP6 target/draft tentative rows       448 slots
physical target or draft arena          266,688 slots
physical target or draft pages            4,167 pages
```

The MTP0-only target tentative floor is 64 slots. Requested target and draft
arenas are rounded up to complete pages before their KV and indexer byte terms
are computed. `SystemMemoryPlan.v2` emits one `CacheArenaLayout`, derived from
the smallest physical rank arena, that converts directly to
`SequencePageTable` configuration. A draft arena larger than its target arena
fails closed.

The changed blocked-profile terms per rank are:

```text
target KV          7,655,012,352
target indexer       739,259,136
draft KV              98,141,184
draft indexer         35,202,816
total required     94,010,248,704
```

The profile remains `conversion_allowed = false`, has no post-context
measurements, and lists every prior blocker. Its new hash deliberately does
not match the old hash pinned by `cn4-phase-b.sh`, so the qualification and
conversion path remains fail-closed.

The complete local gate passed at the candidate: 198 tests, workspace Clippy
with warnings denied, CUDA FFI type checks, all deterministic proof fixtures,
and the external pinned tokenizer proof.

## Requested adversarial questions

1. Is one full 64-token page per active sequence per rank a sufficient and
   non-understated bound for C64 ownership alignment and partial tails?
2. Is 448 target tentative slots sufficient for every C64 x MTP6 verifier
   schedule, including correction or bonus output? Is using the same 448
   bound for draft conservative and valid?
3. Can a combination of committed positions, page rounding, and tentative
   positions consume more than 4,167 pages on one owner rank while remaining
   within the advertised C64 and 1,048,576-position limits?
4. Re-derive all four byte terms and the 94,010,248,704-byte rank total from
   the candidate fields. Do not trust the checked-in constants.
5. Does using the smallest rank arena yield one rank-invariant page-table
   configuration without concealing stranded capacity or enabling a
   rank-local fallback?
6. Can any zero, overflow, non-page-aligned, target-only, excess-draft, or
   under-slack input undercount physical bytes or produce an invalid
   `PageTableConfig`?
7. Is the serialized `SystemMemoryPlan` change correctly identified as v2?
8. Does the changed profile hash remain fail-closed against every existing
   Phase-B and conversion authorization path?
9. Does this fully resolve prior finding `m4`, or is another page-slack term
   still missing?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Withhold
any acceptance token unless every blocker and major is resolved. State
separately whether:

- the static cache-arena arithmetic may be used by serving integration;
- prior `m4` is closed;
- the profile remains correctly blocked from conversion; and
- any finding requires a `SystemMemoryPlan.v3`.

