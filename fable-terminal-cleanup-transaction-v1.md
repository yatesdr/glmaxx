# Fable review: terminal cleanup transaction v1

Date: 2026-07-31
Reviewer: Fable (adversarial design-gate review)
Handoff: `docs/fable-terminal-cleanup-transaction-v1-handoff.md`

Location note: the handoff requests this result at the repository root as
`fable-terminal-cleanup-transaction-v1.md`; the operator directed all review
results into `docs/reviews/`, so it is written here instead.

## Reviewed candidate commit

6535248bb217b20d56ec0d6670c8fb6f33791205

Reviewed in a detached worktree pinned at that commit. `main` was never used
as review substrate.

## Verified input hash table

Every input named by the handoff was hashed with SHA-256 at review start and
again at review finish; both hash sets were identical and matched the
handoff's pinned values exactly (10/10 inputs, zero mismatches). `glmaxx
review-proof` against this handoff reports verdict PASS with all
expected/actual hashes equal.

| Input at candidate commit | SHA-256 (verified) |
|---|---|
| `spec/engine-v0.md` | efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a |
| `crates/glm-serving/src/cache.rs` | f265314cc36e5453219b96b351a2a6adad04dbf12b6647b9682b5a5cb7f80ea0 |
| `crates/glm-serving/src/lib.rs` | 5f67b28a7a2169564687822c49b3f6c26710352f8edd1361f4daf834f21346b0 |
| `crates/glm-cache/src/residency.rs` | a83edc26e750573878888888cc58f1eb08846c39633ef1f9084db7cfeaba295c |
| `crates/glm-scheduler/src/lib.rs` | 5a820b2e5013f038f07b26f14ddc24d69d00e18d3d55837ab5ff3a68daee3074 |
| `docs/prefix-release-atomicity-proof-v1.md` | 7fbe0f4ced91d7ddc8da4f38b6c9c9a8bc73f524eb257ef1ca9a537f095bb9f4 |
| `docs/selected-step-failure-finalization-proof-v1.md` | 36be571d84cff086ad3058f3426fc0fee6bdd4d33b1c4317473128e4d861512e |
| `docs/serving-page-transaction-v1.md` | e3a9a1d9f2eb26dc5312d7c42297fa3d832e444f7e3f269094746a85fb3deac2 |
| `docs/terminal-cleanup-transaction-proof-v1.md` | 5998b9abb4e1587ef5a4a83ebbc1c2e6bee551122fdd075af0ea1cac01172862 |
| `scripts/local-checks.sh` | 839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f |

Provenance anomaly (procedural, not staleness): the handoff file itself is
not present at the candidate commit (the gating commit postdates it; the
same pattern holds across this queue). All hashed inputs exist at the
candidate and match; `review-proof` was run in the pinned worktree with the
handoff supplied alongside the pinned tree and passed.

## Gate commands

Run in the pinned worktree: `cargo test --offline -p glm-cache` (all
passed), `cargo test --offline -p glm-serving` (all passed),
`cargo clippy --offline -p glm-cache -p glm-serving --all-targets --
-D warnings` (clean), `glmaxx review-proof` (PASS).

## Answers to the 17 required adversarial questions

1. YES (defect confirmed against the pre-fix source). The prior
   successful-step loop committed the whole scheduler batch, then iterated
   rows pushing events and calling `release_request_prefix(...)?` /
   `release_request_tokens(...)?` per row — a later row's error coexisted
   with earlier published events and released leases.
2. YES. `plan_successful_step_publication` runs before
   `complete_batch_with_results(true, ...)` (glm-serving/src/lib.rs:541-551)
   and contains every fallible operation: checked prefill arithmetic
   (633-638), row/output lookups (624-627, 650), event staging with
   `ok_or(Overflow)` (183-191), `require_event_space` (694), cumulative
   prompt-byte `checked_sub` (784-795), lease lookups, `plan_release_many`
   with owner derivation and overflow-checked counts, and
   `validate_unpin_count` per unique page (cache.rs:315-332,
   residency.rs:432-445). Event capacity is additionally pre-reserved at
   step start (457).
3. QUALIFIED YES. Prefill, output-limit, EOS/terminal, and accepted-draft
   ordinal calculations are expression-equivalent to scheduler completion
   and `StepOutput` validation. One textual gap: the planner does not
   duplicate the scheduler's per-depth commit cap (`committed_tokens <=
   depth + 1` for `Verify{depth}`); a 7-token row in a lower-depth verify
   batch passes planning and then fails the entire batch at scheduler
   preflight — before any mutation — routing into `fail_selected_step`.
   Fail-safe in effect (no partial publication is possible), equivalence
   holds in effect but not textually (MINOR 2).
4. YES. Each page-key slice restarts its logical ordinal at zero before
   `owner_rank` (`enumerate()` per slice, cache.rs:320-323), matching the
   restore path's per-request enumeration. No flattened global ordinal.
5. YES. Repeated `(rank, page_key)` values accumulate with `checked_add`
   across all request slices before any validation (cache.rs:324-330); the
   regression produces a count of 2 for the shared page.
6. NO (one pin cannot satisfy two releases). `validate_unpin_count`
   requires `Residency::Hbm` and `pin_count >= count` for the full
   cumulative count (residency.rs:441); the test proves both shared pins
   survive by two independent post-failure releases.
7. NO (residency cannot change). `plan_release_many` takes `&self`; the
   only call between plan and `commit_release` is the scheduler commit,
   which never touches the cache; `ResidencyManager` has no interior
   mutability and no reentrant path exists.
8. YES. After scheduler commit, `commit_request_releases`
   (lib.rs:822-836) contains only `expect`-guarded `unpin_count` on the
   preflighted plan, `BTreeMap::remove`, and assignment of the precomputed
   byte counter, followed by `events.extend`. No `?`, no fallible lookup,
   no arithmetic. Two non-Result caveats: `VecDeque::extend`/`BTreeSet::
   insert` can heap-allocate (allocation failure aborts, cannot return an
   error), and the `expect`s panic only on invariants excluded by preflight
   under `&mut self`.
9. YES. Request IDs dedup into a `BTreeMap` with `Prefix` dominating
   `Tokens` (lib.rs:777-781); each retained token buffer is subtracted
   exactly once and each entry removed once.
10. YES. `MAXIMUM_STEP_EVENTS = MAX_ACTIVE_SEQUENCES * (MAX_MTP_DEPTH + 2)`
    = 64 x 8 = 512, derived from the engine constants (lib.rs:23,
    step.rs:9,11); the boundary test builds 64 rows each yielding 7 Token
    plus 1 Finished events and asserts `events.len == MAXIMUM_STEP_EVENTS`
    — all 512 fixed slots exercised, not a smaller shape.
11. YES. Staging is fixed arrays; ordinary nonterminal decode produces an
    empty release map (no allocation) and `prefix: None`; the older
    progress/completion `Vec`s are explicitly excluded by the proof
    (lines 100-102), not hidden.
12. YES. In `fail_selected_step` (lib.rs:733-748), releases and event
    space are preflighted into held `Result`s, then
    `complete_batch(false)?` consumes the inflight batch unconditionally,
    then the held cleanup error returns with zero lease/pin/token/event
    mutation; the test proves all three requests Failed, all leases
    retained, no events, and an idle next tick.
13. YES. `emit_terminal_transitions` (lib.rs:701-726) preflights the full
    cancelled set (event space and release plan) before any commit, event,
    or terminal marker; a failed attempt mutates nothing serving-side and
    the identical set is recomputed next idle tick — exact retry proven
    after repair (lib.rs:1901-1940).
14. YES. The distinguishing regression (lib.rs:1731-1945) has requests 100
    and 101 sharing one page plus corrupt request 102, proves the expected
    residency error, all three Failed with leases retained, zero events,
    both shared pins surviving (by two independent releases), and no
    inflight batch remaining.
15. YES. The prior code would have committed the scheduler, published rows
    100 and 101's events, and released their shared pins before row 102's
    mid-loop error — exactly the claimed partial-success failure, confirmed
    by reading the pre-fix source.
16. YES. The cancellation phase independently proves no partial
    release/event on a late invalid pin and exact retry (emitting exactly
    the two ordered Cancelled events) after repair.
17. QUALIFIED YES (verified). Release-map assembly is bounded by
    HBM-resident leased pages at O(P log P), not O(n^2); exactly 241
    `#[test]` functions statically; 42 handoffs minus 2 historical skips =
    40, matching the claim; GPU/model/performance non-claims match the
    pure-CPU code. Tests and clippy were run in the pinned worktree by this
    review and pass.

