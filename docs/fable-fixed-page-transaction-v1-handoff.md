# Fable handoff: fixed-capacity page transaction v1

Date: 2026-07-29

Status: adversarial design review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`e1dd8d805801118750e0d93f3eb137fd5a493c0d`

Required result path:
`fable-fixed-page-transaction-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`fixed-page-transaction-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/fixed-page-transaction-v1.md` | `c03dd66f78b8e81ce5b0743d34091449d84c43d08e620a694a0c66b318a5d6fc` |
| `docs/serving-page-transaction-v1.md` | `31983cce95ee01a5968213d5daf12c7a855f75f8735314700f2b4a9e55625d1a` |
| `docs/page-reuse-quarantine-proof-v1.md` | `94b6c39ee57fafa926d6bc375bf2841c00f8586c38fe99700d54e9b86065d84c` |
| `docs/prefill-graph-profile-abi-v2.md` | `37154c9e31109acdf35a382c6be87b3a865e2b7f6ae8f801969526789dd41f91` |
| `crates/glm-engine/src/step.rs` | `4963a58da7c9c6bbed0fb57fb7ef56d90d1e0f09fe54da8cc02c35891f743359` |
| `crates/glm-cache/src/sequence.rs` | `8c0491d4f2d3e50da12e15961c8ac65a2fe5449a3527d40a38cdaa5ef27d644e` |
| `crates/glm-cache/src/delta.rs` | `71ac2da15e869a6f2470c3551a7cd6ec4ff387850a23240e9a44ad96a538ff16` |
| `crates/glm-engine/src/worker.rs` | `39a0c0b917921149869d2afc5d652815986bf776eca5ddc9b7abee41b4892652` |
| `crates/glm-serving/src/lib.rs` | `362312a48e1269f09f2f3f6e090dffcf896a8b6c688b65d6060e6b505aae0bae` |
| `docs/production-punchlist.md` | `c33a99d25c2c3ca0efcab8fd7caa02dcc3442fb7c241e86cae7664d26bd62a21` |
| `docs/results-index.md` | `62ea0224b77cdbbc202908465e3e8c4339ad33a5159f357ebcdd4cd946e3fcfe` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-fixed-page-transaction-v1-handoff.md
```

Independently enumerate every legal combination of:

```text
1 <= active rows <= 64
0 <= initial tail occupancy <= 63
sum(prefill tokens) <= 3072
0 <= MTP depth <= 6
1 <= committed verify count <= depth + 1
```

Do not accept the 174/128 capacities from the prose without an independent
derivation or exhaustive/optimization check.

## Review boundary

This review covers only the fixed-capacity CPU/control-plane design:
transaction segmentation, exact bounds, undo/delta storage, generation
sequence, receipt ordering, terminal removal, admission ownership, and
cache-only command choice.

It does not accept an implementation, CUDA-visible table, upload event,
stream dependency, device payload arena, tier movement, checkpoint
execution, model output, quality, capacity under live allocations, or
performance.

## Required adversarial questions

1. Is 174 a valid and tight maximum for page metadata edits under C64 and
   3,072 total prefill tokens over every initial tail occupancy?
2. Is 128 sufficient for depth-6 verification, including a page boundary on
   every row?
3. Are 64 row undos and 64 rejected-page retirements sufficient for every
   legal decode/verify batch?
4. Can any legal current or planned graph/profile shape exceed a stated
   count, including the 3,072-row prefill-control ABI?
5. Does the journal capture every field required to reverse tail mutation,
   page allocation, target/draft coupling, committed/tentative counts, and
   delta first-changed ordinals?
6. Can the delta be constructed from the journal without scanning/copying
   unchanged 1M context prefixes?
7. Does all-row/all-owner preflight precede mutation and eliminate
   rank-local allocation fallback?
8. Are pre-rank local rollback, post-rank successor rollback, commit, and
   publication generations ordered without reuse?
9. Can a failed or partial four-rank reservation/rollback make an ID
   allocator-visible while a live rank still resolves it?
10. Is separating terminal removal from the 174-entry reversible journal
    sound for empty, shared, MTP, and 16,384-page sequences?
11. Does acknowledge-before-destructive removal have a fail-closed answer
    if host invariant validation fails after ranks have removed the sequence?
12. Can cancellation be combined with commit only when complete removal
    preflight is already immutable and valid?
13. Is the standalone `ApplyDelta` command sufficient as the cache-only ABI
    without a compute `StepPlan`, graph, or collective?
14. Is large-prefix admission storage explicitly charged and owned without
    smuggling it into the compute-step journal?
15. Does startup allocate every structure whose compute path otherwise
    grows, including sequence page indexes, owner bitmaps, host delta
    staging, and rank/device staging?
16. Can the retained clone oracle and production fixed implementation be
    compared without allowing a rank-local or request-local route choice?
17. Is the required proof matrix sufficient to distinguish an off-by-one
    bound, lost undo, target/draft asymmetry, stale suffix, early reuse,
    partial publication, and hidden heap allocation?
18. Are all implementation/device/model/performance exclusions accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- all fixed capacities are sufficient and the prefill bound is tight;
- undo metadata is complete;
- direct suffix-delta construction is sound;
- reservation/commit/rollback generations are unambiguous;
- terminal removal is safe without a 1M-page undo;
- standalone cache-only application is implementable;
- admission and startup allocation ownership are complete; and
- the proof plan and exclusions are accurate.

Only if all eight answers are unqualified `YES`, end with the requested
token. Withhold it for a conditional pass, stale input, loose or insufficient
bound presented as exact, unjournaled mutation, full-prefix scan, ambiguous
generation, early reuse, rank-local fallback, unsafe post-receipt removal,
compute-plan cache-only dependency, uncharged admission storage, hidden
allocation, nondistinguishing proof, or overstated device/model claim.

The token accepts only this design and permits its CPU implementation. It
does not open cn4, authorize CUDA work, or accept production serving.
