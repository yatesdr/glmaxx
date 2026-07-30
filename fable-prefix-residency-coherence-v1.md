# Fable adversarial review: prefix/residency generation coherence v1

Reviewer: Fable (adversarial CPU implementation review)
Review window: 2026-07-29 to 2026-07-30
Reviewed candidate commit: `72e60716cf58632dd9aba5ead41ba0d128f59395`
Reviewed in a detached worktree pinned at the candidate commit. No cn4
connection, no CUDA, no GPU work.

Handoff: `docs/fable-prefix-residency-coherence-v1-handoff.md`
(SHA-256 `53f45145a17e46e0e8e9c5cf99a298db2eb44e07925575b8117676ff6629455c`).

The operator directed review artifacts into docs/reviews/; the handoff
declares the repository root; this file may need moving on acceptance.

## Provenance

### Start hashes (review start, worktree pinned at candidate commit)

| Input | SHA-256 | Handoff match |
|---|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` | yes |
| `crates/glm-cache/src/lib.rs` | `d7727125c2b022b2cd1da7e51b07b1e06365da3ed530b2735478b3ac40f67b06` | yes |
| `crates/glm-cache/src/prefix.rs` | `7b4aff1407f83b2e12216d7a051049c1a5359f0bae7fb88724e8999077260f70` | yes |
| `crates/glm-cache/src/residency.rs` | `04ffe885557b81ca91797b84f31bf6ae3f6f35bc4b7a5dae6bdc9ab08983e664` | yes |
| `crates/glm-serving/src/cache.rs` | `709ab616feca96818f6fc6ce1331becd93de9f67324d2b278503f6f2ad3efe1f` | yes |
| `crates/glm-cache/src/tier.rs` | `c31b07d7f9054f3d51bc5d24c2c414b6c9a134d88f042502bc0f82e29cad500f` | yes |
| `docs/online-prefix-publication-v1.md` | `67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873` | yes |
| `docs/prefix-generation-integrity-proof-v1.md` | `4db63b0ddde70e2afe6371fd4b609bd57ad4965bb48cd45c6dfc5d06587473a0` | yes |
| `docs/prefix-residency-coherence-proof-v1.md` | `3f99eeb1f4f003f211922a906939ce9d6bbe03fb9b43ed13091fd38349bd194c` | yes |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` | yes |

### Finish hashes (after temp-file removal, `git status --porcelain` clean, HEAD still `72e6071`)

| Input | SHA-256 | Start match |
|---|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` | yes |
| `crates/glm-cache/src/lib.rs` | `d7727125c2b022b2cd1da7e51b07b1e06365da3ed530b2735478b3ac40f67b06` | yes |
| `crates/glm-cache/src/prefix.rs` | `7b4aff1407f83b2e12216d7a051049c1a5359f0bae7fb88724e8999077260f70` | yes |
| `crates/glm-cache/src/residency.rs` | `04ffe885557b81ca91797b84f31bf6ae3f6f35bc4b7a5dae6bdc9ab08983e664` | yes |
| `crates/glm-serving/src/cache.rs` | `709ab616feca96818f6fc6ce1331becd93de9f67324d2b278503f6f2ad3efe1f` | yes |
| `crates/glm-cache/src/tier.rs` | `c31b07d7f9054f3d51bc5d24c2c414b6c9a134d88f042502bc0f82e29cad500f` | yes |
| `docs/online-prefix-publication-v1.md` | `67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873` | yes |
| `docs/prefix-generation-integrity-proof-v1.md` | `4db63b0ddde70e2afe6371fd4b609bd57ad4965bb48cd45c6dfc5d06587473a0` | yes |
| `docs/prefix-residency-coherence-proof-v1.md` | `3f99eeb1f4f003f211922a906939ce9d6bbe03fb9b43ed13091fd38349bd194c` | yes |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` | yes |

## Command outcomes

All commands ran offline in the pinned worktree.

