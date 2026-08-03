# Fable adversarial review: normative startup order v1

Date: 2026-07-31

Reviewer: Fable (adversarial CPU prerequisite review; no GPU/cn4 work)

Handoff: `docs/fable-normative-startup-order-v1-handoff.md`

Reviewed candidate commit (detached worktree; no modification, no commit):

7420657a8528ef2ed780974bb0b8a699db9cfb0f

Result-path note: the handoff requested `fable-normative-startup-order-v1.md`
at the repository root. The operator directed reviews into `docs/reviews/`;
this artifact is written there under that directive.

## Provenance

All pinned inputs were hashed with `shasum -a 256` in the detached worktree at
review start and re-hashed at review finish; both sets matched the handoff
table exactly. No stale candidate. The only non-pinned file in the worktree
was a byte-identical copy of the handoff (verified against the main tree,
SHA-256 `50c11aeb7cac158de53f723717b0a42fea7d689d8e872b5c992cc1d17f0a76f8`)
required by `review-proof`, which returned `"verdict": "PASS"`.

| Input at candidate commit | Pinned = Start = Finish SHA-256 |
|---|---|
| `spec/engine-v0.md` | efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a |
| `docs/checkpoint-load-transaction-v1.md` | 79d9c376201f3540f247344c24c37dd7d819d629f10459899a73c15f8b27015f |
| `crates/glm-engine/src/startup.rs` | 54d41acc810c90cc49fe4acc0623b6a13bb2c09b72b2f8e5fb6615250ead2ddd |
| `crates/glm-engine/src/lib.rs` | b3ca0da8e0e61f05a92a3b15bc9dc7822395545733ebbdc270c9ff1fb21d6a54 |
| `crates/glm-serving/src/backend.rs` | c1f9e9d06b44674a1d1d0ef3c24553a9ebe63e913805946bff7c2780233fe94b |
| `docs/normative-startup-order-proof-v1.md` | a46f88464b030a348b2041581ce63f620770fc854ff4a889a341ab383c4d9c27 |
| `docs/production-punchlist.md` | 002edf6e86679aefab6507a465b99db2ff02d9e984c9f101bcf9304daef5038c |
| `docs/results-index.md` | c986587f513a9dc1b30621aead73dca7b40c2462704f257ad2eaac3c4e6fd5cc |
| `scripts/local-checks.sh` | 839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f |

Evidence commands executed in the worktree: `review-proof` (PASS);
`cargo test --offline -p glm-engine` (48 passed);
`cargo test --offline -p glm-serving` (41 passed);
`cargo test --offline --workspace` (295 passed — matching the proof's
workspace count); `cargo clippy --offline -p glm-engine -p glm-serving
--all-targets -- -D warnings` (clean). Cross-referenced review:
`docs/reviews/fable-sm120-rank-executor-v1.md` (row 67, withheld).

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

**MINOR-1 — A pinned serving test is nondeterministic (metrics-lag race).**
`backend::tests::concurrent_tenants_complete_with_exact_lifecycle_totals`
(crates/glm-serving/src/backend.rs:2191, assertion at :2227) fails
intermittently (~1 in 5-10 runs observed) with
`missing metric: glmaxx_backend_completed_total 4`. The test drains each
handle's terminal `Finished` event and then immediately asserts the
aggregate counters; the runtime publishes the client-visible terminal event
before the completion counter increment becomes visible, so a fast reader
observes completion with lagging metrics. This is outside this handoff's
review boundary (lifecycle metrics, not startup order or health admission),
does not affect any correctness invariant under review, and the proof's
"41 glm-serving tests passed" is an accurate report of its recorded run —
but the serving gate is not deterministic and the event-before-counter
ordering should either be inverted or the test should poll with a deadline
like its sibling `slow_completion_receiver_is_cancelled_without_blocking_runtime`.
This must also be weighed by the pending rank-mirror step-transaction review,
whose boundary does include this file.

