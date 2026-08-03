# Review: HBM residency admission atomicity v1

Date: 2026-07-31

Reviewer: Fable (adversarial design-gate review)

Handoff: `docs/fable-residency-admission-atomicity-v1-handoff.md`

Candidate commit:

c84da2a4686c37227de5a0dd4694409fdf42f25b

Result location note: the handoff requests the result at the repository root
(`fable-residency-admission-atomicity-v1.md`); the operator directed reviews
into `docs/reviews/` instead of the repo root, so this artifact lives at
`docs/reviews/fable-residency-admission-atomicity-v1.md`.

## Provenance

The review ran in a detached worktree checked out at the candidate commit
(`git rev-parse HEAD` = the candidate, confirmed at review start and finish).
Every pinned input was hashed with `shasum -a 256` at review START and again
at review FINISH; both hash sets matched the handoff table exactly, with no
drift between start and finish. `review-proof` on the handoff returned
`verdict: PASS` with every expected/actual pair equal. One mid-review
experiment temporarily replaced `crates/glm-cache/src/residency.rs` with a
hybrid (old implementation + new tests) to prove the regression test bites;
the file was restored via `git checkout --` and its hash re-verified equal to
the pinned value immediately afterward and again at finish.

Verified hashes (pinned input, SHA-256, matched at start and finish):

| Input | SHA-256 |
|---|---|
| `spec/engine-v0.md` | efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a |
| `crates/glm-cache/src/residency.rs` | cd15cbbcf1031adb1fc73e5416fbf5d5149ff87096f193c8ad1b0709417f9629 |
| `crates/glm-cache/src/tier.rs` | c31b07d7f9054f3d51bc5d24c2c414b6c9a134d88f042502bc0f82e29cad500f |
| `crates/glm-cli/src/cache_proof.rs` | 3371395bb723d2ec092c16cfd28bcb25b54ca1e38fc2096dff471941b2ac9358 |
| `fixtures/cache-lifecycle-proof-v1.json` | 8d75a281e127f669f52065c7ca2fa0945a4d090e3624f17f857410122dde0dfc |
| `docs/cache-lifecycle-proof-v1.md` | 11ad4936fea7cd0887e660911f50778d5b0918c21a6cebaca1a98a244b2e2de1 |
| `docs/residency-admission-atomicity-proof-v1.md` | 2412d03f3f1f91cf4bfa12556281b962792da5752805ec1d58c467b385908e97 |
| `scripts/local-checks.sh` | 839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f |

## Gate results

All commands run once from the worktree:

- `review-proof docs/fable-residency-admission-atomicity-v1-handoff.md`: PASS
  (handoff sha256 `9730df25e179f0624e0ae508008abbab9429e3a477bd1bbd439884ee99bab9df`)
- `cargo test --offline -p glm-cache`: 48 passed, 0 failed
- `cargo test --offline -p glm-cli`: 7 passed, 0 failed
- `cargo test --offline -p glm-serving`: 24 passed, 0 failed
- `cargo clippy --offline -p glm-cache --all-targets -- -D warnings`: clean

Additional verification performed for this review:

- `cargo test --workspace --offline`: 234 passed total, 0 failed (48+7+11+38+60+3+21+12+24+10)
- Hybrid run (old `make_hbm_room` implementation grafted under the new test
  module): `failed_multi_victim_admission_does_not_demote_any_page` FAILED at
  residency.rs:869 with `left: Some(Dram), right: Some(Hbm)` — the exact
  partial-demotion defect. The new implementation was restored afterward.
- `cache-lifecycle-proof` regenerated into scratch and `cmp` against
  `fixtures/cache-lifecycle-proof-v1.json`: byte-identical.
