# TP4 checkpoint load protocol CPU proof v1

Date: 2026-07-30

Status: implementation candidate; adversarial review pending

GPU claim: none

## Candidate

The proved implementation is commit
`e30017b8964553a471bb2da22d330e447de18492`.

| Input | SHA-256 |
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
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

## Implemented transaction

`Tp4WorkerPool::load_weights` now owns one process-common checkpoint load
transaction across the four persistent rank threads:

```text
exclusive pool permit
    -> prepare rank 0, 1, 2, 3
    -> construct one PreparedRankSet
    -> acknowledge that same rank set on rank 0, 1, 2, 3
    -> obtain one AdoptedRankSetReceipt
    -> finalize rank 0, 1, 2, 3
    -> return success only with four validated finalize acknowledgements
```

The dispatcher constructs exactly one `RankSetLoadCoordinator` from the
immutable plan, nonzero load-attempt generation, and four nonzero
owner-allocation generations. Every rank receives the same `Arc<RankSetLoadPlan>`,
the same prepared rank set, the same adopted receipt, and the same common
abort command. There is no rank-local route selection.

The dispatcher collects and rank-sorts all four responses at each successful
phase. Missing, duplicate, out-of-range, or reordered identities are rejected.
Prepared receipts and adoption acknowledgements are submitted to the existing
coordinator. Finalize acknowledgements independently bind rank, plan digest,
owner-allocation generation, and adopted-rank-set digest. A successful
`WeightLoadOutcome` retains all four prepared receipts, all four adoption
acknowledgements, the common adopted receipt, and all four finalize
acknowledgements.

Rank threads deliberately do not exit after a prepare, acknowledgement, or
finalize error. They remain available for the process-common abort command.
An abort error is terminal for that rank thread.

## Common abort and physical cleanup evidence

Any prepare, acknowledgement, finalize, coordinator, rank-set, channel, or
phase-timeout failure after coordinator creation broadcasts the same
`RankSetAbortCommand` to all four ranks. The command binds the common plan
digest and load-attempt generation; each cleanup acknowledgement additionally
binds its rank and planned owner-allocation generation.

`WeightLoadFailure` separates:

- `cause`, the original transaction failure;
- `cleanup_failure`, any later abort/cleanup failure; and
- four rank-indexed optional cleanup acknowledgements.

This preserves successful cleanup evidence even when another rank times out,
returns an error, disconnects, duplicates an identity, or emits malformed
identity fields. It never synthesizes a four-rank cleanup result from fewer
than four validated acknowledgements.

The dispatcher treats every load failure as fatal for the worker generation.
After returning the typed failure to the caller it closes rank command
channels and joins the worker threads. A successful load is single-shot for
that worker generation; a second load attempt is rejected as a transition
error.

## Deadlines and boundedness

Prepare, acknowledgement, finalize, and abort collection each use one common
deadline computed from `Instant::now() + phase_timeout`. The timeout is for
the entire phase, not multiplied by four sequential receive calls.

This is a fail-closed evidence deadline, not an operating-system thread
cancellation guarantee. Rust cannot force-stop a rank thread blocked inside a
native call. A timeout returns an incomplete cleanup result, but subsequent
pool destruction or dispatcher shutdown can still block while joining that
thread. Production requires a process-level supervisor/fail-stop boundary or
a cancellable native operation before this can be claimed as bounded recovery
from a permanently wedged CUDA or storage call.

## Exclusive worker capacity

Checkpoint load uses the existing pool outstanding counter as an exclusive
permit. It can reserve only from zero and stores an internal `usize::MAX`
sentinel until the transaction response is ready. Concurrent step submission,
page-table initialization, page-delta application, or another weight load
therefore fails closed as saturated.

`outstanding()` clamps the private sentinel to the configured public maximum.
The normal step reservation path uses an explicit comparison and
`checked_add`; it cannot evaluate `usize::MAX + 1` while the load owns the
pool. The exclusive permit restores the counter to zero exactly once on every
dispatcher path.

## Exhaustive CPU fault matrix

The mock rank executor records rank-local states and cleanup counts. The
candidate proves:

1. the success route reaches four resident ranks, returns four exact finalize
   acknowledgements, performs no cleanup, releases exclusivity, and permits a
   later step;
2. every one of the 12 rank-by-phase failures across prepare,
   acknowledgement, and finalize triggers the same common abort on all four
   ranks and returns four exact cleanup acknowledgements;
3. every one of the four possible cleanup-rank failures preserves the other
   three acknowledgements, reports the cleanup failure separately, and never
   claims the failed rank is clean;
4. a delayed prepare exceeding the common deadline returns a prepare timeout,
   broadcasts abort, reports an abort timeout, and retains the three cleanup
   acknowledgements that arrived; and
5. a concurrent step is rejected while a delayed successful load owns the
   exclusive permit.

The failure matrix exercises 17 distinct injected cases in addition to the
success and exclusivity cases. Existing persistent-worker, queue,
page-mirror, collective-consensus, and serving tests continue to pass.

## Exact local gate

At exact implementation candidate
`e30017b8964553a471bb2da22d330e447de18492`,
`scripts/local-checks.sh` passed:

- 332 Rust unit and integration tests with zero failures;
- workspace formatting and Clippy with warnings denied;
- CUDA-FFI host type checking and Clippy with warnings denied;
- deterministic CPU, matrix, manifest, native-rank, engine, serving, and
  cache proof regeneration and byte comparison;
- all 83 then-current adversarial-review handoffs, with no configured result
  accepted by the local verifier; and
- C++ syntax parsing of `kernels/include/glmaxx_kernel.h`.

The external tokenizer proof was skipped because `GLMAXX_TOKENIZER_DIR` was
unset. The local host has no `nvcc`.

## Explicit exclusions

No CUDA translation unit was compiled, no native library was linked, no
checkpoint file was opened, no pinned or device allocation was made, and no
GPU work ran. The four-rank protocol is exercised by deterministic CPU mocks.

The candidate does not yet provide the concrete `RankExecutor` that owns
`NativeRankContext`, `NativeRankReader`, and the CUDA checkpoint typestates.
It therefore does not yet invoke `PreparedCudaRank::load` through
`Tp4WorkerPool`. It also does not establish process-level recovery from a
permanently stuck rank thread.

No production checkpoint load, small-checkpoint smoke, target-layer
execution, startup-health transition, SM120 correctness, capacity, quality,
or performance result is claimed. The next implementation gate is the native
rank adapter, followed by native compilation and the progressive cn4
qualification sequence after renewed operator authorization.