1. `cargo run --offline -p glm-cli --bin glmaxx -- review-proof docs/fable-prefix-residency-coherence-v1-handoff.md`
   — `"verdict": "PASS"`; `repository_head` = `candidate_commit` =
   `72e60716cf58632dd9aba5ead41ba0d128f59395`; all ten inputs matched.
   Note: the tool rejects a handoff path outside the repository and the
   handoff does not exist at the candidate commit, so the handoff was
   copied verbatim from the operator tree into the worktree `docs/` for
   the run and removed afterward (it is not in its own provenance table;
   `git status --porcelain` was clean at finish).
2. `cargo test --offline -p glm-serving --lib prefix_registration_uses_the_monotonic_index_record_atomically`
   — ok. 1 passed; 0 failed (32 filtered out).
3. `cargo test --offline -p glm-cache` — ok. 52 passed; 0 failed; doc-tests
   0.
4. `cargo clippy --offline -p glm-cache -p glm-serving --all-targets -- -D warnings`
   — clean, `Finished dev profile`.

Additional verification beyond the required set:
`cargo test --workspace --offline` — 250 passed, 0 failed across all
workspace targets (52+7+11+38+60+3+21+15+33+10), matching the proof's
250-test claim and the 250 `#[test]` functions counted at the candidate
commit.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

- M1. `register_prefix` clones the entire `PrefixIndex` on every call
  (`crates/glm-serving/src/cache.rs:105`). Registration is O(index size)
  per call, so N registrations of distinct prefixes cost Theta(N^2) total
  clone work, and the index only grows (no page removal API in
  `crates/glm-cache/src/prefix.rs`). Triggering input: any long-lived
  coordinator registering many distinct prefixes — e.g. 100k unique
  one-page prompts implies roughly 5e9 cumulative `PrefixPage` clones.
  Measured probe: 1,000 one-page registrations took 3.12 s, 4,000 took
  36.9 s — 4x the registrations cost 11.8x the time (quadratic trend, not
  linear). The clone copies only `TierRecord` metadata (roughly 200
  bytes/page), never page payloads, and the transient allocation is
  dropped or adopted, so there is no unbounded payload clone and no leak.
  This is a state-complexity/performance concern, and performance is
  excluded from the acceptance boundary; it should still be fixed before
  online publication (a delta-undo log or preflight-on-live-index removes
  the clone).
- M2. `PrefixRestoreError::Record` is overloaded across at least five
  distinct failure meanings: nonempty-index constructor rejection
  (`crates/glm-serving/src/cache.rs:81`), key-count mismatch
  (`cache.rs:109`), index/residency coherence mismatch (`cache.rs:117`),
  missing candidate record (`cache.rs:121`), and duplicate rollback
  restore (`cache.rs:465`). A coherence violation — the condition this
  correction exists to catch — is indistinguishable from a shape error at
  the call site.
- M3. `restore_longest` (`crates/glm-serving/src/cache.rs:147-155`) spins
  forever with `park_timeout(1ms)` and no deadline if a restore never
  completes; bounded-deadline discipline exists only in tests. Not
  fail-open (it never yields wrong data), but an availability hazard on a
  wedged restore worker.
- M4. The rank count 4 is hard-coded independently in
  `PrefixRestoreCoordinator::new` (`crates/glm-serving/src/cache.rs:85`),
  `owner_rank` (`crates/glm-cache/src/page.rs:53-55`),
  `RestoreService::try_submit` (`crates/glm-cache/src/residency.rs:129`),
  and `ResidencyManager::begin_restore` (`residency.rs:351`). A drifting
  constant would silently break deterministic ownership; a single shared
  constant would remove the risk.
- M5. `PrefixIndex::insert` splits the reference-overflow preflight
  (`crates/glm-cache/src/prefix.rs:144-147`, `checked_add` result
  discarded) from the unchecked increment (`prefix.rs:161`,
  `existing.references += 1`). Correct today because the first loop
  preflights each key at most once per call (duplicate keys already
  rejected via `pending_keys`), but the invariant linking the two loops is
  implicit and uncommented.

### QUESTION