- `review-proof-all`: verified 35 review handoffs (34 tracked then-present +
  this review's untracked handoff; 2 legacy handoffs skipped as historical),
  verdict PASS.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. `crates/glm-cache/src/residency.rs:473` (`hbm_after + incoming_bytes` in
   the no-victim branch) and `:505` (`dram_after += bytes` in the victim
   loop) are unchecked operations whose safety depends on an identical
   `checked_add` evaluated immediately above (lines 466-469 and 500-503).
   Correct today and clippy-clean, but fragile under refactor; binding the
   checked sum to a local (or a comment tying the pairs) would remove the
   coupling.
2. `ResidencyError::Overflow` is also used for underflow (`checked_sub` of
   `dram_release_bytes` at residency.rs:462-465 and of victim bytes at
   :497-499). A distinct variant or comment would make counter-drift
   diagnosis less ambiguous.
3. There is no direct unit test for a failed `promote_dram` (Q7). The
   property holds by construction — `plan_hbm_admission` takes `&self` and
   every promotion error path returns before any mutation — and successful
   promotion is exercised by the cache-lifecycle proof, but a one-assert
   failure test would pin it.
4. The planner's residual-capacity failure is named `ResidencyError::Pinned`
   (residency.rs:512-517). It is accurate only because of an implicit
   invariant: every page that can reach HBM or DRAM was admission-checked
   against `config.hbm_bytes` (via `begin_restore`'s size preflight at
   residency.rs:328 or a prior successful admission), so residual pressure
   after evicting all unpinned pages can only come from pins. The invariant
   is real but unasserted; a debug assertion or comment would harden it.

### QUESTION

1. `docs/residency-admission-atomicity-proof-v1.md` names implementation
   candidate `94f8d572` while the review candidate is its doc-only child
   `c84da2a4` (which adds only the proof doc; all pinned source hashes are
   identical at both). Assumed intentional; confirm.
2. The success test (`successful_multi_victim_admission_commits_one_bounded_plan`)
   also passes against the old incremental implementation (observed in the
   hybrid run). This matches the proof doc's stated role split — the failure
   test distinguishes the old implementation; the success test exercises the
   multi-victim commit — but confirm that split is the intent.

## Answers to the 14 required questions

1. **Old partial demotion — yes, the defect was real.** Old `make_hbm_room`
   (git `012914e^:crates/glm-cache/src/residency.rs`) looped
   `min_by_key`/`demote`, mutating residency and both byte counters per
   victim, then returned `ResidencyError::Pinned` from the same loop when no
   candidate remained — after unrelated pages had already moved.
   Empirically confirmed: the new failure test run against the old
   implementation fails with the first page found in `Dram` instead of `Hbm`.
2. **Yes.** `plan_hbm_admission` (residency.rs:455-527) takes `&self`, so no
   path can mutate. Every named path returns an error before any caller
   mutation: pinned-capacity residual (:512-517), incoming-byte overflow
   (:466-468, :490-492, :512-514, :519-521), victim-byte failure via
   `entry_bytes` inside the fallible collect (:484-485), counter underflow
   (:497-499), and DRAM-counter overflow (:500-503). Callers
   (`complete_restore` :375, `promote_dram` :396) invoke the planner before
   touching any entry.
3. **Yes.** Candidates filter to `Some(**key) != excluded`, `residency ==
   Residency::Hbm`, `pin_count == 0` (:481-483) and are sorted by
   `(last_touch, key)` (:486) — a total, deterministic order since page keys
   are unique. The target is excluded explicitly and, additionally, is never
   in `Hbm` residency at planning time (`Restoring` or `Dram`).
4. **Yes.** Each victim's DRAM-vs-NVMe destination (:500-509) and the final
   `hbm_bytes` (:519-521) and `dram_bytes` (:462-465, :505) are computed
   entirely inside the `&self` planner and returned in `HbmAdmissionPlan`
   before any state changes in the caller.
5. **No fallible operation in the window.** In `complete_restore`
   (:376-386) and `promote_dram` (:397-405), after target mutation only
   infallible field assignments, the infallible `apply_hbm_admission`
   (:529-541 — pure assignments, no arithmetic), and the pre-computed
   `next_clock` store occur. `next_clock` is checked before planning
   (:374, :395). The `get_mut(...).ok_or(Missing)?` between plan and target
   mutation is fallible but precedes all mutation and cannot fire (the entry
   was just read).
6. **Yes.** On a planner error in `complete_restore`, the `?` at :375
   returns before :376-385: `Restoring` state, `pending_restore` identity,
   all other entries, `restored` payloads, pins, `hbm_bytes`/`dram_bytes`,
   and `clock` are untouched. Test
   `failed_multi_victim_admission_does_not_demote_any_page` (:862-918)
   asserts locations, `Restoring` state, both counters, and that
   `abort_restore`/`unpin` still succeed (pending identity and pin intact).
7. **Yes.** In `promote_dram`, the planner error at :396 returns before any
   mutation; the target remains `Residency::Dram` and `dram_bytes` still
   carries its charge exactly once — the `checked_sub` of the release
   happens only inside the plan's shadow copy, and the old post-mutation
   `checked_sub` on `self.dram_bytes` is gone. (Verified by construction;
   see MINOR 3 for the missing direct test.)
8. **Yes.** Subtracting `dram_release_bytes` first (:462-465) models the
   DRAM space the target vacates in the same atomic commit, letting victims
   use it instead of spilling to NVMe unnecessarily. It cannot overcommit:
   every victim placement re-checks `dram_after + bytes <=
   config.dram_bytes` (:500-503), and on success the target really leaves
   DRAM, so the committed `dram_bytes` is exact. The cache-lifecycle proof
   observes the improvement directly: after promotion, `keys[2]` is now
   `Dram` rather than the old unnecessary `Nvme` (cache_proof.rs:177-181).
9. **Yes, sufficiently.** All additions and subtractions that can move the
   totals are checked. The two apparently plain operations —
   `hbm_after + incoming_bytes` at :473 and `dram_after += bytes` at :505 —
   each execute only after an identical `checked_add` of the same operands
   succeeded in the immediately preceding condition, so they cannot wrap.
   (Fragility noted as MINOR 1.)
10. **Yes.** `apply_hbm_admission` clears `entry.restored` only when the
    destination is `Residency::Nvme` (:535-537); DRAM victims keep their
    restored payload, and `promote_dram` back to HBM retains it.
11. **Yes.** `pin_hbm` (:408-426) reads with `get`, checks `Hbm` state, and
    computes `next_pin_count` and `next_clock` with checked arithmetic
    before the `get_mut`; `Missing`, `State`, and both `Overflow` errors all
    return before any mutation of state, clock, or pin count. The old
    version incremented `self.clock` before the state check.
12. **Yes.** The failure test empirically fails the old incremental
    implementation (hybrid run: assertion at residency.rs:869, first page
    `Dram` instead of `Hbm`), and both tests use the two-victim geometry
    where the incoming `mtp_page` (TargetKv + TargetIndexer + DraftSidecar)
    is strictly larger than either resident two-piece page (asserted at
    :870), forcing a genuine two-victim plan. The success test (:920-967)
    commits both deterministic victims to DRAM and checks the exact final
    counters (`hbm_bytes == incoming_bytes`, `dram_bytes == hbm_capacity`).
    The success test alone also passes the old implementation — consistent
    with the proof doc's role split (QUESTION 2).
13. **Yes.** The fixture diff is exactly one field,
    `pages_sha256_after_corruption`, changed because
    `cache_proof.rs` now corrupts the `TargetKv` byte of `recovered[0]`
    (the page actually on NVMe after bounded promotion — final posture
    `keys[0]=Nvme, keys[1]=Hbm, keys[2]=Dram`) instead of `recovered[2]`
    (now on DRAM). `journal_sha256` and every boolean are unchanged.
    Reproduced deterministically: regenerated proof is byte-identical to
    the pinned fixture.
14. **Yes.** `cargo test --workspace --offline` at the candidate passes
    exactly 234 tests with zero failures. `review-proof-all` verifies 34
    then-present tracked handoff proofs (35 including this review's
    untracked handoff; 2 legacy handoffs are skipped as historical). The
    changed code is pure CPU Rust: no CUDA, GPU, direct I/O, io_uring, or
    model execution is touched, and the proof doc's non-claims (no cn4, no
    device cache correctness, no 1M-context or performance claims,
    tokenizer proof skipped with unchanged fixture) match the tree.

