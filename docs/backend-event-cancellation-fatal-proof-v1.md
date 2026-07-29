# Backend event-cancellation fatal propagation CPU proof v1

Date: 2026-07-29

Implementation commit:
`fa8e3b345c1a8ffe0abff2006c40950bd21037cb`

Status: CPU/control-plane correction passed; independent review pending

GPU claim: none

## Defect and invariant

Backend event dispatch removes an `ActiveRequest` temporarily while it
validates and decodes each event. Several abnormal or client-terminal paths
then request collective-safe coordinator cancellation:

- duplicated or oversized admission event;
- invalid prefill progress;
- invalid output position/speculation metadata;
- slow completion receiver;
- decoder stop-string completion; and
- output decoder failure.

Those paths previously ignored every `ServingCoordinator::cancel` result.
They failed or completed the user and removed the external owner even when
the coordinator had rejected cancellation. The scheduler could then continue
executing a request for which the backend had no decoder, event channel, or
tenant owner.

Event cancellation now follows:

```text
remove active record for validation/decoding
    -> request coordinator cancellation
    -> on error, reinsert the exact active record
    -> return EVENT_CANCELLATION_FAILED
    -> runtime fatal-drains all users

coordinator accepted cancellation
    -> only then publish backend failure/success
    -> only then remove external owner
```

No mismatch, stop-string, decoder-error, or slow-client path can discard its
active record before cancellation is accepted.

## Runtime propagation

`dispatch_events` now returns a structured error. Both ordinary dispatch
sites set the fatal flag, drain active and queued users through `fail_all`,
and return from the runtime thread on an error.

When rank/step execution has already failed, pending coordinator events are
still dispatched first. If that dispatch also discovers a cancellation
failure, the more specific `EVENT_CANCELLATION_FAILED` code is used for the
fatal drain; otherwise the existing `ENGINE_STEP_FAILED` code is retained.

The reinsertion refuses to overwrite a request identity that unexpectedly
reappeared under exclusive backend access.

This correction covers immediate coordinator cancellation rejection. An
accepted scheduler cancellation is still applied at the next collective-safe
tick boundary. If its later resource cleanup fails, the existing
`tick_observed` error path fatal-drains and drops the coordinator.

## Distinguishing CPU proof

`dispatch_cancellation_failure_preserves_request_for_fatal_drain` admits a
real backend request and dispatches its valid admission event. It then forces
the coordinator generation counter to `u64::MAX` and supplies an invalid
token position.

The mismatch path attempts cancellation, receives the deterministic
generation-overflow error, and proves:

- `EVENT_CANCELLATION_FAILED` is returned;
- the exact active request was reinserted;
- the external tenant owner remains;
- the scheduler request remains observable; and
- no user completion was emitted.

The old path ignores the overflow, fails and drops the active request, and
removes the owner, so it fails these assertions for the claimed reason.

The test then repairs the generation, submits cancellation again, advances
the collective-safe idle boundary, dispatches the cancellation event, and
proves exact active/owner removal plus one structured
`REQUEST_CANCELLED` client completion.

The existing slow-client, stop-string, decoder-error, fatal-step, and
concurrent-tenant tests remain green through the common helper and new
fallible dispatch boundary.

## Gate result and exclusions

The full local gate passed 245 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, and all 43 then-present review
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
crates/glm-serving/src/backend.rs
d4c1b2daaa6f6952d3c27158d33a0123abd891cef09ec894da006af8d7d7f8b0

crates/glm-serving/src/lib.rs
3797647a8535b8a8ca80efd76b4d91407330e3147e5c8e3e0a728b5005043e11

docs/backend-admission-rollback-fatal-proof-v1.md
fd9c2e12a09af096afcf13dafc282e847c8984776a8ea70ef285d9d791866499

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, or model execution
was used.

This correction covers CPU backend event-dispatch ownership and immediate
cancellation-error propagation. It does not make user-event dispatch atomic
across different requests, add process supervision, implement active page
tables or physical-ID quarantine, execute CUDA, or establish model quality
or serving performance.
