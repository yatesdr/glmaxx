# Fable handoff: NF3/ModelOpt-NVFP4 native rank manifest v1 r2

Date: 2026-08-03

Status: adversarial corrective-design review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4 or run CUDA. Review the pinned candidate and checked-in
source/header evidence only.

Review candidate commit:
`2b8785907c11d2b58d8c5fa7f782845fae03e3ad`

Required result path:
`fable-nf3-nvfp4-native-rank-manifest-v1-r2.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`nf3-modelopt-nvfp4-native-rank-manifest-v1-r2-design-accepted`

## Provenance

Use a detached worktree. Hash all inputs at start and finish and withhold the
token for any mismatch.

| Candidate input | SHA-256 |
|---|---|
| `docs/nf3-nvfp4-native-rank-manifest-v1-r2.md` | `ee81b6fc50a9a948af48aaf60d992fceb4c1bcb0c1687ac8cbe6f133a15baf9a` |
| `docs/nf3-nvfp4-hybrid-source-and-kernel-v1-r2.md` | `80a055c354971e7ff82b0eeeb54413cd6acafb7ee31611426f3154accc35b2aa` |
| `docs/hybrid-mtp3-capacity-ledger-v1-r2.md` | `6efeee90addafb4f8d645610bf617f1f4dd9b1bd630096f570193f407c49c9c6` |
| `docs/cn4-hybrid-r2-contract-audit-20260803.md` | `2e80f773468ffc89972ea8dbb6dee82b51fec6c0b3f49b319cc2ebf913698573` |
| `docs/cn4-hybrid-source-inventory-20260803.md` | `caa270096611f2acfdfdef5f8cafd743b49d3b675e9779813dc5aeb7c400e247` |
| `docs/hybrid-serving-manifest-v1.md` | `934787ea37a5dbd9b6778844adbeb0b40fd365d4653991fc7cbfe77df3c685cf` |
| `docs/nvfp4-fused-routed-moe-v1-r3.md` | `f60d3adb777321ed715ae465abcf20895dbbf470c20970855636ed9b2b3a4db0` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `crates/glm-format/src/container.rs` | `2fbe22c55d481e40699a02be9929f9a964f50bc08199853772dd314c85ade47f` |
| `crates/glm-format/src/rank_manifest.rs` | `cb57a81ad3643a86992c4fd9c2166e9ee238cc7e66063732355d14e485f73410` |
| `crates/glm-engine/src/weight.rs` | `d658cefefc17757a28258bafd0e13f5309e8adcbf2b30c4d2bdc97be9899ca19` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

Run `./scripts/local-checks.sh` at the candidate and record its exit status.

## Required independent work

1. Re-derive every target, draft, routed, protected, and total descriptor
   count, then independently generate and unsigned-UTF8-sort every name.
2. Simulate the complete payload planner with the protected inventory and
   prove every plane length, zero gap, unrounded tail, and total on all ranks.
3. Reproduce the two metadata-record counts, raw file metadata, 256-byte device
   stride, final tail, padding, and immutable arena. Explain why increasing
   ModelOpt records from 128 to 192 changes raw bytes but not the exact device
   arena.
4. Audit minor 3, profile 4, flags 59, codec `0x0300`, codec `0x0102`, layouts,
   schemas, domains, catalog, descriptor binding, and target-program scalar
   arity for every old-profile or W4A4 alias.
5. Verify that the 256-byte descriptor and 224-byte catalog carry every
   required semantic through existing authenticated fields without assigning
   meaning to a reserved byte.
6. Recompute catalog/descriptor bytes and every sensitivity number. Attack
   hidden file, host, device, loader, and runtime metadata copies and every
   implicit fit claim.
7. Attack the mutation matrix for ordering, one-byte-short, final-tail,
   one-scale fused, overflow, rank-consensus, and source-binding escapes.

## Decisions

Answer each `YES` or `NO`:

1. Are the source/profile authorities distinct, closed, and fail-closed?
2. Are all expert and descriptor counts and UTF-8 tensor IDs exact?
3. Are every routed/protected payload plane and the 94,006,274,048-byte weight
   arena exact with no hidden payload gap?
4. Are 7,471,104 raw metadata bytes, 9,961,408 device bytes, and 2,490,304
   padding bytes exact?
5. Do the format/profile/codec/layout/schema/domain identities prevent all
   old-profile, one-scale, and W4A4 aliases?
6. Can the existing descriptor and 224-byte catalog bind all required
   semantics without reserved-field reinterpretation?
7. Is two-scale FC1 and one-scale FC2 arity authenticated through metadata,
   target program, resident binding, and graph identity?
8. Are immutable-arena and capacity-sensitivity values exact and explicitly
   not allocation, fit, checkpoint, quality, or speed evidence?
9. Is the CPU proof sufficient and correctly placed before implementation
   review and any cn4 allocation?

Return ordered findings, derivations, and all nine decisions. Only if all are
`YES`, attest the candidate and all twelve hashes, then end with the requested
token as the only bare acceptance line.