## Eight summary determinations

- All successful-step publication errors precede scheduler mutation: YES.
- Shared prefixes use cumulative multi-request pin counts: YES.
- The post-scheduler release/event commit is infallible under safe
  exclusive access: YES.
- Selected-step failure preserves the no-stranded-inflight invariant even
  when cleanup preflight fails: YES.
- Cancellation cleanup is all-or-nothing and retryable: YES.
- Fixed staging covers the exact maximum verifier event shape: YES.
- The distinguishing regression fails the prior code for the claimed
  reason: YES.
- The CPU proof and all scope exclusions are accurate: YES.

## Findings

### BLOCKER

None.

### MAJOR

None within the review boundary.

### MINOR

1. Failed-step resource recoverability gap (prominent; follow-up handoff
   recommended). When `fail_selected_step`'s cleanup preflight fails,
   requests become Failed with the inflight batch correctly consumed and
   zero partial mutation — the handoff's required behavior — but their
   leases, token buffers, and `retained_prompt_bytes` remain held with no
   public reclamation path (`cancel()` on a Failed request early-returns;
   `emit_terminal_transitions` filters only Cancelled), and no Failed event
   reaches the client. Reachable only behind an already-corrupt cache
   invariant, and the composed backend treats the returned error as
   process-fatal, so no long-running leak arises in the deployed
   composition; the sibling failure-finalization proof discloses this as
   the open cross-request cleanup boundary. Failed requests deserve the
   same terminal-marker/retry treatment cancellation received.
