# Terminal cleanup transaction CPU proof v1

Date: 2026-07-29

Implementation commit:
`a7087e716e3d9e1e201ff443939001c8ef428680`

Status: CPU/reference correction passed; independent review pending

GPU claim: none

## Defect and invariant

After worker consensus, `ServingCoordinator::tick_observed` previously
committed the complete scheduler batch and then processed rows one at a time.
Each row could publish token/terminal events and release its prompt tokens,
prefix lease, and residency pins before a later row reported a cache error.
The result combined a successful scheduler commit with partially published
events and partially released multi-user resources.

Cancellation and failed-step cleanup used the same row-at-a-time release
pattern.

The serving boundary now follows:

```text
validate worker output
    -> stage every event in fixed storage
    -> derive every terminal/token release
    -> preflight the cumulative prompt-byte result
    -> preflight all counted residency unpins across all requests
    -> commit the scheduler batch
    -> commit the preflighted releases
    -> publish the staged events
```

No successful scheduler state becomes visible until every fallible
publication and resource-release check has succeeded.

## Multi-request prefix release

`PrefixRestoreCoordinator::plan_release_many` accepts separate page-key
slices for each request. Logical ordinals restart at zero for every slice, so
owner rank selection is identical to each request's restore path. The planner
then combines all `(owner rank, page key)` entries and counts repeated keys
before validating any unpin.

This is required for concurrent prefix sharing. If two requests lease the
same page, one remaining pin cannot satisfy a two-request terminal cleanup.
The cumulative plan requires `pin_count >= 2` and returns before either pin
changes.

`commit_release` consumes the validated plan under the same exclusive
coordinator access. Each entry is unique, and no intervening operation can
change residency. Counted unpins are therefore infallible without unsafe or
reentrant mutation.

The number of unique plan entries is bounded by HBM-resident leased pages.
The page map is allocated only when terminal, cancellation, or failure
cleanup actually has prefix pages; it is not part of ordinary nonterminal
decode.

## Serving publication plan

`plan_successful_step_publication` reconstructs the exact post-step state
from the immutable selected batch, starting progress, and rank-consensus
output:

- prefill prompt progress uses the selected prompt-token count;
- decode/verify positions use the starting generated count;
- speculative ordinals cover only accepted draft tokens;
- EOS and configured output length derive the terminal state; and
- token-only versus full-prefix release is explicit per request.

The resulting release planner:

- deduplicates request IDs;
- lets full-prefix release dominate token-only release;
- subtracts each retained token buffer exactly once;
- validates the cumulative retained-prompt byte result; and
- preflights every leased page across every terminal request.

After atomic scheduler completion, `commit_request_releases` contains no
fallible arithmetic, lookup, or residency validation. It consumes the cache
plan, removes the exact leases/token buffers, publishes the precomputed byte
counter, and then appends staged events.

## Fixed-capacity event boundary

Step-event staging uses no heap allocation. Its capacity is derived rather
than copied as a magic number:

```text
C64 * (MTP6 maximum 7 committed tokens + 1 terminal event) = 512 events
```

Request-release staging is a fixed C64 array. A dedicated boundary test
constructs 64 MTP6 verifier rows, each committing seven tokens and
terminating, and proves all 512 events and 64 releases fit exactly.

The existing starting-progress and completion vectors remain an older
allocation boundary and are not claimed allocation-free by this correction.

## Failure and cancellation behavior

If successful-step publication preflight fails, the unchanged inflight batch
enters `fail_selected_step`. That path preflights all selected-row cleanup,
then atomically completes the scheduler batch as failed. If resource
preflight itself fails, scheduler failure still consumes the inflight batch,
but no lease, pin, token buffer, or event changes.

Idle cancellation derives the complete cancelled set, preflights every
release, commits them together, and only then publishes cancellation events
and terminal markers. A failed cancellation cleanup remains exactly
retryable after the cache invariant is repaired.

## Distinguishing CPU proof

`late_terminal_cleanup_failure_does_not_partially_publish_the_batch` creates
three concurrently terminating prefix hits:

- requests 100 and 101 share the same physical prefix page and hold two pins;
- request 102 owns a different page;
- the request-102 pin is deliberately removed behind its retained lease.

The old implementation commits all three requests, releases both shared
pins and publishes the earlier rows before request 102 fails. The new code:

1. returns the expected cache-state error;
2. marks all three selected requests failed rather than successfully
   finished;
3. preserves all three leases;
4. publishes no partial token, terminal, or failed events;
5. proves both shared pins remain by releasing them independently after the
   failure; and
6. proves the scheduler has no stranded inflight batch.

The same fixture then admits and cancels two more cached requests. A late
invalid pin preserves both leases and publishes no cancellation event.
After both pins are repaired, retry emits exactly the two ordered
cancellation events and removes both leases.

The full local gate passed 241 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, and all 40 then-present review
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
crates/glm-serving/src/cache.rs
f265314cc36e5453219b96b351a2a6adad04dbf12b6647b9682b5a5cb7f80ea0

crates/glm-serving/src/lib.rs
5f67b28a7a2169564687822c49b3f6c26710352f8edd1361f4daf834f21346b0

crates/glm-cache/src/residency.rs
a83edc26e750573878888888cc58f1eb08846c39633ef1f9084db7cfeaba295c

crates/glm-scheduler/src/lib.rs
5a820b2e5013f038f07b26f14ddc24d69d00e18d3d55837ab5ff3a68daee3074

docs/prefix-release-atomicity-proof-v1.md
7fbe0f4ced91d7ddc8da4f38b6c9c9a8bc73f524eb257ef1ca9a537f095bb9f4

docs/selected-step-failure-finalization-proof-v1.md
36be571d84cff086ad3058f3426fc0fee6bdd4d33b1c4317473128e4d861512e

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

This correction covers scheduler/result publication, request-owned prompt
buffers, prefix leases, residency pins, terminal events, failed-step
cleanup, and idle cancellation. It does not integrate active sequence page
tables, private target/draft tails, rank page-table deltas, physical-ID
quarantine, admission rollback, direct tier I/O, CUDA execution, or
process-crash recovery. It does not establish model quality or serving
performance.
