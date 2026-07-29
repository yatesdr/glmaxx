# Backend admission rollback fatal-drain CPU proof v1

Date: 2026-07-29

Implementation commit:
`6050d8ecfb3164ac50a3cb14e51fa26fb4e3eed8`

Status: CPU/control-plane correction passed; independent review pending

GPU claim: none

## Defect and ownership boundary

The coordinator now retains a pending admission when cache rollback cannot be
completed safely. The API backend previously ignored that distinction.

On every `poll_admission` error it removed the request ID from the backend
pending set and failed the user request. The active-request record and
external owner registry disappeared even when `ServingCoordinator` still
owned:

- the pending admission;
- its prompt token buffer and byte reservation;
- the cache pending restore record; and
- one or more live restore handles.

The cancellation command had the same defect in a different order: it
removed the backend pending ID before calling coordinator cancellation, then
failed and forgot the user whenever cancellation rollback was blocked.

The runtime could continue accepting work with a coordinator-owned request
that no backend registry could poll, cancel, or attribute. Reuse of that
request ID would fail inside the coordinator, and its cache/prompt resources
could remain retained for the life of the process.

## Corrected propagation

`ServingCoordinator::has_pending_admission` exposes only request ownership;
it does not expose or mutate cache internals.

The backend now classifies admission poll errors as follows:

```text
coordinator no longer owns request
    -> remove backend pending ID
    -> fail that request normally

coordinator still owns request
    -> preserve backend pending ID
    -> preserve active user and external owner
    -> return ADMISSION_ROLLBACK_FAILED
    -> runtime enters fatal drain
```

Cancellation removes the backend pending ID only after coordinator
cancellation succeeds. If cancellation fails while coordinator ownership
remains, it preserves every backend registry and returns
`CANCELLATION_ROLLBACK_FAILED`.

Both nonblocking command-consumption paths propagate that fatal result. The
runtime sets its fatal flag before draining all active and queued requests
with the structured code and returning. The coordinator is then dropped with
the runtime thread rather than left live with unattributed work. The outer
thread guard also clears the external owner registry.

## Why fail-stop

A retained coordinator admission after poll failure means its rollback
preflight found a broken residency/pin invariant. The lower coordinator
keeps the exact state retryable for diagnostics and controlled repair, but
the production API backend has no authority or mechanism to repair physical
cache metadata.

Treating this as an ordinary request-local error would silently continue from
corrupt global cache state. Busy-loop retry would consume the coordinator
thread indefinitely. The safe production behavior is therefore a structured
fatal drain that preserves ownership until the coordinator is destroyed.

This policy is limited to errors for which
`has_pending_admission(request_id)` remains true. A restore error whose
rollback completed and removed cache ownership remains an ordinary
request-local admission failure.

## Distinguishing CPU proof

`retained_admission_rollback_forces_fatal_signal_without_losing_owner`
publishes and begins a real file-backed one-page prefix admission through the
same backend command helper used by the runtime. It then aborts the exact
restore identity behind the coordinator and waits for the worker result to
reach the corrupted state.

The admission-poll helper returns `ADMISSION_ROLLBACK_FAILED` and proves all
of the following are unchanged:

- backend active request;
- backend pending-admission ID;
- mutex-protected external tenant owner;
- serving pending admission;
- cache pending restore; and
- the exact 256-byte retained prompt reservation.

It then submits a cancellation command against the still-corrupt state.
That path returns `CANCELLATION_ROLLBACK_FAILED` and again proves all three
backend registries and coordinator ownership remain.

The old code fails both phases: poll removes/fails the user immediately, and
cancellation removes the pending ID before discovering rollback failure.

Finally, the test repairs the exact restore identity and repeats cancellation.
It proves coordinator and backend pending state disappear, prompt bytes
return to zero, the active/owner records are removed only when the
cancellation event is dispatched, and the client receives exactly the
structured `REQUEST_CANCELLED` completion.

Existing fatal-step coverage independently proves runtime fatal drain fails
active and queued users with structured events and clears the external active
count. This correction reuses that same `fail_all` path for retained
admission/cancellation rollback errors.

## Gate result and exclusions

The full local gate passed 244 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, and all 42 then-present review
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
8b4f34d771374c9f442d69ba98c6ca29c501cd6f3644fef417923b258084d30a

crates/glm-serving/src/lib.rs
3797647a8535b8a8ca80efd76b4d91407330e3147e5c8e3e0a728b5005043e11

crates/glm-serving/src/cache.rs
e7686a678537b1644608f655e0a8bd40133e0772e0cdf12865c02bbefb15b54b

docs/pending-admission-rollback-proof-v1.md
cfd008dacc26f7d82c3f524ad7347da9d492168ebd9e53bb255bdc6cbcbfddfd

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, or model execution
was used.

This correction covers CPU API-backend ownership and fatal propagation for
pending admission rollback. It does not add automatic cache-metadata repair,
process restart supervision, active sequence page tables, private target or
draft tails, rank page-table deltas, physical-ID quarantine, direct tier I/O,
CUDA execution, model quality, or serving performance evidence.
