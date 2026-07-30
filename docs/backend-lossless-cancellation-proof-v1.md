# Queue-independent backend cancellation CPU proof v1

Date: 2026-07-29

Implementation and regression commit:
`f56e0bc03dfd12fd2d5f8f03da1a57d5b66e5dcf`

Status: CPU/control-plane correction passed; independent review pending

GPU claim: none

## Defect and delivery invariant

Public `CoordinatorApiBackend::cancel` previously sent a `Cancel` command
through the same bounded channel as new submissions. If that channel was
full, cancellation returned `ENGINE_OVERLOADED`. An HTTP deadline,
disconnect, or explicit request cancellation could therefore abandon its
completion receiver while its already-accepted request remained queued or
active in the runtime.

The corrected backend records cancellation in an owner-bound in-process
registry:

```text
authenticated active owner
    -> fatal/shutdown recheck while retaining the owner gate
    -> insert Requested, or coalesce an existing marker
    -> return success

runtime turn
    -> consume at most the configured command quota
    -> dispatch at most the same cancellation quota
    -> poll pending admissions
    -> execute the next collective-safe scheduler tick
```

Cancellation no longer competes for submission-channel capacity. The
registry contains at most one entry for an already-accepted request, so its
cardinality cannot exceed the existing owner registry. It is not an
unbounded command channel.

## Ownership and failure behavior

The public path locks request owners before cancellation markers. The
runtime uses the same `owners -> cancellations` order. A marker for a submit
command that is still queued remains `Requested` while its authenticated
owner exists. Once that submit becomes active, the runtime dispatches
`ServingCoordinator::cancel` before admission polling or another scheduler
step.

An accepted dispatch becomes `Dispatched`, coalescing repeat public calls
without repeatedly cancelling the coordinator request. Once terminal
cleanup removes the owner, the marker is pruned. Active requested markers
are selected before stale-marker pruning, and work per runtime turn remains
bounded by `maximum_commands_per_tick`.

Registry poisoning, owner/active disagreement, or coordinator cancellation
failure remains a structured runtime-fatal condition. The existing retained
active record and owner are preserved on immediate cancellation failure, and
the fatal drain handles every active and queued request. Runtime teardown
clears both registries.

## Distinguishing CPU proof

`cancellation_survives_a_saturated_submission_queue` constructs a
one-element command channel and permits one command per runtime turn. Four
rank executors block the first request inside its first physical TP4 step.
While that step owns the runtime thread, a second request fills the only
submission slot but has not yet entered the runtime active map.

Two cancellation calls for the queued request both succeed and coalesce to
one `Requested` marker. Releasing the physical step proves:

- the queued submit is consumed;
- its retained marker is dispatched before that request can execute;
- the queued request receives exactly `REQUEST_CANCELLED`;
- the peer completes normally at its one-token length limit;
- the delivered marker is pruned;
- no active backend owner remains; and
- no rank or global runtime failure is introduced.

The previous implementation attempts `try_send(Cancel)` while the one-slot
channel contains the second `Submit`, returns `ENGINE_OVERLOADED`, and fails
the first cancellation assertion for the claimed reason.

The distinguishing schedule passed ten consecutive focused runs. All 39
`glm-serving` tests also passed.

## Gate result and exclusions

The full local gate passed 269 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, and all 63 then-present review
handoff provenance proofs.

Commands:

```text
cargo test --offline -p glm-serving \
  backend::tests::cancellation_survives_a_saturated_submission_queue
cargo test --offline -p glm-serving
cargo clippy --offline -p glm-serving --all-targets -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-serving/src/backend.rs
8c9ec8fc9ce37f6d3261940c95d8efae802fa1d95c4095b8f305c3e91dc16078

crates/glm-serving/src/lib.rs
c3ed1b80f5b0561626fc0e61426054a76f9e507b095e0f136accb29ac26a0c07

docs/backend-admission-rollback-fatal-proof-v1.md
fd9c2e12a09af096afcf13dafc282e847c8984776a8ea70ef285d9d791866499

docs/backend-event-cancellation-fatal-proof-v1.md
04794fb247b103e90d03a07e9827f13ce82d89e0a50dccb543c5e010f0f9bde5

docs/retained-http-request-ownership-proof-v1.md
83616d0878a4177d0df2e268d36de02a58d1380361cff42bda414178ce8c0971

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, or model execution
was used.

This correction proves logical cancellation delivery while the in-process
runtime remains healthy, including submission-queue saturation. It does not
interrupt a physical collective already in progress. Dispatch latency is
bounded by the current collective-safe physical step plus runtime polling;
an idle runtime may wait up to its configured idle poll interval before
observing the marker. It does not add syscall cancellation, eventfd wakeup,
the final nonblocking HTTP transport, process supervision, checkpoint
execution, model quality, or serving-performance evidence. A disconnected
client may not receive its terminal event even though its model work is
cancelled.
