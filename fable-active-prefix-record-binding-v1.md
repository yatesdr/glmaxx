# Fable review: active-prefix record binding v1

Date: 2026-07-31

Reviewer: Fable (adversarial design-gate review, CPU only; no cn4, no CUDA)

Reviewed candidate commit:

92568f6045bf70a1d607435de318cebd6b4ef249

Note on result location: the operator directed review artifacts into
`docs/reviews/` instead of the repository root named by the handoff; this
file is the required result artifact at the operator-directed path.

## Provenance

The review ran in a detached worktree pinned at the candidate commit
(`git rev-parse HEAD` = the 40-hex word above, verified at review start and
again at review finish). Every pinned input was hashed with `shasum -a 256`
at review START and re-hashed at review FINISH; both hash sets matched the
handoff table exactly, with no drift between start and finish. The untracked
worktree copy of the handoff is byte-identical (SHA-256
`2912bf462384a92489c4e844ab827ec95864b2131fc9c02f612b36acfea65226`) to
`docs/fable-active-prefix-record-binding-v1-handoff.md` in the main tree.

Verified input hash table (all values observed in the worktree at start and
finish, matching the handoff verbatim):

| Input at candidate commit | SHA-256 (verified) |
|---|---|
| `spec/engine-v0.md` | efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a |
| `crates/glm-cache/src/sequence.rs` | e5902ffe36366916b728c54cd78f62331daf63136190d72cbc81d107e5150c36 |
| `crates/glm-cache/src/lib.rs` | 0d9d1fcdbb9c8350b1702d1c41263c24818861936d3ff37f4f4f73125cb6e269 |
| `crates/glm-cache/src/tier.rs` | 0a1541f13462bcdec92284911f96531b06869b60c7fe85fc5e9669c80fabe693 |
| `crates/glm-cache/src/prefix.rs` | ad0bc0e498050d948807c9f1e27e5f98ea02c4fa334725428a8dab1dab068298 |
| `crates/glm-cache/src/residency.rs` | 2846361e521f66752cb4455c908b2f30fa2f2a27a59a8059866e43b2402a2d6d |
| `crates/glm-serving/src/cache.rs` | 099bffde185307365f5932c84f14b15c1ccc4b4cfe29f00612265f69a46a9839 |
| `crates/glm-serving/src/lib.rs` | 8f4d33b6972bcee3a45f46416c3dfe2b4679a12b539704336c3f61f58fe73cb3 |
| `crates/glm-cli/src/cache_proof.rs` | f88effadfae758e8afda8ed1ffed9fb2c50530d4476200644b5b6ef905d7f814 |
| `docs/active-prefix-record-binding-proof-v1.md` | 9bb87c359d78c340d740ef9723ac78ef23510af5fabf4b29b1630211499b4c12 |
| `docs/serving-page-transaction-v1.md` | 05466da477fd9de88e9d8849cca67952b1f8999563743aea0599e741dc8e4c26 |
| `docs/offline-serving-spine.md` | 24230e2503b386391bd01274ae6586808c751202c619aa47aa81f9d8c277e8c7 |
| `docs/sequence-removal-atomicity-proof-v1.md` | 0baa3ff73b3fad73dd3471ee89fca9ab3278d5223fdae85c40f0e9066f11bc2b |
| `docs/prefix-residency-coherence-proof-v1.md` | 3f99eeb1f4f003f211922a906939ce9d6bbe03fb9b43ed13091fd38349bd194c |
| `docs/production-punchlist.md` | 2b38129a5b5179dfc1917975f691618b77c0720e16719b1289d80d476f525487 |
| `docs/results-index.md` | 9e66ebe429005252893761bc10f82d323ef55c6c29ea08b7a384d14a8ab46bf1 |
| `scripts/local-checks.sh` | 839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f |

## Gate results

All commands run from the pinned worktree, each suite once:

