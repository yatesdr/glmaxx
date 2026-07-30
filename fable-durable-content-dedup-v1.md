# Fable review: durable content deduplication v1

Date: 2026-07-30

Reviewer: Fable (adversarial CPU implementation review)

Handoff: `docs/fable-durable-content-dedup-v1-handoff.md` (queue row 18)

Reviewed candidate commit:
`b097703b0a6def10d3732ae70835881c93a954dd`

Implementation commit named by the proof
(`85d950ee45294f2551d674736b35781986dda874`) verified as an ancestor of the
candidate with an identical `store.rs` hash; its parent was inspected to
confirm the prior implementation genuinely lacked the relation preflight
(`relation_to` absent from the prior `store.rs`; prior `prefix.rs` used
`records_are_logically_compatible` with generation-replacement).

Location note: the handoff declares the result path at the repository root;
the operator directed all review artifacts into `docs/reviews/`, so this
file may need moving to the declared path on acceptance.

## Provenance

All 12 input hashes were verified with `git show <commit>:<path> |
shasum -a 256` at review start and re-verified at review finish; both sets
matched the handoff table exactly at the pinned candidate. `main` has
drifted on `lib.rs`, `store.rs`, `residency.rs`, and serving `cache.rs`, so
the review ran in a detached worktree at the pinned commit. The handoff
file itself postdates the candidate, so `review-proof` was replaced by
direct hash verification of every table row.

