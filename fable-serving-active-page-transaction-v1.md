# Review: serving active-page transaction v1

Date: 2026-07-31

Reviewer: Fable (adversarial design-gate review, CPU scope only)

Candidate commit:

326158a25f6ca0c68e1b543195984c5537542df4

Handoff: `docs/fable-serving-active-page-transaction-v1-handoff.md`
(SHA-256 `53edd67354a4f1f51c009d3f624dca0bcf388d9fece62b21c6c805c12d8c4f08`,
reported by `review-proof`, verdict PASS).

Result-path note: the handoff declares the required result path as
`fable-serving-active-page-transaction-v1.md` at the repository root. The
operator directed that review results be written into `docs/reviews/`
instead of the repository root; this artifact follows the operator
direction and is at `docs/reviews/fable-serving-active-page-transaction-v1.md`.

## Provenance verification

The review was performed in a detached worktree checked out at the candidate
commit (`git rev-parse HEAD` = the candidate at both review start and review
finish). Every pinned input was hashed with `shasum -a 256` at review START
and again at review FINISH; all seventeen digests matched the handoff's
expected values at both points, and `review-proof` independently confirmed
the same input set with verdict PASS. No stale or divergent input was found.

| Input | Verified SHA-256 (start and finish) |
|---|---|
| `spec/engine-v0.md` | efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a |
| `crates/glm-cache/src/sequence.rs` | e5902ffe36366916b728c54cd78f62331daf63136190d72cbc81d107e5150c36 |
| `crates/glm-cache/src/lib.rs` | 0d9d1fcdbb9c8350b1702d1c41263c24818861936d3ff37f4f4f73125cb6e269 |
| `crates/glm-serving/src/cache.rs` | 099bffde185307365f5932c84f14b15c1ccc4b4cfe29f00612265f69a46a9839 |
| `crates/glm-serving/src/lib.rs` | d63508beaee3fdc5baed8d47f3435460c4f3143298c406d6e084babd02bf3da7 |
| `crates/glm-serving/src/backend.rs` | a1dca883453d03e0e69a7896370f9d0b95cc1e7271443b6b91686a8d0d6e44e9 |
| `crates/glm-scheduler/src/lib.rs` | 5fd0c4506002c4da5679f1ca3bf96a880ca7b0b348d5f55ada26a2e06ae7ff4d |
| `crates/glm-scheduler/src/compile.rs` | 220cf549c0b5882d109ebce4ebd646e9b28ebbab80a83fa579ef5a2c591a070a |
| `crates/glm-cli/src/main.rs` | 2af7739f311520b60601b18b2d14d3617320df535de24ecd310596add7ac3ff4 |
| `fixtures/cpu-serving-proof-v1.json` | c95e1049bc52f8a8aaacd5a2d704008df9e8cfe72c8f3486982568adbaa7b47e |
| `docs/serving-active-page-transaction-proof-v1.md` | 073706cfe3c77afc42863cff9d3598ed74ef64e9ce1ea18d4dbeec4e5c147871 |
| `docs/serving-page-transaction-v1.md` | 266b4ca53a92be9a0ba77d367bac7f4da9d8500fd9437a0738c3e612e94e0b4b |
| `docs/offline-serving-spine.md` | 008f7e72507d67a11269fb6c450bbde369ba4394cece1975774adefa5776175a |
| `docs/active-prefix-record-binding-proof-v1.md` | 9bb87c359d78c340d740ef9723ac78ef23510af5fabf4b29b1630211499b4c12 |
| `docs/production-punchlist.md` | a2374599452a4254357972671c54cbcbb95b8215bd1c8b4264d89672ee8d91dc |
| `docs/results-index.md` | 079faf15bac2d1bf091a7e097f35daec2a63b2564526152fcef39aec528469cd |
| `scripts/local-checks.sh` | 839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f |

## Gate results (each run once, from the detached worktree)

- `review-proof docs/fable-serving-active-page-transaction-v1-handoff.md`:
  verdict PASS, repository head equals the candidate.
