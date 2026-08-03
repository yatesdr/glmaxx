# Fable handoff: hybrid MTP3 capacity ledger v1

Date: 2026-08-03

Status: design review requested

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch GPU, container, storage-device,
or network work for this review

Review candidate commit:
`94b09e7a2c4281116f38eed10b4ce97e35ebf833`

Required result path: `fable-hybrid-mtp3-capacity-ledger-v1.md` at the
repository root.

Requested acceptance token, only if every blocker and major is resolved:
`hybrid-mtp3-capacity-ledger-v1-design-accepted`

## Provenance

Review the exact commit in a detached worktree. Hash every input at start and
finish and withhold the token for any mismatch.

| Input | SHA-256 |
|---|---|
| `docs/hybrid-mtp3-capacity-ledger-v1.md` | `465af873bd388166beee5e84e3d2d4272f9501813bdfe95c1e5a3c05b062c2a8` |
| `docs/cn4-hybrid-source-inventory-20260803.md` | `caa270096611f2acfdfdef5f8cafd743b49d3b675e9779813dc5aeb7c400e247` |
| `docs/nf3-nvfp4-hybrid-source-and-kernel-v1.md` | `bd4606d4335f4450b6f5611f54c95a06077ef4ed644812b4dac111aca1c7e01b` |
| `docs/nvfp4-fused-routed-moe-v1-r3.md` | `f60d3adb777321ed715ae465abcf20895dbbf470c20970855636ed9b2b3a4db0` |
| `docs/offline-engine-contract.md` | `b5a51b15a0a600031fcddb7d840d4d499cf915d4c22f26099cdb1dc188d74fe1` |
| `docs/native-engine-plan.md` | `493c0d218d93a3a8d7cf83da45a934fc44570fc190e85340c5eaba74edd50bdd` |
| `crates/glm-engine/src/memory.rs` | `2131c999b6762a9b7e505cfe542c957877d95af4ee04056affa9d677156e9491` |
| `crates/glm-cache/src/lib.rs` | `331c724306021969f7f9174589b680ca93077017a3248c678a693358187ba4f4` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

## Required independent work

1. Re-derive the per-rank protected, 14,400-expert NF3, 5,056-expert NVFP4,
   and minimum total record charges from the source shapes and native record
   contracts. Identify any omitted metadata, padding, alignment, or duplicate
   protected allocation.
2. Independently derive the DCP4 committed slots, C64 MTP3 tentative slots,
   page slack, physical pages, all four cache byte terms, committed bytes, and
   slack bytes. Attack the premise that MTP3 needs four target and four draft
   tentative positions per active sequence.
3. Determine whether retaining one-million-position addresses while admitting
   only 524,288 logical positions can preserve page-table correctness, prefix
   identity, DCP ownership, and global/per-rank quota enforcement.
4. Recompute the sensitivity ledger from 101,955,141,632 bytes: the
   3,611,725,824-byte residual, 102,235,729,920-byte old-term total,
   280,588,288-byte deficit, and 12,152,832-byte post-staging deficit. Treat
   every native-alignment byte as additional until measured.
5. Adversarially audit `max(load_peak, serve_peak)`. Look for a loader,
   readback, graph, collective, allocator, or hot-reload lifetime that can
   overlap KV serving and invalidate the alias. Confirm that the collective
   quiesce/free/verify/allocate transition is sufficient or require a stronger
   invariant.
6. Evaluate the proposed CPU-planner inputs and rejection rules against the
   existing hard-coded capacity-EXL3/MTP6 planner. Look for arithmetic,
   asymmetric-rank, address-domain, page-table, or rank-local fallback escapes.
7. Decide whether allocating, writing, device-checksumming every byte, proving
   per-rank escrow, and reconciling allocation deltas is a sufficient physical
   HBM capacity gate. A virtual reservation or sampled touch must not pass.
8. Verify that the candidate makes no fit, conversion, checkpoint, quality,
   throughput, or SM120 evidence claim.

## Decisions

Answer each `YES` or `NO`:

1. Are the minimum weight-record identities, counts, and byte totals exact and
   explicitly subordinate to the completed native manifest's arena charge?
2. Are 524,288 admitted positions, 131,072 committed slots per rank, 135,424
   physical slots, 2,116 pages, and every cache byte total exact for C64/MTP3?
3. Can the profile preserve the 1,048,576-position address domain without
   falsely advertising that much physical residency?
4. Does the phase-aware budget charge every simultaneous HBM owner and permit
   aliasing only across an enforced collective lifetime boundary?
5. Are the sensitivity result and current non-fit conclusion conservative and
   arithmetically correct?
6. Do the planner requirements fail closed for profile drift, overflow,
   asymmetric ranks, hidden bytes, insufficient escrow, or insufficient HBM?
7. Is the four-rank allocation/write/checksum gate sufficient to establish
   physically backed capacity rather than configuration or virtual capacity?
8. May this design enter checked CPU-planner implementation, followed by a
   separately reviewed implementation and authorized cn4 gate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, followed
by independent derivations and all eight decisions. Only if every decision is
an unqualified `YES`, attest all nine hashes and end with the requested token
as the only bare acceptance line.