**MINOR-2 — The mock coordinator has no receive timeout; a wedged or
panicked worker hangs rather than erroring.** `run_coordinator`
(startup.rs:191-209) performs four blocking `recv` calls per stage. The
shipped `mock_worker` has no panic path, so this is unreachable in the
current tests, but if a worker thread panicked mid-stage its channel clone
would drop while three live senders keep `recv` blocking forever: the mock
would hang instead of returning `StartupError::WorkerPanic`. Fail-closed in
the sense that `Healthy` is unreachable, but the liveness gap should be
noted so the real coordinator (rank-executor design) does not inherit a
timeout-free barrier.

### QUESTION

**Q-1** — `StartupState` derives `Ord`/`PartialOrd` with `Failed = 255`, so
any future code writing `state >= StartupState::Healthy` would treat
`Failed` as past-healthy. The only current consumer compares with exact
equality (`backend.rs:207`, correct), and `NORMATIVE_ORDER` excludes
`Failed`; is the ordering derive actually needed, or should it be dropped to
make the misuse unrepresentable?

## Answers to the twelve required adversarial questions

1. **NORMATIVE_ORDER exact?** Yes. The 11-element array
   (startup.rs:26-38) is `Created, HostValidated, CudaContextsReady,
   TopologyValidated, ModulesReady, MemoryPlanned, WeightsLoaded,
   GraphsCaptured, KvReady, CollectivesVoted, Healthy` — character-for-
   character the engine-v0 section 7 sequence, with no omission,
   duplication, or transposition. The test
   `startup_order_exactly_matches_the_normative_engine_sequence` also proves
   the successor chain reproduces the array.
2. **successor exact; Healthy and Failed terminal?** Yes. `successor()`
   (startup.rs:40-54) encodes the identical chain and returns `None` for
   both `Healthy` and `Failed`. At `Healthy`, `advance` returns
   `Terminal` without state change (success-terminal); at `Failed` it
   returns `AlreadyFailed` (failure-terminal).
3. **Created start, immediate-successor-only, exact rank set?** Yes. `new()`
   starts at `Created`; `advance` computes exactly one successor and
   requires every report's `reached` to equal it; the rank set is checked as
   `BTreeSet == {0,1,2,3}` plus `reports.len() == 4`, so duplicates,
   missing ranks, out-of-range ranks, and wrong counts all fail
   (`RankCount`/`RankAgreement`) and poison the coordinator.
4. **MemoryPlanned before WeightsLoaded on every successful path?** Yes,
   structurally: the only transition into `WeightsLoaded` is
   `MemoryPlanned.successor()`, and `advance` only ever moves to the
   immediate successor, so no successful path can bypass memory planning.
5. **Distinguishing obsolete-order regression?** Yes.
   `obsolete_weight_before_memory_sequence_fails_closed` advances through
   `ModulesReady`, submits `WeightsLoaded` (the obsolete
   weights-before-memory transition), observes `Err(RankAgreement)`, and
   asserts terminal `Failed`. Under the obsolete seven-stage machine
   (`Cold -> ContextReady -> InventoryVerified -> WeightsLoaded ->
   MemoryProved -> ...`) that submission would have been legal, so the test
   distinguishes.
6. **Any recoverable/healthy escape?** No. Wrong stage → `RankAgreement` +
   `Failed`; a rank `Err` → `Failed` via `inspect_err` with the original
   worker error preserved; wrong count → `RankCount` + `Failed`; a zero
   digest in the consensus reference → `DigestAgreement` + `Failed` (a zero
   digest on a non-reference rank differs from the nonzero reference and
   fails the same check); a digest that changes across stages →
   `DigestChanged` + `Failed` via the stored consensus; channel failure
   surfaces `StartupError::Channel` and the mock never reports healthy;
   worker panic is surfaced at join as `WorkerPanic` (with the MINOR-2
   liveness caveat for a mid-stage panic). `a_failed_coordinator_never_
   recovers` proves poisoning is permanent (`AlreadyFailed`).