## Six summary statements

- All failure paths are mutation-free: **YES.**
- Deterministic victim selection and tier placement are accepted: **YES.**
- Success commits the complete bounded plan exactly once: **YES.**
- Promotion, pin, and restored-payload accounting are accepted: **YES.**
- Both regression tests distinguish the relevant old and new behavior:
  **YES** (failure test empirically fails the old implementation; the pair
  covers the atomicity defect and the bounded multi-victim commit).
- The CPU proof and its non-claims are accurate: **YES.**

## Architecture & maintainability

The plan/apply split is the right shape: a `&self` planner that returns a
complete `HbmAdmissionPlan` (victim map plus final counters) makes
no-mutation-on-error a type-level property rather than a discipline, and the
infallible `apply_hbm_admission` closes the partial-commit window. Callers
follow a clean preflight-plan-mutate-apply sequence with the clock computed
up front. Determinism comes cheaply from `(last_touch, page_key)` sorting
over BTreeMap iteration. Remaining roughness is minor: the checked/unchecked
arithmetic pairing is correct but refactor-fragile, `Overflow` doubles as an
underflow signal, and the fits-in-HBM invariant that justifies the `Pinned`
error name is implicit. The planner is O(n log n) in HBM-resident pages per
admission, which is fine at current scale; if entry counts grow, an
LRU-indexed structure would drop the per-admission sort. Test coverage of
the new path is strong on the restore side and thinner on promotion failure.

## Token decision

No blocker or major findings; all six summary statements are unqualified
YES; provenance verified at start and finish with no drift. The token below
accepts only this CPU correction and does not open cn4, direct I/O,
checkpoint conversion, or model execution.

residency-admission-atomicity-v1-accepted
