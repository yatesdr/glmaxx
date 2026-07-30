# Coordinator API backend v3 CPU proof

Date: 2026-07-30

Status: implementation candidate; consolidated adversarial review required

GPU evidence: none

## Purpose

Fable's review of the v2 backend candidate at `8aaef8e` accepted the
single-mutex fatal/submission linearization but withheld the token for two
older ownership defects:

1. event-dispatch branches ignored a rejected coordinator cancellation; and
2. failed admission or cancellation rollback could be reported as
   request-local after the coordinator retained ownership.

It also identified a timing-dependent fatal-drain test and requested an
explicit owner-registry poison boundary.

Current source contains the two separately pinned ownership corrections and
this candidate closes both minor findings. This proof consolidates the
current bytes for one backend acceptance gate. It does not replace the
nonblocking HTTP transport, implement model execution, or establish GPU,
quality, sustained-load, or throughput evidence.

## Ownership state

The API and runtime share these bounded identities:

```text
owners[request_id]          authenticated external owner
cancellations[request_id]   requested or dispatched cancellation
pending_admissions          coordinator-owned admission in progress
active[request_id]          runtime decoder and completion sender
command queue               accepted but not yet runtime-owned submission
```

An API submission is acknowledged only after `owners` is installed and its
bounded command is published while the same owner mutex remains held. Fatal
drain stores fatal before taking that mutex, drains active plus accepted
queued submissions, and then clears valid ownership. Cancellation is
owner-bound and independent of command-queue capacity.

For every recoverable runtime transition, an acknowledged request therefore
ends in exactly one of:

- normal completion and owner removal;
- collective-safe cancellation and owner removal; or
- fatal drain with one best-effort structured failure and owner removal.

The completion sender is bounded and nonblocking. A full/disconnected
receiver becomes a slow-consumer cancellation; it cannot block the
coordinator.

## Event-cancellation failure

Current `dispatch_events` funnels every cancellation-required branch through
`cancel_dispatch_request`:

- duplicate or oversized admission;
- invalid prefill progress;
- invalid output position;
- invalid speculation metadata;
- completion receiver saturation/disconnect;
- decoder stop-string completion; and
- decoder failure.

The helper calls `ServingCoordinator::cancel` before removing the active
decoder/event sender. If cancellation fails, it reinserts the complete
`ActiveRequest` and returns a fatal `ApiBackendError`. Runtime fatal drain
then still owns:

- the request ID;
- authenticated tenant;
- decoder;
- completion sender;
- request start time; and
- owner-registry entry.

It cannot leave scheduler work executing without a backend owner. The
dedicated
`dispatch_cancellation_failure_preserves_request_for_fatal_drain` test
injects the rejected cancellation and proves the request remains active for
fatal draining instead of being forgotten.

This is the correction first pinned by `fa8e3b3`.

## Retained admission rollback

`poll_pending_admission` distinguishes an ordinary failed admission from one
whose coordinator still reports pending ownership. The retained case returns
`ADMISSION_ROLLBACK_FAILED` without:

- removing `pending_admissions`;
- removing `active`;
- removing `owners`; or
- sending a request-local terminal event.

The runtime classifies that result as fatal and enters the common
active-plus-queued drain. Likewise, cancellation processing may remove a
pending ID only after the coordinator proves rollback completed. A rejected
rollback remains attributed and becomes fatal.

`retained_admission_rollback_forces_fatal_signal_without_losing_owner`
injects this path and proves the exact pending ID, owner, active request, and
256 retained prompt bytes survive until the test repairs the injected state
and performs the identity-bound cancellation.

This is the correction first pinned by `6050d8e` and its review candidate
`3ab3110`.

## Deterministic fatal-drain schedule

The former `DelayedFailExecutor` used a 100 ms sleep. The test thread had to
submit three commands during that interval; host descheduling could let the
step fail first and make a later `submit_chat(...).unwrap()` observe
`ENGINE_NOT_HEALTHY`.

The replacement `GatedFailExecutor` uses two five-party barriers:

```text
four rank executors --entered--> test thread
test submits three queued requests
test thread --release--> four rank executors
all four ranks return the injected failure
```

The first barrier proves every physical rank is inside the same step before
the test submits queued work. The runtime is synchronously waiting for that
four-rank result and cannot perform fatal drain during queue construction.
Only after all three nonblocking submissions succeed does the test release
the ranks.

A `BarrierReleaseGuard` releases the second barrier during unwinding if any
post-entry assertion or submission panics. The backend can therefore join
its worker threads instead of converting a test failure into a deadlock.

The final guarded test passed 100 consecutive fresh-process invocations. The
same barrier schedule without the unwind guard passed 500 consecutive
fresh-process invocations before the guard was added. The guard changes only
failure cleanup, not the successful schedule.

The test still proves:

- request one gets `ENGINE_REQUEST_FAILED`;
- queued requests two through four get `ENGINE_STEP_FAILED`;
- all four receivers get one structured terminal event;
- fatal health becomes visible;
- submitted = failed = request-time observations = 4;
- completed = cancelled = successful prefill-step observations = 0; and
- active owners = 0.

## Poisoned owner registry

The structured-terminal guarantee is intentionally limited to recoverable
rank/backend failures while the owner registry is valid.

A poisoned owner mutex means Rust panicked while holding the sole external
ownership map. Its bytes may violate the invariant that owner, active,
pending-admission, cancellation, and queued-command state agree. `fail_all`
therefore does not recover the poisoned guard, inspect those bytes, or claim
exact terminal delivery from them. Active state already owned exclusively by
the runtime is failed, but queued receivers may observe channel disconnect
when the process-fatal runtime exits.

This state:

- publishes fatal health;
- admits no replacement work;
- requires process supervision and a fresh runtime generation; and
- is excluded from the recoverable exactly-one-structured-terminal claim.

`docs/coordinator-api-backend-v1.md` now states this boundary explicitly.
Weakening it into continued service or silently recovering a potentially
inconsistent map is forbidden.

## Metrics question

`glmaxx_backend_active_requests` derives from valid owner-registry
cardinality, so ordinary fatal drain reaches zero through `owners.clear()`.
`pending_admissions` is runtime-local bookkeeping, has no separately
published active gauge, and is destroyed when the terminal runtime loop
returns. It must not be described as a second externally visible active
count.

In the poisoned process-fatal case, exact post-failure owner cardinality is
not trustworthy and is not a healthy-service metric. Supervision replaces
the process; it does not reuse those counters as a new generation.

## Verification

Run:

```text
cargo fmt --all -- --check
cargo test --offline -p glm-serving
cargo clippy --offline -p glm-serving --all-targets -- -D warnings
```

Targeted final stress:

```text
for run_id in {1..100}; do
  cargo test --offline -q -p glm-serving \
    fatal_step_fails_active_and_queued_requests_with_structured_events
done
```

The targeted guarded schedule passed 100/100. The complete crate test and
Clippy gates must pass again at the pinned candidate.

## Scope boundary

This candidate proves only the current retained CPU backend's request
ownership, recoverable fatal drain, deterministic injected-failure schedule,
and process-fatal poison documentation. It does not accept or prove:

- the production nonblocking HTTP transport;
- startup deadline or liveness supervision;
- non-greedy distributed sampling;
- checkpoint-backed rank execution;
- model logits or text;
- MTP execution;
- sustained concurrency or slow-client scale;
- CUDA, SM120, checkpoint, quality, capacity, latency, or throughput
  evidence; or
- cn4 access.