- Q-A. Durable-store drift: `FileTierStore` retains only the highest
  published generation per key (`crates/glm-cache/src/store.rs:143-150`;
  journal `recover` keeps max generation,
  `crates/glm-cache/src/tier.rs:328-337`). If a newer byte-compatible
  target-only generation is durably published for a key the coordinator
  retains as MTP gen-2, a draft-required restore submits with
  `minimum_generation = 2`, the worker returns the gen-3 target-only page,
  and `complete_restore` fails exact-record equality
  (`crates/glm-cache/src/residency.rs:428-437`) with `Stale`. Fail-closed
  — capability is never falsely satisfied — but the prefix becomes
  draft-unrestorable from NVMe while the index still advertises MTP. This
  sits at the durable-store boundary the handoff excludes; flagged so the
  online-publication design accounts for it.
- Q-B. The shipped collision-atomicity assertion checks rank 0 and one key
  only (`crates/glm-serving/src/cache.rs:692-694`) — sufficient for a
  single-page registration, and the multi-rank pinned-failure case is
  covered in `multi_page_restore_is_submitted_without_blocking_admission`
  (`cache.rs:866-875`) — but no shipped test exercises a later-rank plan
  failure while earlier ranks have genuine pending changes. My probe 2
  covered an equivalent all-rank case and passed; a shipped test with
  asymmetric per-rank change sets would close this permanently.

## Answers to the 15 handoff questions

1. **No.** `PrefixIndex::insert` retains the MTP record for a
   byte-compatible newer target-only input
   (`crates/glm-cache/src/prefix.rs:156-160`, guard
   `record.generation > existing.record.generation && (!existing.record.mtp || record.mtp)`),
   so the candidate record equals the prior record and `register_prefix`
   produces no residency update for that key
   (`crates/glm-serving/src/cache.rs:119-124`, `if prior != Some(next)`).
   The caller's input record is never sent to residency; only
   `candidate_index.record(key)` is. Verified by the shipped regression
   (`cache.rs:676-683`) and probe 1.
2. **No.** With coherence held, the residency record equals the index's
   MTP record, and `complete_restore` requires exact record equality
   (`crates/glm-cache/src/residency.rs:428-437`), so a draft-required
   lookup only advertises what the identical residency record can
   validate. Divergence injected out of band is caught fail-closed at the
   next registration (`cache.rs:115-118`) or surfaces as a `Stale` restore
   error — never as false capability. (See Q-A for the excluded
   durable-store drift case, which also fails closed.)
3. **Yes — that was the prior defect, now removed.** The pre-correction
   coordinator (`git show a3f5957~1:crates/glm-serving/src/cache.rs`)
   inserted into a candidate index (counting the reference on adoption)
   and validated/registered the caller's records against the ranks;
   `validate_nvme_registration` rejects `generation >= existing` as
   `Stale` (`residency.rs:330-332`), so exact/lower-generation dedup
   failed. The corrected path performs dedup with no residency write and
   no failure: probe 1's second registration of the identical gen-1 record
   succeeded.
4. **Yes.** `cache.rs:105-107`: `candidate_index = self.index.clone()`
   then `candidate_index.insert(...)`. Insert errors propagate before any
   live index or rank mutation; the live index is assigned only at
   `cache.rs:134` after all four plans commit.
5. **Yes.** For every derived key, `cache.rs:113-118` requires
   `self.ranks[rank].record(key.0) == self.index.record(key)` — both
   `None` for a new key, exact `TierRecord` equality otherwise — and
   returns `Err(Record)` on mismatch before any plan is built.
6. **Yes.** The only records pushed into rank update sets are
   `candidate_index.record(key)` clones (`cache.rs:119-124`). The caller's
   input vector is consumed by `candidate_index.insert` and never reaches
   residency.
7. **Yes.** When `prior == Some(next)` no update is pushed
   (`cache.rs:122`), so `commit_nvme_registrations` receives nothing for
   that key; the reference increment happened inside the candidate index
   (`prefix.rs:161`), which is adopted at `cache.rs:134`. The shipped
   regression asserts references 4 with unchanged index and residency
   records (`cache.rs:681-683`).
8. **Yes.** Changes are grouped by `owner_rank(ordinal)` = `ordinal % 4`
   (`crates/glm-cache/src/page.rs:53-55`) at `cache.rs:112-114`. A page
   key encodes its parent chain (`prefix.rs:59-70`), so a given key always
   occurs at the same chain depth, hence the same ordinal and the same
   deterministic rank across registrations.
