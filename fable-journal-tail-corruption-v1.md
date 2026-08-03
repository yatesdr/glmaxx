# Fable review: complete journal-tail corruption v1

Date: 2026-07-31

Reviewer: Fable (adversarial CPU implementation review)

Handoff: `docs/fable-journal-tail-corruption-v1-handoff.md` (queue row 30)

Reviewed candidate commit:
`8612ec3a29421f707f0f231e3496a59bb81504b0`

Implementation commit named by the proof (`2da5724`) verified as an ancestor
of the candidate with an identical final `store.rs`.

Location note: the handoff declares the result path at the repository root;
the operator directed all review artifacts into `docs/reviews/`.

## Provenance

All 7 input hashes were verified by SHA-256 of the exact bytes in a detached
worktree at the pinned candidate, at review start and re-verified at review
finish; both sets matched the handoff table exactly. The handoff file itself
postdates the candidate commit, so it was copied into the worktree untracked
to run `review-proof`, which passed, and removed afterward.

| Input | SHA-256 (start = finish) |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/store.rs` | `30a281fbd79bccd58ebecfdb906029985ace6df50907b2b84476f044586b8fc0` |
| `crates/glm-cache/src/tier.rs` | `0a1541f13462bcdec92284911f96531b06869b60c7fe85fc5e9669c80fabe693` |
| `docs/durable-content-dedup-proof-v1.md` | `75fd16886ef50e4509fb0a7b0701417a1469a0dad78809b39ce08e3e736a7514` |
| `docs/durable-store-write-fail-stop-proof-v1.md` | `067c2b1f93a72ca0a5e661be02bad665da658154e3bfdf9ab33744137873dd09` |
| `docs/journal-tail-corruption-proof-v1.md` | `d8bb0b738caba8afe5a5084ffbc969dacd9d01fa6eae298b9adb289cfa950a7d` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Commands run in the detached worktree (isolated target dir):

- `glmaxx review-proof docs/fable-journal-tail-corruption-v1-handoff.md` —
  pass.
- `cargo test --offline -p glm-cache store::tests` — 9 passed,
  0 failed, including the new corrupt-tail regression.
- `cargo test --offline --workspace` — 254 passed, 0 failed (used for the 254-test
  claim; see answer 10).
- `cargo clippy --offline -p glm-cache --all-targets -- -D warnings` —
  clean.

Independent computational checks (run in a scratch copy of the candidate,
never in the reviewed worktree): a single bit flip in an interior record's
payload area fails both constructors with `JournalChecksum`; a bit flip in a
non-CRC payload byte of the final record (transaction field of the final
publish record) fails closed; and a complete 512-byte garbage trailing
record is rejected by `FileTierReader::open` as well as the writer (the
committed regression asserts the garbage case for the writer only). All
passed.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. The committed regression asserts the appended complete-garbage-record
   case through `FileTierStore::open` only; `FileTierReader::open` shares
   the same `decode_journal`, and I verified its rejection independently
   (see above), but a reader-side assertion would pin the shared-decoder
   property against future divergence.
2. At this candidate a writable reopen after an ignored sub-record tail
   still seeks to the physical end of file, so a later publication appends
   off the record boundary and the strict decoder then fails the NEXT open
   closed (`JournalChecksum`) — availability loss, though no longer the
   silent catalog loss this correction removes. Out of this handoff's
   framing-only boundary; corrected separately at `da22ab4`/`8fb3adf`
   (torn-journal-resume v1, queue row 75).

### QUESTION

None open.

## Answers to the handoff's required questions

1. YES. The prior `decode_journal` matched `Err(_) if index + 1 ==
   full_records => break` — every decoding error in the final complete
   record was suppressed, not just short-write artifacts.
2. YES. The suppressed `Err(_)` covered CRC (`JournalChecksum`), magic,
   version, event type, tier, piece-table, and `TierRecord::validate`
   failures (`JournalEncoding`/`Tier`), since all surface through the same
   `decode_journal_event` result.
3. YES. Dropping a corrupt final `Publish` record leaves its `Begin` and
   `PieceDurable` events as an invisible orphan; `recover` reopens
   successfully with the previously published page silently missing.
4. YES. The corrected loop propagates every `decode_journal_event` error for
   every `chunks_exact` record, including the last.
5. YES. Only the sub-512-byte remainder that `chunks_exact` excludes is
   ignored as a crash tail.
6. YES. The regression publishes a real page, flips the final byte of the
   final complete record (inside the CRC field of the trailing `Publish`
   record), and proves both `FileTierStore::open` and `FileTierReader::open`
   return exactly `JournalChecksum`. My scratch-copy check confirms the same
   for a non-CRC payload byte.
7. YES. The same regression separately appends a complete `0xaa` garbage
   record to a second valid store and proves rejection (writer-side; reader
   side verified independently — MINOR 1).
8. YES. `torn_trailing_journal_record_is_ignored` is unchanged at this
   candidate: a 113-byte tail after a valid journal reopens and restores the
   page, fixing the exact complete-record boundary from both sides.
9. YES. `decode_journal` is pure over the byte slice; neither constructor
   truncates, rewrites, or salvages at this candidate (writer-side tail
   repair arrives only with row 75's correction).
10. YES. The workspace run passed 254 tests with zero failures,
    matching the 254-test claim; the candidate tree contains 52 handoff
    documents of which 2 are historical without provenance tables, matching
    the 50 claim; the retained-CPU-boundary statement and the
    GPU/direct-I/O/model/performance exclusions match the change content.

## Required summary statements

- No complete corrupt journal record can be silently ignored. YES.
- Final-publish corruption cannot become silent catalog loss. YES.
- A genuinely short crash tail remains recoverable. YES.
- Writer and snapshot-reader construction use the same strict decoder
  (`decode_journal` is the single shared entry). YES.
- The regression distinguishes the previous last-record exception (the old
  decoder passes both corrupt-tail cases silently, losing the page in the
  first). YES.
- The CPU proof and all exclusions are accurate. YES.

## Architecture & maintainability

The fix deletes the special case rather than adding one: the decoder is now
a uniform strict map over complete records, with the torn-tail policy
expressed solely by `chunks_exact`'s remainder. That leaves exactly one
boundary to reason about and it is pinned from both sides by regressions.
CRC coverage is whole-record (CRC field zeroed before computation), so any
single-bit corruption anywhere in a complete record is detected — confirmed
computationally. Deliberately deferring truncation/repair keeps this change
observational and reviewable in isolation; the repair transaction that
builds on this strictness is row 75's separately reviewed correction.

## Token decision

All six required answers are unqualified YES; zero blockers, zero majors.
Input hashes verified identical at review start and finish; `review-proof`
passed against the pinned bytes. The token accepts only this retained CPU
replay-boundary correction; it does not open cn4, authorize CUDA work, or
accept online publication or model execution.

journal-tail-corruption-v1-accepted