| Input | SHA-256 (start = finish) |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/lib.rs` | `0c287e5a542c242e18a3d20c25d8ef8e61bba69ce04c854e714915d915aadab0` |
| `crates/glm-cache/src/tier.rs` | `0a1541f13462bcdec92284911f96531b06869b60c7fe85fc5e9669c80fabe693` |
| `crates/glm-cache/src/store.rs` | `fd16e7e795ce742aff0b72125988b019b3f36cbfebd1f67dab2dd9ea8d72c5ad` |
| `crates/glm-cache/src/prefix.rs` | `ad0bc0e498050d948807c9f1e27e5f98ea02c4fa334725428a8dab1dab068298` |
| `crates/glm-cache/src/residency.rs` | `04ffe885557b81ca91797b84f31bf6ae3f6f35bc4b7a5dae6bdc9ab08983e664` |
| `crates/glm-serving/src/cache.rs` | `3026b4d3353839c0a644944e8a6103f2b168e741d25d272ea2d7d330e1610635` |
| `docs/online-prefix-publication-v1.md` | `67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873` |
| `docs/prefix-generation-integrity-proof-v1.md` | `4db63b0ddde70e2afe6371fd4b609bd57ad4965bb48cd45c6dfc5d06587473a0` |
| `docs/prefix-residency-coherence-proof-v1.md` | `3f99eeb1f4f003f211922a906939ce9d6bbe03fb9b43ed13091fd38349bd194c` |
| `docs/durable-content-dedup-proof-v1.md` | `75fd16886ef50e4509fb0a7b0701417a1469a0dad78809b39ce08e3e736a7514` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Commands run in the worktree:

- `cargo test --offline -p glm-cache` — 54 passed, 0 failed (includes
  `durable_content_dedup_upgrade_and_collision_are_preflighted`,
  `recovery_applies_dedup_upgrade_and_collision_matrix`,
  `same_key_generations_require_identical_bytes_and_never_downgrade_mtp`).
- `cargo test --offline -p glm-serving --lib cache::tests` — 5 passed,
  0 failed (includes `multi_rank_mtp_upgrade_is_atomic_on_a_late_pinned_rank`
  and `prefix_registration_uses_the_monotonic_index_record_atomically`).
- `cargo clippy --offline -p glm-cache -p glm-serving --all-targets --
  -D warnings` — clean.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. **Per-registration full-index clone.**
   `PrefixRestoreCoordinator::register_prefix`
   (`crates/glm-serving/src/cache.rs:105`) clones the entire `PrefixIndex`
   to obtain a candidate for atomic swap. Each registration is O(index
   pages); registering n prefixes into a growing index is O(n²) aggregate.
   At the contract's 16,384-page scale that is ~134M cloned page entries
   over a full fill. It buys real atomicity (plan-all-then-commit across
   four ranks plus index swap) and this coordinator is retained CPU
   scaffolding with performance explicitly excluded, so it is not
   blocking — but the pending production coordinator must not inherit
   clone-per-registration (an undo log or persistent map achieves the same
   atomicity in O(changed pages)).
2. **Split reference-overflow handling in `PrefixIndex::insert`.** The
   validation pass checks `existing.references.checked_add(1)` and
   discards the result; the apply pass then does `existing.references += 1`
   unchecked (`prefix.rs:156-174`). Correct today because the two passes
   see identical state within one call, but the checked/unchecked split is
   a refactoring hazard — the apply pass should reuse the checked value.
3. **Store-side allocator predates the catalog-extent correction.** This
   candidate's `FileTierStore::open` still derives `next_data_offset` from
   the live-catalog maximum, later corrected by the catalog-extent
   candidate (`de2d43a4`, reviewed separately). Out of this handoff's
   boundary (registration/dedup semantics only) and already fixed
   downstream; noted so the acceptance is not read as covering the
   allocator.
4. **Recovery fails the whole open on a non-newer MTP upgrade**
   (`tier.rs` recover: `record.generation <= existing.generation` →
   `TierError::Journal`). This is the correct fail-closed posture for a
   journal the corrected writer can never produce (the writer preflights
   `StaleGeneration`), but it means one forged/legacy record makes the
   entire store unopenable rather than quarantining the record — worth a
   sentence in the proof; deliberate per the strict-replay philosophy.

### QUESTION

1. `relation_to` compares `(byte_length, sha256)` per required piece.
   Since `validate` pins `byte_length == expected_bytes` for every piece,
   the length component is constant and the identity is effectively the
   SHA-256 pair — confirmed intentional (keeps identity total if piece
   sizes ever become variable)?

## Answers to the handoff's 16 questions

1. **Yes.** The prior `store.rs` (parent of `85d950ee`) contains no
   `relation_to` and no logical-hash comparison; any candidate with a
   larger generation for an existing page key appended a full new record.
   Confirmed by direct inspection of the prior source.
2. **Yes.** The prior `prefix.rs` replaced the stored record whenever a
   logically compatible candidate carried a larger generation, including
   exact same-content records — contradicting the revision-retention rule.
3. **Yes.** Prior replay selected the largest generation with no collision
   or MTP-capability enforcement (per the prior `recover`; the current
   regression `recovery_applies_dedup_upgrade_and_collision_matrix` fails
   against it).
4. **Yes.** `TierRecord::relation_to` (`tier.rs:117`) first runs complete
   `validate()` on both records, then requires exact namespace and page
   key, then compares `logical_piece_identity` — `(byte_length, sha256)` —
   for TargetKv and TargetIndexer always, and for DraftSidecar in the
   MTP/MTP cell. `tier` and `storage_offset` never participate in
   identity.
5. **Yes, all seven cells.** (none, candidate) → append (no existing
   record path); (target, same target) → `ExactDedup`; (target, same +
   draft) → `MtpUpgrade`; (target, different target) → `Collision`;
   (MTP, same target-only) → `RetainMtp`; (MTP, all-same MTP) →
   `ExactDedup`; (MTP, different target or draft) → `Collision`. MTP
   downgrade is unrepresentable: the (true, false) arm returns
   `RetainMtp`, never a replacement.
6. **Yes.** `MtpUpgrade` is the only relation for which the store
   (`store.rs:195-199`), the prefix index (`prefix.rs:146-160`, `replace`
   flag), and replay (`tier.rs` recover) will substitute the stored
   record, and all three independently require
   `candidate.generation > existing.generation` (store:
   `StaleGeneration`; prefix: `Collision`; replay: journal failure).
7. **Yes.** `ExactDedup` and `RetainMtp` return the existing record
   (store), keep `replace == false` (prefix), and skip re-insertion
   (replay) regardless of the candidate's generation — proven by the
   generation-9 dedup and generation-3 target-only-after-MTP steps of the
   preflight regression, both byte-exact on journal and data lengths.
8. **Yes.** In `publish_inner` the relation classification (line 187)
   happens strictly before `publish_prevalidated`, which contains the
   first `TierJournal::begin`, journal append, data write, transaction
   advance, and catalog insert. Piece hashing for classification uses the
   request bytes only.
9. **Yes.** Exact dedups return before any mutation; the regression
   asserts journal length, data length, and the published record are
   unchanged after both a same-content larger generation and post-upgrade
   retained candidates.
10. **Yes.** Target and draft collisions surface `ContentCollision` from
    the preflight, before `begin`; `write_poisoned` is set only on
    `publish_prevalidated` failure, so a collision does not poison later
    writes — the regression publishes a different page successfully after
    two collisions and confirms file lengths unchanged.
11. **Yes.** Post-mutation failures (fail-points after Begin) still set
    `write_poisoned`, and every later publish returns `WritePoisoned`
    until close/reopen replay — retained behavior proven by the existing
    poison regression, unchanged.
12. **Yes.** `TierJournal::recover` applies the same `relation_to` in
    transaction order: exact dedup/`RetainMtp` records are retained
    without replacement, an MTP upgrade is adopted only when strictly
    newer (else the open fails), and a fully durable collision fails
    recovery (`relation_to` error propagates). The regression builds the
    journal by hand and proves selection of only the upgrade, then
    rejection of a durable conflicting draft record.
13. **Yes.** `same_key_generations_require_identical_bytes_and_never_
    downgrade_mtp` rejects a same-revision MTP upgrade atomically
    (multi-page insert leaves no partial mutation) and proves an exact
    same-content larger-generation MTP candidate retains the prior durable
    revision.
14. **Yes.** `multi_rank_mtp_upgrade_is_atomic_on_a_late_pinned_rank`
    restores both target pages to HBM on their owner ranks, pins rank 1,
    attempts the two-page upgrade, asserts `ResidencyError::Pinned`, and
    then asserts — for every rank — the index record, the rank record,
    and HBM residency all still equal the original targets. After unpin,
    the retry upgrades both ranks and completes a real draft-required
    asynchronous file restore of both pages. The plan-all/commit-all
    structure of `register_prefix` (all four `plan_nvme_registrations`
    must succeed before any `commit_nvme_registrations` or the index swap)
    is what the test pins down.
15. **Yes.** The proof states explicitly that the rule "supersedes the
    earlier prefix-generation candidate's claim that a same-content larger
    MTP record may refresh physical placement", names that behavior
    inconsistent with `online-prefix-publication-v1`, and confines the
    historical proof to its pinned candidate.
16. **Yes.** 51 tracked handoff documents at the candidate minus the 2
    historical umbrella handoffs = 49, matching "49 then-present review
    handoff provenance proofs". `cargo test --offline --workspace` in the
    detached worktree passed exactly 253 tests with zero failures
    (54 cache + 7 cli + 11 cuda + 38 engine + 60 format + 3 nvfp4-proof +
    21 reference + 15 scheduler + 34 serving + 10 tokenizer), matching the
    proof's 253 precisely.
    The boundary and exclusion list match the code: the candidate contains
    no online publication service, no parent/ordinal recovery catalog, no
    direct I/O, no shared live catalog, no real DRAM/HBM movement, and no
    CUDA/checkpoint/model/performance content; content collision is a
    typed error left for the production coordinator to classify as
    engine-fatal, exactly as disclosed.

## Separate statements required by the handoff

- All three retained layers implement one exact logical-content relation:
  **YES** (single `TierRecord::relation_to`, called from store preflight,
  prefix insert, and journal replay).
- Exact dedup performs no durable write and retains the revision: **YES**.
- MTP upgrade is the only allowed same-key replacement: **YES**, strictly
  newer revision required in all three layers.
- Collisions fail before mutation and replay refuses preexisting
  collisions: **YES**.
- The regressions distinguish the prior store, replay, prefix, and
  cross-rank behaviors: **YES** (prior implementations inspected directly;
  each regression targets a behavior the prior code exhibits).
- The CPU proof and all exclusions are accurate: **YES**.

## Architecture & maintainability

- Centralizing the seven-cell matrix in one validated `relation_to` and
  reusing it verbatim in three layers is the right structure; the layers
  can no longer drift, and the matrix in the proof reads directly off the
  match arms.
- The plan/commit split in `register_prefix` and in
  `ResidencyManager::plan_nvme_registrations`/`commit_nvme_registrations`
  gives clean cross-rank atomicity; its cost is MINOR 1's full-index
  clone, which the production coordinator must replace.
- Error-type mapping is slightly lossy: the prefix layer folds every
  relation failure (including a stale upgrade) into `PrefixError::
  Collision`, while the store distinguishes `StaleGeneration` from
  `ContentCollision`. Consistent typed errors across layers would make
  the pending engine-fatal classification simpler.
- `logical_piece_identity` returning `Option<(u64, [u8;32])>` and
  comparing options is a neat way to make "piece absent" a first-class
  identity value; the (true, true) draft comparison relies on it.

## Token decision

All six required answers are unqualified YES; zero blockers, zero majors;
regressions verified distinguishing against the directly inspected prior
implementation; tests and clippy reproduced green in the detached
worktree; input hashes identical at start and finish. The token accepts
only this retained CPU content-matrix correction; it does not open cn4,
authorize CUDA work, or accept online publication or model execution.

durable-content-dedup-v1-accepted
