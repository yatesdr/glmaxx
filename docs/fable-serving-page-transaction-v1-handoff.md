# Fable handoff: serving page transaction v1

Date: 2026-07-29

Status: adversarial design review; implementation token withheld by Sol

GPU authorization conveyed by this handoff: none

Review candidate commit:
`e7bc4778119d43da7e1c76bfc584e5993d1fbb73`

## Provenance

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/serving-page-transaction-v1.md` | `e3a9a1d9f2eb26dc5312d7c42297fa3d832e444f7e3f269094746a85fb3deac2` |
| `docs/step-execution-io-v1.md` | `e8681e9278034b25fe6928c059ad58730818ce014fb3e0251549f678aa1621d5` |
| `docs/offline-serving-spine.md` | `27b24d4cbafc8203937d3620e7bcd85d47fcb86cc4d8b89e237025e5d40a62f9` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/sequence.rs` | `fe42a717a42b53f0c739b87f84303715a2a7b0c79c2efdf4af8691fe02e16b08` |
| `crates/glm-cache/src/residency.rs` | `b2495d7f656616ee0cd1eeadfa234f9e7555af6bd7b32f06da9d772bbed6e629` |
| `crates/glm-engine/src/memory.rs` | `3a50581a8a60970a92ccf5a2c0e83c23d25ad975f1124c2332e9a2e646dbc837` |
| `crates/glm-engine/src/step.rs` | `4963a58da7c9c6bbed0fb57fb7ef56d90d1e0f09fe54da8cc02c35891f743359` |
| `crates/glm-engine/src/output.rs` | `1a82e9990af4e2892831e950f5cd3db256032ab5e43642faeb08229cbf2f1c2c` |
| `crates/glm-engine/src/worker.rs` | `400c7c22f2c74d3f386fefe2d144da3437f27e6c99ce9f2d4bbf87ffe98fe437` |
| `crates/glm-scheduler/src/lib.rs` | `5651a507ad240f19755d50336f09eb3ca97e32f8be51f90e0fe49ef304350f38` |
| `crates/glm-serving/src/lib.rs` | `d9c50636c64f93d8648eb21ede2ed674e481d9af1381790b7fc55ed9ba417f8b` |
| `crates/glm-serving/src/cache.rs` | `786c7c7e5ce2f417749a78e8c48aa8a7d0a5cb617e0883e960a8e7c17d781720` |

Hash every input at review start and finish. Review the exact candidate commit
in a worktree if `main` advances. This handoff composes two separately pending
reviews—active sequence page table v1 and step execution I/O v1—but does not
presume either verdict.

## Candidate decisions

The candidate requires:

- prefix attachment, scheduler admission, and active page allocation to be
  one rollback-safe transaction;
- maximum target/draft write positions reserved before worker launch;
- a canonical generation-to-generation `PageTableDelta`;
- persistent owner-local device page tables with one global delta digest;
- an explicit page-delta-digest binding added to the pending `StepInput.v1`;
- commit preserving the physical IDs written by kernels;
- rollback of every row after any rank/output/collective failure;
- CACHE_ONLY removal updates and physical-ID quarantine until four-rank
  acknowledgment; and
- a fixed-capacity, page-granular undo log instead of the CPU oracle's
  whole-table clone and per-token loops.

## Requested adversarial questions

1. Is admission ordered so no scheduler-visible row can outlive a failed
   prefix/page attachment, and no restored pin can leak after a late
   scheduler error?
2. Must page reservation occur before or after `ScheduledBatch` becomes
   inflight? Specify the scheduler API needed to defer a capacity-blocked,
   unlaunched batch without failing or reordering it incorrectly.
3. Are prefill, MTP0, and MTP1–6 maximum reservations exactly sufficient,
   including target correction/bonus, accepted draft EOS, page crossing, and
   the model-position clamp?
4. Does the delta carry enough state for owner-local attention reads over the
   complete prior context, not merely current write pages?
5. Can suffix-only deltas, generation reuse, rank restart, cancellation, or
   an upload failure leave one device page table at a different logical
   mapping while all ranks acknowledge the same digest?
6. Is a global digest plus owner-local upload acknowledgment sufficient, or
   must every rank also hash its expected local projection and device-visible
   bytes?
7. Does the pending `StepInput.v1` require an ABI amendment, a companion
   immutable object, or a new v2? Identify the exact field and canonical
   preimage required.
8. Can rollback preserve every physical ID and payload byte needed by a
   partial MTP accept without free-and-reacquire behavior?
9. Is CACHE_ONLY cleanup sufficient to prevent ABA page-ID reuse when the
   service becomes idle, a rank fails, or a terminal event is backpressured?
10. Which page-table mutations advance the generation, and can two mutations
    legally coalesce into one delta without obscuring scheduler or RNG
    atomicity?
11. Is the proposed fixed-capacity undo-log bound derivable from C64, prefill
    chunk size, MTP6, and 64-token pages for every admitted graph?
12. Does terminal cleanup order preserve shared prefix references while
    releasing private target/draft tails and residency pins exactly once?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Withhold
the implementation token unless every blocker and major is resolved. State
separately whether:

- the transaction design may proceed to CPU implementation;
- `StepInput.v1` may survive with an amendment or requires v2;
- `SequencePageTable` needs an API change before integration; and
- any issue blocks independent qualified CUDA kernel work.

