# Fable handoff: active sequence removal atomicity v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`876e4ca59be4c7a8243288c57cf79ef3cbebc5d4`

Required result path:
`fable-sequence-removal-atomicity-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`sequence-removal-atomicity-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/sequence.rs` | `c31f74eda75c9dfa93c03ce2d569175b3cda67c5fa8f0a56506c778b596a79c8` |
| `docs/serving-page-transaction-v1.md` | `e3a9a1d9f2eb26dc5312d7c42297fa3d832e444f7e3f269094746a85fb3deac2` |
| `docs/sequence-removal-atomicity-proof-v1.md` | `0baa3ff73b3fad73dd3471ee89fca9ab3278d5223fdae85c40f0e9066f11bc2b` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-sequence-removal-atomicity-v1-handoff.md
cargo test --offline -p glm-cache
cargo clippy --offline -p glm-cache --all-targets -- -D warnings
```

## Review boundary

This review covers only atomic, retryable removal in the clone-on-error CPU
`SequencePageTable` oracle. It does not accept the production fixed-capacity
undo log, rank page-table deltas, physical-ID quarantine, serving
integration, CUDA, checkpoint execution, model output, or performance.

## Required adversarial questions

1. Did prior `remove_sequence` remove the sequence before releasing pages
   one-by-one in reverse order?
2. Could a late release error therefore coexist with earlier freed physical
   IDs, changed reference counts, removed prefix mappings, and no sequence
   handle for retry?
3. Do admission, append, fork, tentative reservation/commit/rollback already
   use the same snapshot-on-error CPU oracle pattern?
4. Does corrected removal snapshot every sequence, physical page, prefix map,
   and target/draft free set before mutation?
5. On any removal error, is the complete pre-call state restored rather than
   reconstructing only the failed sequence?
6. Is tentative-sequence rejection also restored without a special
   remove/reinsert path?
7. Does the distinguishing regression create exactly two physical pages and
   corrupt ordinal zero so reverse release mutates ordinal one before failing
   late?
8. Does it prove the sequence, ordinal-one physical record/reference, and
   ordinal-one free-set exclusion all survive the failed attempt?
9. Would the old implementation fail those assertions for the claimed
   reverse-release ordering?
10. Does repair and retry remove both pages and return both IDs to their exact
    owner-local free sets?
11. Are clone/allocation costs explicitly limited to the CPU oracle and not
    misrepresented as the production fixed-capacity hot path?
12. Are the 246-test claim, 44-handoff claim, and every GPU/model/performance
    non-claim accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- sequence removal is all-or-nothing on every returned error;
- physical references, prefix mappings, and free sets restore together;
- the failed removal remains exactly repairable and retryable;
- the distinguishing regression fails the prior code for the claimed
  reason;
- clone-on-error CPU-oracle scope is accurate; and
- the CPU proof and all exclusions are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, partial release, lost
sequence, incorrect owner free set, non-retryable repair, a
nondistinguishing regression, or an overstated production claim.

The token accepts only this CPU metadata correction. It does not open cn4,
authorize CUDA work, or accept real model execution.
