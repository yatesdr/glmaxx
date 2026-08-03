# Fable review: pending admission rollback v1

Date: 2026-07-31
Reviewer: Fable (adversarial design-gate review)
Handoff: `docs/fable-pending-admission-rollback-v1-handoff.md`

Location note: the handoff requests this result at the repository root as
`fable-pending-admission-rollback-v1.md`; the operator directed all review
results into `docs/reviews/`, so it is written here instead.

## Reviewed candidate commit

bfbe7f46cbd9db52aa766950aec1432c7677778d

Reviewed in a detached worktree pinned at that commit. `main` was never used
as review substrate.

## Verified input hash table

Every input named by the handoff was hashed with SHA-256 at review start and
again at review finish; both hash sets were identical and matched the
handoff's pinned values exactly (8/8 inputs, zero mismatches).

| Input at candidate commit | SHA-256 (verified) |
|---|---|
| `spec/engine-v0.md` | efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a |
| `crates/glm-cache/src/residency.rs` | f63aaee9f96997e2a39e12f9a908d0b2bdee2a3f8a5c3c17f794b878fc0843ec |
| `crates/glm-serving/src/cache.rs` | e7686a678537b1644608f655e0a8bd40133e0772e0cdf12865c02bbefb15b54b |
| `crates/glm-serving/src/lib.rs` | d33b6e9efc231fabdd1065f64db83cb74c95ee82138a49c8ee1798a130465eca |
| `crates/glm-scheduler/src/lib.rs` | 5a820b2e5013f038f07b26f14ddc24d69d00e18d3d55837ab5ff3a68daee3074 |
| `docs/terminal-cleanup-transaction-proof-v1.md` | 5998b9abb4e1587ef5a4a83ebbc1c2e6bee551122fdd075af0ea1cac01172862 |
| `docs/pending-admission-rollback-proof-v1.md` | cfd008dacc26f7d82c3f524ad7347da9d492168ebd9e53bb255bdc6cbcbfddfd |
| `scripts/local-checks.sh` | 839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f |

Provenance anomaly (procedural, not staleness): the handoff file itself is
not present in the tree at the candidate commit; it was added by the later
gating commit. All hashed inputs exist at the candidate and match. The
`review-proof` run was performed in the pinned worktree with the handoff file
supplied alongside the pinned tree.

## Gate commands

Run in the pinned worktree: `cargo test --offline -p glm-cache` (48 passed,
0 failed), `cargo test --offline -p glm-serving` (all passed),
`cargo clippy --offline -p glm-cache -p glm-serving --all-targets --
-D warnings` (clean), and `glmaxx review-proof` against this handoff
(all pinned hashes verified).

## Answers to the 20 required adversarial questions

1. YES. The prior `cancel_restore` removed the pending registry entry first
   (`self.pending.remove(&request_id).ok_or(UnknownRequest)?`) and then rolled
   pages back one at a time in reverse, collecting a `first_error` while
   continuing — a late error coexisted with completed unpins/aborts and no
   retry handle remained. Verified against the actual pre-fix source via
   `git diff` of the fix commit.
2. YES. The prior `poll_restore` error paths had the same
   remove-before-rollback partial-mutation defect (`rollback_pending(&pending)?`
   with no reinsert after removal at entry).
3. YES. `plan_pending_rollback` (cache.rs:442-445) accumulates a checked
   count per `(rank, page key)` including repeated keys, then validates the
   cumulative count via `validate_unpin_count` (residency.rs:467-480).
4. YES. `PendingPage.ordinal` and `PendingRestore.request_id` are retained;
   `begin_restore` records `PendingRestoreIdentity { request_id, page_ordinal }`
   (residency.rs:333-336) and abort requires it (residency.rs:379-393).
5. NO (fails safely). `validate_abort_restore_identity` (residency.rs:360-377)
   returns `ResidencyError::State` on any mismatch without mutation;
   residency.rs:904-915 proves wrong request and wrong ordinal both fail with
   `Residency::Restoring` intact.
6. YES. Duplicate restoring `(rank, page key)` entries are rejected during
   planning (`restores.insert(...).is_some() => Err(Record)`, cache.rs:446-452),
   before any mutation.
7. YES. `cancel_restore` (cache.rs:304-317) plans from the still-present
   entry, removes it only after a fully validated plan, and commits via
   infallible (`expect`-guarded) operations under exclusive `&mut self` access.
8. YES. `fail_polled_restore` (cache.rs:412-432) reinserts the owned pending
   record — including page states updated during the poll loop and live
   `RestoreHandle`s — via a vacant-entry check when rollback preflight fails.
9. YES. Successful rollback returns the original error (cache.rs:418-419);
   failed rollback reinserts the record and returns the blocking rollback
   error, leaving poll/cancel retryable (proven at lib.rs:1843-1873).
10. YES. On restore-service saturation, `plan_pending_rollback_with_restore`
    (cache.rs:470-492) plans the just-started identity together with every
    earlier page before any abort; the prior code aborted the newest page
    ahead of a separately fallible rollback.
