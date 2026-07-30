# Fable handoff: TP4 checkpoint load protocol v1

Date: 2026-07-30

Status: adversarial CPU transaction review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4, compile
remotely, allocate a GPU, or launch CUDA for this review

Review candidate commit:
`d64753549881f7ecb5a3920bff888d81ee3345a0`

Required result path:
`docs/reviews/fable-tp4-checkpoint-load-protocol-v1.md`

Requested acceptance token, only if every blocker and major is resolved:
`tp4-checkpoint-load-protocol-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |
| `crates/glm-engine/Cargo.toml` | `1e4fafe2079b1f529a35b6153cdbfcc39420728fcba0cb72c5920715cf591401` |
| `crates/glm-engine/src/worker.rs` | `1fdf4137a72b187a368be9443fccbac40a741ad36fc6c8adda50c6dcfd32b66c` |
| `crates/glm-engine/src/lib.rs` | `611d903e10702f96518da4485426b8b4181b7b706a9c07fdb7bbddb65b1ba525` |
| `crates/glm-engine/src/checkpoint_load.rs` | `736b8f7dd75e470a9fdae83376df40165245f6a0683fe6846f8df9f022a4e09f` |
| `crates/glm-engine/src/checkpoint_cuda.rs` | `6bceb0e8fa8c32e8d981bc3044163b03f5d2bcc5b3c3527ccfed215012240cc4` |
| `docs/checkpoint-load-transaction-v1.md` | `184199144075e3f7c511b31002ea81c1d2dbaad1087e01fe7dd4533dc36a3c21` |
| `docs/rank-set-load-coordinator-proof-v1.md` | `4076872eec5adcdb2f4a0445418ed58d695a188a1d8aeb4e95496ef7ec52196a` |
| `docs/rank-local-checkpoint-loader-proof-v1.md` | `62fd339b8f8d6b45a72af0c3e4e13af2eddf64baad7ec722f964e4e0089ed0ea` |
| `docs/sm120-rank-runtime.md` | `e52ecab7bd378d6aed2c9033ff6404a34c880ed860a3ac9e7ed4c8d4d0a11b02` |
| `docs/tp4-checkpoint-load-protocol-proof-v1.md` | `d536e281d694eb0cbdd123d2cea9527e0d7cb9348556c6c3be0c9da13323ccb7` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-tp4-checkpoint-load-protocol-v1-handoff.md
cargo test --offline -p glm-engine worker
cargo clippy --offline --workspace --all-targets -- -D warnings
GLMAXX_KERNEL_LIB_DIR=/tmp cargo clippy --offline -p glm-cli \
  --features cuda-ffi -- -D warnings
```

## Review boundary

This review covers the CPU-verifiable `Tp4WorkerPool` transaction that sends
process-common prepare, acknowledge, finalize, and abort commands to four
persistent rank executors. It covers coordinator integration, exact phase
identities, common deadlines, explicit partial-cleanup evidence, terminal
worker-generation behavior, and exclusive pool capacity.

It assumes the earlier rank-local loader and rank-set coordinator reviews
only at their typed boundaries. It does not accept those candidates by
implication.

It does not cover the not-yet-implemented native rank adapter, a checkpoint
file load through the pool, CUDA compilation or linking, physical GPU
cleanup, a process supervisor, checkpoint smoke, target-layer execution,
SM120 correctness, capacity, quality, or performance.

## Required adversarial questions

1. Verify every candidate input hash at review start and finish. Does the
   proof describe the exact candidate rather than moving `main`?
2. Enumerate every command sent in the success route. Do all four ranks
   receive the identical plan, prepared rank set, adopted receipt, and
   coordinator-derived route, with only owner-allocation generation varying
   by the immutable rank array?
3. Does the dispatcher collect exactly four distinct rank identities before
   advancing each phase? Can a missing, duplicate, out-of-range, reordered,
   stale, or malformed reply be mistaken for consensus?
4. Is `PreparedRankSet` constructed only from four validated prepared
   receipts? Is its adoption command compared with the coordinator action
   before acknowledgement begins?
