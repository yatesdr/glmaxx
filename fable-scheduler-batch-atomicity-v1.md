# Fable review: scheduler batch-completion atomicity v1

Date: 2026-07-31
Reviewer: Fable (adversarial design-gate review)
Handoff: `docs/fable-scheduler-batch-atomicity-v1-handoff.md`

Location note: the handoff requests this result at the repository root as
`fable-scheduler-batch-atomicity-v1.md`; the operator directed all review
results into `docs/reviews/`, so it is written here instead.

## Reviewed candidate commit

2f7d0ce30392d1fe5c3256058e4d8604100791f2

Reviewed in a detached worktree pinned at that commit. `main` was never used
as review substrate.

## Verified input hash table

Every input named by the handoff was hashed with SHA-256 at review start and
again at review finish; both hash sets were identical and matched the
handoff's pinned values exactly (8/8 inputs, zero mismatches). `glmaxx
review-proof` against this handoff also reports verdict PASS with all
expected/actual hashes equal.

| Input at candidate commit | SHA-256 (verified) |
|---|---|
| `spec/engine-v0.md` | efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a |
| `crates/glm-scheduler/src/lib.rs` | 5a820b2e5013f038f07b26f14ddc24d69d00e18d3d55837ab5ff3a68daee3074 |
| `crates/glm-scheduler/src/compile.rs` | 220cf549c0b5882d109ebce4ebd646e9b28ebbab80a83fa579ef5a2c591a070a |
| `crates/glm-serving/src/lib.rs` | 9d011012cb103149aed5ff531f356746d50f0ed29398854f2d2516c42d82aeab |
| `docs/offline-serving-foundation.md` | 9a722fdcc77522ac361493ca8fc02fea1e4692a28d1c22bd2e96607568cd4ce0 |
| `docs/offline-serving-spine.md` | 27b24d4cbafc8203937d3620e7bcd85d47fcb86cc4d8b89e237025e5d40a62f9 |
| `docs/scheduler-batch-atomicity-proof-v1.md` | ea351b40c481aaab129eec263101a916de8d0b84fcb197e22b12bca73a8b2f71 |
| `scripts/local-checks.sh` | 839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f |

Provenance anomaly (procedural, not staleness): the handoff file itself is
not present in the tree at the candidate commit (added by the later gating
commit). All hashed inputs exist at the candidate and match; `review-proof`
was run in the pinned worktree with the handoff supplied alongside the pinned
tree and passed.

## Gate commands

Run in the pinned worktree: `cargo test --offline -p glm-scheduler` (all
passed), `cargo test --offline -p glm-serving` (all passed),
`cargo clippy --offline -p glm-scheduler --all-targets -- -D warnings`
(clean), `glmaxx review-proof` (PASS).

## Answers to the 13 required adversarial questions

1. YES (defect confirmed against the pre-fix source). The prior code took
   `self.inflight.take()` first and then mutated request rows and tenant
   totals in row order with fallible `checked_add`/`ok_or(Overflow)` per row;
   a tenant near `u64::MAX` overflows on a later same-tenant row after
   earlier rows and the inflight batch had already mutated.
2. YES. In `complete_batch_internal` (glm-scheduler/src/lib.rs:320-485)
   every fallible lookup, shape check, arithmetic operation, duplicate check,
   completion binding, request-state check, and cumulative tenant total
   occurs before `self.inflight.take()` at line 465; everything after is
   assignment-only.
3. YES. The planner searches staged `tenant_service_units` first and
   continues from the staged total for a repeated tenant
   (lib.rs:386-395), committing cumulatively at 456-461.
4. YES. Prefill preflight errors (399-407) return before any mutation; the
   regression asserts progress, tenant total, `decode_burst == 0`, and exact
   inflight identity unchanged (lib.rs:1153-1174).
5. YES. Decode/verify errors (417-445, 460) all precede line 465; no
   `get_mut` occurs before the apply phase.
6. YES. `!success` stages every row as `RequestState::Failed`
   (lib.rs:377-380) with only the request lookup fallible, and mutation
   occurs only in the apply phase after all identities are staged;
   malformed failure calls are rejected at line 328.
7. YES. Duplicate inflight rows (339-345), unknown/extra completions
   (348-352), duplicate completions (353-355), and length mismatches
   (330-334) are all rejected before plan construction.
8. YES. `MAX_ACTIVE_SEQUENCES: u16 = 64` (glm-engine/src/step.rs:9) feeds
   `MAXIMUM_BATCH_ROWS` (scheduler lib.rs:18), configuration validation
   (147-153), and the defensive re-check at completion (327); both fixed
   arrays are sized from the shared ABI constant.
9. NO (cannot). Distinct tenants <= rows <= 64 and the 64-slot array always
   has a free slot when row i <= 63 is processed; the `ok_or(Overflow)` at
   line 393 is unreachable fail-closed defense.
10. NO. The three apply-phase `expect`s (inflight, request, tenant;
    lib.rs:465-482) are each proven `Some` in the same `&mut self` call with
    no intervening removal path, interior mutability, reentrancy, or unsafe
    code; the claimed infallible commit is true.
11. YES. `complete_batch_with_results` passes the caller's slice through;
    the internal path uses only stack arrays (`[None; MAXIMUM_BATCH_ROWS]`).
    The bounded linear search is O(64^2) worst case on hard-capped input —
    acceptable, no hidden unbounded input. The legacy
    `complete_batch_with_commits` still collects a `Vec`, as disclosed.
12. YES. `late_tenant_overflow_preserves_inflight_and_every_row`
    (lib.rs:1136-1193) forces overflow only on the cumulative second
    same-tenant row, proves exact inflight identity, both progress records,
    tenant total, and decode burst unchanged, then retries the identical
    batch successfully. On the prior code, row 1 committed, row 2's
    `prompt_done` mutated, and inflight was consumed — the identity and
    progress assertions fail and retry returns `NoInflight`.
13. YES (verified). Static deduplicated count of `#[test]` functions is
    exactly 237 with zero `#[ignore]`; the 37-handoff provenance count
    matches (39 handoffs minus 2 historical skips); tests and clippy were
    run in the pinned worktree by this review and pass; GPU/model/
    performance non-claims are consistent with the pure-CPU code.

