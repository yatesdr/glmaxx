# Fable handoff: hybrid MTP3 capacity ledger v1 r2

Date: 2026-08-03

Status: adversarial corrective-design review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4 or allocate GPU/storage memory for this review.

Review candidate commit:
`2b8785907c11d2b58d8c5fa7f782845fae03e3ad`

Required result path:
`fable-hybrid-mtp3-capacity-ledger-v1-r2.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`hybrid-mtp3-capacity-ledger-v1-r2-design-accepted`

## Provenance

Review in a detached worktree, hash every input at start and finish, and
withhold the token for any mismatch.

| Candidate input | SHA-256 |
|---|---|
| `docs/hybrid-mtp3-capacity-ledger-v1-r2.md` | `6efeee90addafb4f8d645610bf617f1f4dd9b1bd630096f570193f407c49c9c6` |
| `docs/nf3-nvfp4-native-rank-manifest-v1-r2.md` | `ee81b6fc50a9a948af48aaf60d992fceb4c1bcb0c1687ac8cbe6f133a15baf9a` |
| `docs/nf3-nvfp4-hybrid-source-and-kernel-v1-r2.md` | `80a055c354971e7ff82b0eeeb54413cd6acafb7ee31611426f3154accc35b2aa` |
| `docs/offline-engine-contract.md` | `b5a51b15a0a600031fcddb7d840d4d499cf915d4c22f26099cdb1dc188d74fe1` |
| `docs/native-engine-plan.md` | `493c0d218d93a3a8d7cf83da45a934fc44570fc190e85340c5eaba74edd50bdd` |
| `crates/glm-engine/src/memory.rs` | `2131c999b6762a9b7e505cfe542c957877d95af4ee04056affa9d677156e9491` |
| `crates/glm-cache/src/kv.rs` | `fe5f4b8e07c8a32c6534f6217d62057f3ddd7c4b1abfcc00489c550a39660721` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

Run `./scripts/local-checks.sh` at the candidate and retain its exit status.

## Required independent work

1. Recompute corrected minimum records, exact immutable arenas, raw/device
   metadata difference, and all sensitivity values.
2. Independently derive DCP4 committed slots, C64 owner slack, MTP3 tentative
   slots, pages, every KV/indexer term, committed bytes, and extra bytes.
3. Prove 1,048,576-position addressing remains valid while global admission
   and per-rank physical quotas cap residency at 524,288.
4. Attack `max(load_peak,serve_peak)` for loader, conversion, readback, graph,
   collective, hot-reload, and cache owners that can overlap.
5. Audit the checked planner and four-rank physical write/checksum gate for
   virtual-reservation, aggregate-HBM, unexplained-byte, asymmetric-rank,
   insufficient-escrow, and partial-touch escapes.

## Decisions

Answer each `YES` or `NO`:

1. Are corrected weight and metadata charges exact and subordinate to the
   complete native planner?
2. Are 524,288 admitted positions, 135,424 slots/rank, 2,116 pages/rank, and
   all four cache byte terms exact for C64/MTP3?
3. Does the one-million address domain remain honest and fail-closed under the
   smaller physical quota?
4. Does phase-aware accounting charge every simultaneous owner and permit
   lifetime aliasing only after an enforced four-rank transition?
5. Are the residual and provisional margin exact and clearly not fit evidence?
6. Do planner and physical gates establish genuinely backed capacity with at
   least 1 GiB per-rank escrow?
7. May only CPU ledger/planner implementation begin after acceptance?

Return ordered findings, derivations, and all seven decisions. Only if all are
`YES`, attest the candidate and all eight hashes, then end with the requested
token as the only bare acceptance line.