- `cargo test --offline -p glm-scheduler`: 16 passed, 0 failed.
- `cargo test --offline -p glm-serving`: 41 passed, 0 failed.
- `cargo clippy --workspace --all-targets --offline -- -D warnings`: clean.
- `scripts/local-checks.sh`: exit 0, including workspace formatting, the full
  workspace test run, CUDA FFI type checks, deterministic fixture
  regeneration and byte comparison (`cpu-serving-proof-v1.json` compared
  equal), and `review-proof-all` verifying 66 review handoffs (the 65
  then-present handoffs claimed by the proof document plus this review's own
  untracked handoff copy). Tokenizer proof skipped as disclosed
  (`GLMAXX_TOKENIZER_DIR` unset); nvcc absent, no CUDA executed.
- Workspace test enumeration (`cargo test --workspace -- --list`): exactly
  273 tests, matching the proof document's 273-test claim.

No GPU, cn4, or CUDA work was performed.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. Double-invariant-breach cleanup retains terminal mappings.
   `crates/glm-serving/src/lib.rs` `fail_selected_step` (lines 830-852) and
   the analogous tick error path first mark every selected row Failed via
   `scheduler.complete_batch(false)?` and only then commit the release plan
   (`releases?`). If the release plan itself failed (possible only when a
   separate invariant is already broken, e.g. prefix pins released out from
   under a lease, as constructed in
   `late_terminal_cleanup_failure_does_not_partially_publish_the_batch`),
   rows become terminally Failed while their active-table mappings and
   prefix leases are retained with no Failed events and no retry path
   (unlike Cancelled rows, which `emit_terminal_transitions` retries every
   tick). A caller that ignored the returned error and kept ticking would
   strand those pages permanently. The production backend
   (`crates/glm-serving/src/backend.rs` `runtime_loop`) treats every tick
   error as fatal and drains all requests, so the exposure is bounded and
   the retain-rather-than-corrupt posture is deliberate, but the asymmetry
   with cancellation cleanup deserves either a retry path or an explicit
   invariant comment.
2. Unbounded terminal-request retention. `Scheduler::requests`
   (`crates/glm-scheduler/src/lib.rs`) never evicts Finished, Cancelled, or
   Failed rows, and `ServingCoordinator::terminal_events` only grows.
   `ordered_requests`, `request_ids`, and `emit_terminal_transitions`
   iterate every request ever admitted on each tick, so a long-running
   backend degrades and grows without bound. This is beyond the excluded
   clone-per-step cost because it scales with historical, not active, load.
3. `ServingCoordinator::cancel` (`crates/glm-serving/src/lib.rs` line 508)
   calls `self.next_sequence_generation()?` and discards the result. It is
   an overflow preflight only; the generation actually advances at boundary
   cleanup. A comment or a dedicated preflight name would avoid the
   appearance of a lost generation advance.
4. `admit_tokens` blocks in a `park_timeout(1ms)` poll loop with no waker
   and no deadline (`crates/glm-serving/src/lib.rs` lines 328-339). Only
   the CLI proof and tests use it (the backend uses begin/poll), but an
   unbounded blocking admission helper on the coordinator API is an
   attractive nuisance for future callers.

### QUESTION

1. The proof document names implementation commit
   `f480ef179ec7088005b1dbcdc04be113c289974d` while the handoff pins its
   doc-recording child as the candidate. All pinned source hashes are
   identical at both commits, so provenance is intact; confirm this
   two-commit pattern (implement, then record proof) is the intended
   convention so future reviews do not flag it.
2. The pinned candidate's `glm-cli` has no `review-acceptance-lint`
   subcommand; it exists only in the current main tree. Acceptance linting
   therefore necessarily runs against a newer, unpinned CLI. Consider
   pinning the lint tool version in future handoffs.

## Answers to the 22 required questions

1. YES. At `f480ef1^`, `ServingCoordinator` owned only
   `sequence_table_generation: u64` with no `SequencePageTable`,
   `active_pages`, or `PageTableConfig`; scheduler progress and
   `try_submit` proceeded with no capacity reservation.
2. YES. `ServingConfig.page_table: PageTableConfig` is a mandatory field
   (`crates/glm-serving/src/lib.rs` line 50) and `ServingCoordinator::new`
   constructs and owns exactly one `SequencePageTable` (lines 141, 248).
3. YES. `validate_context_limit` (lines 1021-1027) rejects
   `prompt + maximum_new > MAXIMUM_CONTEXT_TOKENS = 1_048_576` and is the
   first statement of both `begin_admit_tokens` (line 346) and
   `admit_active_sequence` (line 295), before any scheduler, page, byte, or
   event mutation. Regression: request 97 in
   `exact_one_million_context_is_admitted_accounted_executed_and_released`.
