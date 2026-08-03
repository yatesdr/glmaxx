# Fable review: durable tier-store single-writer ownership v1

Date: 2026-07-31

Reviewer: Fable (adversarial CPU implementation review)

Handoff: `docs/fable-durable-store-single-writer-v1-handoff.md` (queue row 21)

Reviewed candidate commit:
`535a8d6764ff968a21cb5d668e1d895ef0e940fb`

Implementation commits named by the proof (`ef14161` writer lock/reader
split, `3726864` snapshot-isolation regression) verified as ancestors of the
candidate with an identical final `store.rs`.

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
| `Cargo.lock` | `392c03e631b234e57cf9950078f2add73d06ae427e1616438725f001fe414bec` |
| `crates/glm-cache/Cargo.toml` | `176be2353dcee1c479714247fedf380cd36de29a8390069406e4853250d89e67` |
| `crates/glm-cache/src/lib.rs` | `1ade53e58b2f9f9f122185ad4a6c986dd4b8fa815e7533cf7ccf7ea8bb07b00e` |
| `crates/glm-cache/src/store.rs` | `5a4229e2c82c158ed6172a574c912ee2145438959bf676491282cd61d9d49247` |
| `crates/glm-cache/src/residency.rs` | `30ad6f64069b5766c71d9c8c78e90ad4e25a8cbf2db66a70d36a36c1eeda3c3f` |
| `docs/online-prefix-publication-v1.md` | `67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873` |
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `docs/durable-store-write-fail-stop-proof-v1.md` | `067c2b1f93a72ca0a5e661be02bad665da658154e3bfdf9ab33744137873dd09` |
| `docs/durable-store-single-writer-proof-v1.md` | `cc8e5182bad079c53504780c8ab1f6a7a7f410f094610965e5acd140837f4f47` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Commands run in the detached worktree (isolated target dir):

- `glmaxx review-proof docs/fable-durable-store-single-writer-v1-handoff.md`
  — pass.
- `cargo test --offline -p glm-cache` — 51 passed, 0 failed, including
  `journal_lock_enforces_one_live_store_writer`.
- `cargo test --offline -p glm-serving --lib` — 32 passed, 0 failed
  (matches the "all 32 serving tests" claim).
- `cargo test --offline --workspace` — 248 passed, 0 failed (used for the 248-test
  claim; see answer 14).
- `cargo clippy --offline -p glm-cache --all-targets -- -D warnings` —
  clean.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. `lock_shared_snapshot` maps `EWOULDBLOCK` to `StoreError::WriterLocked`.
   That name is accurate for the only real contention source (readers never
   conflict with readers under `LOCK_SH`), but the same variant is also what
   a writer receives when a reader is mid-snapshot; callers cannot
   distinguish "another writer is live" from "a snapshot is being taken".
   Both are transient and advisory; naming/reporting nit only.
2. `FileTierStore::open` creates `pages.dat` and `journal.log`
   (`create(true)`) before acquiring the exclusive lock. A losing second
   writer can therefore create empty files in a fresh directory before
   failing with `WriterLocked`. No content is written and replay semantics
   are unaffected; noted for completeness.

### QUESTION

None open.

## Answers to the handoff's required questions

1. YES. Before `ef14161` there was no lock; each `open` replayed the journal
   independently and derived its own `next_transaction`, `next_data_offset`,
   `TierJournal`, and `published` map from the same bytes.
2. YES. Two live instances would allocate the same transaction ID, compute
   the same aligned append offsets, interleave journal appends through
   separate descriptors, and disagree on durable visibility.
3. YES. `RestoreService::spawn` previously constructed `FileTierStore`
   (write-capable) though it only ever called `restore`; the residency diff
   shows the direct substitution to `FileTierReader`.
4. YES. `lock_exclusive_writer` (`flock LOCK_EX | LOCK_NB`) runs immediately
   after the journal descriptor opens, before `read_to_end`, decode, replay,
   or the construction of any writable handle.
5. YES. The locked descriptor is the `journal_file` field of the returned
   store; `flock` releases only when that descriptor closes on drop, so the
   lock spans publication and poisoned-state operation for the full writer
   lifetime.
6. YES. A second `FileTierStore::open` fails at the lock call with
   `WriterLocked` before any replay byte is read and before any mutation.
7. YES. `FileTierReader` has only `open` and `restore`; both `journal.log`
   and `pages.dat` are opened with read-only options; there is no
   publication or append surface.
8. YES. The reader's `LOCK_SH | LOCK_NB` fails with `WriterLocked` while a
   writer holds `LOCK_EX`. The shared lock is held by the local
   `journal_file` for the whole of `FileTierReader::open` — through
   `read_to_end`, strict decode, `from_events`, and `recover` — and released
   on scope exit, so the complete read/replay interval is protected.
9. YES. The regression opens four simultaneous readers after the writer
   drops and all four restore the same page with SHA-256 verification.
10. YES. A later writer replays the journal and appends only at or beyond
    the aligned end of all published extents; the regression's readers
    re-restore page one (checksum-verified) after the second writer
    publishes, which would fail on any overwrite.
11. YES. Reader catalogs are private and immutable after open; the
    regression proves the later-published page is absent from every existing
    reader.
12. YES. `RestoreService` owns `FileTierReader` only; `cargo test -p
    glm-serving --lib` passes all 32 tests at the candidate, including
    four-worker coordinator construction and restoration.
13. YES. The old implementation succeeds on the second `open`, failing the
    `Err(WriterLocked)` match; a lifetime shared reader lock would make step
    7 of the regression (writer opening while four readers are live) fail
    with `WriterLocked` instead of succeeding.
14. YES. `flock` is described as an advisory retained-blocking-store guard,
    not the final `TierIoService` ownership mechanism; private reader
    catalogs and their deliberate staleness are stated; the workspace run
    passed 248 tests matching the 248-test claim; the candidate
    tree contains 48 handoff documents of which 2 are historical without
    provenance tables, matching the 46 claim; no GPU/model/performance
    material is claimed.

## Required summary statements

- The retained store now has exactly one live writable authority. YES.
- Restore workers have no journal/data mutation capability. YES.
- Snapshot creation is serialized safely against the writer. YES.
- Four read workers and a later writer preserve immutable snapshot extents.
  YES.
- The regressions distinguish both the old multi-writer and the
  overlocked-reader alternatives. YES.
- The CPU proof and all exclusions are accurate. YES.

## Architecture & maintainability

The writer/reader type split is the right ownership expression: capability
is encoded in the type (`FileTierReader` cannot publish by construction),
not in runtime checks. Holding the exclusive lock via the long-lived journal
descriptor makes lock lifetime equal writer lifetime with zero extra state.
The shared-then-release reader protocol gives exactly the isolation the
retained store needs — a consistent snapshot without blocking later writers
— and its limits (advisory, private catalogs, deliberate staleness) are
documented rather than papered over. `restore_published` deduplicates the
read path between both types. The unsafe `flock` calls are minimal, fenced,
and commented with the safety argument.

## Token decision

All six required answers are unqualified YES; zero blockers, zero majors.
Input hashes verified identical at review start and finish; `review-proof`
passed against the pinned bytes. The token accepts only this retained
synchronous CPU ownership correction; it does not open cn4, authorize CUDA
work, or accept real model execution.

durable-store-single-writer-v1-accepted