## Six summary determinations

- All completion errors precede mutation: YES.
- Cumulative request/tenant planning is complete and bounded: YES.
- The apply phase is infallible under safe exclusive access: YES.
- Failure preserves an exactly retryable inflight step: YES.
- The regression distinguishes the prior partial commit: YES.
- The CPU proof and its non-claims are accurate: YES.

## Findings

### BLOCKER

None.

### MAJOR

None within the review boundary.

### MINOR

1. Terminal requests are never evicted from `Scheduler.requests`
   (pre-existing, outside the declared boundary): `ordered_requests`
   iterates and sorts every request ever admitted per `next_batch`
   (lib.rs:668-701, plus scans at 266-273), and `admit` scans all requests
   per admission (211-221). With M lifetime requests each step is
   O(M log M) and admissions total O(M^2); a long-running server with 10^6
   completed requests scans 10^6 entries per batch selection. Memory grows
   likewise (`terminal_events` in glm-serving). Should be tracked on the
   punchlist before this scheduler fronts a long-lived server.
2. `decode_burst` policy on `success == false`: failed decode/verify
   completions still increment the burst counter and failed prefill resets
   it (lib.rs:361-364, 483). Atomic, but debatable policy; deserves a
   comment or test.
3. Unchecked subtraction in serving output validation
   (glm-serving/src/lib.rs:444, `maximum_new_tokens - generated`) relies on
   a scheduler invariant; the scheduler's equivalent uses `checked_sub`.
   Symmetric fail-closed treatment recommended.
4. The handoff file is absent at the pinned commit (gating-commit ordering);
   awkward for self-contained reproducibility, no staleness.

### QUESTION

1. After a successful `complete_batch_with_results(true, ...)`, subsequent
   serving-level event emission and lease/token release can still fail with
   the scheduler already committed (glm-serving/src/lib.rs:456-516). The
   handoff excludes the pending serving-page transaction design; confirm
   that design owns this window. (Event-queue capacity itself is provably
   reserved: 512 = 64 x 7 + 64.)
2. On `success == false`, requests are marked `Failed` without a state
   check (lib.rs:377-380); currently unreachable for terminal rows, but a
   debug assertion would make the invariant explicit.

## Architecture & maintainability

A textbook preflight/commit split: a `BatchCompletionPlan` of `Copy` structs
in fixed arrays sized from the single shared ABI constant, every fallible
operation completed before the single `inflight.take()` boundary, and an
apply phase of pure assignment. The tenant-slot search doubling as an
unreachable fail-closed `Overflow` guard is sound belt-and-suspenders, and
replacing the old `BTreeMap` completion index with a stack array removes
both allocation and the missing-row/duplicate ambiguity. Main structural
debt is upstream: lifetime retention of terminal requests with per-step
rescans, and the legacy `complete_batch_with_commits` Vec shim that should
be retired in favor of the explicit-result path.

## Token decision

All six summary determinations are unqualified YES; there are no BLOCKER or
MAJOR findings; the input hash set matched at review start and finish;
`review-proof`, both test suites, and clippy pass in the pinned worktree;
the proof's 237-test and 37-handoff counts verify exactly. The minors are
pre-existing or out-of-boundary tracking items.

scheduler-batch-atomicity-v1-accepted
