# Review: active sequence removal atomicity v1

Date: 2026-07-30

Reviewer: Fable (adversarial design-gate review, detached worktree)

Reviewed candidate commit: `876e4ca59be4c7a8243288c57cf79ef3cbebc5d4`

Handoff: `docs/fable-sequence-removal-atomicity-v1-handoff.md`
(SHA-256 `d67345b0075ea17116799c7f4b3beb306f37bcd8396bc09f3fbd7beb3d073747`)

Worktree: detached checkout of the candidate at
`/private/tmp/claude-501/-Users-derek-glm5-native/f0e57b4e-b3ca-4b43-a75e-93057551ef6b/scratchpad/wt-row63`.
No cn4 connection, no CUDA, offline cargo only.

Result-path note: the operator directed review artifacts into
`docs/reviews/`, so this file is written at
`docs/reviews/fable-sequence-removal-atomicity-v1.md`. The handoff declares
the required result path as `fable-sequence-removal-atomicity-v1.md` at the
repository root; on acceptance this file may need to be moved (or copied) to
that root path for `review-proof-all` to bind it.

## Input hash verification

All five pinned inputs were hashed with `shasum -a 256` inside the worktree
at review start and again at review finish. Both sets matched the handoff
table exactly; the worktree HEAD was `876e4ca5…` at both points.

| Input | SHA-256 (identical at start and finish) | Match |
|---|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` | yes |
| `crates/glm-cache/src/sequence.rs` | `c31f74eda75c9dfa93c03ce2d569175b3cda67c5fa8f0a56506c778b596a79c8` | yes |
| `docs/serving-page-transaction-v1.md` | `e3a9a1d9f2eb26dc5312d7c42297fa3d832e444f7e3f269094746a85fb3deac2` | yes |
| `docs/sequence-removal-atomicity-proof-v1.md` | `0baa3ff73b3fad73dd3471ee89fca9ab3278d5223fdae85c40f0e9066f11bc2b` | yes |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` | yes |

Throwaway probe tests were added to the worktree copy of `sequence.rs`
during the review and fully reverted before the finish hash; the finish hash
of `sequence.rs` above proves the reviewed source is bit-identical to the
candidate.

## Commands reproduced

- `cargo run --offline -p glm-cli --bin glmaxx -- review-proof docs/fable-sequence-removal-atomicity-v1-handoff.md`
  — verdict `PASS`, all five inputs verified against the candidate commit
  via `git cat-file` (see MINOR-1 for the required handoff import).
- `cargo test --offline -p glm-cache` — 49 passed, 0 failed.
- `cargo clippy --offline -p glm-cache --all-targets -- -D warnings` — clean.
- `cargo test --offline --workspace` — 18 suites, 246 passed, 0 failed
  (run against pristine candidate sources).
- Static count: `git grep -c '#[test]' 876e4ca -- '*.rs'` sums to exactly
  246 at the candidate commit.
- Handoff census at the candidate tree: 46 `docs/fable-*-handoff.md` files,
  of which 2 are the declared historical handoffs
  (`fable-phase-a-engine-handoff.md`, `fable-review-handoff.md`), leaving 44
  verifiable handoffs — matching the 44-handoff claim.

## Independent re-derivation of the removal ordering

`git show 435f514 -- crates/glm-cache/src/sequence.rs` in the worktree shows
the prior `remove_sequence` verbatim:

```rust
let sequence = self.sequences.remove(&sequence_id)
    .ok_or(SequencePageError::Sequence)?;
if sequence.tentative.is_some() {
    self.sequences.insert(sequence_id, sequence);
    return Err(SequencePageError::Transaction);
}
for page in sequence.pages.into_iter().rev() {
    self.release_page(page.physical)?;
}
```

The sequence record is removed first, pages are released one-by-one in
reverse ordinal order, and the `?` returns mid-loop on the first
`release_page` error. `release_page`
(`crates/glm-cache/src/sequence.rs:653-679` at the candidate) decrements the
reference count, removes the physical record at refcount zero, removes the
prefix mapping, and inserts the target (and any draft) local ID into the
owner-rank free set. Therefore a late release error (e.g. a missing or
corrupt ordinal-zero record) could coexist with: freed physical IDs already
in free sets, decremented reference counts on shared pages, removed prefix
mappings, and no sequence handle left for repair-and-retry. (The old code
did reinsert the sequence on the tentative-rejection path only; release
errors had no such protection.)

The corrected `remove_sequence` (`sequence.rs:369-389`) takes
`let snapshot = self.clone();` before any mutation. `SequencePageTable`
derives `Clone` over all six fields (`sequence.rs:78-86`): `config`,
`free_target: [BTreeSet<u32>; 4]`, `free_draft: [BTreeSet<u32>; 4]`,
`physical`, `prefixes`, and `sequences` — i.e. every sequence, every
physical page record and refcount, every prefix mapping, and both the
target and draft owner-local free sets. On any error the whole-table
assignment `*self = snapshot;` (`sequence.rs:384-387`) restores the complete
pre-call state, including the tentative-rejection path, which now errors
inside the closure with no remove/reinsert special case.