9. **Yes.** `plan_nvme_registrations`
   (`crates/glm-cache/src/residency.rs:269-304`) rejects duplicate page
   keys (`BTreeSet`, lines 276-279), and per record
   `validate_nvme_registration` (`residency.rs:324-342`) rejects invalid
   shape/tier, stale-or-equal generation, pinned entries, and `Restoring`
   entries; accounting uses `checked_sub`/`checked_add`
   (`residency.rs:283-296`, `634-640`) so underflow fails the plan. All
   before any commit.
10. **Yes.** `cache.rs:126-130` builds all four plans via
    `collect::<Result<Vec<_>, _>>()` — the first error aborts with zero
    commits. Only then does `cache.rs:131-133` run the infallible
    `commit_nvme_registrations` on every rank, followed by candidate-index
    adoption at `cache.rs:134`.
11. **Yes.** Updates clone `TierRecord` only (`cache.rs:123`);
    `NvmeRegistrationPlan` holds `Vec<TierRecord>`
    (`residency.rs:243-247`); `commit_nvme_registrations` installs entries
    with `restored: None` (`residency.rs:306-322`). No `RestoredPage`
    payload is cloned anywhere in the registration path. (The whole-index
    metadata clone is finding M1; it too contains no payloads.)
12. **Yes.**
    `prefix_registration_uses_the_monotonic_index_record_atomically`
    (`crates/glm-serving/src/cache.rs:608-718`) covers: exact dedup (two
    identical gen-1 registrations, lines 666-671), target-to-MTP upgrade
    (gen-2 MTP, lines 672-674), MTP-preserving target-only dedup (gen-3
    target-only retained as gen-2 MTP in both index and rank-0 residency,
    references 4, lines 676-683), target collision atomicity (conflicting
    digest rejected with index, references, and residency unchanged, lines
    685-694), and a real draft-required restore from the on-disk
    `FileTierStore` returning `page_has_draft == [true]` (lines 696-709),
    plus nonempty-constructor rejection (lines 712-715).
13. **Yes.** The pre-correction `register_prefix` (shown at `a3f5957~1`)
    validates the caller's records against the ranks, so the regression's
    step-2 repeat gen-1 registration fails `validate_nvme_registration` as
    `Stale` (`residency.rs:330-332`). Had exact dedup been omitted from
    the regression, the old code would install the caller's target-only
    gen-3 input into residency while the index retained MTP gen-2, and the
    later restore of the durable gen-2 page would fail `complete_restore`
    exact-record validation (`residency.rs:428-437`). Either way the
    regression distinguishes the prior coordinator.
14. **Yes.** `cache.rs:80-82` rejects a nonempty initial index with
    `Err(Record)`. Ranks always start empty, so accepting a populated
    index would violate the per-key equality invariant on the first
    registration touching an indexed key (residency `None` vs index
    `Some`) — an immediately divergent coordinator. Probe 3 and the
    shipped regression (`cache.rs:712-715`) both confirm rejection; the
    proof's rationale (no durable parent/ordinal recovery snapshot yet) is
    accurate.
15. **Yes.** `docs/prefix-residency-coherence-proof-v1.md` states the
    plans are synchronous in-process CPU metadata transactions and
    excludes durable `insert_child`/`recover_namespace`, online
    publication, a shared catalog, direct I/O, registered buffers, real
    DRAM/HBM movement, cross-rank fatal propagation, performance, and all
    GPU/checkpoint/model execution — consistent with the code (no CUDA and
    no store I/O in the registration path). 250-test claim: the workspace
    contains exactly 250 `#[test]` functions at the candidate commit and
    `cargo test --workspace --offline` passed 250 with zero failures in
    this review. 48-handoff claim: `docs/` contains 50 `fable-*-handoff.md`
    files at the candidate commit and `verify_all_review_handoffs` skips
    exactly the two configured `HISTORICAL_HANDOFFS`
    (`crates/glm-cli/src/review.rs:19-22`), leaving 48 verified. The proof
    commit it names, `a3f5957b6e8d526cedb2ab58fa2204bb34d9f8b7`, is the
    direct parent of the candidate commit, which adds only the proof
    document.

