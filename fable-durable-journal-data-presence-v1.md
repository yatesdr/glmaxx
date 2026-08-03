# Fable review: durable journal/data presence v1

Date: 2026-07-31

Reviewer: Fable (adversarial CPU implementation review)

Handoff: `docs/fable-durable-journal-data-presence-v1-handoff.md`
(queue row 19)

Reviewed candidate commit:
`f72437917bec6df3ab8382575f5521ce491d356d`

Implementation commit named by the proof (`d3ca693`) verified as an ancestor
of the candidate with an identical final `store.rs`.

Location note: the handoff declares the result path at the repository root;
the operator directed all review artifacts into `docs/reviews/`.

## Provenance

All 11 input hashes were verified by SHA-256 of the exact bytes in a
detached worktree at the pinned candidate, at review start and re-verified
at review finish; both sets matched the handoff table exactly. The handoff
file itself postdates the candidate commit, so it was copied into the
worktree untracked to run `review-proof`, which passed, and removed
afterward.

| Input | SHA-256 (start = finish) |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/store.rs` | `7408e65a42e4e15598a761587dec31b63736c316f4f49a0d42c47cfd44884dff` |
| `docs/durable-store-write-fail-stop-proof-v1.md` | `067c2b1f93a72ca0a5e661be02bad665da658154e3bfdf9ab33744137873dd09` |
| `docs/torn-journal-resume-proof-v1.md` | `2c0a5f131e72b41cf76f06e1127db7c9d492ba17332d9dec643392d301f85180` |
| `docs/journal-tail-corruption-proof-v1.md` | `d8bb0b738caba8afe5a5084ffbc969dacd9d01fa6eae298b9adb289cfa950a7d` |
| `docs/durable-catalog-extent-integrity-proof-v1.md` | `b8f38cf3ab3fde74d505ea7a118d063d3e235dd4049b6ce6e47c071099a2ea7d` |
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `docs/durable-journal-data-presence-proof-v1.md` | `fc19414d706e317dd59491b2c284b9931c911161fc176e220fe121211c480b26` |
| `docs/production-punchlist.md` | `471030142d4bc80113d81ca18b875a5225725faa0f55129f19d88bb4609b5e4e` |
| `docs/results-index.md` | `5c8ea74bbba0b066b47c8053fbeb3941945ef6cceefcf59d35c46265e7d4e963` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Commands run in the detached worktree (isolated target dir):

- `glmaxx review-proof docs/fable-durable-journal-data-presence-v1-handoff.md`
  — pass.
- `cargo test --offline -p glm-cache
  store::tests::nonempty_data_without_a_complete_journal_fails_closed` —
  pass.
- `cargo test --offline -p glm-cache store::tests` — 13 passed,
  0 failed.
- `cargo test --offline --workspace` — 267 passed, 0 failed (used for the 267-test
  claim; see answer 13).
- `cargo clippy --offline -p glm-cache --all-targets -- -D warnings` —
  clean.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. The invariant is deliberately one-sided (zero complete records plus
   nonempty data). Nonempty data alongside at least one valid record is
   accepted as potential crash orphans, so replacement of the journal with a
   DIFFERENT valid shorter journal remains detectable only insofar as
   catalog-extent and (later, row 20) transaction-sequence validation catch
   it. Correctly stated in the proof's exclusions; noted to fix the boundary
   of what this token accepts.

### QUESTION

None open.

## Answers to the handoff's required questions

1. YES. `publish_prevalidated` runs `journal.begin` (in-memory), appends the
   complete 512-byte `Begin` via `append_journal_record`, and calls
   `sync_data` on the journal before the first `seek`/`write_all` against
   `pages.dat`.
2. YES. Both the append (`write_all`) and `sync_data` propagate errors with
   `?` before the payload loop, so a legitimately torn first `Begin` can
   only coexist with an empty data file; the write-fail-stop latch
   additionally prevents any same-process continuation.
3. YES. Once one complete `Begin` is durable, no retained code path removes
   complete records: appends only extend, the row-75 repair truncates only
   the sub-record remainder, and no automatic salvage exists. At least one
   complete record therefore survives any later data/piece/publish failure.
4. YES. Previously an empty (or fully truncated, or torn-only) journal
   decoded to zero events, `recover` returned an empty catalog,
   `validate_catalog_extents` passed vacuously against any data length, and
   both constructors reopened as a successful empty cache — silent total
   catalog loss.
5. YES. Both constructors compute
   `journal_len / JOURNAL_RECORD_BYTES * JOURNAL_RECORD_BYTES` and pass that
   to `validate_journal_data_presence`; a 113-byte torn-only journal counts
   as zero complete bytes, not as nonzero raw length.
6. YES. `validate_journal_data_presence` returns exactly
   `StoreError::UnjournaledData` iff `valid_journal_bytes == 0 &&
   data_bytes != 0`.
7. YES. The check runs in `FileTierStore::open` and `FileTierReader::open`
   before `decode_journal`, before replay/`recover`, before catalog-extent
   validation — and in the writer before the truncation branch, so a
   torn-only journal next to nonempty data is preserved as evidence rather
   than repaired to empty.
8. YES. The regression writes 4,096 data bytes against an empty journal and
   asserts `UnjournaledData` from both constructors, then writes a 113-byte
   journal fragment and asserts the same from both constructors — four
   distinguishing assertions.
9. YES. The prior implementation (parent of `d3ca693`) succeeds on all four:
   empty journal and torn-only journal each yield an empty catalog through
   both constructors (the writer additionally truncating the torn fragment
   to zero under the row-75 repair, destroying the evidence).
10. YES. With at least one complete record, `valid_journal_bytes != 0` and
    the presence check is inert; `torn_trailing_journal_record_is_ignored`
    still proves reader passthrough, exact 113-byte writer repair, post-
    repair publication, and full recovery on a further reopen.
11. YES. A complete corrupt record reaches strict `decode_journal` and fails
    with `JournalChecksum`/`JournalEncoding`;
    `complete_corrupt_trailing_journal_record_is_never_ignored` still passes
    at this candidate, and the presence check cannot misclassify it because
    complete bytes are nonzero.
12. YES. The invariant is deliberately one-sided: data beyond the last
    published extent after at least one valid record is a legitimate
    append-only crash orphan (begin-journaled/data-synced failpoints produce
    exactly that), protected by the physical-EOF allocation rule.
13. YES. The workspace run passed 267 tests with zero failures,
    matching the 267-test claim; the candidate tree contains 63 handoff
    documents of which 2 are historical without provenance tables, matching
    the 61 claim; the GPU/direct-I/O/model/performance exclusions match the
    change content (CPU-only store code, no CUDA, no online publication).

## Required summary statements

- Retained write ordering makes zero complete records plus nonempty data
  unreachable without journal loss or corruption (durable `Begin` strictly
  precedes the first payload byte). YES.
- Both writer and snapshot reader fail closed on that state. YES.
- The complete-record versus raw-tail boundary is exact
  (floor-to-512 in both constructors). YES.
- Adjacent torn-tail and complete-corrupt-record behavior remains intact.
  YES.
- The regression distinguishes all four prior successful-open paths. YES.
- The CPU proof and all exclusions are accurate. YES.

## Architecture & maintainability

The invariant is a two-line pure function shared verbatim by both
constructors — no policy duplication, no new state. Its placement before
decode/replay/truncation is the subtle part and is correct: it turns
"journal history vanished" into a preserved, diagnosable startup error
instead of an auto-repaired empty cache. The one-sidedness (MINOR 1) is the
right cut for this store: the presence check owns the zero-history case,
extent validation owns bounds/overlap, and row 20's sequence rule owns
interior deletion, each separately reviewable.

## Token decision

All six required answers are unqualified YES; zero blockers, zero majors.
Input hashes verified identical at review start and finish; `review-proof`
passed against the pinned bytes. The token accepts only this retained CPU
startup-integrity correction; it does not open cn4, authorize CUDA work, or
accept online publication or model execution.

durable-journal-data-presence-v1-accepted