4. YES. Production admission flows only through
   `admit_tokens`/`begin_admit_tokens` -> `finish_token_admission`, which
   passes exactly `restored.page_attachments()` with private cached tokens
   hardwired to 0 (line 465); `RestoredPrefix::validate` binds keys to
   attachments, and `SequencePageTable::admit_with_prefix` revalidates
   namespace/generation/piece hashes. The cached-position bypass
   `admit_prevalidated` is `#[cfg(test)] pub(crate)` (lines 275-277) and
   does not exist in production builds.
5. YES. `admit_active_sequence` mutates a clone of the active table
   (attachment plus private positions), then admits the scheduler row, and
   only afterwards installs the clone, advances the precomputed generation,
   and publishes the Admitted event (lines 313-325); event space is
   preflighted first, and any failure leaves all coordinator state
   untouched (prefix pins released by `finish_token_admission`).
6. YES. `reserve_active_step` (lines 951-982) requires, for every selected
   row, `committed_tokens == prompt_done + generated` exactly (lines
   964-969) before any reservation, and it runs before `try_submit` (line
   572).
7. YES. On one cloned all-row candidate: prefill appends exactly
   `row.prompt_tokens`; decode reserves `begin_tentative(_, 1)`; verify
   reserves `begin_tentative(_, depth + 1)` (lines 970-979).
8. YES. The reservation happens before `try_submit`, so a capacity error
   discards the candidate clone and routes to `fail_selected_step`, which
   fails and cleans every selected row.
   `page_capacity_failure_is_atomic_and_never_reaches_rank_workers` proves
   the rank call counter stays at 16 across the failing fifth step and all
   pages are released.
9. YES. `effective_decode_depth` (`crates/glm-scheduler/src/lib.rs` lines
   737-762) selects the maximum captured verify depth `<= min(configured
   depth, remaining_new - 1)` with `unwrap_or(0)` MTP0 fallback;
   `mtp_depth_clamps_to_captured_tail_shape_and_falls_back_to_decode`
   exercises 6 -> 5 -> 0.
10. YES. `has_graph_for_request` (lines 717-735) requires a prefill graph,
    an MTP0 decode graph, and a verify graph at the exact configured depth
    at admission, so a partially accepting request always has a captured
    tail (any uncaptured intermediate depth falls back to decode).
11. YES. `Tp4WorkerPool` consensus precedes output handling; prefill
    requires `output_rows.is_empty()` (lines 599, 991) and decode/verify
    commit exactly `output.count()` per row on the candidate via
    `commit_tentative` (line 1000), which also bounds the commit by the
    reservation.
12. YES. `plan_successful_step_publication` stages all events (with
    `require_event_space`) and the full release plan (prefix unpin counts
    validated, terminal sequences removed on the candidate) before
    `complete_batch_with_results`; `commit_request_releases` installs the
    adopted active table (line 934) before `commit_release` unpins any
    prefix (lines 936-941).
13. YES for the enumerated failure classes. Compile, worker submit/receive,
    consensus, output, and capacity failures all route through
    `fail_selected_step` before any candidate installation; the candidate
    is a local clone and unreachable afterwards, and the release plan
    removes the selected rows' mappings from the last committed table.
    (The only path retaining mappings requires an additional, independent
    invariant breach; see MINOR 1.)
14. YES. `tick_observed` calls `apply_cancellations_at_boundary` (inflight
    forbidden) and then `emit_terminal_transitions`, which removes active
    mappings and prefix pins and emits Cancelled events, strictly before
    `next_batch` can select a runnable peer (lines 527-531). Failed plans
    are retried on the next tick, so a continuously runnable peer cannot
    postpone cleanup.
15. YES. The generation advances exactly once per published admission
    (line 320), per successful step and per failure/terminal cleanup batch
    (via `plan_request_releases` -> `commit_request_releases`), with
    checked overflow. The proof document explicitly withholds the future
    rank-visible reserve/commit generation claim (proof lines 59-62).