- `review-proof docs/fable-active-prefix-record-binding-v1-handoff.md`:
  verdict `PASS`, all 17 expected/actual hashes equal.
- `cargo test -p glm-cache sequence::tests::prefix_attachment_binds_generation_and_every_logical_piece_hash`:
  1 passed, 0 failed (62 filtered).
- `cargo test -p glm-serving cache::tests::prefix_registration_uses_the_monotonic_index_record_atomically`:
  1 passed, 0 failed (38 filtered).
- `cargo test -p glm-cli cache_proof::tests::cache_lifecycle_is_bounded_recoverable_and_fail_closed`:
  1 passed, 0 failed (6 filtered).
- `cargo clippy --workspace --all-targets --offline -- -D warnings`: clean.
- Additional verification for question 22: `cargo test --workspace --offline`
  = 270 tests passed, 0 failed across all suites;
  `review-proof-all` at the candidate: verdict `PASS`, 65 verified handoffs
  (64 tracked-then-present plus this review's untracked handoff copy),
  2 skipped historical.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. Clone-on-error snapshot cost is linear in whole-table size on every
   mutation. `SequencePageTable::admit_with_prefix`,
   `append_committed`, `begin_tentative`, `commit_tentative`,
   `rollback_tentative`, and `remove_sequence`
   (`crates/glm-cache/src/sequence.rs:196,297,386,439,451,461`) each do
   `self.clone()` of all sequences, physical pages, and free sets. A
   per-decode-token `begin_tentative`/`commit_tentative` cycle on a table
   with S sequences and P resident pages costs O(S+P) per token, so a full
   decode of T tokens is O(T*(S+P)); at DCP4 scale (4x4096 target pages,
   hundreds of sequences) this is quadratic aggregate work on the CPU
   metadata hot path. The handoff and proof explicitly exclude performance
   and list "fixed-capacity undo logs" as future work, so this is minor for
   this gate, but it must not survive into ServingCoordinator integration.
2. `TierJournal` scans its entire unbounded event vector per operation:
   `piece_durable` (`crates/glm-cache/src/tier.rs:252-261`) and
   `open_record` (`tier.rs:398-414`) are O(E) per call, giving O(E^2) over a
   store session (E = 3-4 events per published page). Durable-publish path
   only, not decode, so minor; flag for the punchlist.
3. `ResidencyManager::abort_restore` (`crates/glm-cache/src/residency.rs:407`)
   remains public and identity-free next to `abort_restore_identity`
   (`residency.rs:439`); given only a page key it clears another request's
   pending-restore identity. `PrefixRestoreCoordinator` internally uses only
   the identity-checked variants (`crates/glm-serving/src/cache.rs:496-501,
   536-540`), and the manager is coordinator-private, so this is API
   sharpness at the oracle layer, not a reachable production hole.

### QUESTION

1. Provenance observation for Sol: the proof doc
   (`docs/active-prefix-record-binding-proof-v1.md:141-142`) pins
   `docs/serving-page-transaction-v1.md` at
   e3a9a1d9f2eb26dc5312d7c42297fa3d832e444f7e3f269094746a85fb3deac2, which is
   that file's hash at the implementation commit
   6a5c574bf7a3d4060cb28ef78bc0425bd61f305a; the recording commit (the
   candidate) then updated the same doc to the handoff-pinned value. Both
   hashes verify at their respective commits, so this is consistent, but the
   pattern of a recording commit revising a doc that the recorded gate hashed
   is exactly the drift shape the provenance discipline watches for. Please
   confirm intent.
2. Fresh-page admission accepts any strictly valid attachment, including one
   whose generation is older than the index's retained record
   (`crates/glm-cache/src/sequence.rs:253-263`): after a shared prefix page's
   references drop to zero, `release_page` removes the `prefixes` entry
   (`sequence.rs:760-763`), and a later direct-oracle caller could re-admit a
   stale-generation attachment onto a fresh physical page. The production
   path is immune because `PrefixRestoreCoordinator` constructs attachments
   only from the monotonic index (`crates/glm-serving/src/cache.rs:220-222`),
   and payload upload is out of scope, but confirm the direct-oracle contract
   is intended to be "valid record" rather than "index-newest record".

## Answers to the 22 required adversarial questions

1. YES. At the pre-correction source (parent commit
   3365cb9), `admit_with_prefix` took
   `prefix_pages: &[(PrefixPageKey, bool)]` and each element destructured as
   `(key, has_draft)`; no namespace, generation, target hash, indexer hash,
   or draft hash existed anywhere in the active-table ABI (verified via
   `git show 6a5c574^:crates/glm-cache/src/sequence.rs`, lines 110-127).
2. YES. In that old source the existing-page branch validated only sealed
   state, full token count, owner rank, and `page.prefix == Some((key,
   ordinal))`, then allocated a draft slot whenever `mtp &&
   draft_local_page_id.is_none()` — any same-key `has_draft = true` claim,
   stale-generation or content-conflicting, was accepted and given a draft
   slot (old lines 134-152).
3. YES. All five `PrefixPageAttachment` fields are private
   (`crates/glm-cache/src/sequence.rs:23-30`), and the only constructor,
   `from_tier_record`, calls `record.validate()?` before reading any field
   (`sequence.rs:33-34`). No `Default`, no public field, no other
   construction path in-crate or exported.
4. YES. The constructor retains namespace, exact key, generation, the
   target-KV and target-indexer piece hashes, and the draft-sidecar hash
   exactly when `record.mtp` (`sequence.rs:35-45`, via `piece_hash` at
   `sequence.rs:820-827` which fails closed on a missing piece).
5. YES. Reuse requires `page.state == PageState::HbmSealed`,
   `valid_tokens == PAGE_TOKENS`, `physical.owner_rank ==
   owner_rank(ordinal)`, stored `current_ordinal == ordinal`, and a
   compatible relation (`sequence.rs:210-221`); duplicate keys within one
   admission are rejected by `seen.insert(key)` (`sequence.rs:205-207`).
6. YES. `relation_to` returns `SequencePageError::Prefix` on any mismatch of
   namespace, key, target-KV hash, or target-indexer hash
   (`sequence.rs:64-69`) before any draft-capability case is considered.
7. YES. `(None, Some(_))` maps to `DraftUpgrade` only when
   `candidate.generation > self.generation` (`sequence.rs:74-76`); the
   equal-or-older case falls into the rejection arm (`sequence.rs:78`).
   Target identity is already forced equal by the prelude.
8. YES. `(Some(_), None)` yields `AttachmentRelation::RetainDraft`
   (`sequence.rs:73`); the stored attachment is rewritten only on
   `DraftUpgrade` (`sequence.rs:229-234`), so a compatible target-only
   candidate reuses the page and the MTP attachment is retained, never
   downgraded.
9. YES. `(Some(current), Some(next))` is `Exact` only when `current == next`
   (`sequence.rs:77`); differing draft-sidecar hashes are rejected
   (`sequence.rs:78`).
10. YES. On `DraftUpgrade` the stored `(attachment, ordinal)` pair is
    replaced before the draft-slot and reference mutations
    (`sequence.rs:229-234`), and a draft slot is allocated only when the
    admitting sequence is MTP and `draft_local_page_id.is_none()`
    (`sequence.rs:235-243`).
11. YES. `admit_with_prefix` takes a full `self.clone()` snapshot
    (`sequence.rs:196`) and restores it wholesale on any error
    (`sequence.rs:282-285`), which exactly restores stored attachments,
    reference counts, physical records, the `prefixes` map, and both
    per-rank free sets. Verified behaviorally by the stats-equality
    assertions in the regression (`sequence.rs:1091-1099,1105-1109`).
12. YES. `begin_restore_longest_with_capability` builds each attachment via
    `self.index.record(key)` then `PrefixPageAttachment::from_tier_record`
    (`crates/glm-serving/src/cache.rs:220-222`) — the authoritative
    post-registration index record. `register_prefix` never lets a
    discarded/downgraded caller record reach the index or ranks: it inserts
    into a candidate index (dedup/retain/upgrade semantics in
    `crates/glm-cache/src/prefix.rs:138-163`), verifies rank/index
    coherence (`cache.rs:139-141`), and pushes only genuinely-changed
    records to rank registration (`cache.rs:143-149`).
13. YES. A page in `PendingPageState::Restoring` becomes part of a Ready
    result only after `manager.complete_restore(result)` — which enforces
    the pending request/ordinal identity and `entry.record ==
    result.page.record` exactly
    (`crates/glm-cache/src/residency.rs:455-467`) — followed by
    `manager.pin_hbm` (`cache.rs:308-317`); Ready is emitted only when
    every page is `Pinned` (`cache.rs:325-334`).
14. YES. For an already-resident page the attachment still comes from the
    same `self.index.record(key)` call (`cache.rs:220-222`), and the rank's
    stored record is coherent with the index by the `register_prefix`
    invariant (`cache.rs:139-141`, asserted end-to-end at
    `cache.rs:704-705,853-854`); pinning (`cache.rs:225-240`) binds the
    returned attachment to that record.
15. YES. `RestoredPrefix.page_keys` and `.page_attachments` are private
    (`cache.rs:10-14`), so external construction of a forged ready result is
    impossible despite the public `matched_tokens` field; read-only
    inspection is provided by `page_keys()`/`page_attachments()`
    (`cache.rs:44-52`), and `empty()` is `pub(crate)` (`cache.rs:17`).
16. YES. `admit_prevalidated` is now `pub(crate)`
    (`crates/glm-serving/src/lib.rs:267`); at the parent commit it was
    `pub`. External production callers must go through
    `admit_tokens`/`begin_admit_tokens`, which derive the cached token
    count from the coordinator's own restore
    (`lib.rs:289-351,396-445`).
17. YES. `prefix_attachment_binds_generation_and_every_logical_piece_hash`
    (`sequence.rs:1076-1110`) rejects the same-generation draft claim and
    the changed-target record with `stats()` restored exactly to `before`
    (atomicity), accepts the single generation-5 MTP upgrade on the same
    physical target page (capacity is 1 target page per rank, so success
    proves reuse; draft usage becomes `[1,0,0,0]`), then rejects the
    changed-draft record with stats unchanged from the upgraded state.
18. YES. The prior ABI `&[(PrefixPageKey, bool)]` has no generation or piece
    hashes, so the strict regression's `attachment(key, generation,
    target_marker, draft_marker)` constructions cannot be expressed — it
    cannot compile. Translating all four candidates to the only old
    representation `(same_key, true)` hits the old existing-page branch,
    which accepted any such claim and allocated a draft slot (old lines
    134-152); the stale upgrade and both content conflicts would be
    accepted. Both halves of the claim are accurate.
19. YES. `prefix_registration_uses_the_monotonic_index_record_atomically`
    (`cache.rs:630-743`) registers the target twice (dedup), the
    generation-2 MTP upgrade, and a generation-3 target-only downgrade
    attempt, then runs a real draft-required async restore and asserts
    `restored.page_attachments() ==
    [PrefixPageAttachment::from_tier_record(&upgrade).unwrap()]`
    (`cache.rs:731-734`) — exact equality against the retained MTP
    generation-two record.
20. YES. `prove_page_table_lifecycle` now takes the actual torn-journal
    recovered `&[TierRecord]` and maps them through
    `PrefixPageAttachment::from_tier_record`
    (`crates/glm-cli/src/cache_proof.rs:325-336`), called with `&recovered`
    (`cache_proof.rs:207`). The prior source passed
    `keys.iter().map(|key| (key, true))` — hard-coded booleans (verified at
    `6a5c574^`).
21. YES. `docs/active-prefix-record-binding-proof-v1.md:69-73` states
    explicitly that a standalone attachment is not a payload-transfer
    receipt and that a direct oracle user can construct one without proving
    CUDA-visible bytes were uploaded; the future serving-page transaction
    must bind the transfer acknowledgment.
22. YES. Independently recounted: `cargo test --workspace --offline` at the
    candidate = 270 passed, 0 failed (63+7+11+40+60+3+22+15+39+10). The
    "64 then-present" handoff claim reconciles with `review-proof-all` at
    the candidate: 66 tracked handoff files minus 2 skipped historical = 64
    verified then; with this review's untracked handoff present the suite
    verifies 65 and still passes. The exclusions
    (no `SequencePageTable` in `ServingCoordinator` — confirmed by grep, no
    CUDA/GPU/model/perf/live-concurrency claims) are accurate; cn4 was not
    contacted and no CUDA was launched.

## Eight required summary statements

1. The old capability-forging boundary is real: YES — the parent-commit ABI
   accepted `(same_key, true)` and allocated MTP draft slots with no
   identity binding.
2. The attachment binds every required logical record identity: YES —
   namespace, key, generation, target-KV hash, target-indexer hash, and
   optional draft-sidecar hash, derived only after strict validation.
3. Exact reuse, retain-MTP, newer-MTP upgrade, and collision rejection are
   correct and atomic: YES — the relation matrix at `sequence.rs:63-80` is
   fail-closed and the snapshot-rollback restores the exact table on any
   late failure.
4. Restore and active-table metadata consume the same authoritative record:
   YES — coordinator attachments come only from the monotonic prefix index,
   with rank/index coherence checked at registration and exact-record
   validation at restore completion.
5. External serving code cannot forge a restored-prefix result or use the
   prevalidated bypass: YES — private vectors block construction and
   `admit_prevalidated` is crate-private.
6. The regression distinguishes the missing old ABI and resulting old
   acceptance behavior: YES — the strict cases cannot compile against the
   old ABI, and their only old translation was accepted by it.
7. Payload-transfer and serving-integration scope remain accurately
   excluded: YES — stated in the proof and confirmed absent from the code.
8. The CPU proof and all gate counts are accurate: YES — 270/270 tests,
   clippy clean, review-proof PASS, review-proof-all PASS, and the
   cache-lifecycle proof derives its capability facts from real recovered
   records.

## Architecture & maintainability

The correction is well-shaped: capability is now a value type derivable only
from a validated durable record, the relation matrix is a single small total
function, and the coordinator's plan/commit split (validate-everything, then
infallible commit with `expect` on preflighted state) gives clean two-phase
semantics across ranks. The three-layer agreement (monotonic `PrefixIndex`,
per-rank `ResidencyManager` records, per-page `PrefixPageAttachment`) is
enforced at every writer, so a disagreement is a hard invariant error rather
than silent drift. Main debts, all acknowledged in the exclusions: the
clone-on-error snapshots (MINOR 1) will not scale to per-token mutation and
must become bounded undo logs before ServingCoordinator integration;
`TierJournal`'s linear-scan-per-event design (MINOR 2) should gain an index
if store sessions grow; and the identity-free `abort_restore` (MINOR 3)
should eventually be demoted or removed in favor of the identity-checked
variant. Test quality is high — regressions assert exact state equality
before/after rejected operations, and the proof booleans are derived, not
declared.

## Token decision

All four listed gate commands pass at the pinned candidate, all 22 answers
are unqualified YES, all eight summary statements are unqualified YES, and
there are no BLOCKER or MAJOR findings. Input hashes were verified at both
review start and review finish with no drift. The acceptance below covers
only the retained CPU prefix-record binding correction; it conveys no cn4
access, no CUDA authorization, and no checkpoint-serving acceptance.

active-prefix-record-binding-v1-accepted
