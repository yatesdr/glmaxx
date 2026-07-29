# Fable handoff: pending admission rollback v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`bfbe7f46cbd9db52aa766950aec1432c7677778d`

Required result path:
`fable-pending-admission-rollback-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`pending-admission-rollback-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/residency.rs` | `f63aaee9f96997e2a39e12f9a908d0b2bdee2a3f8a5c3c17f794b878fc0843ec` |
| `crates/glm-serving/src/cache.rs` | `e7686a678537b1644608f655e0a8bd40133e0772e0cdf12865c02bbefb15b54b` |
| `crates/glm-serving/src/lib.rs` | `d33b6e9efc231fabdd1065f64db83cb74c95ee82138a49c8ee1798a130465eca` |
| `crates/glm-scheduler/src/lib.rs` | `5a820b2e5013f038f07b26f14ddc24d69d00e18d3d55837ab5ff3a68daee3074` |
| `docs/terminal-cleanup-transaction-proof-v1.md` | `5998b9abb4e1587ef5a4a83ebbc1c2e6bee551122fdd075af0ea1cac01172862` |
| `docs/pending-admission-rollback-proof-v1.md` | `cfd008dacc26f7d82c3f524ad7347da9d492168ebd9e53bb255bdc6cbcbfddfd` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-pending-admission-rollback-v1-handoff.md
cargo test --offline -p glm-cache
cargo test --offline -p glm-serving
cargo clippy --offline -p glm-cache -p glm-serving --all-targets -- -D warnings
```

## Review boundary

This review covers only CPU pending-prefix restore rollback, cancellation,
poll failure propagation, exact restore abort identity, retained prompt-token
accounting, and pending request-ID exclusion. It does not accept the complete
active-page admission transaction, private KV tails, rank page-table deltas,
physical-ID quarantine, CUDA, direct tier I/O, checkpoint execution, model
output, or performance.

## Required adversarial questions

1. Did the prior cancellation path remove the pending registry entry before
   rolling pages back one at a time, allowing a late error to coexist with
   earlier unpins/aborts and no retry handle?
2. Did the prior poll-error path have the same remove-before-rollback and
   partial-mutation defect?
3. Does `PendingRollbackPlan` count every pinned `(rank, page key)` before
   validating any unpin, including repeated keys?
4. Does every restoring page retain and validate its exact request ID and
   logical page ordinal before abort?
5. Can a wrong request or ordinal abort another pending restore, or does it
   fail while leaving `Residency::Restoring` intact?
6. Are duplicate restoring `(rank, page key)` entries rejected before any
   mutation?
7. Does cancellation complete the entire rollback preflight while the
   registry entry still exists, and remove it only before an infallible
   commit under exclusive coordinator access?
8. If polling fails and rollback preflight fails, is the exact updated
   pending record reinserted with every page state and live handle preserved?
9. Does successful rollback return the original poll/restore error, while a
   failed rollback returns its blocking error and remains retryable?
10. On restore-service saturation, is the just-started identity planned with
    every earlier page before any abort, rather than aborted ahead of a
    separately fallible rollback?
11. Can safe or reentrant code change residency between rollback preflight
    and commit, or can any commit operation still return a recoverable error?
12. On a cache polling error, does serving remove its pending admission and
    prompt reservation if and only if that cache request is no longer
    pending?
13. Is the post-release prompt-byte value computed before cache polling or
    cancellation, leaving no fallible subtraction after cache mutation?
14. Can `admit_prevalidated` take an ID already reserved by pending token
    admission, a prefix lease, or a retained prompt-token map?
15. Does the multi-page corruption regression prove the later restoring page
    is not partially aborted, the pending request remains, and cancellation
    succeeds after repair?
16. Would the prior implementation fail that regression for the claimed
    remove-before-rollback reason?
17. Does the serving-level corruption regression prove both registries, the
    exact 256-byte prompt reservation, scheduler exclusion, event silence,
    repair, and exact cancellation cleanup?
18. Does the five-page/per-rank-one saturation regression deterministically
    revisit rank zero and prove every started restore and outstanding handle
    is rolled back?
19. Are rollback plan allocations bounded by admitted prefix pages and absent
    from ordinary decode and successful pending polls?
20. Are the 243-test claim, 41-handoff claim, and every GPU/model/performance
    non-claim accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- cancellation rollback is all-or-nothing and retryable;
- poll failure preserves the exact pending record when rollback is blocked;
- restore abort is bound to request ID and logical ordinal;
- service saturation uses the same complete rollback transaction;
- serving admission and prompt accounting remain coupled to cache ownership;
- pending request IDs cannot enter through the prevalidated route;
- the distinguishing regressions fail the prior code for the claimed
  reasons; and
- the CPU proof and all scope exclusions are accurate.

Only if all eight answers are unqualified `YES`, end with the requested
token. Withhold it for a conditional pass, stale input, undercounted pins,
identity-insensitive abort, remove-before-preflight, partial rollback, a lost
pending record, prompt-accounting drift, request-ID collision, an unbounded
hot-path allocation, a nondistinguishing regression, or an overstated proof.

The token accepts only this CPU correction. It does not open cn4, authorize
the pending prefill ABI implementation, or accept real model execution.