2. The planner omits the scheduler's per-depth commit cap for
   `Verify{depth<6}` (see question 3) — fail-safe whole-batch failure, but
   breaks the letter of the expression-equivalence claim.
3. Original error masked on preflight failure in `fail_selected_step`
   (lib.rs:741): the cleanup-preflight error replaces the root-cause step
   error. Observability only.
4. Unbounded terminal-request retention (pre-existing, outside the
   boundary; identical finding recorded in the sibling reviews):
   `scheduler.requests` and `terminal_events` never prune, and
   `emit_terminal_transitions` allocates and scans an all-time request-ID
   `Vec` every idle tick — O(all-time requests) per idle tick and O(n^2)
   cumulative admission scans. Punchlist tracking recommended.
5. The `events` VecDeque is never reserved to `event_capacity`, so the
   post-commit `extend` can grow the heap (abort-on-OOM only, not a
   Result).

### QUESTION

1. The handoff is absent at the pinned commit (gating-commit ordering);
   recommend gating commits include the handoff in the pinned candidate.
2. The `StagedEvents::push` overflow arm is structurally unreachable given
   `require_event_space` and scheduler shape caps; no test pins it. Fine to
   leave, worth a note.

## Architecture & maintainability

The correction converts three previously interleaved mutate-as-you-go paths
(successful publication, failed-step cleanup, idle cancellation) into a
uniform plan/commit shape, mirroring the scheduler's own preflight/apply
split. The invariant "no `?` after commit" is structurally visible — the
commit functions take owned plans and return `()`, so a future fallible
addition would not compile without signature changes, which is the right
kind of friction. Fixed-size staging with derived capacities keeps the hot
path allocation-free and the boundary provable by a single exact-fit test.
Remaining debt is concentrated and pre-existing: unbounded terminal-request
retention, the disclosed legacy progress/completion `Vec`s, and the
Failed-state resource story (MINOR 1), which deserves the same
terminal-marker treatment cancellation received. Test quality is high; the
shared-pin proof-by-independent-release technique is convincing.

## Token decision

All eight summary determinations are unqualified YES; there are no BLOCKER
or MAJOR findings; the input hash set matched at review start and finish;
`review-proof`, both test suites, and clippy pass in the pinned worktree;
the proof's 241-test and 40-handoff counts and its HBM-bounded release-map
claim verify. MINOR 1 is reachable only behind an already-corrupt cache
invariant, is process-fatal in the deployed composition, satisfies the
letter of every handoff question and determination, and is disclosed as the
open cross-request cleanup boundary — a follow-up handoff, not a condition
on this correction.

terminal-cleanup-transaction-v1-accepted