## Computational probes (throwaway, reverted, all passed)

Five probe tests were added to the worktree `sequence.rs` test module and
run with `cargo test -p glm-cache probe_` (5 passed, 0 failed), then
reverted:

- probe (a): three-page target-only sequence including one shared-prefix
  sealed page; ordinal-zero physical record removed behind the table. Failed
  removal returns `Invariant` and the full `Debug` rendering of the table
  (all BTree containers, deterministic order) is byte-identical before and
  after the failed call — sequence present with all three pages, surviving
  ordinals' records and refcounts intact, free sets and prefix map
  unchanged. Repair and retry then drains `sequences`, `physical`, and
  `prefixes` completely.
- probe (b): MTP sequence with two pages carrying draft sidecars, same
  late-failure injection. After repair and retry, both target local IDs and
  both draft local IDs are present in exactly their owner-rank free sets and
  all four rank free sets (target and draft) are back to full capacity, so
  no ID landed in a wrong owner-local set and no draft slot leaked.
- probe (c): removal of a sequence holding a tentative transaction returns
  `Transaction` and the full table `Debug` state is byte-identical,
  tentative record included; `rollback_tentative` followed by
  `remove_sequence` then succeeds and frees everything.
- probe (d): the pre-435f514 `remove_sequence` was reconstructed verbatim in
  the worktree copy and run against the exact committed-regression setup
  (two pages, corrupted ordinal zero). It fails every distinguishing
  assertion: the sequence handle is lost, the ordinal-one physical record is
  gone, the ordinal-one target ID is already in its rank free set, and a
  retry returns `Sequence` (unrepairable) — proving the regression
  distinguishes old from new for exactly the claimed reverse-release
  ordering.
- probe (e): the uniform snapshot path preserves the tentative record on
  `Transaction` rejection with no special-case reinsert.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. The handoff's own provenance command cannot run as written inside a
   clean checkout of the candidate: `docs/fable-sequence-removal-atomicity-v1-handoff.md`
   does not exist at commit `876e4ca` (it postdates the candidate), so
   `glmaxx review-proof` fails at `repository_file()`'s `canonicalize()`
   (`crates/glm-cli/src/review.rs:614`) before any verification. The
   reviewer must import the handoff into the worktree as an untracked file
   (done here; hash `d67345b0…` recorded above), after which the proof
   passes because inputs are verified via `git cat-file` at the pinned
   commit. Fail-closed (an error, never a false pass), hence MINOR, but the
   provenance section should state that the handoff file must be imported
   into candidate checkouts.
2. That failure surfaces as the bare message
   `glmaxx: No such file or directory (os error 2)` with no path —
   `ReviewProofError::Io` forwards the raw `io::Error` without naming the
   file (`crates/glm-cli/src/review.rs`). Diagnosability gap only.
3. The label "clone-on-error" (proof doc and
   `docs/serving-page-transaction-v1.md:150`) is imprecise: the clone is
   unconditional at the start of every mutating call — including fully
   successful calls and calls rejected trivially (e.g. removing a
   nonexistent sequence at `sequence.rs:370`, which clones the whole table
   before the existence check). Only the restore is on-error. The cost
   scoping to the CPU oracle remains accurate, so no claim is overstated;
   "snapshot-before, restore-on-error" would be the honest name.
4. `remove_sequence` could pre-check `sequences.contains_key` and the
   tentative flag before cloning (as `commit_tentative` pre-checks at
   `sequence.rs:340-347`), avoiding an O(table) deep clone on trivially
   rejected calls. Oracle-scope only.

### QUESTION

1. The proof doc's "Relevant hashes" list includes
   `docs/backend-event-cancellation-fatal-proof-v1.md`, which is not among
   the handoff's pinned inputs — intentional cross-reference to the sibling
   gate, or leftover?
2. The main working tree has advanced past the candidate (handoffs trailing
   their candidate commits appears to be standing practice, per MINOR-1) —
   confirm this trailing-handoff flow is intended so `review-proof` docs can
   be adjusted once rather than per-review.

## Answers to the 12 required adversarial questions

1. YES. The pre-435f514 code removed the sequence record first, then
   released pages in reverse ordinal order (diff shown above; old
   `sequence.rs:369-381` at 435f514^).
2. YES. The mid-loop `?` return in the old code left later-ordinal pages
   already released — refcounts decremented, IDs in owner free sets, prefix
   mappings removed — with the sequence record gone, so retry returned
   `Sequence` and the invariant was unrepairable. Demonstrated
   computationally by probe (d).
3. YES. `admit_with_prefix` (`sequence.rs:122`), `append_committed` (:206),
   `fork_sequence` (:233), `begin_tentative` (:295), `commit_tentative`
   (:348), and `rollback_tentative` (:360) all use the identical
   `let snapshot = self.clone(); … *self = snapshot;` pattern.
4. YES. `remove_sequence` clones the entire `SequencePageTable`
   (`sequence.rs:370`); the derived `Clone` covers `sequences`, `physical`,
   `prefixes`, and both `free_target` and `free_draft` arrays
   (`sequence.rs:78-86`).
