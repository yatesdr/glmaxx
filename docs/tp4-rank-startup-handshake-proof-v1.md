# TP4 rank startup handshake CPU proof v1

Date: 2026-07-29

Implementation commit:
`bc5d7d2d56f85383bd8b4fd9b6c25ecc845b2519`

Status: retained CPU rank-startup correction passed; independent review
pending

GPU claim: none

## Defect and startup correction

`Tp4WorkerPool::spawn` previously returned success as soon as it created the
dispatcher thread. The dispatcher created the four persistent rank threads
after the constructor had returned. If any rank-thread spawn failed,
`dispatch_loop` silently returned, detached already-started rank join
handles, and left the caller holding an apparently healthy pool whose first
observable failure was a later disconnected submission.

The corrected constructor and dispatcher use a bounded startup channel:

1. the constructor starts the dispatcher but does not return a pool;
2. the dispatcher attempts to create ranks 0 through 3;
3. each successful rank thread sends its own rank identity from inside that
   thread before entering `rank_loop`;
4. the dispatcher requires the exact, nonduplicate ready mask `0b1111`;
5. only then does it send startup success to the constructor; and
6. only after receiving that success does the constructor publish the pool.

An OS thread-spawn error drops every rank command sender, joins every
already-started rank thread, destroys the unstarted executors, and returns
`WorkerError::Thread`. A panic during partial cleanup supersedes that result
with `WorkerError::WorkerPanic`. A missing, invalid, or duplicate rank receipt
returns `WorkerError::RankStartup` after the same join discipline. If the
dispatcher exits or panics before any startup result, the constructor joins
it and returns `Closed` or `WorkerPanic`; it cannot return a pool.

The startup receive intentionally has no wall-clock deadline in this retained
CPU correction. A bounded production startup watchdog, factory construction
on owner threads, normative stage barriers, and structured stage receipts
remain part of the pending production executor contract.

## Distinguishing CPU proof

`pool_spawn_waits_for_all_four_ranks_and_cleans_partial_startup` routes the
same internal construction path through a deterministic rank-2 thread-spawn
failure. Four executors share a drop counter:

- ranks 0 and 1 have already started and sent their ready receipts;
- rank 2 follows the injected spawn-error branch;
- rank 3 remains unstarted in the fixed executor array; and
- `spawn_inner` must return `WorkerError::Thread`, not a pool.

The test then requires the drop count to be exactly four before the failed
constructor returns. This proves the dispatcher joined the two partial
workers and destroyed both unstarted executors instead of detaching resources
or deferring cleanup. The former implementation's equivalent rank-spawn
error branch returned silently from the dispatcher after the public
constructor had already returned success.

Existing success tests still prove the returned pool owns four persistent
thread-affine executors and can complete exact rank-set consensus. The
operation-quota regression remains green through this added startup barrier.

## Gate result and exclusions

The full local gate passed 263 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, the unchanged serving/cache
fixtures, and all 57 then-present review-handoff provenance proofs.

Commands:

```text
cargo test --offline -p glm-engine worker::tests -- --nocapture
cargo clippy --offline -p glm-engine --all-targets -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-engine/src/worker.rs
8c0742920847145e13975aae3db1b3a76054f94475b5a0b1ac4a4a9d05cba3ff

docs/offline-serving-spine.md
27b24d4cbafc8203937d3620e7bcd85d47fcb86cc4d8b89e237025e5d40a62f9

docs/sm120-rank-runtime.md
19638590ee3b42da32bfab7673986c26488da064649c635df895700838da5624

docs/sm120-rank-executor-v1.md
e97c54b865ed50c40ff8b15f6580d0edc18dbd0783135bc1c17d11cc19986fd4

docs/fable-sm120-rank-executor-v1-handoff.md
fe6fc7060d17db41901d545f4328a863b45737fd7e01be9c32a83bf013c2c031

docs/tp4-step-operation-quota-proof-v1.md
ab5e025afe5f4c236738ea6658d1bbcd9d7a3eac73fd53d4bf0b1cc4600f2d88

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, or model execution
was used.

This correction does not construct `RankExecutor` instances on their owner
threads, implement the proposed `RankExecutorFactory`, add the normative
production startup stages, enforce device identity, allocate arenas, load
weights, capture graphs, initialize collectives, add a startup timeout,
execute a model, or establish throughput. It proves only that the retained
CPU pool cannot publish success before all four rank threads are alive and
that partial thread startup is synchronously and completely cleaned up.
