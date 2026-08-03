# Fable review: durable journal transaction sequence v1

Date: 2026-07-31

Reviewer: Fable (adversarial CPU implementation review)

Handoff: `docs/fable-durable-journal-transaction-sequence-v1-handoff.md`
(queue row 20)

Reviewed candidate commit:
`397c76c8e0b8e04e43c3f4ed19f1ac55ec730018`

Implementation commit named by the proof (`a4bbfb0`) verified as an ancestor
of the candidate with an identical final `store.rs`.

Location note: the handoff declares the result path at the repository root;
the operator directed all review artifacts into `docs/reviews/`.

## Provenance

All 12 input hashes were verified by SHA-256 of the exact bytes in a
detached worktree at the pinned candidate, at review start and re-verified
at review finish; both sets matched the handoff table exactly. The handoff
file itself postdates the candidate commit, so it was copied into the
worktree untracked to run `review-proof`, which passed, and removed
afterward.

| Input | SHA-256 (start = finish) |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/store.rs` | `0a2cd6f96bceb3ed352e5ade9fca302ed5f1498e0280de59a4b57286672dff0c` |
| `docs/durable-journal-data-presence-proof-v1.md` | `fc19414d706e317dd59491b2c284b9931c911161fc176e220fe121211c480b26` |
| `docs/durable-store-write-fail-stop-proof-v1.md` | `067c2b1f93a72ca0a5e661be02bad665da658154e3bfdf9ab33744137873dd09` |
| `docs/torn-journal-resume-proof-v1.md` | `2c0a5f131e72b41cf76f06e1127db7c9d492ba17332d9dec643392d301f85180` |
| `docs/journal-tail-corruption-proof-v1.md` | `d8bb0b738caba8afe5a5084ffbc969dacd9d01fa6eae298b9adb289cfa950a7d` |
| `docs/durable-catalog-extent-integrity-proof-v1.md` | `b8f38cf3ab3fde74d505ea7a118d063d3e235dd4049b6ce6e47c071099a2ea7d` |
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `docs/durable-journal-transaction-sequence-proof-v1.md` | `3c3a863c29246da7e2e4666872604aed3e031c2c1c3ab2e40f94b421899079bc` |
| `docs/production-punchlist.md` | `ad42562663ad015af60e804354d59a9d1dfa9f63aded965be7817bc87a2af210` |
| `docs/results-index.md` | `437a67cff6f2fa30ff188fef44323091d2b22cb48acf2a0e95f233ea9c747822` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Commands run in the detached worktree (isolated target dir):

- `glmaxx review-proof
  docs/fable-durable-journal-transaction-sequence-v1-handoff.md` — pass.
- `cargo test --offline -p glm-cache
  store::tests::missing_complete_transaction_group_fails_closed` — pass.
- `cargo test --offline -p glm-cache store::tests` — 14 passed,
  0 failed.
- `cargo test --offline --workspace` — 268 passed, 0 failed (used for the 268-test
  claim; see answer 16).
- `cargo clippy --offline -p glm-cache --all-targets -- -D warnings` —
  clean.

Independent computational checks (run in a scratch copy of the candidate,
never in the reviewed worktree), all passed:

- skipped ID (`Begin` txn 4 after contiguous 1..2), a late `PieceDurable`
  for txn 1 after txn 2, a decreasing `Begin` txn 1 after txn 2, and a
  journal whose first complete record is a non-`Begin` — each rejected with
  exactly `JournalSequence`;
- orphan continuation at all three crash phases (begin-journaled,
  data-synced, first-piece-journaled): reopen, contiguous publication,
  further reopen restoring both durable pages with the orphan invisible.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. The proof's sentence "The complete store suite also proves the legal
   adjacent case" is loosely attributed for two of the three orphan phases:
   the committed suite proves post-orphan contiguous publication for the
   piece-attested phase
   (`failed_publication_poison_writes_until_replay_but_not_preflight_errors`)
   and orphan invisibility for all three phases
   (`crash_before_publication_leaves_only_invisible_orphans`), but no
   committed test publishes after reopening across a begin-only or
   data-synced orphan. I verified that behavior holds at the candidate with
   an independent experiment (all three phases pass); the claimed behavior
   is true, only its suite attribution is imprecise. Suggest extending the
   crash-phase test with a post-reopen publication per phase.

### QUESTION

None open.

## Answers to the handoff's required questions

1. YES. On an empty journal the decoder yields `next_transaction = 1`; each
   publication appends `Begin` at exactly the decoder-derived
   `next_transaction`, cross-checked against the in-memory journal's own
   counter (`JournalSequence` + poison on divergence), then increments by
   one under checked arithmetic.
2. YES. A crash leaves the current group incomplete; reopen derives
   `current + 1` from the orphan's own `Begin`, so the next group is
   contiguous and no later event references the orphan — verified
   computationally for all three phases.
3. YES. The prior decoder tracked only `maximum_transaction`; a journal
   whose first remaining transaction was two decoded cleanly and replayed as
   a catalog containing only page two.
4. YES. Same mechanism: 1 and 3 with the complete group 2 deleted decode
   cleanly under max-tracking and replay as pages one and three.
5. YES. The corrected loop inspects every complete record in physical order;
   any changed transaction ID other than checked `current_transaction + 1`
   returns `JournalSequence`.
6. YES. A changed ID is accepted only when the record is a `Begin`,
   including the file's first complete record (`current` starts at 0, so the
   first record must be `Begin` txn 1) — verified computationally with a
   lone `PieceDurable` journal.
7. YES. Decreasing IDs, late events for an older transaction, and skipped
   IDs all fall into the `transaction != current` arm with
   `transaction != current + 1` (or non-`Begin`) — each verified
   computationally with crafted CRC-valid records.
8. YES. `TierJournal::recover` independently rejects duplicate begins
   (`Journal`), pieces for unknown or already-published transactions
   (`Journal`), wrong-hash or duplicate pieces (`Checksum`), premature or
   duplicate publication (`NotDurable`), and applies the
   dedup/upgrade/generation matrix on recovered records.
9. YES. `next_transaction = current_transaction + 1` (checked) where
   `current_transaction` is the final validated contiguous ID; the
   max-scan derivation is gone from the file decoder.
10. YES. The regression rewrites the journal from the original bytes,
    removing exact 4-record (2,048-byte) groups; retained records and CRCs
    are byte-identical slices of the original file.
11. YES. Prefix deletion (group one removed) and interior deletion (group
    two of three removed) are each asserted through both
    `FileTierStore::open` and `FileTierReader::open` with exactly
    `Err(StoreError::JournalSequence)`.
12. YES. Under max-tracking both tampered journals decode and replay
    cleanly; recovery would expose only page two in the prefix case and
    pages one/three in the interior case — silent catalog loss.
13. YES on substance, with MINOR 1's attribution caveat: the committed
    crash-phase tests prove orphan invisibility for all three phases and
    post-orphan contiguous publication for the piece-attested phase; my
    independent experiment at the candidate proves post-orphan publication
    and full recovery for begin-only and data-synced orphans as well, so
    the decoder demonstrably accepts every legal writer sequence.
14. YES. At this candidate: torn-tail repair with post-repair publication
    (`torn_trailing_journal_record_is_ignored`), complete-corrupt-record
    rejection (`complete_corrupt_trailing_journal_record_is_never_ignored`),
    zero-history rejection
    (`nonempty_data_without_a_complete_journal_fails_closed`), and
    catalog-extent bounds/overlap tests all pass unchanged.
15. YES. The proof states that deletion of the final complete group remains
    indistinguishable from a legitimate crash before that group became
    durable, absent an independently durable high-water mark — that is
    exactly the residual exposure of a suffix-deletion attack, and it is
    excluded, not claimed.
16. YES. The workspace run passed 268 tests with zero failures,
    matching the 268-test claim; the candidate tree contains 64 handoff
    documents of which 2 are historical without provenance tables, matching
    the 62 claim; the GPU/direct-I/O/model/performance exclusions match the
    change content.

## Required summary statements

- The retained writer establishes the asserted contiguous sequence. YES.
- The decoder enforces that sequence for every complete record. YES.
- Both prefix and interior complete-group deletion fail closed. YES.
- Legal crash-orphan continuation remains accepted (verified for all three
  orphan phases). YES.
- The regression distinguishes the prior silent catalog loss. YES.
- The CPU proof and all exclusions are accurate (MINOR 1 records a test-
  attribution imprecision whose underlying behavioral claim I verified to
  be true; the proof's factual claims about behavior, counts, and
  exclusions hold). YES.

## Architecture & maintainability

The continuity rule lives where it belongs — in the single shared file
decoder — so the writer, the reader, and the in-memory `TierJournal` retain
their existing responsibilities untouched: framing/CRC per record, sequence
across records, semantic legality per transaction. The two-condition guard
(`current + 1` and `Begin`) is the entire rule, O(1) per record, and
composes correctly with the row-19 presence check (which owns the
zero-history case the sequence rule cannot see) and with the row-75 repair
(which never removes the complete records the rule inspects). The stated
final-group-deletion residual honestly bounds what a linear scan can prove
without a durable high-water mark.

## Token decision

All six required answers are unqualified YES; zero blockers, zero majors;
one MINOR (test-attribution wording, behavior independently verified true).
Input hashes verified identical at review start and finish; `review-proof`
passed against the pinned bytes. The token accepts only this retained CPU
transaction-continuity correction; it does not open cn4, authorize CUDA
work, or accept online publication or model execution.

durable-journal-transaction-sequence-v1-accepted