5. YES. The error path restores by whole-table assignment
   (`sequence.rs:384-387`), not by reconstructing the failed sequence;
   probe (a) proves byte-identical `Debug` state across a failed call.
6. YES. Tentative rejection errors inside the closure
   (`sequence.rs:376-377`) and is restored by the same snapshot assignment;
   the old remove/reinsert special case is gone. Probes (c) and (e).
7. YES. The regression (`sequence.rs:831-868`) appends 65 tokens with
   `PAGE_TOKENS = 64` (`lib.rs:37`), producing exactly two physical pages on
   distinct DCP owners (ranks 0 and 1 via `owner_rank = ordinal % 4`,
   `page.rs:53`), then removes the ordinal-zero record so reverse release
   frees ordinal one before failing late with `Invariant`.
8. YES. It asserts the sequence still holds both logical pages (:849), the
   ordinal-one physical record survives with `references == 1` (:850), and
   the ordinal-one target ID is excluded from its rank free set (:851-854).
9. YES. Probe (d) ran the reconstructed old implementation against the
   identical setup: sequence handle lost, ordinal-one record freed,
   ordinal-one ID present in the free set, retry impossible — every
   distinguishing assertion fails for exactly the claimed reverse-release
   ordering.
10. YES. The committed test (:856-867) and probe (b) show repair-and-retry
    removes both records and returns both target IDs — and, in the MTP
    probe, both draft IDs — to exactly their owner-rank free sets, with all
    four rank sets restored to full capacity (no wrong-owner placement).
11. YES. The proof doc's closing paragraph and
    `docs/serving-page-transaction-v1.md:138-151` explicitly scope full-table
    cloning to the CPU oracle and forbid it for the production
    fixed-capacity hot path ("must not clone the full page table or loop
    once per prompt token"; fixed-capacity undo log; ID quarantine). No
    production performance claim is made. See MINOR-3 for the naming nit.
12. YES. 246 tests confirmed twice (static `#[test]` count at the candidate
    and a 246-pass workspace run); 44 handoffs = 46 present minus 2 declared
    historical; the GPU/model/performance non-claims are accurate — no
    CUDA, cn4, checkpoint, model execution, or performance evidence exists
    or is claimed anywhere in the reviewed inputs.

## Six required summary statements

1. Sequence removal is all-or-nothing on every returned error: YES.
2. Physical references, prefix mappings, and free sets restore together:
   YES.
3. The failed removal remains exactly repairable and retryable: YES.
4. The distinguishing regression fails the prior code for the claimed
   reason: YES.
5. The clone-on-error CPU-oracle scope is accurate: YES.
6. The CPU proof and all exclusions are accurate: YES.

## Architecture & maintainability

- Duplication: the snapshot/restore epilogue
  (`let snapshot = self.clone(); … if let Err(e) = result { *self = snapshot; return Err(e); }`)
  now appears verbatim in seven methods. A single
  `fn transactional(&mut self, f: impl FnOnce(&mut Self) -> Result<(), SequencePageError>)`
  helper would delete ~40 lines and, more importantly, make it structurally
  impossible for a future mutation method to forget the pattern — which is
  precisely the defect class this candidate fixed.
- Duplication: the per-token page-extension loops in
  `append_committed_inner` (:472-513) and `reserve_tentative` (:522-561)
  are ~30 nearly identical lines; a shared "ensure writable tail page"
  helper would collapse them.
- Hot-path cost honesty: within the oracle, `remove_sequence` is
  O(total table size) per call for the unconditional clone plus
  O(pages × log P) for release; `append_committed` is O(clone + T log P).
  No superlinear surprise beyond the documented oracle clone; the
  production contract in `serving-page-transaction-v1.md` correctly forbids
  carrying this pattern forward.
- Simplification: `remove_sequence` (and `begin_tentative`) could hoist
  cheap rejections (unknown ID, tentative present, bad token count) above
  the clone, as `commit_tentative` and `fork_sequence` partially do —
  cheaper and makes the clone's purpose (mutation protection, not
  validation) clearer.
- Error granularity: `SequencePageError::Invariant` covers many distinct
  conditions; a failed removal does not say which ordinal failed.
  Acceptable for an oracle whose callers snapshot-compare, but the
  production undo log will want positional evidence.
- API surface: minimal and coherent (admit/append/fork/tentative
  begin-commit-rollback/remove plus read-only `pages`, `stats`,
  `committed_tokens`); tests reach private fields in-module only; layering
  between the oracle crate and the pending serving integration is clean and
  deliberately unintegrated, matching the review boundary.

## Token decision

All six summary statements are unqualified YES; there are zero BLOCKER and
zero MAJOR findings; start and finish input hashes matched the handoff
table at the pinned candidate. The acceptance token is emitted. Per the
handoff, this token accepts only the CPU metadata correction; it does not
open cn4, authorize CUDA work, or accept real model execution.

sequence-removal-atomicity-v1-accepted
