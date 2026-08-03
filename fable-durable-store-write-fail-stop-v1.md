# Fable review: durable tier-store write fail-stop v1

Date: 2026-07-31

Reviewer: Fable (adversarial CPU implementation review)

Handoff: `docs/fable-durable-store-write-fail-stop-v1-handoff.md` (queue row 22)

Reviewed candidate commit:
`a5019aafa7400f82928d944b0fb9a31ddae0605d`

Implementation commits named by the proof (`10a7bca` fail-stop, `a96f3b1`
preflight tail alignment) verified as ancestors of the candidate with an
identical final `store.rs`.

Location note: the handoff declares the result path at the repository root;
the operator directed all review artifacts into `docs/reviews/`.

## Provenance

All 9 input hashes were verified by SHA-256 of the exact bytes in a detached
worktree at the pinned candidate, at review start and re-verified at review
finish; both sets matched the handoff table exactly. The handoff file itself
postdates the candidate commit, so it was copied into the worktree untracked
to run `review-proof`, which passed, and removed afterward.

| Input | SHA-256 (start = finish) |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `crates/glm-cache/src/store.rs` | `8658f495486cfe35e9b7bc9581520201cad30a5704d9be4a2f44b36c09df07ed` |
| `crates/glm-cache/src/tier.rs` | `c31b07d7f9054f3d51bc5d24c2c414b6c9a134d88f042502bc0f82e29cad500f` |
| `docs/online-prefix-publication-v1.md` | `67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873` |
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `docs/cache-lifecycle-proof-v1.md` | `11ad4936fea7cd0887e660911f50778d5b0918c21a6cebaca1a98a244b2e2de1` |
| `docs/durable-store-write-fail-stop-proof-v1.md` | `067c2b1f93a72ca0a5e661be02bad665da658154e3bfdf9ab33744137873dd09` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Commands run in the detached worktree (isolated target dir):

- `glmaxx review-proof docs/fable-durable-store-write-fail-stop-v1-handoff.md`
  — pass.
- `cargo test --offline -p glm-cache store::tests::` — 6 passed,
  0 failed, including both distinguishing regressions.
- `cargo test --offline --workspace` — 247 passed, 0 failed (used for the 247-test
  claim; see answer 13).
- `cargo clippy --offline -p glm-cache --all-targets -- -D warnings` — clean.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. At this candidate, a writable reopen after a genuinely torn journal tail
   still seeks to the physical end of file, so the first post-reopen append
   lands off the 512-byte record boundary. Fail-stop closes the in-process
   window (a partial `write_all` poisons the writer, so no same-process
   append can follow it), but the cross-restart window is out of this
   handoff's boundary and was corrected separately at `da22ab4`/`8fb3adf`
   (torn-journal-resume v1, queue row 75). No silent loss: the strict decoder
   of that era suppressed only final-record errors, and the fail-stop change
   itself neither widens nor narrows that replay boundary.
2. `TierJournal::piece_durable`/`publish`/`open_record` each scan the full
   in-memory event vector, so live publication cost grows linearly with
   journal history (quadratic cumulative over many publications). Open-time
   also reads the whole journal into memory. Acceptable for the retained
   blocking store's declared nonproduction role; the production path is the
   separately reviewed direct-tier design.

### QUESTION

None open.

## Answers to the handoff's required questions

1. YES. Before the correction the durable sequence ran inline in
   `publish_inner`; any error after `journal.begin`, the durable `Begin`
   append/sync, payload writes/sync, piece-durable appends, or in-memory
   publication returned as an ordinary error with no state latch.
2. YES. Nothing marked the store; a second `publish` re-entered the append
   path on the same descriptors.
3. YES. A failed `write_all` can leave a partial 512-byte record; a
   subsequent append converts that trailing fragment into interior bytes
   that fixed-size decoding consumes as a corrupt complete record, which the
   trailing-crash-tail exception cannot excuse. This is the concrete hazard
   the poison latch closes for the live process.
