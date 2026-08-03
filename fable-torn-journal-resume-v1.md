# Fable review: torn journal resume v1

Date: 2026-07-31

Reviewer: Fable (adversarial CPU implementation review)

Handoff: `docs/fable-torn-journal-resume-v1-handoff.md` (queue row 75)

Reviewed candidate commit:
`8fb3adf9535683b0de9b54fe2743cb5651b9bdc2`

Implementation commit named by the proof (`da22ab4`, fixture refresh
`c768220`) verified as ancestors of the candidate with identical final
`store.rs` and fixture bytes.

Location note: the handoff declares the result path at the repository root;
the operator directed all review artifacts into `docs/reviews/`.

## Provenance

All 8 input hashes were verified by SHA-256 of the exact bytes in a detached
worktree at the pinned candidate, at review start and re-verified at review
finish; both sets matched the handoff table exactly. The handoff file itself
postdates the candidate commit, so it was copied into the worktree untracked
to run `review-proof`, which passed, and removed afterward.

| Input | SHA-256 (start = finish) |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/store.rs` | `48fb1db40c25109d80b3c2a7ee0fc09346ac52a5758ade1e752a1e4fa577e6e0` |
| `crates/glm-cli/src/cache_proof.rs` | `3371395bb723d2ec092c16cfd28bcb25b54ca1e38fc2096dff471941b2ac9358` |
| `fixtures/cache-lifecycle-proof-v1.json` | `c1151c34a3a9bee4fd97dea11e807603a56c2af4d37deab813cc9b5631177d6a` |
| `docs/journal-tail-corruption-proof-v1.md` | `d8bb0b738caba8afe5a5084ffbc969dacd9d01fa6eae298b9adb289cfa950a7d` |
| `docs/cache-lifecycle-proof-v1.md` | `11ad4936fea7cd0887e660911f50778d5b0918c21a6cebaca1a98a244b2e2de1` |
| `docs/torn-journal-resume-proof-v1.md` | `2c0a5f131e72b41cf76f06e1127db7c9d492ba17332d9dec643392d301f85180` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Commands run in the detached worktree (isolated target dir):

- `glmaxx review-proof docs/fable-torn-journal-resume-v1-handoff.md` — pass.
- `cargo test --offline -p glm-cache store::tests` — 9 passed,
  0 failed, including the extended torn-tail regression.
- `cargo test --offline -p glm-cli cache_lifecycle` — pass.
- `glmaxx cache-lifecycle-proof` output compared byte-for-byte (`cmp`)
  against `fixtures/cache-lifecycle-proof-v1.json` — byte-identical.
- `cargo test --offline --workspace` — 254 passed, 0 failed (used for the 254-test
  claim; see answer 12).
- `cargo clippy --offline -p glm-cache -p glm-cli --all-targets -- -D
  warnings` — clean.

Independent computational checks (run in a scratch copy of the candidate,
never in the reviewed worktree): torn tails of 1 and 511 bytes are each
repaired to exactly the last record boundary with subsequent publication and
a further reopen restoring both pages; and a complete invalid record
followed by a 113-byte fragment makes `FileTierStore::open` fail with
`JournalChecksum` while the journal length stays byte-identical — no
truncation happens on any failed open. All passed.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. The post-truncation `sync_data` durability of a pure size shrink is
   filesystem-dependent (it is a metadata change). If the shrink is not yet
   durable at a crash, the fragment can reappear on the next open — but the
   repair is idempotent and re-runs before any append, so no misaligned
   append is reachable either way. Worth a comment; no behavior change
   needed.

### QUESTION

None open.

## Answers to the handoff's required questions

1. YES. The prior writable open decoded (ignoring the sub-record tail),
   then `seek(SeekFrom::End(0))` — the next append landed `journal_len %
   512` bytes past the last record boundary, permanently misaligned.
2. YES. Fixed-size `chunks_exact` decoding then consumes the fragment plus
   the head of the new record as one corrupt complete record; under the
   strict row-30 decoder the store becomes unrecoverable after having
   appeared to recover.
3. YES. The corrected order in `FileTierStore::open` is: exclusive lock →
   full read → strict `decode_journal` of every complete record →
   `TierJournal::from_events`/`recover` → `next_data_offset` derivation →
   only then the truncation branch. Any earlier error returns before
   `set_len` — verified computationally (journal byte-identical after a
   failed open).
4. YES. `valid_journal_bytes = journal_len / 512 * 512`; `set_len` runs only
   when a remainder exists and cuts exactly to the last complete boundary —
   verified for 1-, 113-, and 511-byte fragments. No complete record can be
   removed by construction.
5. YES. `set_len` is followed by `sync_data` before `seek(End)` and before
   any publication can append (MINOR 1 notes the metadata-durability
   nuance; idempotent re-repair covers the crash window).
6. YES. A complete invalid final record fails `decode_journal` inside the
   pre-truncation phase; the truncation branch is unreachable on that path.
7. YES. `FileTierReader::open` opens the journal read-only, never calls
   `set_len`, and the regression asserts the on-disk length is unchanged
   after a reader snapshot across a torn tail.
8. YES. The extended regression records the tainted length, proves the
   writer reopen leaves exactly `length_with_tail - 113`, publishes page B,
   and proves a further close/reopen restores both A and B.
9. YES. The prior implementation passes the reader step, does not truncate,
   appends B misaligned, and the second reopen's strict decode fails —
   `second_reopen` would panic on `JournalChecksum`.
10. YES. The fixture diff is exactly one field, `journal_sha256`
    (`4e408ba9…` → `daf77c50…`); all other fields and the 951-byte artifact
    shape are unchanged (byte count confirmed).
11. YES. `scripts/local-checks.sh` regenerates the lifecycle artifact and
    `cmp`s it against the fixture; the regeneration/compare pair reproduced
    byte-for-byte in the pinned worktree, and the deterministic
    `cache_lifecycle` test passes.
12. YES. The workspace run passed 254 tests with zero failures,
    matching the 254-test claim (the correction extends an existing test
    rather than adding one, so the total matches row 30's); the candidate
    tree contains 53 handoff documents of which 2 are historical without
    provenance tables, matching the 51 claim; the retained-CPU boundary and
    GPU/direct-I/O/model/performance exclusions match the change content.

## Required summary statements

- Writable recovery resumes only from a validated record boundary. YES.
- No complete invalid record is truncated or repaired. YES.
- Read-only snapshots never mutate the short tail. YES.
- Post-repair publication remains recoverable on the next restart. YES.
- The regression and refreshed fixture distinguish the prior behavior. YES.
- The CPU proof and all exclusions are accurate. YES.

## Architecture & maintainability

The repair is the minimum mutation the strict decoder permits: validate
everything first, then remove only bytes that cannot constitute a record.
Computing `valid_journal_bytes` once and reusing it for both the presence of
a remainder and the truncation target avoids any second source of truth for
the boundary. Keeping the reader strictly non-mutating preserves the row-21
ownership model (only the exclusive-lock holder may change the file). The
proof doc's eight-step open sequence matches the code line-for-line, which
makes future drift easy to catch. MINOR 1 (a one-line comment on shrink
durability and idempotence) is the only suggested polish.

## Token decision

All six required answers are unqualified YES; zero blockers, zero majors.
Input hashes verified identical at review start and finish; `review-proof`
passed against the pinned bytes. The token accepts only this retained CPU
torn-tail resume correction; it does not open cn4, authorize CUDA work, or
accept online publication or model execution.

torn-journal-resume-v1-accepted
