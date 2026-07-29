# Pending admission rollback CPU proof v1

Date: 2026-07-29

Implementation commit:
`7bae533137e737d466b2c059fddd58425253e8a7`

Status: CPU/reference correction passed; independent review pending

GPU claim: none

## Defects and invariant

Pending prefix restore previously had two remove-before-rollback paths:

- `cancel_restore` removed the request from the pending registry and then
  rolled pages back one at a time; and
- `poll_restore` removed the request before polling and, on a restore or pin
  failure, also rolled pages back one at a time.

A late rollback error could therefore coexist with earlier unpins or restore
aborts. The coordinator had already discarded the only request-level handle,
so the caller could neither poll nor cancel the remaining work.

The serving layer compounded that loss by removing its pending admission and
releasing the retained prompt-byte reservation on every cache error, without
checking whether the cache rollback itself had failed. Prompt-byte
subtraction also remained a fallible operation after cache state changed.

Pending rollback now follows:

```text
aggregate every pin and exact restore identity
    -> validate every cumulative pin count
    -> validate every request/ordinal-bound restore
    -> remove the pending registry entry
    -> commit only the preflighted unpins and aborts
```

If polling encounters an operation error and rollback preflight also fails,
the exact pending record is reinserted before the rollback error is returned.
If rollback succeeds, no pending cache work remains and the original
operation error is returned.

## Exact restore ownership

Every pending page now retains its logical page ordinal, and every pending
request retains its request ID. `ResidencyManager` exposes a read-only
validation operation and an exact abort operation that both require:

- the expected page key;
- `Residency::Restoring`; and
- the exact `(request_id, page_ordinal)` pending identity.

A wrong request or ordinal cannot abort another admission's restore.
`complete_restore` retains its existing full-record and identity validation.

The rollback planner groups pinned pages by `(owner rank, page key)` and
validates the cumulative unpin count. Restoring pages retain one exact
identity per `(owner rank, page key)`; a duplicate fails closed before any
mutation. The commit consumes unique, already validated entries under
exclusive coordinator access.

Queue submission failure uses the same transaction. The just-started restore
is added to the rollback plan with all earlier pages before any of them is
aborted. Thus saturation cannot abort the newest page and then fail while
rolling back an earlier page.

## Retryable poll and cancellation

`cancel_restore` leaves the registry entry in place while the complete
rollback is planned. A failed plan changes no pin, restore, handle, or
registry entry. A repaired invariant can be cancelled again using the same
request ID.

`poll_restore` temporarily owns the pending record while it polls handles. On
an operation failure:

- successful rollback consumes the record and returns the original error;
- failed rollback reinserts the exact record, including its updated
  `Pinned`, `Resident`, and `Restoring` page states and live handles; and
- reinsertion refuses to overwrite an unexpectedly reappearing request.

The plan maps are bounded by the number of pages in the admitted prefix.
They allocate only during restore failure or cancellation, not ordinary
decode and not a successful pending poll.

## Serving admission coupling

`ServingCoordinator::poll_admission` now removes its pending admission and
prompt reservation only when the cache confirms that request no longer has
pending restore state. If cache rollback failed and retained the request,
the serving admission and its token buffer remain available for repair and
cancellation.

The exact post-release prompt-byte counter is computed before polling or
cancelling the cache. Once cache cancellation succeeds, removal of the
preflighted serving entry and publication of that counter contain no
fallible arithmetic.

Immediate and polled successful admissions also receive a precomputed
post-release counter. Request IDs already present in pending admissions,
prefix leases, or retained prompt-token maps are rejected by the
prevalidated admission route, preventing a second route from occupying a
pending token admission's ID.

## Distinguishing CPU proofs

`multi_page_restore_is_submitted_without_blocking_admission` starts a
two-page restore, then deliberately aborts the first page behind the
coordinator while the second remains restoring. Cancellation:

1. returns the expected residency-state error;
2. retains the pending request;
3. leaves the second page restoring rather than partially aborting it;
4. succeeds after the first identity is repaired; and
5. leaves both pages in NVMe with no pending request.

The prior implementation removed the registry entry, aborted the second
page, failed on the first, and made retry impossible.

`failed_restore_rollback_retains_pending_admission_until_cancel` drives the
same corruption through `ServingCoordinator` with a real file-backed page.
After the worker result reaches the corrupted identity it proves:

- both the cache pending request and serving pending admission remain;
- the 256-byte prompt reservation remains exact;
- the scheduler has no request with the same ID;
- the prevalidated route rejects that reserved ID;
- no event is published; and
- repair followed by cancellation removes both records, publishes the zero
  prompt counter, and emits exactly one cancellation event.

`submit_saturation_rolls_back_every_started_restore` uses five page ordinals
with one outstanding slot per owner rank. The fifth page deterministically
revisits rank zero and is rejected as saturated. The combined rollback leaves
all five pages in NVMe, no pending request, and zero outstanding handles.

The residency identity test independently proves that wrong request and
wrong ordinal aborts fail while the valid restore remains observable as
restoring.

## Gate result and exclusions

The full local gate passed 243 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, and all 41 then-present review
handoff provenance proofs.

Commands:

```text
cargo fmt --check
cargo test -p glm-cache
cargo test -p glm-serving
cargo clippy -p glm-cache -p glm-serving --all-targets -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-cache/src/residency.rs
f63aaee9f96997e2a39e12f9a908d0b2bdee2a3f8a5c3c17f794b878fc0843ec

crates/glm-serving/src/cache.rs
e7686a678537b1644608f655e0a8bd40133e0772e0cdf12865c02bbefb15b54b

crates/glm-serving/src/lib.rs
d33b6e9efc231fabdd1065f64db83cb74c95ee82138a49c8ee1798a130465eca

crates/glm-scheduler/src/lib.rs
5a820b2e5013f038f07b26f14ddc24d69d00e18d3d55837ab5ff3a68daee3074

docs/terminal-cleanup-transaction-proof-v1.md
5998b9abb4e1587ef5a4a83ebbc1c2e6bee551122fdd075af0ea1cac01172862

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, or model execution
was used.

This correction covers CPU pending-prefix restore cancellation and failure,
exact restore abort identity, prompt-token reservation accounting, and
request-ID collision with pending token admission. It does not make the
larger admission sequence a single transaction with active sequence page
tables, private target/draft tails, rank page-table deltas, physical-ID
quarantine, CUDA uploads, or collective execution. It does not implement
direct I/O, device KV movement, process-crash recovery, model quality, or
serving performance.