## Adversarial probes (throwaway tests, removed before finish)

Temporary integration test `crates/glm-serving/tests/adversarial_probe.rs`
(uncommitted, public API only, deleted before finish hashing;
`git status --porcelain` clean afterward). All four probes passed
(`cargo test --offline -p glm-serving --test adversarial_probe`: 4 passed;
0 failed).

1. `probe_exact_dedup_then_mtp_preserving_target_only_then_draft_restore`
   — registered gen-1 twice (exact dedup succeeded; the pre-correction
   coordinator at `a3f5957~1` calls `validate_nvme_registration` on the
   caller's record and returns `Residency(Stale)` here), upgraded to MTP
   gen-2, then registered a fabricated newer target-only gen-3; a
   draft-required restore against the real `FileTierStore` returned
   `page_has_draft == [true]`. Had residency received the gen-3
   target-only record, `complete_restore` exact-record validation against
   the durable gen-2 page would have failed. PASS.
2. `probe_partial_rank_failure_leaves_all_ranks_and_index_unchanged` —
   8 pages across all four ranks (2 per rank), all restored and pinned; a
   generation-2 registration failed at planning (`Residency(Pinned)`) with
   every page still `Hbm` on every rank; after release, the identical
   generation-2 registration succeeded. The retry's success proves the
   candidate index was NOT adopted on the failed attempt: had it been, the
   per-key live-index (gen 2) vs live-residency (gen 1) equality check at
   `cache.rs:115-118` would have failed the retry with `Record`. PASS.
3. `probe_constructor_rejects_prepopulated_index` — a one-page
   prepopulated `PrefixIndex` is rejected by the constructor with
   `Record`. PASS.
4. `probe_registration_cost_scales_with_index_size` — N distinct one-page
   registrations: N=1000 took 3.12 s, N=4000 took 36.9 s; 4x the
   registrations cost 11.8x the time, confirming the O(index size)
   per-call clone (finding M1). Metadata only; memory returns to the
   adopted index size after each call — no leak, no payload clone.
   Evidence for M1, not a correctness failure.

The pre-correction coordinator
(`git show a3f5957~1:crates/glm-serving/src/cache.rs`) validated and then
registered the caller's input records rank-by-rank (`register_nvme` in a
per-record loop), so (a) exact dedup failed `Stale`, and (b) the record
installed into residency was the caller's input, not the index's retained
record — the MTP divergence. It did prevalidate every record before the
first `register_nvme`, so per-record partial mutation required a
validate/commit disagreement; the candidate's four-plans-then-one-commit
sequence removes even that window and adds the per-key index/residency
equality proof, which the old code lacked entirely.

## Six separate statements

1. **The index and owner-rank residency record remain exactly coherent —
   YES.** Every registration proves per-key exact `TierRecord` equality
   between the live index and the owner rank before building plans
   (`cache.rs:115-118`); only post-insert candidate records are written
   (`cache.rs:119-124`); residency transitions never alter the stored
   record (`residency.rs:445-448`, `619-631`); divergence injected out of
   band fails the next registration closed. Verified by the shipped
   regression, probes 1 and 2, and full-file reads.
2. **MTP capability cannot be lost through the coordinator after being
   retained by the index — YES.** The index's monotonic-capability guard
   (`prefix.rs:156-160`) retains the MTP record; the retained record
   equals the prior record so no residency write occurs; the caller's
   target-only input is never eligible for residency. Verified by
   regression step 4 and probe 1's draft-required restore.
3. **Exact dedup succeeds without rewriting residency — YES.**
   `prior == Some(next)` produces no update entry (`cache.rs:122`); the
   candidate index still counts the reference (`prefix.rs:161`) and is
   adopted. Probe 1's repeated gen-1 registration succeeded; the
   pre-correction code demonstrably returns `Residency(Stale)` there.
