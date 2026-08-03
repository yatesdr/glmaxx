# Fable adversarial integrated review: four-rank mirror/step transaction v1

Date: 2026-07-31

Reviewer: Fable (adversarial integrated CPU transaction review; no GPU, CUDA,
or cn4 work performed or authorized)

Handoff: `docs/fable-rank-mirror-step-transaction-v1-handoff.md`

Reviewed candidate commit (detached worktree, reused, no modification, no
commit):

414b8464a298eb749f6bb22e9f56987cc19634e3

Result-path note: the handoff requests `fable-rank-mirror-step-transaction-v1.md`
at the repository root. The operator directed reviews into `docs/reviews/`
rather than the repository root the handoff names; this artifact is written to
`docs/reviews/fable-rank-mirror-step-transaction-v1.md` under that directive.

## Provenance

All fourteen pinned inputs were hashed with `shasum -a 256` in the detached
worktree at review start and re-hashed at review finish. Hashes were verified
at start AND at finish; both measurement sets matched the handoff table
exactly in every row, so one column below represents pinned = start = finish.
No stale candidate.

The handoff file itself is absent at the pinned commit; the worktree carries
an untracked copy verified byte-identical to the main-repo copy (SHA-256
82534a7e9928ff75cd6df3475bd8868405e4a42381c959e9a975376d9a4e5653) before use.
`review-proof docs/fable-rank-mirror-step-transaction-v1-handoff.md` returned
`"verdict": "PASS"` with all fourteen expected/actual pairs equal.

| Input at candidate commit | Pinned = Start = Finish SHA-256 |
|---|---|
| `spec/engine-v0.md` | efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a |
| `crates/glm-engine/src/input.rs` | c3d090429015030416f6c03ddb6fef2dfd569859ff6e0fcc05bcb2d6a163ffa2 |
| `crates/glm-engine/src/worker.rs` | 39a0c0b917921149869d2afc5d652815986bf776eca5ddc9b7abee41b4892652 |
| `crates/glm-engine/src/lib.rs` | b3ca0da8e0e61f05a92a3b15bc9dc7822395545733ebbdc270c9ff1fb21d6a54 |
| `crates/glm-cache/src/delta.rs` | 71ac2da15e869a6f2470c3551a7cd6ec4ff387850a23240e9a44ad96a538ff16 |
| `crates/glm-serving/src/lib.rs` | b70cb901a8ef86545342771c09f285e44f9df8eb226cf728809e0aa4d7040a5b |
| `crates/glm-serving/src/backend.rs` | c1f9e9d06b44674a1d1d0ef3c24553a9ebe63e913805946bff7c2780233fe94b |
| `docs/step-execution-io-v1.md` | 055412c022cfcf9299e95e3ad3f7b888a2d472835388c35c2a8443be71a7422c |
| `docs/serving-page-transaction-v1.md` | 8e2067cc39227bc7acaef82ba71bf887718a8b5a403def32a228517506396dcb |
| `docs/rank-mirror-step-transaction-proof-v1.md` | fc36e7820acd6fe34ebb13ddaa62c08d9c8b0dcf2e822ec4ef47b4c098963a8b |
| `docs/offline-serving-spine.md` | 40eb653baf7b7cf20f054f1234b86de93b4f79509312ded1db330492f9eb974d |
| `docs/production-punchlist.md` | d2272e0c88db849a95581019d0a78729538ac4f5457d08553315248d20f3dd0b |
| `docs/results-index.md` | 5d038591510997577cc2f2cf79302d595f445eb56da762427168ab6eb1d82b4a |
| `scripts/local-checks.sh` | 839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f |

Provenance anomalies: none. The pinned `docs/step-execution-io-v1.md` at this
commit is the AMENDED revision (it contains `page_table_delta_digest` in the
canonical input hash and the split `configured_mtp_depth`/`effective_mtp_depth`
fields, lines 62/77-78), i.e. the revision that the independent row-68
step-execution-io review required; this candidate implements that amended
contract, which is the correct base.

## Gate commands executed (worktree, offline)

