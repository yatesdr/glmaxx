# TP4 step operation quota ownership CPU proof v1

Date: 2026-07-29

Implementation commit:
`e52ce5bc886a212e5e609037ea894957399f2bff`

Status: CPU TP4 worker-backpressure correction passed; independent review
pending

GPU claim: none

## Defect and ownership correction

`Tp4WorkerPool` reserves a bounded outstanding slot before queueing a
four-rank step. Previously, the returned `StepHandle` owned that slot and
released it when:

- the result was received;
- `receive_timeout` timed out; or
- the caller dropped the handle.

Dropping a response receiver does not cancel a queued or running TP4
operation. The pool could therefore report capacity and accept replacement
work while the dispatcher and all four persistent rank executors were still
inside the abandoned step. Execution remained serial at the retained
dispatcher, but the configured outstanding bound no longer measured the
combined queued/running work it was intended to limit.

The corrected ownership is:

1. `try_submit` reserves one atomic slot;
2. an uncloneable `OutstandingPermit` moves into the queued
   `DispatchCommand`;
3. a missing dispatcher or failed `try_send` drops that command and releases
   the slot;
4. the dispatcher retains the permit across all four rank results, output
   validation, and consensus;
5. the dispatcher releases the permit before making the result observable;
   and
6. response receive, timeout, disconnect, or handle drop never changes the
   operation counter.

If the response was abandoned, the bounded result send fails after physical
completion and the result is destroyed. If dispatcher startup or shutdown
drops queued commands, their owned permits release exactly once.
`OutstandingPermit::drop` uses checked atomic decrement so an impossible
double release cannot wrap the counter in an optimized build.

## Distinguishing CPU proof

`step_quota_is_owned_by_operation_after_handle_abandonment` installs four
persistent test executors behind a five-party entry barrier and a five-party
release barrier. The test:

1. submits one step to a pool with maximum outstanding one;
2. waits until all four rank executors are physically inside that step;
3. drops the only response handle;
4. records the outstanding count and attempts a replacement step;
5. releases and drains the original work without risking a test deadlock;
6. requires the recorded count to remain one and the replacement to be
   rejected as saturated; and
7. proves a later step succeeds after the abandoned physical operation
   completes.

The former handle-owned implementation records zero and accepts the
replacement while the four rank executors remain blocked, so it fails both
distinguishing assertions. The test deliberately releases the barriers
before asserting, which also lets the prior implementation terminate
cleanly instead of hiding the defect behind a deadlocked test destructor.

The normal result path still proves `outstanding()` reaches zero before the
received result is observable. Existing backend failure, malformed output,
rank divergence, strict step order, thread affinity, and exact four-rank
consensus tests remain green.

## Gate result and exclusions

The full local gate passed 262 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, the unchanged serving/cache
fixtures, and all 56 then-present review-handoff provenance proofs.

Commands:

```text
cargo test --offline -p glm-engine worker::tests -- --nocapture
cargo clippy --offline -p glm-engine --all-targets -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-engine/src/worker.rs
47206d2ef44fcbaef0cee3a1179605ff811ba7329e09f3493fc4f7a1333d3192

docs/offline-serving-spine.md
27b24d4cbafc8203937d3620e7bcd85d47fcb86cc4d8b89e237025e5d40a62f9

docs/sm120-rank-runtime.md
19638590ee3b42da32bfab7673986c26488da064649c635df895700838da5624

docs/sm120-rank-executor-v1.md
e97c54b865ed50c40ff8b15f6580d0edc18dbd0783135bc1c17d11cc19986fd4

docs/restore-operation-quota-proof-v1.md
6f7fc39db0a7cdc97c3ee9dd51d37b2adaeeb8dd3e087cb4c3fe85ff102a0128

docs/coordinator-api-backend-v1.md
ccfe6a07e5e9327822a3b9708d4119c5797172677d65dc116958f0e9b3378949

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, or model execution
was used.

This correction does not cancel a running host or device step, implement the
pending production executor ABI, construct device resources on owner
threads, add startup barriers, load weights, capture graphs, initialize
collectives, execute a model, or establish throughput. It proves only honest
queued/running TP4 operation accounting and safe abandoned-result handling
in the retained CPU worker pool.
