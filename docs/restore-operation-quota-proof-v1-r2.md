# Restore operation quota ownership CPU proof v1 r2

Date: 2026-07-30

Status: corrective CPU implementation candidate; independent rereview required

GPU evidence: none

## Scope

This correction responds to the findings recorded in
`docs/reviews/fable-restore-operation-quota-v1.md`. The first candidate moved
quota ownership from the response handle to an uncloneable permit carried by
the queued/running operation. R2 preserves that ownership and closes the two
release-build robustness gaps identified by the review:

1. an impossible permit underflow is now observable and permanently poisons
   further admission; and
2. an unexpected rollback-preflight failure after `begin_restore` retains a
   coordinator-owned recovery record instead of orphaning the page.

This remains the retained blocking CPU restore service. It is not the
production direct-I/O implementation.

## Release-visible quota poison

One atomic machine word contains:

```text
highest bit = poisoned
remaining bits = queued/running physical-operation count
```

The configured maximum cannot use the poison bit. Admission uses one
`fetch_update`:

- a non-poisoned count below the configured maximum increments exactly once;
- saturation leaves the word unchanged and returns `Saturated`; and
- poison leaves the word unchanged and returns `Poisoned`.

Permit release also uses one `fetch_update`:

- a positive count decrements while preserving an existing poison bit; and
- a zero count remains zero and sets the poison bit.

Consequently an impossible double release cannot wrap, cannot be silently
ignored in an optimized build, and cannot allow later work. `outstanding()`
continues to expose only the physical-operation count, while
`quota_poisoned()` exposes the invariant failure separately.

`quota_underflow_poison_is_release_visible_and_blocks_admission`
distinguishes the old release-build behavior. It deliberately releases a
permit against count zero, proves the count remains zero, proves poison is
visible, and requires every later reservation to return `Poisoned`.

## Recoverable failed-submission rollback

The prior submit-failure branch planned rollback with `?` before it had put
the just-reserved page into coordinator state. Under the current exclusive
ownership discipline its preflight could not normally fail. If a future
refactor violated that assumption, however, the early return could leave the
page `Restoring` with no pending record through which it could be cancelled
or repaired.

R2 gives that interval an explicit `RestoreReserved` state. If
`RestoreService::try_submit` fails:

1. the just-begun page is appended to the same `PendingRestore` as all prior
   pages;
2. the ordinary all-or-nothing rollback plan validates every pin and restore
   identity;
3. successful preflight commits the rollback and returns the original submit
   error; or
4. failed preflight installs the complete pending record before returning
   the rollback error.

A retained `RestoreReserved` record is never treated as a submitted I/O.
Polling it retries the same identity-bound rollback and returns `Busy` after
successful cleanup. If preflight still fails, the ordinary fail-polled path
reinserts the pending record. Coordinator ownership is therefore retained
until recovery succeeds.

`failed_submission_rollback_retains_recoverable_coordinator_state`
deliberately invalidates the just-begun identity before rollback. It proves:

- rollback reports `ResidencyError::State`;
- the request remains present in the pending map;
- repairing the exact request/page identity makes a subsequent poll clean up
  the reservation; and
- final state is no pending request and NVMe residency.

The former special-case helper returned on the failed preflight without
inserting pending state and fails this regression.

## Response-buffer boundary

The first review correctly observed that an operation slot is released after
read/hash completion and before delivery of its result. This quota measures
physical restore operations; it does not charge undelivered response
payloads. The production direct-tier contract separately preallocates and
generation-binds a fixed set of 2,052,096-byte registered, CUDA-pinned
buffers and retains buffer ownership until the terminal completion boundary.
That contract is specified in `docs/direct-tier-io-v1.md` and
`docs/direct-tier-durable-format-v1.md`; it is not implemented or accepted by
this CPU correction.

## Reproduced commands

The candidate passes:

```text
cargo fmt --all -- --check
cargo test --offline -p glm-cache residency::tests
cargo test --offline -p glm-serving cache::tests
cargo clippy --offline -p glm-cache -p glm-serving --all-targets -- \
  -D warnings
```

The exact filters report ten cache-residency tests and six serving-cache
tests. Both filters were then run fifty consecutive times:

```text
for run_index in {1..50}; do
  cargo test --offline --quiet -p glm-serving cache::tests >/dev/null ||
    exit 1
done
for run_index in {1..50}; do
  cargo test --offline --quiet -p glm-cache residency::tests >/dev/null ||
    exit 1
done
```

All one hundred invocations completed with exit code zero.

## Retained behavior

R2 preserves the first candidate's accepted properties:

- the quota counts queued/running physical reads, not response handles;
- handle timeout, disconnect, or drop cannot release an active operation;
- send failure, worker shutdown, or command unwind releases the operation
  permit exactly once;
- the worker retains its permit through complete read and SHA-256 work;
- permit release precedes result visibility;
- abandoned payloads are destroyed by the worker;
- request/ordinal-bound rollback prevents late adoption; and
- logical rollback is separate from physical-operation drain.

## Exclusions

This candidate does not prove or implement:

- syscall cancellation or production waiter deduplication;
- io_uring, registered files/buffers, direct I/O, or device storage;
- production response-buffer accounting;
- a persistent catalog, online publication, segment cleaner, or endurance
  accounting;
- actual HBM/DRAM allocation or copy;
- asynchronous CUDA event ordering;
- native SM120 rank workers, checkpoint/model execution, cold/warm reuse,
  one-million-token execution, quality, capacity, or performance; or
- cn4 authorization.

## Required rereview

The rereviewer must verify:

1. candidate hashes at review start and finish;
2. poison/count bit arithmetic for every reachable count and configuration
   boundary;
3. no admission race can increment a poisoned word or exceed the configured
   count;
4. every normal release decrements exactly once while preserving poison;
5. zero-count release sets poison without wrapping or changing count;
6. the poison regression distinguishes optimized prior behavior;
7. the submit-failure branch owns the just-begun page before any fallible
   rollback preflight can escape;
8. failed rollback preflight preserves the complete prior-page and reserved
   page set under the request identity;
9. polling a reserved page cannot adopt nonexistent I/O and either completes
   rollback or reinserts the request;
10. the recovery regression distinguishes the former orphan path;
11. previously accepted permit lifetime, response ordering, logical
    rollback, and late-result rejection remain unchanged;
12. the two fifty-run stress loops, targeted tests, formatting, and Clippy
    commands reproduce; and
13. the response-buffer statement and every exclusion are accurate.

Withhold the token for stale provenance, counter wrap, poison-clearing
admission, double release, a recoverability gap, partial rollback, a
nondistinguishing regression, or any production/GPU overstatement.