- `review-proof` on the handoff: PASS (14/14 hashes matched).
- `cargo test --offline -p glm-engine worker::tests`: 10 passed, 0 failed.
- `cargo test --offline -p glm-serving`: run ten consecutive times to
  characterize the known flaky test; 41 passed / 0 failed on all ten runs
  (the flake did not reproduce here; see MINOR-1 for the code-level defect,
  which is confirmed by inspection).
- `cargo clippy --workspace --all-targets --offline -- -D warnings`: clean.
- `cargo test --offline --workspace`: 284 passed, 0 failed
  (68 glm-cache + 7 glmaxx + 11 glm-cuda + 46 glm-engine + 60 glm-format +
  3 nvfp4_proof + 22 glm-reference + 16 glm-scheduler + 41 glm-serving +
  10 glm-tokenizer; doc-tests 0). This reproduces the proof document's
  284-test claim exactly.
- `scripts/local-checks.sh`: PASSED, exit 0, 3m05s wall on this host —
  formatting, the full workspace suite (every `test result` line zero
  failures), both Clippy passes, the CUDA FFI type checks, all deterministic
  proof regenerations byte-compared against pinned fixtures, and
  `review-proof-all` ("verified 69 review handoffs and 0/50 configured
  results (0 accepted, 0 withheld)"). The tokenizer proof was skipped
  (`GLMAXX_TOKENIZER_DIR` unset) and CUDA compilation skipped (no `nvcc`),
  matching the proof document's own disclosures.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

**MINOR-1 — The client-visible terminal event is published before the
completion counter increments, making a mandated gate test intermittently
flaky.** In `finish_request` (`crates/glm-serving/src/backend.rs` lines
1180-1192), `request.events.try_send(ApiCompletionEvent::Finished { .. })`
executes before `counters.increment_completed()`. A test or client thread
that receives the final `Finished` event and immediately reads
`backend.metrics()` can observe `glmaxx_backend_completed_total` one short —
exactly the known intermittent failure of
`backend::tests::concurrent_tenants_complete_with_exact_lifecycle_totals`
(`backend.rs` line 2192, "missing metric: glmaxx_backend_completed_total 4").
The flake did not reproduce in ten consecutive `cargo test -p glm-serving`
runs on this host, but the ordering defect is unambiguous by inspection.
Grading: MINOR, because this is a metrics-visibility lag only. `ServingMetrics`
(`crates/glm-serving/src/metrics.rs`) is written by `increment_*`/`observe_*`
and read exclusively by `render()`; the completed counter participates in no
admission decision (admission gates on the `fatal`/`shutdown` atomics and the
owner registry), no commit/rollback path, and no receipt. The fix is a
one-line reorder (increment before the terminal send, or count at removal
from the owner registry). Until fixed, the serving suite remains a
nondeterministic gate at roughly the reported 1-in-5-to-10 rate on loaded
hosts, which future gate runs must anticipate.

**MINOR-2 — A doubly-failing cleanup path can replace the original error,
including the original fatal worker error.** In `fail_selected_step` and
`fail_selected_step_after_worker_fatal` (`crates/glm-serving/src/lib.rs`
lines 971-1020), `self.scheduler.complete_batch(false)?`, `releases?`, and
`self.commit_request_releases(releases)?` each propagate their own error
instead of the `error` argument. On the worker-fatal path, if
`plan_request_releases` fails (e.g. a prefix-cache residency error, which the
late-cleanup regression shows is reachable), the returned error is the cache
error and the original `WorkerError` is lost. The failure remains fail-stop
(no forged receipt, no partial publication, the runtime backend goes fatal
either way), so this is an error-identity/diagnostics defect in a
double-fault corner, not a correctness hole. Preserving the original error
(logging or wrapping the secondary one) would make Q12's guarantee
unconditional.

### QUESTION

**Q-A — Is the racy overlap check on `Tp4WorkerPool` acceptable as a
permanent contract?** `initialize_page_table` and `apply_page_delta`
(`crates/glm-engine/src/worker.rs` lines 334-370) reject overlap via
`self.outstanding() != 0`, but the pool methods take `&self` and the check is
not atomic with the enqueue, so two threads could in principle interleave a
bound step and a standalone delta. The dispatcher serializes all commands and
mirror generation exactness turns any misordering into a fail-stop
`Generation` error rather than corruption, and the sole production caller
(`ServingCoordinator`) owns the pool behind `&mut self`, so this is safe
today. State the single-owner assumption (or make the check atomic) before a
second caller appears.

**Q-B — Bound-step rank receipts attest the delta, not the post-apply mirror
state.** `execute_rank` returns `page_table_local_digest =
delta.rank_local_digest(rank)` computed from the same shared `Arc` delta the
dispatcher already holds, so the receipt proves the rank verified and applied
that exact delta (apply enforces exact `generation_before` and full-mirror
revalidation), but no digest of the resulting mirror state crosses the
consensus boundary. For the CPU tier this is disclosed ("a CPU receipt only",
serving-page-transaction) and the deterministic mirror makes divergence
unreachable without a delta failure; the device tier must add the
upload-visibility acknowledgment the serving-page transaction already
requires. Confirm the device receipt will bind post-apply state, not only
delta identity.

## Answers to the nineteen required adversarial questions

1. **Four persistent mirrors at nonzero generation before construction
   returns?** YES. `ServingCoordinator::new` calls
   `workers.initialize_page_table(Arc::new(active_pages.clone()), 1)` before
   `Ok(Self { .. })` (`glm-serving/src/lib.rs` lines 249-251).
   Initialization fans out to all four rank threads, each constructing a
   `PageTableMirror::from_table(.., generation)` that rejects generation zero
   (`glm-cache/src/delta.rs` lines 286-297), and the dispatcher requires the
   exact acknowledgment set {0,1,2,3} via the bitmask walk in
   `initialize_rank_page_tables` (`worker.rs` lines 553-584). Duplicate
   initialization is `PageTableInitialized`; a bound step before
   initialization is `PageTableUninitialized` (proven by
   `bound_step_requires_one_exact_initial_mirror_generation`).
2. **Admission preflights scheduler, applies one successor delta to all
   mirrors, verifies every receipt, then publishes?** YES.
   `admit_active_sequence` (`lib.rs` lines 298-351) validates
   context/sampling, preflights event space, builds the successor pages and
   `PageTableDelta::between(current, next, g, g+1)`, admits into a
   **cloned** scheduler, then calls `workers.apply_page_delta(delta)` —
   which requires all four `PageDeltaAck`s with exact rank set, exact
   `generation_after`, exact global digest, and each rank's expected local
   digest (`worker.rs` lines 586-618) — and only after success assigns
   `self.scheduler`, `self.active_pages`, `self.sequence_table_generation`,
   the retained sampling, and the `Admitted` event. Failure publishes
   nothing.
3. **Can any non-test caller submit a plan without `StepInput` and
   `PageTableDelta`?** NO — enforced by the compiler. `try_submit` is
   `#[cfg(test)]` (`worker.rs` line 284); `try_submit_inner` is private; the
   only public submission API is `try_submit_bound`, which requires both
   `Arc<StepInput>` and `Arc<PageTableDelta>` and verifies the input against
   plan/schedule/delta before enqueue. The plan-only `RankExecutor::execute`
   is reached only when `binding` is `None`, which only the test-gated path
   can produce. A workspace grep found no other non-test caller; the serving
   test-only helpers (`admit_prevalidated`, the zero-fill prompt branch in
   `build_step_input` lines 1244-1253) are `#[cfg(test)]`, with the non-test
   branch returning `ServingError::Request`.
4. **All four commands share the same immutable input/delta allocations?**
   YES. `StepBinding` holds `Arc<StepInput>` and `Arc<PageTableDelta>`;
   `dispatch_one` clones the binding per rank, which clones the `Arc`s only
   (`worker.rs` lines 642-651). Neither type exposes interior mutability or
   any post-construction mutator; ranks receive `&StepInput`. Nothing
   mutates the payload after dispatch.
5. **Each rank independently verifies plan, schedule, input, and delta
   before atomically applying the reservation?** YES. `execute_rank`
   (`worker.rs` lines 769-814) runs `plan.verify(schedule)`, then
   `binding.input.verify(plan, schedule, &binding.delta)` (which re-derives
   the canonical hash, re-verifies the delta digest, and re-checks
   generation/digest/row/page bindings), then `page_table.apply(&delta)`.
   `PageTableMirror::apply` is atomic: it verifies the delta, requires
   `self.generation == delta.generation_before` exactly, mutates a cloned
   candidate, revalidates the entire resulting mirror (collision, owner-rank,
   page-count, token accounting), and only then swaps (`delta.rs` lines
   309-356). Failure leaves the mirror untouched (proven by the tamper
   tests) and is fatal for the worker generation.
6. **Receipt binds plan, schedule, input, global delta, expected rank-local
   delta, output digest, rank, and step ID?** YES. `RankStepAck` carries
   exactly those eight fields (`worker.rs` lines 110-120), all populated
   from rank-side computation in `execute_rank`; no field is defaulted on
   the bound path (unbound zeros exist only on the test-only plan path).
7. **Consensus validates the exact rank set, all common fields, each
   rank-specific local digest, and does NOT require local digests equal to
   one another?** YES. `dispatch_one` sorts acknowledgments by rank and
   requires index-exact ranks (so exactly {0,1,2,3}, no duplicate, no
   out-of-range); per rank it checks `input_hash`, `page_table_global_digest`,
   and `page_table_local_digest` against the expected rank-specific value
   from the shared delta; the cross-rank equality check covers step ID, plan
   hash, schedule hash, input hash, global digest, output digest, and the
   full output — and deliberately excludes the local digest (`worker.rs`
   lines 657-687). The delta test asserts the four local digests are
   pairwise distinct, so requiring equality would fail every real step; it
   does not.
8. **Exact prompt slices in batch-row order; exact context, configured and
   effective MTP, limits, sampling bits, seed, RNG counter retained?** YES.
   `build_step_input` (`lib.rs` lines 1215-1280) iterates `batch.rows` in
   order, slices `tokens[prompt_done .. prompt_done + row.prompt_tokens]`
   from the canonical retained vector with checked arithmetic and
   contiguous `prompt_payload_offset`, sets `context_tokens_before =
   prompt_done + generated`, carries `maximum_new_tokens`,
   `configured_mtp_depth = progress.mtp_depth`, `effective_mtp_depth` from
   the batch kind (0 for prefill/decode, `depth` for verify), and copies the
   one canonical `StepSampling` (kind/temperature/top-p/top-k/seed/
   rng_counter_before) stored at admission. `StepInput::validate_shape`
   re-checks every binding, including `configured >= effective == plan
   depth` for VERIFY and the delta's per-row committed/tentative
   expectations, and the canonical hash covers every row field, every prompt
   token, the generation, and the delta digest (`input.rs` lines 301-337).
9. **Prefill reservation-as-final; decode/verify second exact
   commit/rollback/removal successor?** YES. On the success path
   (`lib.rs` lines 742-773), prefill sets
   `releases.sequence_table_generation = reservation_generation` and applies
   no second delta (the reservation already appended the exact committed
   prompt tokens); decode/verify build one
   `PageTableDelta::between(reserved, post_commit_and_removal,
   reservation_generation, reservation_generation + 1)` that carries both
   the tentative-commit and any terminal removals, and apply it to all four
   mirrors with full receipt verification before any host publication.
10. **Scheduler completion preflighted on a clone before the irreversible
    post-output delta?** YES. `committed_scheduler = self.scheduler.clone()`
    and `complete_batch_with_results(true, &completions)` run on the clone
    (lines 733-741) before `apply_page_delta(commit_delta)`; the clone is
    adopted only after the rank delta succeeds. A preflight failure rolls the
    rank reservation back and fails the step with the authoritative
    scheduler untouched.
11. **Late host publication failure issues an explicit successor rollback
    and aligns host generation before retryable cleanup?** YES.
    `rollback_rank_reservation` (lines 1022-1039) builds an explicit
    successor delta from the reserved state back to the authoritative
    pre-step pages at `reservation_generation + 1`, applies it through the
    same four-receipt path, and assigns
    `self.sequence_table_generation = rollback_generation` so host and
    mirrors agree before `fail_selected_step` performs the (retryable,
    delta-acknowledged) terminal cleanup. Output-shape mismatch, commit
    failure, publication-planning failure, and scheduler-preflight failure
    all route through it. The late-prefix-release regression
    (`late_terminal_cleanup_failure_does_not_partially_publish_the_batch`)
    then proves the repaired path removes all three sequences through
    further acknowledged deltas with no partial publication.
12. **Worker/consensus failure closes the generation, preserves the original
    worker error, avoids forging a cleanup receipt?** YES, with MINOR-2 as
    the double-fault caveat. Any rank execution, malformed-output,
    rank-set, or consensus error breaks the dispatch loop, which drops the
    rank senders and joins all rank threads — the worker generation is
    closed and every later submission returns `Closed`.
    `fail_selected_step_after_worker_fatal` sets
    `releases.rank_synchronized = true` so host cleanup applies no second
    delta to the dead rank set (no forged receipt; commented explicitly at
    `lib.rs` lines 1011-1013) and returns the original worker error on the
    normal path. The `corrupt_generation`, divergence, and delayed-fail
    regressions confirm no cleanup receipt is fabricated and events/pages
    fail closed.
13. **Cancellation and terminal removal reach all mirrors before host page
    and prefix publication?** YES. `commit_request_releases` (lines
    1128-1157) applies the removal delta to the workers first (when not
    already rank-synchronized), and only then updates `self.active_pages`,
    the generation, the prefix-release commit, the lease/sampling/token
    maps, and the retained-byte count; `emit_terminal_transitions` pushes
    `Cancelled` events only after `commit_request_releases` succeeds. On
    the step path, terminal removals ride the same commit delta that the
    mirrors acknowledge before any event or prefix release.
14. **Initialization and standalone mutation reject overlap with an
    outstanding physical step?** YES. Both `initialize_page_table` and
    `apply_page_delta` return `Saturated` when `outstanding() != 0`
    (`worker.rs` lines 339, 359), and the quota is owned by the queued/
    running TP4 operation, not the response handle —
    `step_quota_is_owned_by_operation_after_handle_abandonment` proves an
    abandoned handle still blocks initialization until the operation drains.
    See Q-A for the (currently safe) non-atomicity of the check.
15. **Bound CPU output depends on input hash/request ID; custom serving
    executor observes exact seed/context on all ranks?** YES.
    `cpu_bound_output` hashes the domain, plan hash, schedule hash,
    `input.canonical_hash()`, per-row `request_id`, and the row index
    (`worker.rs` lines 845-876), so it cannot regress to the legacy
    plan-only token function (which hashes no input).
    `serving_delivers_exact_sampling_and_context_to_all_four_bound_executors`
    installs a custom `BoundInputRankExecutor` that fails the step unless
    seed 0xdead_beef_cafe_babe, context 64, zero generated/prompt tokens,
    and an empty prompt payload arrive, and asserts all four rank calls
    happened.
16. **Greedy seed survives HTTP admission without opening probabilistic
    sampling?** YES. `submit_chat` (`backend.rs` lines 334-391) rejects any
    non-greedy tuple with `SAMPLING_ABI_NOT_PROMOTED` before tokenization
    side effects, and constructs
    `StepSampling::greedy(request.sampling.seed.unwrap_or(request_id))` —
    an explicit seed is preserved, an omitted seed is materialized as the
    unique request ID. The seed flows through `BackendCommand::Submit` into
    `begin_admit_tokens_with_sampling`, is validated (`greedy` requires
    zero temperature bits, top-p = 1, top-k = 0, rng_counter_before = 0),
    stored per request, and delivered in the hashed `StepInput`.
    `probabilistic_requests_fail_closed_before_admission` pins the
    rejection.
17. **Corruption, late cleanup, 1M tail, MTP fallback, divergence, and
    multi-user regressions still meaningful under the new generation
    count?** YES. Each was re-read at this commit:
    `corrupt_generation_fails_closed_without_forging_rank_cleanup` (forces
    generation 0, expects `Delta(Generation)` with zero events and no
    forged rank cleanup); `late_terminal_cleanup_failure...` (injected
    prefix failure, no partial publication, repair then full removal
    through acknowledged deltas);
    `exact_one_million_context_is_admitted_accounted_executed_and_released`
    (1,048,575-token prompt, full page accounting to [4096;4] and back to
    zero, MTP variant, and the over-limit reject);
    `mtp_tail_falls_back_to_decode_at_the_request_generation_limit`
    (MTP6-configured request with one remaining token runs as decode —
    exercising `configured=6/effective=0`, which the amended contract and
    `configured_mtp6_tail_binds_to_effective_mtp0_reservation` also pin at
    the input layer); `rank_divergence_fails_every_row_in_the_selected_batch`
    (single-rank token flip fails consensus and every row);
    `multi_user_prefix_mtp_and_streaming_lifecycle_runs_end_to_end`
    (two tenants, cached prefixes, MTP and non-MTP, exact event counts,
    all pages released). The generation-count observability is directly
    asserted where it matters
    (`step_observation_captures_exact_graph_routes_bytes_and_host_split`
    asserts generations 2, 3, then 5 across admission, prefill-as-final,
    and decode's reserve+commit pair). Each test fails if its guarded
    behavior regresses; none is vacuous under the mirror transaction.
18. **284-test, 68-handoff, formatting, Clippy, FFI, deterministic proof
    claims reproducible?** YES, reproduced on this host at the pinned
    commit, with two honest qualifications recorded. (a) The full offline
    workspace suite passed 284/284, matching the claim exactly; Clippy with
    `-D warnings` is clean; `scripts/local-checks.sh` — formatting, the
    workspace suite, both Clippy passes, the CUDA FFI type checks
    (`cuda-ffi` feature check + clippy and the kernel-header syntax check),
    the deterministic proof regenerations against pinned fixtures, and
    `review-proof-all` — all passed with exit 0. The tokenizer proof was
    skipped (`GLMAXX_TOKENIZER_DIR` unset) and CUDA compilation skipped (no
    `nvcc`), matching the proof document's own disclosure. (b) The
    "68 then-present review handoffs" and "0/49 configured result
    artifacts" figures were true of the implementation commit `e1d51ce`
    named in the proof document; at the pinned review candidate
    `review-proof-all` verifies 69 review handoffs and 0/50 configured
    results, all passing — growth since implementation, not inaccuracy.
    (c) The reproducibility of the serving-suite gate is nondeterministic
    at the margin: the known flaky metrics test passed 10/10 here, but
    MINOR-1 is a real race that will intermittently fail
    `cargo test -p glm-serving` on other runs. The gate counts themselves
    are accurate.
19. **Clone allocation, missing device receipt/quarantine/cache-only/RNG
    output, and every model/quality/capacity/performance exclusion
    accurate?** YES. Verified against the code: the authoritative table and
    every mirror apply clone (`self.active_pages.clone()` per step;
    `PageTableMirror::apply` clones the candidate) and the delta path
    allocates owned vectors — exactly as the proof's exclusions state, and
    the fixed-capacity hot path remains the serving-page transaction's
    open item. No upload event, stream dependency, physical-ID quarantine,
    or direct tier transfer is claimed or present. `CACHE_ONLY` is
    genuinely unresolved (`StepInput` rejects it, `StepInputError::Mode`)
    and disclosed. Probabilistic sampling is fail-closed at the HTTP
    boundary because `StepOutput` carries no final RNG counter — accurate.
    No model output, quality, live-tier capacity, or performance claim
    appears anywhere in the proof. On hot-path complexity: the only
    superlinear structure found is the per-row `find` over delta updates in
    `StepInput::validate_shape` (O(rows x updates), bounded 64x64) and the
    full-mirror revalidation per apply — both bounded, disclosed as
    CPU-tier clone/alloc behavior, and excluded from performance claims;
    no unbounded quadratic hot-path work exists.

## Eight acceptance statements

1. **Mirror initialization and every mutation are generation-exact:** YES.
   Nonzero initial generation on all four mirrors before the coordinator
   exists; every delta requires `generation_before == mirror generation`
   exactly and `generation_after == generation_before + 1`; admission,
   reservation, commit, rollback, and removal each advance exactly one
   acknowledged generation; stale, duplicate, and zero generations fail
   closed with untouched mirrors.
2. **Immutable compute input and reservation are identical across ranks:**
   YES. One `Arc<StepInput>` and one `Arc<PageTableDelta>` are shared by
   all four rank commands, with no mutation path after dispatch, and every
   rank independently re-verifies the canonical hash and delta digest of
   the shared allocations.
3. **Global and local receipts are complete and correctly compared:** YES.
   Eight-field receipts; exact rank set; common-field equality across
   ranks; rank-specific expected local digests; local digests correctly
   NOT required equal across ranks.
4. **Commit, rollback, cancellation, and terminal removal are ordered
   safely:** YES. Mirrors acknowledge before any host page, prefix,
   scheduler, or event publication on every path; prefill's reservation is
   final; decode/verify use one exact commit successor carrying removals;
   rollback is an explicit acknowledged successor that aligns the host
   generation before cleanup.
5. **Fatal worker errors cannot become false cleanup success:** YES. A
   failed worker generation is closed permanently; host cleanup on the
   fatal path applies no second command to the dead rank set and forges no
   receipt; the original error is returned (with the MINOR-2 double-fault
   diagnostics caveat, which never converts a failure into success).
6. **Prompt/sampling state reaches explicit bound executors exactly:** YES.
   Canonical prompt vector with cursor-derived slices, one canonical
   `StepSampling` per request retained through admission, hashed into
   `StepInput`, and verified by a bound custom executor on all four ranks.
7. **All regressions and gate counts are accurate:** YES. All six named
   regressions distinguish their guarded behavior at this commit; 284/284
   workspace tests, clean Clippy, and the full local-checks gate all
   reproduced (exit 0); the known serving-suite flake (MINOR-1) is a real race
   in a metrics assertion that did not reproduce in ten runs and does not
   touch transaction accounting; the 68-handoff figure is accurate for the
   implementation commit it describes (70 now present, all passing).
8. **Every device/model/performance exclusion is accurate:** YES. CPU
   metadata mirrors only; no upload/stream/graph/quarantine/tier/
   checkpoint/model/quality/capacity/performance claim is made, and the
   clone-allocation and CACHE_ONLY limitations are disclosed exactly where
   they exist in the code.

## Architecture & maintainability

This is the strongest integration step in the serving line so far, because it
closes the gap the earlier rank-executor review (row 67) flagged as the core
risk: host state and rank state can no longer drift silently. Every mutation
of the active table — admission, reservation, commit, rollback, cancellation,
removal — is one canonical `PageTableDelta` with a forced exact-successor
generation, applied to four persistent mirrors whose acknowledgments are
verified field-by-field before the host publishes anything. The
publish-after-receipts discipline is applied uniformly rather than per-path,
which is what makes the failure matrix reviewable at all. Three design
choices deserve explicit credit: making the plan-only submission compile-time
test-only rather than policy-only; making prefill's reservation its final
page state (removing a whole class of second-commit failure modes); and
preflighting the scheduler on a clone so the irreversible rank commit is the
last fallible act before adoption. The main structural debts are already
named in the pinned contracts: the clone-per-step authoritative table and
clone-per-apply mirror are proof-tier implementations that the fixed-capacity
hot-path API must replace, and the mirror receipt attests delta identity
rather than post-apply state (Q-B), which the device tier's upload
acknowledgment must strengthen. The `tick_observed` error ladder is verbose
(seven near-identical rollback arms) and would benefit from a small
`StepTransaction` guard type before the CUDA executor lands on it; MINOR-2 is
the kind of defect that refactor would eliminate by construction.

## Token decision

Findings: 0 BLOCKER, 0 MAJOR, 2 MINOR, 2 QUESTION. Provenance verified clean
at start and finish; `review-proof` PASS; all eight acceptance statements are
YES; every blocker and major is vacuously resolved. The two MINOR findings
(a metrics-counter ordering race behind the known flaky test, and error-
identity loss in a doubly-failing cleanup corner) do not withhold acceptance
under the handoff's criterion and should be fixed in the next routine change
to `backend.rs`/`lib.rs`.

The requested token is emitted. It accepts only this integrated CPU
mirror/step transaction; it does not open cn4, authorize CUDA work, or accept
production serving.

rank-mirror-step-transaction-v1-accepted
