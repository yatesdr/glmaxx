# Selected-step failure finalization CPU proof v1

Date: 2026-07-29

Implementation commit:
`11bb8939aeb72c2b732d963848b4ebe53f926bbb`

Status: CPU/reference correction passed; independent review pending

GPU claim: none

## Defect and invariant

`ServingCoordinator::tick_observed` selects a batch through
`Scheduler::next_batch`, which installs that batch as the scheduler's sole
inflight step. Several later operations previously used an immediate `?`
return:

- graph lookup;
- step-plan compilation;
- collective-count conversion;
- starting-progress lookup;
- worker submission; and
- successful completion validation.

An error from any of those operations returned while the selected batch
remained inflight. Every subsequent tick then failed with
`SchedulerError::Inflight`; the affected requests were neither runnable nor
terminal.

Every error after successful selection and before successful scheduler
completion now enters one finalization path:

```text
selected batch
    -> graph/compile/observe/submit/receive/output/commit error
    -> atomically complete the exact inflight scheduler batch as failed
    -> release request-owned prefix/token resources
    -> emit one Failed event per selected row
    -> return the original step error when finalization succeeds
```

No normal error branch can return while retaining the selected batch in
`Scheduler::inflight`.

## Scheduler boundary

`fail_selected_step` first calls `Scheduler::complete_batch(false)`. The
separate scheduler batch-atomicity proof establishes that this operation
preflights every row before removing the inflight batch and changing all
selected requests to `Failed`.

The selected batch is immutable and the serving coordinator has exclusive
access between selection and failure finalization. Graph lookup,
compilation, observation, and worker submission do not remove scheduler
requests. Therefore failure completion is valid for every reachable branch;
an error there indicates an internal invariant violation and remains
fail-closed.

The event queue reserves `MAXIMUM_STEP_EVENTS == 512` slots before
selection, while one legal batch has at most 64 rows. Failed-event capacity
cannot be exhausted by this path. Prefix/token release can still report a
cache invariant error, but it runs only after scheduler finalization, so
such an error cannot resurrect or strand the inflight step.

## Covered failure routes

The common finalizer is used for:

- a missing selected graph;
- graph/route/plan compilation failure;
- collective observation count overflow;
- a selected request missing its starting progress;
- worker-pool saturation, closure, or plan rejection before submission;
- worker receive, rank execution, rank output, ordering, or consensus
  failure;
- a worker output that does not fit the selected requests; and
- a failure from atomic successful-completion validation.

The success path remains unchanged after
`complete_batch_with_results(true, ...)` succeeds.

## CPU proof

Two new distinguishing regressions force failures which the old code did not
finalize:

1. `compile_failure_fails_selected_rows_without_stranding_inflight` sets the
   sequence-table generation to the compiler's invalid zero value. The first
   tick selects a real prefill batch and then returns `Compile(Batch)`. The
   request is `Failed`, exactly one failed event exists, and the next tick is
   idle rather than returning `Scheduler(Inflight)`.
2. `submit_failure_fails_selected_rows_without_stranding_inflight` occupies
   the only bounded worker slot before a real prefill is selected. Submission
   returns `Worker(Saturated)`. The request is `Failed`, exactly one failed
   event exists, and after releasing the test reservation the next tick is
   idle rather than returning `Scheduler(Inflight)`.

Existing worker-fault and invalid-output tests continue to prove the
post-submission failure routes.

The full local gate passed 239 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, and all 39 then-present review
handoff provenance proofs.

Commands:

```text
cargo fmt --check
cargo test -p glm-serving
cargo clippy -p glm-serving --all-targets -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-serving/src/lib.rs
1aee2f7fccac9c79124edaeeef0f9759faed2bc621b3e7228ce9914eb4e432d6

crates/glm-scheduler/src/lib.rs
5a820b2e5013f038f07b26f14ddc24d69d00e18d3d55837ab5ff3a68daee3074

crates/glm-engine/src/worker.rs
400c7c22f2c74d3f386fefe2d144da3437f27e6c99ce9f2d4bbf87ffe98fe437

docs/offline-serving-spine.md
27b24d4cbafc8203937d3620e7bcd85d47fcb86cc4d8b89e237025e5d40a62f9

docs/serving-page-transaction-v1.md
e3a9a1d9f2eb26dc5312d7c42297fa3d832e444f7e3f269094746a85fb3deac2

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, or model execution
was used.

This correction does not make post-success event publication or cleanup for
multiple terminal requests one transaction. It also does not implement the
wider serving page/rank undo log, active page-table mutation, direct tier
I/O, device execution, process-crash recovery, model quality, or serving
performance. In particular, a cache error while emitting several terminal
rows may still leave some earlier rows released; that separate
cross-request cleanup boundary remains open.