4. **All rank registration failures are atomic across ranks and the index
   — YES.** All four plans are built fallibly before the first infallible
   commit (`cache.rs:126-133`); any plan error returns with zero rank
   mutations and no candidate adoption. Probe 2 proved both halves: no
   rank state changed on failure, and the successful retry proved the
   index had not been adopted.
5. **The regression distinguishes the prior cross-component defect —
   YES.** The pre-correction `register_prefix` (shown at `a3f5957~1`)
   fails the regression at exact dedup with `Residency(Stale)`; with dedup
   omitted it would install the caller's target-only record and fail the
   later exact-record restore validation. The current test covers all five
   required behaviors (question 12).
6. **The CPU proof and all exclusions are accurate — YES.**
   `docs/prefix-residency-coherence-proof-v1.md` matches the code and its
   listed hashes match the shipped files; 250 `#[test]` functions exist at
   the candidate commit and the full workspace suite passed 250 with zero
   failures in this review; 48 = 50 fable handoffs minus the 2 configured
   historical skips; the synchronous-CPU-boundary statement and every
   GPU/publication/model/performance exclusion is consistent with what the
   code actually does.

## Architecture & maintainability

- The plan/validate-then-infallible-commit discipline is applied uniformly
  (`plan_nvme_registrations`/`commit_nvme_registrations`,
  `plan_hbm_admission`/`apply_hbm_admission`,
  `validate_unpin_count`/`unpin_count`,
  `plan_pending_rollback`/`commit_pending_rollback`,
  `plan_release_many`/`commit_release`). The `expect()` calls in commit
  paths are justified by exclusive `&mut self` access and carry accurate
  messages. This is the codebase's strongest pattern; keep it.
- Duplication: the four-rank constant (M4); the near-identical rollback
  planners `plan_pending_rollback` and `plan_pending_rollback_with_restore`
  (`cache.rs:448-506`) differ only by one pre-seeded restore entry and
  could be one function taking an optional extra restore;
  `validate_abort_restore_identity` is run once in the planner and again
  inside `abort_restore_identity` at commit — harmless but doubled.
- Layering is clean: `glm-cache` owns index/residency/tier primitives with
  no coordinator knowledge; `crates/glm-serving/src/cache.rs` is the only
  composition point; the coherence invariant lives in exactly one
  function.
- Simplification opportunities: replace the full index clone with an undo
  log or two-phase preflight on the live index (M1); split
  `PrefixRestoreError::Record` into distinct variants (M2); share a
  `RANKS` constant through `glm-cache` (M4); give `restore_longest` a
  deadline (M3); document the two-loop reference-overflow invariant in
  `PrefixIndex::insert` (M5).
- API surface: `PrefixRestoreCoordinator` exposes a reasonable minimal
  surface. `ResidencyManager`'s full plan/commit surface is public, which
  is what lets tests (and any future caller) mutate ranks behind the
  coordinator's back; the shipped tests already reach into
  `coordinator.ranks` directly. Fine for same-module tests, but it shows
  the invariant is maintained by discipline, not by construction —
  long-term the coordinator should own rank access.
- `PrefixIndex` has no removal/decref API; references only grow.
  Acceptable for the retained boundary; needs a lifecycle before
  production.

## Verification summary and token decision

- Provenance: start and finish hash sets both match the handoff table
  exactly; `review-proof` verdict PASS at the candidate commit; the
  worktree remained pinned at
  `72e60716cf58632dd9aba5ead41ba0d128f59395` and `git status --porcelain`
  was clean at finish after temp-file removal.
- Findings: 0 BLOCKER, 0 MAJOR, 5 MINOR, 2 QUESTION. No index/residency
  divergence, no MTP downgrade, no partial rank mutation, no unbounded
  payload clone (the per-call index clone is metadata-only and recorded as
  MINOR M1 under the handoff's performance exclusion), no fail-open path,
  no use-after-release, and the regression is distinguishing.
- All six required statements are an unqualified YES.

The token below accepts only the retained in-process CPU coordinator
correction reviewed here. It does not open cn4, authorize CUDA work, or
accept durable online publication, restart reconstruction, or model
execution.

prefix-residency-coherence-v1-accepted