5. Are all four adoption acknowledgements fed to the same coordinator? Can
   final adoption be inferred locally, or can finalize begin before the
   coordinator returns the one `AdoptedRankSetReceipt`?
6. Reconstruct every field in `RankWeightFinalizeAck`. Can a finalize success
   for the wrong rank, plan, owner generation, or adopted-rank-set digest
   enter a successful `WeightLoadOutcome`?
7. For every prepare, acknowledgement, finalize, coordinator, rank-set,
   channel, and timeout failure after coordinator creation, is the same
   `RankSetAbortCommand` broadcast to all four ranks? Is there any rank-local
   fallback or early return that skips common abort?
8. Do rank threads remain alive after prepare, acknowledgement, and finalize
   errors long enough to receive abort? Is an abort error terminal rather
   than silently recoverable?
9. Reconstruct every field in `RankWeightCleanupAck`. Does
   `WeightLoadFailure` preserve the original cause separately from cleanup
   failure and retain each independently validated partial acknowledgement?
   Can fewer than four acknowledgements ever be represented as complete
   cleanup?
10. Test all 12 rank-by-phase prepare/acknowledgement/finalize failures and
    all four cleanup-rank failures independently. Do the assertions prove the
    claimed common-abort state and exact cleanup count rather than merely
    inspect a dispatcher error?
11. Are phase deadlines common per phase, using the remaining time rather
    than granting a fresh timeout to every receive? Does zero duration or
    deadline overflow fail closed?
12. Is the proof explicit that a timed-out native rank cannot be force-killed
    by Rust and that pool shutdown can still block on `join`? Does any API,
    test, or prose incorrectly claim bounded process recovery?
13. Trace the exclusive permit through invalid arguments, reservation
    failure, channel send failure, dispatcher success, dispatcher failure,
    response drop, and pool drop. Is the private `usize::MAX` state released
    exactly once?
14. Can normal step reservation overflow while the exclusive sentinel is
    present? Can `outstanding()` expose the sentinel, or can a concurrent
    step, page-table operation, page delta, or second load enter the pool?
15. Is every load failure terminal for the dispatcher generation? Can queued
    work execute after a partial or failed checkpoint transaction, or can a
    second successful publication occur in one generation?
16. Does a successful result require four finalize acknowledgements before
    the permit is released and a step can execute? Can response-channel
    failure publish a false success or leak quota?
17. Are the exact 332-test result, review-handoff count, absent tokenizer
    fixture, absent `nvcc`, and every GPU/native/checkpoint exclusion
    accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- all phase routes and payloads are process-common;
- exactly four distinct rank responses gate each transition;
- coordinator adoption cannot be bypassed;
- finalize acknowledgements bind all required identities;
- every post-coordinator failure broadcasts one common abort;
- partial cleanup evidence cannot be promoted to complete cleanup;
- rank threads survive intermediate errors for cleanup;
- original and cleanup failures remain distinguishable;
- the rank-by-phase and cleanup-rank fault matrices are exhaustive;
- phase deadlines have the claimed whole-phase semantics;
- the stuck-thread/process-supervisor limitation is stated accurately;
- exclusive pool capacity is overflow-safe and released exactly once;
- load failure is terminal for the worker generation;
- success precedes later step execution only after four finalize acks; and
- proof claims and explicit exclusions are accurate.

Only if all fifteen answers are unqualified `YES`, end with the requested
token. Withhold it for a stale candidate, rank-local route, phase advance
without four exact replies, adoption bypass, malformed finalize acceptance,
abort omission, forged cleanup completion, premature rank exit, lost primary
failure, untested fault position, multiplied timeout, hidden stuck-thread
claim, quota race/overflow, post-failure execution, premature success, or
evidence overstatement.

The token accepts only this CPU-proven TP4 checkpoint transaction. It does not
authorize cn4 access or accept the native rank adapter, any checkpoint file
load, CUDA compilation, physical allocation or cleanup, GPU execution,
process supervision, checkpoint smoke, SM120 correctness, capacity, quality,
or performance.