4. YES. Stale generation, piece sort/duplicate/empty and per-piece size
   checks, checked extent arithmetic, final tail alignment
   (`align_up(next_offset)` hoisted by `a96f3b1`), and `record.validate()`
   all complete in `publish_inner` before `publish_prevalidated` is entered.
5. YES. `publish_inner` wraps the single call site:
   `if result.is_err() { self.write_poisoned = true; }` — every error path
   out of `publish_prevalidated`, including `JournalSequence`, every append,
   write, sync, journal transition, and all three injected crash failpoints,
   sets the latch before control returns to the caller.
6. YES. `write_poisoned` is checked first in `publish_inner`, before the
   stale-generation read, any file operation, offset/sequence change,
   in-memory journal mutation, or published-map access.
7. YES. Preflight errors return before the wrapper and do not set the latch;
   the regression proves an empty-pieces rejection and a stale-generation
   rejection are each followed by a successful publication, with validation
   unchanged.
8. YES. `restore` does not consult `write_poisoned` and reads only the
   `published` map, which is inserted into only after the final publish
   sync; the failed transaction is never visible. Verified by the
   regression's post-failure restore of the prior page.
9. YES. Reopen replays the durable journal; `TierJournal::recover` exposes
   only transactions with a durable `Publish` record and all attested
   pieces; orphans stay invisible; the fresh handle starts unpoisoned.
   There is no in-process unpoison path.
10. YES. The regression records both file lengths after the injected
    failure, proves the poisoned rejection changes neither, proves prior-page
    readability, failed-page invisibility after replay, and a successful
    post-replay publication of the same key.
11. YES. `crash_before_publication_leaves_only_invisible_orphans` covers
    `BeginJournaled`, `DataSynced`, and `FirstPieceJournaled`, asserting
    `WritePoisoned` on the next publication for each phase. I additionally
    verified in a scratch copy that each orphan phase can be followed after
    reopen by a successful contiguous publication with the orphan invisible.
12. YES. The prior implementation (parent of `10a7bca`) had the failpoints
    but no latch; the second publication succeeds there, so the
    `Err(WritePoisoned)` matches fail at all three phases and in the
    file-length regression.
13. YES. `cargo test --offline --workspace` in the pinned worktree passed
    247 tests with zero failures, matching the 247-test claim. The
    candidate tree contains 47 handoff documents, of which 2 are historical
    without provenance tables (consistent with the repo's `review-proof-all`
    convention), matching "all 45 then-present review handoff provenance
    proofs". The proof's GPU/model/performance exclusions match the change
    content: CPU-only store code and tests, no CUDA, no model execution, no
    performance claims.

## Required summary statements

- Preflight/commit boundary: complete and correctly placed — validation-only
  work before `publish_prevalidated`, all uncertain-durability work inside
  it. YES.
- Every uncertain publication failure fail-stops later writes. YES.
- Ordinary validation failures remain safely retryable. YES.
- Poisoned reads and reopen/replay preserve only durable visibility. YES.
- The regressions distinguish the prior behavior at every injected phase.
  YES.
- The CPU proof and all exclusions are accurate. YES.

## Architecture & maintainability

The two-phase split is the smallest structure that makes the latch
verifiable: one wrapper owns the poison decision, so no individual error
path inside the commit phase can forget it. Moving the final tail-alignment
computation into preflight removes the last post-mutation arithmetic error
source. The latch is a plain `bool` on a single-threaded owner type —
correct here because the single-writer lock work (row 21) later guarantees
one live writer per journal. The in-memory `TierJournal` event-vector scans
are the main scaling debt (MINOR 2) and are confined to the retained
blocking store. Test hygiene is good: failpoints are compiled into the
non-test path as an `Option` parameter threaded through `publish_inner`,
which keeps injected and real error paths byte-identical.

## Token decision

All six required answers are unqualified YES; zero blockers, zero majors.
Input hashes verified identical at review start and finish;
`review-proof` passed against the pinned bytes. The token accepts only this
synchronous CPU durability correction; it does not open cn4, authorize CUDA
work, or accept real model execution.

durable-store-write-fail-stop-v1-accepted
