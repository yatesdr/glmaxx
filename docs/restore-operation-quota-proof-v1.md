# Restore operation quota ownership CPU proof v1

Date: 2026-07-29

Implementation commit:
`7cf1ef4cb75af774a183f98657b23862c5fea97c`

Status: CPU restore-backpressure correction passed; independent review pending

GPU claim: none

## Defect and ownership correction

`RestoreService` reserves a bounded outstanding slot before queueing a
blocking file restore. The prior `RestoreHandle` owned that slot and released
it when:

- the result was received;
- `receive_timeout` timed out; or
- the caller dropped the handle during cancellation/rollback.

Dropping a response handle does not cancel a queued or running
`read_exact`/SHA-256 operation. The service could therefore report a free
slot and admit replacement work while the original read still occupied the
worker, file handle, payload allocations, and CPU hash path. Repeated
abandonment made the configured outstanding limit a response-handle limit,
not a physical-operation limit.

The corrected ownership is:

1. `try_submit` reserves one atomic slot;
2. an uncloneable `OutstandingPermit` moves into the queued
   `RestoreCommand`;
3. send failure or worker shutdown drops the command and releases the slot;
4. the worker retains the permit across the complete read and checksum;
5. after physical completion, the worker releases the permit before sending
   the response; and
6. response receive, timeout, disconnect, or handle drop never changes the
   operation counter.

If a response was abandoned, the worker's failed response send drops the
restored payload. Residency rollback remains request/ordinal-bound and the
late result cannot become observable.

`OutstandingPermit::drop` uses checked atomic decrement. An impossible
underflow leaves the counter unchanged and triggers a debug assertion instead
of wrapping it to `usize::MAX`.

## Distinguishing CPU proofs

`restore_quota_is_owned_by_operation_after_handle_abandonment` constructs the
same command/handle ownership pair used by submission, forces an immediate
response timeout, and proves:

- consuming and dropping the timed-out handle leaves outstanding count at
  one; and
- only dropping the still-owned command permit decrements it to zero.

The prior handle-owned implementation reports zero immediately after the
timeout and fails this regression.

`submit_saturation_rolls_back_every_started_restore` still forces the fifth
page to revisit a rank with one allowed outstanding operation. Coordinator
rollback immediately removes the logical pending request and returns every
page to NVMe, but now waits with a hard deadline for abandoned physical reads
to drain before asserting zero service operations. This separates logical
rollback from physical I/O completion instead of conflating dropped handles
with completed reads.

The normal receive path still proves the service reports one while submitted
and zero before a delivered result becomes observable.

## Gate result and exclusions

The full local gate passed 259 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, the unchanged cache-lifecycle
fixture, and all 54 existing review-handoff provenance proofs.

Commands:

```text
cargo test --offline -p glm-cache residency::tests
cargo test --offline -p glm-serving cache::tests
cargo clippy --offline -p glm-cache -p glm-serving --all-targets -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-cache/src/residency.rs
2846361e521f66752cb4455c908b2f30fa2f2a27a59a8059866e43b2402a2d6d

crates/glm-serving/src/cache.rs
46962a84ce6c3edec0217a4d4edaac0f7a7e4e283f555bccea8492772881b229

docs/direct-tier-io-v1.md
7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2

docs/pending-admission-rollback-proof-v1.md
cfd008dacc26f7d82c3f524ad7347da9d492168ebd9e53bb255bdc6cbcbfddfd

docs/cache-lifecycle-proof-v1.md
11ad4936fea7cd0887e660911f50778d5b0918c21a6cebaca1a98a244b2e2de1

fixtures/cache-lifecycle-proof-v1.json
c1151c34a3a9bee4fd97dea11e807603a56c2af4d37deab813cc9b5631177d6a

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, or model execution
was used.

This correction does not cancel a blocking syscall, deduplicate same-key
waiters, use io_uring, register fixed buffers, share a live catalog, perform
direct I/O, isolate decode from storage work, or move real HBM/DRAM bytes.
It proves only honest bounded accounting and safe result abandonment in the
retained blocking CPU service.
