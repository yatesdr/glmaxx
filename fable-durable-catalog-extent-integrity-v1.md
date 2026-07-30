# Fable review: durable catalog extent integrity v1

Date: 2026-07-30

Reviewer: Fable (adversarial CPU implementation review)

Handoff: `docs/fable-durable-catalog-extent-integrity-v1-handoff.md`
(queue row 17)

Reviewed candidate commit:
`de2d43a44474427d6f67fdb7fa300307d7b1caed`

Implementation commit named by the proof
(`a44a69156e3a16ff71d609158c54598332745303`) verified as an ancestor of the
candidate with an identical `store.rs` hash.

Location note: the handoff declares the result path at the repository root;
the operator directed all review artifacts into `docs/reviews/`, so this
file may need moving to the declared path on acceptance.

## Provenance

All 9 input hashes were verified with `git show <commit>:<path> |
shasum -a 256` at review start and re-verified at review finish; both sets
matched the handoff table exactly at the pinned candidate. `main` has
drifted on `crates/glm-cache/src/store.rs`, so the review ran in a detached
worktree at the pinned commit. The handoff file itself postdates the
candidate, so `review-proof` was replaced by direct hash verification of
every table row.

| Input | SHA-256 (start = finish) |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/store.rs` | `a68c672d51f59b79efeb514f8690aa2263730b2df5ed1ece688793ed6f897996` |
| `crates/glm-cache/src/tier.rs` | `0a1541f13462bcdec92284911f96531b06869b60c7fe85fc5e9669c80fabe693` |
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `docs/durable-store-single-writer-proof-v1.md` | `cc8e5182bad079c53504780c8ab1f6a7a7f410f094610965e5acd140837f4f47` |
| `docs/torn-journal-resume-proof-v1.md` | `2c0a5f131e72b41cf76f06e1127db7c9d492ba17332d9dec643392d301f85180` |
| `fixtures/cache-lifecycle-proof-v1.json` | `c1151c34a3a9bee4fd97dea11e807603a56c2af4d37deab813cc9b5631177d6a` |
| `docs/durable-catalog-extent-integrity-proof-v1.md` | `b8f38cf3ab3fde74d505ea7a118d063d3e235dd4049b6ce6e47c071099a2ea7d` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Commands run in the worktree:

- `cargo test --offline -p glm-cache store::tests` — 12 passed, 0 failed,
  including all three new regressions.
- `cargo test --offline --workspace` — full suite green (used to check the
  proof's total-count claim; see answer 13).
- `cargo clippy --offline -p glm-cache --all-targets -- -D warnings` —
  clean.

Independent computational check: adjacent-pair overlap detection after
sorting was compared against O(n²) brute force over 5,000 randomized sets
of nonempty half-open intervals — equivalent in every case.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. **Duplicated overlap logic.** `TierRecord::validate`
   (`tier.rs:106-112`) checks intra-record piece overlap with an O(p²)
   scan while `validate_catalog_extents` (`store.rs:293`) re-checks the
   same pieces cross-record with the sort/adjacent method. Correctness is
   unaffected (p ≤ 3), but two hand-kept overlap implementations for the
   same invariant is a divergence risk; the catalog-level check subsumes
   the per-record one for live records.
2. **Whole-journal buffering at open.** `open` reads the entire
   `journal.log` into memory before decoding. Fine for the retained CPU
   store's scale and consistent with its boundary, but worth noting since
   the journal grows without bound across publications (no checkpointing
   in this store by design); the direct-tier contract's
   checkpoint/catalog-epoch design is the real fix.
3. **`resident_bytes` counts logical bytes of every live record**, i.e.
   catalog-referenced bytes, not physical file bytes; with the new
   physical-EOF allocator these diverge permanently after any crash orphan.
   Not wrong — just a metric-name hazard when tier telemetry arrives.

### QUESTION

1. `validate_catalog_extents` accepts a live extent that overlaps a crash
   orphan's bytes (orphans are invisible to the catalog and the allocator
   never reuses below physical EOF, so this cannot happen for records
   written by this implementation — only for a forged journal that aliases
   orphan space; such a journal is internally consistent and undetectable
   without payload authentication, which the boundary explicitly excludes).
   Confirmed out of scope; noted for the startup payload-authentication
   gate.

## Answers to the handoff's 13 questions

1. **Yes.** The prior reader and writer validated each record
   independently; two fully published records with individually valid,
   CRC-clean layouts whose piece intervals alias each other replayed to a
   healthy catalog. The forged-overlap regression constructs exactly this
   journal and proves the prior acceptance (see answer 10).
2. **Yes.** The prior open never compared live extent ends with
   `pages.dat`'s physical length; a one-byte truncation still reported
   healthy and failed only at restore time as a checksum/short-read error.
3. **Yes.** `validate_catalog_extents` derives every live half-open
   interval with `checked_add` (overflow → `StoreError::Overflow`), sorts
   `(start, end)` tuples with `sort_unstable` (deterministic total order on
   u64 pairs), and rejects any adjacent pair with `next.start < prev.end`.
4. **Yes — proven.** For nonempty half-open intervals sorted by
   `(start, end)`: if any pair (i, j), i < j, overlaps, then
   `start_{i+1} <= start_j < end_i`, so the adjacent pair (i, i+1) also
   overlaps; conversely adjacent overlap is overlap. Nonemptiness is
   guaranteed because `TierRecord::validate` (run on every replayed Begin)
   forces `byte_length == piece.expected_bytes() > 0`. Also verified
   empirically against brute force over 5,000 randomized interval sets.
   Equal-start duplicates are caught (`start_{i+1} = start_i < end_i`).
5. **Yes.** Writable open validates at `store.rs:91` before constructing
   the store; reader open validates at `store.rs:284`; both propagate
   `CatalogOverlap`/`CatalogOutOfBounds` before returning success, proven
   for both by the two negative regressions.
6. **Yes.** In `open`, `validate_catalog_extents` (line 91) precedes the
   short-tail `set_len` repair (lines 96-100); an invalid catalog therefore
   aborts open with the journal file untouched. The torn tail itself is
   excluded from decoding by `chunks_exact`, so validation operates on the
   exact record set that would survive the repair.
7. **Yes.** Empty journal → no events → empty catalog → no extents → no
   window check → `Ok`; `align_up(0) = 0`; a fresh store opens and
   publishes normally (exercised by every test's first open).
8. **Yes.** Trailing unreferenced bytes are never truncated (no
   `set_len` on `pages.dat` anywhere), and
   `next_data_offset = align_up(physical_len, 4096)` is at or beyond
   physical EOF; the sentinel regression proves both byte preservation and
   at-or-beyond allocation.
9. **Yes.** Extent-end overflow and allocation-offset overflow both
   surface `StoreError::Overflow` from `checked_add`/`align_up` before the
   journal repair and before any publish-path mutation; a publish-path
   overflow returns before `TierJournal::begin`, so it does not even
   poison the writer.
10. **Yes, distinguishing.** `startup_rejects_cross_page_extent_overlap`
    rewrites the second `Begin` record to alias the first page's
    target-KV interval, re-validates the forged record in-test
    (`second.validate().unwrap()` — per-record validity retained),
    re-encodes with fresh CRCs via the production encoder, and keeps all
    PieceDurable/Publish events. A per-record-only validator accepts this
    journal; the corrected open must fail with exactly `CatalogOverlap`
    for both writer and reader.
11. **Yes.** The one-byte truncation regression fails a lazy-bound
    implementation that only discovers the short file at restore; the
    8,192-byte sentinel regression fails the prior live-maximum allocator,
    which would place the second page at the old live end and overwrite
    the sentinel — the test asserts allocation at/beyond aligned physical
    EOF and byte-exact sentinel preservation.
12. **Yes.** Checksum (`data_corruption_fails_closed`), torn-tail
    (`torn_trailing_journal_record_is_ignored`), complete-corrupt-tail
    (`complete_corrupt_trailing_journal_record_is_never_ignored`),
    crash-orphan, poison, dedup, single-writer, and v1-journal tests all
    pass unchanged in the worktree (12/12 in `store::tests`), and the
    lifecycle fixture hash is pinned in the verified input set.
13. **Counts verified.** 54 tracked handoff documents at the candidate
    minus the 2 historical umbrella handoffs = 52, matching the proof's
    "52 existing review-handoff provenance proofs". `cargo test --offline
    --workspace` in the detached worktree passed exactly 257 tests with
    zero failures (58 cache + 7 cli + 11 cuda + 38 engine + 60 format +
    3 nvfp4-proof + 21 reference + 15 scheduler + 34 serving +
    10 tokenizer), matching the proof's 257 precisely; the
    tokenizer-dependent external proof is environment-gated exactly as the
    proof discloses. The CPU-only boundary and the exclusion list match
    the code: no payload authentication at startup, no obsolete-extent
    validation, no direct I/O, epochs, cleaning, async publication, CUDA,
    checkpoint, model, or performance content is present.

## Separate statements required by the handoff

- Reader and writer startup reject every overlapping live catalog: **YES**.
- Reader and writer startup reject every live out-of-bounds extent:
  **YES**.
- Invalid catalogs cause no short-tail repair or other mutation: **YES**
  (validation strictly precedes the only mutation in open).
- Resumed publication preserves every byte before prior physical EOF:
  **YES** (append-only, physical-EOF-aligned allocator; sentinel test).
- The three regressions distinguish the prior implementation: **YES**.
- The CPU proof and all exclusions are accurate: **YES**.

## Architecture & maintainability

- `validate_catalog_extents` is the right shape: a free function over the
  recovered map plus the physical length, shared verbatim by writer and
  reader opens — no duplicated policy between the two entry points.
- The sort/adjacent overlap check is O(n log n) at open, the only place it
  runs; nothing overlap-related sits on the publish hot path (publish adds
  extents strictly above all prior ones by construction).
- MINOR 1 (two overlap implementations) and MINOR 2 (whole-journal read)
  are the only simplification opportunities; both are acceptable within
  the retained store's declared role as a nonproduction control and
  fixture generator.
- The physical-EOF allocator comment ("Segment cleaning is the only future
  reclamation authority") correctly documents why space is never reused;
  it keeps the retained store's semantics a strict subset of the
  direct-tier contract's, which is what makes cross-reads (direct-tier
  proof case 25) meaningful.

## Token decision

All six required answers are unqualified YES; zero blockers, zero majors;
regressions verified distinguishing by construction; tests and clippy
reproduced green in the detached worktree; input hashes identical at start
and finish. The token accepts only this retained CPU catalog-extent
correction; it does not open cn4, authorize CUDA work, or accept the
production direct-tier implementation.

durable-catalog-extent-integrity-v1-accepted
