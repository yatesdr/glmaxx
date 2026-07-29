# Fable handoff: direct DRAM/NVMe tier I/O v1

Date: 2026-07-29

Status: adversarial design review; implementation token withheld by Sol

GPU authorization conveyed by this handoff: none

Review candidate commit:
`69895e040617a79dea78d7eaf1ced88234ccb193`

Requested acceptance token, only if every blocker and major is resolved:
`direct-tier-io-v1-accepted`

## Provenance

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `docs/online-prefix-publication-v1.md` | `67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873` |
| `docs/tenant-resource-quotas-v1.md` | `d779e4d6a4e4a6b5b57e4c76ab1cee504361df76ff8d2d78b174db00e4528cab` |
| `docs/serving-page-transaction-v1.md` | `e3a9a1d9f2eb26dc5312d7c42297fa3d832e444f7e3f269094746a85fb3deac2` |
| `docs/serving-observability-v1.md` | `4058d01d58c0d8f4d7222803e05577a9419cfa6f5d0f20a65c41e9e2779213e6` |
| `docs/benchmark-contract.md` | `cd51d22a8faf2baacfb4682ff5e1dcb5986edc27d8aa3af188105842bb49a507` |
| `crates/glm-cache/src/store.rs` | `d37a1400dc0c393b26c121f72694945bef78c28eda29796abf41a2ed713a17ac` |
| `crates/glm-cache/src/residency.rs` | `b2495d7f656616ee0cd1eeadfa234f9e7555af6bd7b32f06da9d772bbed6e629` |
| `crates/glm-cache/src/tier.rs` | `c31b07d7f9054f3d51bc5d24c2c414b6c9a134d88f042502bc0f82e29cad500f` |
| `crates/glm-cache/src/sequence.rs` | `fe42a717a42b53f0c739b87f84303715a2a7b0c79c2efdf4af8691fe02e16b08` |
| `crates/glm-serving/src/cache.rs` | `786c7c7e5ce2f417749a78e8c48aa8a7d0a5cb617e0883e960a8e7c17d781720` |
| `crates/glm-cache/Cargo.toml` | `5858d83830af59d4b491a42e978ec0bdaf72f36c253c233d93378c8d05f9ea93` |
| `Cargo.toml` | `863c28560b339f1fd7fb6b80c1b812e9fa7bc3f8f8c782126d2a29ceeffc06ea` |
| `Cargo.lock` | `ed694e41e6f1ba1723480d1052846d14d12086d85514b628133f5c2390d69bc1` |
| `crates/glm-engine/src/memory.rs` | `3a50581a8a60970a92ccf5a2c0e83c23d25ad975f1124c2332e9a2e646dbc837` |
| `profiles/profile-budget-v0.json` | `cdbe4eaad9465181b2ba60b3656fe5207eee54467abfbf8d9bc398c3e68c23e0` |
| `docs/production-punchlist.md` | `fb21a98010c8b68e811678b414be8cd1a9b6b86fd35af032b77ac9c3132a0f9f` |
| `docs/results-index.md` | `3cddd4f21fafc1cdabc4f112eb0b1e4f9b1f90bba8dcd361703fa5fd47ff623d` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/native-engine-plan.md` | `33552cd81e3d79b8b484856a99620420f3e2eddfdfa529a23b191353a702ed80` |

Hash every input at review start and finish. Review the exact candidate commit
in a separate worktree if `main` advances. The candidate deliberately
contains no io_uring implementation, storage benchmark, migration, or GPU
evidence.

## Requested adversarial questions

1. Re-derive every piece offset, padding interval, logical byte count,
   493/501-block physical length, and one-page SHA boundary.
2. Is one full-extent direct read/write preferable to separate padded piece
   operations while retaining per-piece durability metadata?
3. Must zero padding and a physical-extent SHA be normative, and does this
   close every unused-byte corruption ambiguity?
4. Are file offset, address, and length alignment requirements sufficient for
   the likely target filesystem/device, subject to the mandatory live probe?
5. Can one buffer be both io_uring-registered and CUDA-pinned safely? Identify
   allocation, registration, mlock, fork, and teardown constraints.
6. Is `TierBufferId(slot,generation)` plus an operation descriptor table
   sufficient to reject late I/O, cancel, hash, and CUDA completions?
7. Does every buffer state have exactly one owner and terminal release,
   including last-waiter cancellation and shutdown?
8. Are the required io_uring opcodes/features complete without SQPOLL? Is any
   stated requirement unavailable or filesystem-dependent in a way that
   blocks the architecture?
9. Does logical abandonment remain correct for every original/async-cancel
   CQE order and cancellation-not-found result?
10. Can CQ overflow or dropped completions occur despite the proposed sizing?
    State the exact startup/runtime invariant needed.
11. Is offloading SHA-256/padding validation to fixed workers safe without
    stalling the I/O authority or reusing a buffer early?
12. Does restore identity correctly handle target-only versus MTP capability,
    durable revision upgrades, and an MTP ticket serving target waiters?
13. Can concurrent waiters join deterministically while global physical and
    per-tenant logical charges remain exact?
14. Is the catalog-epoch pin sufficient if a cleaner or MTP upgrade relocates
    the key after the read is submitted?
15. Does a successful restore create exactly one shared HBM allocation and N
    logical references, and what HBM-copy synchronization must the later CUDA
    contract add?
16. Are DRAM-ready/pinned states backed by real allocations rather than labels,
    and can staging ownership transfer into DRAM without double accounting?
17. Is retaining at most one private partial tail per sequence outside the
    NVMe format acceptable for v1 mandatory offload, or is a session-tail
    format a prerequisite?
18. Do R0/R1/W0/W1 reserves and weighted bytes prevent read-buffer starvation,
    publication-lease starvation, and cleaner interference?
19. Can a publication fsync still damage decode/restore latency beyond the
    stated controls, and is matched measured isolation the only honest gate?
20. Does the durability order preserve all online-publication crash
    invariants? Can per-piece journal syncs be driven through io_uring without
    reordering across data sync and Publish?
21. Is the target-to-MTP upgrade atomic when the old target extent and new
    complete extent coexist through crash/restart?
22. Are immutable segments and the proposed cleaner relocation transaction
    sufficient for bounded long-running capacity?
23. What exact relocation journal/checkpoint fields are missing before a CPU
    implementation can begin?
24. Can old segments be unlinked safely after catalog epoch drain, including
    in-flight checksum jobs and DRAM records?
25. Do copy-on-write catalog shards avoid full-estate clones while providing
    one process-wide visible catalog and exact restart recovery?
26. Are integrity-fatal, tier-degraded, and write-local failure classes
    correctly separated? Identify any `EIO`, checksum, ENOSPC, or fsync case
    in the wrong class.
27. Can existing HBM/DRAM requests safely continue after global tier
    degradation without producing a false healthy state or rank-local route?
28. Does shutdown reap every original/cancel CQE and preserve accepted
    publication durability without relying on destructor timing?
29. Is the 25-case CPU/fault proof sufficient for alignment, ABA, dedup,
    cancellation, live catalogs, cleaning, restart, and final-zero ownership?
30. Are the required target-device performance and resident-decode isolation
    rows matched tightly enough to prevent buffered-cache or device mismatch
    claims?
31. Which durable format, journal, catalog, quota, residency, page
    transaction, publication, metrics, and dependency types must version
    atomically?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Withhold
the token unless every blocker and major is resolved. State separately
whether:

- the aligned full-extent layout is accepted;
- the io_uring/registered-buffer architecture is accepted;
- direct-format and pure CPU state-machine implementation may begin;
- the segment cleaner needs a more detailed pre-implementation amendment;
- the retained blocking store remains a nonproduction control;
- K03 and K05 must remain unpassed;
- HBM↔DRAM CUDA work remains a separate gate;
- any finding changes the 1M capacity or tenant quota arithmetic; and
- no cn4 access, target-storage probe, destructive migration, or GPU launch is
  authorized by the verdict.