7. **Mock traverses all ten transitions?** Yes. `run_coordinator` loops
   `while successor exists`, which is exactly ten advances from `Created` to
   `Healthy`; `four_rank_mock_reaches_healthy` asserts the result. No
   seven-stage path remains anywhere.
8. **Discriminant collision/wrap/Failed-as-stage?** No. Discriminants are
   0..=10 plus 255 — unique, non-wrapping in `u8`, and `Failed` is neither
   in `NORMATIVE_ORDER` nor reachable via `successor`. No arithmetic or
   indexing on discriminants exists. See Q-1 for the latent `Ord` misuse
   surface (currently unused).
9. **Stale variant/discriminant references?** None. Workspace grep for
   `ContextReady`, `InventoryVerified`, `MemoryProved`, `GraphsReady`,
   `CollectivesReady`, and `Cold` returns nothing; the only `StartupState`
   consumers outside `startup.rs` are the `lib.rs` re-export and
   `backend.rs:207`.
10. **Serving admission exact?** Yes. `backend.rs:207-208` rejects unless
    `startup.state() != StartupState::Healthy` fails — an exact-equality
    gate (`EngineNotHealthy`), not an ordering comparison, so `Failed=255`
    cannot pass. The runtime health check
    (`is_production_healthy`) additionally gates request admission.
11. **Counts/hashes/exclusions accurate?** Yes with one caveat recorded as
    MINOR-1. Reproduced: 48 glm-engine tests, 41 glm-serving tests, 295
    workspace tests — all matching the proof exactly; the three
    implementation hashes in the proof match the pinned table; the
    no-GPU/no-nvcc and tokenizer-skip exclusions are consistent with this
    host. The glm-serving suite is nondeterministic due to the out-of-scope
    metrics-lag test (MINOR-1); every startup-scoped test is deterministic
    across all probe runs.
12. **Properly a prerequisite?** Yes. The proof's exclusion section states
    it proves only the CPU coordinator's stage order and fail-closed
    transitions and explicitly disclaims checkpoint-load implementation,
    device arenas, contexts, graphs, KV, collectives, kernels, and real
    `Healthy`. Note for downstream work (not a defect of this candidate):
    the withheld rank-executor review's MAJOR-1 concerns resource-creation
    ordering *within* the engine-v0 stages; this candidate faithfully
    encodes the engine-v0 stage identities, which is exactly its scope.

## Eight acceptance statements

- State identities and normative order exactly match engine-v0: **YES**.
- Memory planning impossible to bypass before weight load: **YES**.
- Rank consensus and terminal failure remain fail-closed: **YES**.
- The obsolete-order regression is distinguishing: **YES**.
- Retained serving health admission remains exact: **YES**.
- No old variant/discriminant dependency survives: **YES**.
- Results and exclusions are accurate: **YES** (MINOR-1 records a
  nondeterministic out-of-boundary serving test; the proof's recorded counts
  and all startup-scoped results reproduced exactly).
- No CUDA/checkpoint-load/executor/smoke/production-health claim: **YES**.

## Architecture & maintainability

The correction is exactly the right altitude for a prerequisite: one public
fixed `NORMATIVE_ORDER` array as the shared stage identity, a `successor`
function that makes illegal transitions unrepresentable, and a consensus
step that folds rank identity, stage agreement, and immutable-digest
agreement into a single poisoning decision. Publishing the array (rather
than each later component re-deriving an order) is the load-bearing
maintainability choice — the checkpoint-load transaction and rank executor
can now bind evidence to these identities. The mock is honest about being a
contract exerciser, not an engine. Improvements worth making: drop the
`Ord` derive (Q-1), add a barrier timeout before the real coordinator
inherits this shape (MINOR-2), and fix the metrics-lag flake at its source
(MINOR-1, serving crate).

## Token decision

Findings: 0 BLOCKER, 0 MAJOR, 2 MINOR, 1 QUESTION. Provenance verified at
start and finish; `review-proof` PASS; all eight acceptance statements are
unqualified YES. The requested token follows.

normative-startup-order-v1-accepted