16. YES. With one 64-token page per rank (256 positions), four prefill
    steps commit exactly 256 positions with 16 rank calls; the fifth step's
    one-token reservation fails with `Pages(Capacity)` and the counter
    stays 16, proving the step never reached the workers. The prior source
    had no capacity object at all, so it would have submitted a 20th call;
    the counter assertion genuinely distinguishes.
17. YES. Request 95 (MTP0) starts at 1,048,575 committed positions with
    stats `target_pages_used == [4096; 4]`, `draft_pages_used == [0; 4]`;
    the final token executes on all four workers (calls 4) and terminal
    cleanup returns stats to all zeros.
18. YES. Request 96 (mtp_depth 6) at the same position owns
    `[4096; 4]` target and `[4096; 4]` draft pages; its one-token tail is
    asserted to run as `StepMode::Decode` (MTP0 fallback, since
    `remaining_new - 1 = 0`), executes on four workers, and both arenas
    return to zero.
19. YES. A 1,048,577-position request is rejected with
    `ServingError::Request` by `validate_context_limit` before any
    mutation; `active_sequences` remains 0.
20. YES. Previously request 202 (MTP6, 7 outputs) always classed as depth 6
    and could never share a batch with the MTP0 request 101, giving
    2 prefill + 4 solo decode + 7 solo verify = 13 steps. With the tail
    bound, after the first Verify{6} step commits one token, 202's
    remaining budget clamps its class to MTP0 (depth 5 is uncaptured), so
    it joins 101's common decode batches; 9 decode/verify steps carry the
    same 11 tokens (two steps carry two rows each): 2 + 9 = 11 steps,
    identical 11 token events, 0 speculative. `local-checks.sh`
    regenerates and byte-compares the fixture deterministically.
21. YES. 273 workspace tests enumerated exactly; 65 then-present handoff
    proofs (66 verified now including this review's own handoff);
    formatting, workspace Clippy with `-D warnings`, FFI checks, and the
    deterministic CPU proofs all passed in this review's single
    `local-checks.sh` run (exit 0). The tokenizer-proof skip is disclosed.
22. YES. Clone-on-step/per-token mutation, the fixed undo log, rank
    delta/digest, upload and removal acknowledgment, physical-ID
    quarantine, CUDA payload, live tiers, real 1M model execution,
    checkpoint serving, quality, and performance are all explicitly listed
    as not implemented/not proven (proof lines 159-169), and nothing in the
    pinned code claims otherwise. No CUDA or cn4 resource was touched by
    this review.

## Eight summary statements

1. The prior missing active-capacity boundary is real: YES.
2. Admission and every selected step are atomic in the retained CPU scope:
   YES.
3. No capacity-failed or malformed step can reach or partially publish rank
   work: YES.
4. MTP tail selection cannot reserve past the request/context budget: YES.
5. Cancellation and terminal cleanup remove active mappings before prefix
   release or peer selection: YES.
6. The MTP0 and MTP6-capable exact-1M regressions account and release every
   page: YES.
7. The regressions distinguish the old missing integration: YES.
8. All gate counts and device/model/performance exclusions are accurate:
   YES.

## Architecture & maintainability

The transaction discipline is uniform and easy to audit: every mutation
site follows plan-on-a-clone / preflight / install (admission, step
reservation, publication, terminal cleanup), with `SequencePageTable`
itself snapshot-rollback safe internally. Ownership boundaries are clean:
the scheduler knows nothing of pages, the page table knows nothing of
tenants, and the coordinator is the single writer joining them under one
generation counter that the compiler stamps into every `StepPlan`. Event
capacity is preflighted twice per tick so the failure paths cannot
themselves be starved by backpressure, which is a subtle and well-executed
detail. Costs are honest about being a CPU oracle (full-table clones per
step, per-token page walks); the two growth concerns worth scheduling are
terminal-request eviction (MINOR 2) and a retry or fatal-invariant story
for the double-breach cleanup path (MINOR 1). Test quality is high: the
distinguishing regressions assert worker-call counters and exact per-rank
page stats rather than merely error types, and the double-fault tests pin
the retain-not-corrupt semantics explicitly.

## Token decision

All gates pass, every pinned input verified at start and finish, and there
are no BLOCKER or MAJOR findings; all eight summary answers are
unqualified YES. The requested acceptance token follows. It accepts only
this retained CPU serving active-page transaction; it does not open cn4,
authorize CUDA work, or accept production serving.

serving-active-page-transaction-v1-accepted