11. NO (correct). Between preflight and commit there are only straight-line
    `&mut self` statements — no callbacks or reentrancy — and commit
    operations are validated-then-`expect`ed; unpinning cannot invalidate a
    restore abort (it touches only `pin_count`).
12. YES. lib.rs:365-374 removes the pending admission and releases the prompt
    reservation if and only if `!cache.has_pending_restore(request_id)`.
13. YES. `retained_prompt_bytes_after_release` is computed with `checked_sub`
    before cache polling (lib.rs:354-360) and before cancellation
    (lib.rs:446-452); only infallible assignment/removal follows cache success.
14. NO (correctly rejected). lib.rs:269-274 rejects IDs already present in
    `pending_admissions`, `prefix_leases`, or `request_tokens` with
    `Backpressure`; the scheduler independently rejects duplicates. Proven at
    lib.rs:1821-1834.
15. YES. The multi-page corruption regression (cache.rs:669-687) proves the
    later restoring page is not partially aborted, the pending request
    remains, and cancellation succeeds after identity repair.
16. YES. The prior remove-first reverse-iteration code would have aborted
    page 1, failed on page 0, and dropped the registry entry — directly
    contradicting the test's assertions. Verified against the pre-fix source.
17. YES. The serving-level regression (lib.rs:1758-1889) proves both
    registries retained, the exact 256-byte prompt reservation, scheduler
    exclusion, event silence, repair, and exact cancellation cleanup ending at
    zero retained bytes with exactly one `Cancelled` event.
18. YES. The five-page/per-rank-one saturation regression (cache.rs:759-839)
    deterministically revisits rank zero (`owner_rank = ordinal % 4`) and
    proves pending=0, all outstanding handles released, and all five pages
    back to `Nvme`.
19. YES. Rollback plan maps are populated only from `pending.pages` (bounded
    by admitted prefix pages) and exist only on failure/cancel paths; ordinary
    decode and successful pending polls do not build them.
20. QUALIFIED YES. Statically verified: exactly 243 `#[test]` functions with
    zero `#[ignore]`; 43 handoff docs minus 2 historical skips = 41, matching
    the claim; GPU/model/performance non-claims consistent with the pure-CPU
    code. Dynamic confirmation: glm-cache and glm-serving test suites and
    clippy were run in the pinned worktree by this review and passed.

## Eight summary determinations

- Cancellation rollback is all-or-nothing and retryable: YES.
- Poll failure preserves the exact pending record when rollback is blocked: YES.
- Restore abort is bound to request ID and logical ordinal: YES.
- Service saturation uses the same complete rollback transaction: YES.
- Serving admission and prompt accounting remain coupled to cache ownership: YES.
- Pending request IDs cannot enter through the prevalidated route: YES.
- The distinguishing regressions fail the prior code for the claimed reasons: YES.
- The CPU proof and all scope exclusions are accurate: YES.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. Fallible `release(...)?` remains in `finish_token_admission` error paths
   (lib.rs:400-407, 408-418, 424-431) between a commit point and the
   accounting restore. Unreachable in practice within a single `&mut self`
   call (the pages were just pinned by the same admission), but it is the one
   remaining spot where a fallible cache mutation precedes an accounting
   restore; candidate for the same plan/commit treatment in a follow-up.
2. Error-identity drift on retried polls after a consumed worker handle: a
   retried `poll_restore` after a blocked rollback reports
   `RestoreError::WorkerClosed` instead of the original cause
   (residency.rs:61-73, cache.rs:266-287). State convergence is correct; only
   the diagnostic changes.
3. Panic-as-fail-closed in commit paths (`expect` in
   `commit_pending_rollback`/`cancel_restore`/`commit_release`, and the
   `panic!` in `fail_polled_restore`). Consistent with the codebase's
   preflight/commit idiom and unreachable under exclusive access.

### QUESTION

1. The handoff file does not exist in the candidate tree (added by the later
   gating commit), so `review-proof` cannot run from the pinned tree alone.
   Hashes all match, so there is no staleness; recommend gating commits
   include the handoff in the pinned candidate in future.
2. `admit_with_prefix` performs an O(n) scan over all requests per admission
   (glm-scheduler/src/lib.rs:211-221). Bounded and off the per-step path;
   awareness only.

## Architecture & maintainability

The correction generalizes the repository's established plan/validate/commit
idiom to pending-restore rollback: fallible work is hoisted into read-only
planning, the registry entry survives preflight, and commits are
infallible-by-invariant under exclusive coordinator access. Identity binding
(request ID plus logical page ordinal) pushed into `ResidencyManager` makes
the abort surface self-defending rather than relying on caller discipline,
and the serving layer's iff-coupling of its registries to cache ownership
removes the previous unilateral-cleanup drift. Allocations are bounded by
admitted prefix pages and confined to failure paths. The residual asymmetry
is `finish_token_admission`'s fallible release in error paths, a suitable
follow-up.

## Token decision

All eight summary determinations are unqualified YES; there are no BLOCKER
or MAJOR findings; the input hash set matched at review start and finish;
tests and clippy pass in the pinned worktree; the proof's counts verify. The
minors are defense-in-depth follow-ups, not conditions.

pending-admission-rollback-v1-accepted
