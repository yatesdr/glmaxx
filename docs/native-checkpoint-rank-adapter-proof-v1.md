# Native checkpoint rank adapter host proof v1

Date: 2026-07-30

Status: implementation candidate; adversarial review pending

GPU claim: none

## Candidate

The proved implementation is commit
`944d176fc28d46c386d6b39c4d38603032b94a7f`.

| Input | SHA-256 |
|---|---|
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |
| `crates/glm-engine/Cargo.toml` | `1e4fafe2079b1f529a35b6153cdbfcc39420728fcba0cb72c5920715cf591401` |
| `crates/glm-engine/src/native_worker.rs` | `bb773418f38f6989fad871b74fc0bc7ab7b390d9b0a930aa3413989ac89e084a` |
| `crates/glm-engine/src/worker.rs` | `466649c2ce7fa10a7680467cd137f3b99907dd70b9afbb203cf8b60e73aad28f` |
| `crates/glm-engine/src/lib.rs` | `73db983e32a7ec4c70906c7be28f6427ec825fd15605003646a29be9f3bd53be` |
| `crates/glm-engine/src/checkpoint_cuda.rs` | `6bceb0e8fa8c32e8d981bc3044163b03f5d2bcc5b3c3527ccfed215012240cc4` |
| `crates/glm-engine/src/checkpoint_load.rs` | `736b8f7dd75e470a9fdae83376df40165245f6a0683fe6846f8df9f022a4e09f` |
| `crates/glm-cuda/src/ffi.rs` | `eee9a388c8c25b9d385ddf0159c7dc766d1a19560078f2915d22fe77d1642817` |
| `crates/glm-cuda/src/load.rs` | `3ffa499c9c985954fc9847d4d75a7356a8be33ded6b77afd6fc593f2e5833392` |
| `crates/glm-format/src/native_reader.rs` | `953a56702ba1ee000f508fe24cbbae7c6137d6496d104720f658fede5572699c` |
| `docs/rank-local-checkpoint-loader-proof-v1.md` | `62fd339b8f8d6b45a72af0c3e4e13af2eddf64baad7ec722f964e4e0089ed0ea` |
| `docs/tp4-checkpoint-load-protocol-proof-v1.md` | `d536e281d694eb0cbdd123d2cea9527e0d7cb9348556c6c3be0c9da13323ccb7` |
| `docs/sm120-rank-runtime.md` | `e52ecab7bd378d6aed2c9033ff6404a34c880ed860a3ac9e7ed4c8d4d0a11b02` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

## Native owner construction

With the `cuda-ffi` feature, `NativeCheckpointRankExecutor::open` now runs on
the persistent rank thread created by `RankExecutorFactory`. It:

1. rejects an out-of-range rank or zero software-provenance digest;
2. binds exactly that rank through `NativeRankContext::bind`, which requires
   four visible SM120 devices and captures the CUDA UUID-bound identity;
3. opens and validates one exclusive immutable `NativeRankReader`; and
4. requires the image's encoded rank to equal the context rank.

`Tp4WorkerPool::spawn_native_checkpoint_loaders` maps the four supplied paths
to four factories in rank order. A constructor failure retains the exact
`RankCheckpointLoadError` and rank in `WorkerError::RankCheckpointLoad`; the
existing startup handshake closes all channels and joins every successfully
created peer before returning failure.

The executor is non-`Send` because its native context and CUDA load backend
are thread-affine. Only the factory and rank path cross the thread boundary.

## Physical typestate integration

The executor owns this state machine:

```text
Vacant
  -> Prepared(PreparedCudaRank<NativeRankLoadBackend>)
  -> Acknowledged(AcknowledgedCudaRank<NativeRankLoadBackend>)
  -> Resident(CudaWeightArena<NativeRankLoadBackend>)
```

Prepare records the plan digest, load-attempt generation, and
owner-allocation generation before creating a load backend. It then calls the
real `PreparedCudaRank::load` composition with the retained reader, observed
native backend, exact plan, owner generation, and software provenance. A
failure before a prepared value leaves the state vacant but retains the
attempt identity so the process-common abort can truthfully acknowledge that
no resource remains.

Acknowledgement requires the common `PreparedRankSet` to contain this rank
and the active plan and owner generation. It consumes the prepared CUDA
typestate and retains quarantine. Finalization requires the active plan and
consumes the acknowledged typestate with the common
`AdoptedRankSetReceipt`. Only the resulting resident state can observe the
arena's rank and device pointers.

The active attempt identity intentionally remains after local finalization.
If another rank fails finalization, the common abort can therefore identify
and physically release this already-resident arena. A successful
process-wide transaction is single-shot, so the retained identity cannot
admit another load.

## Abort and drop ownership

Abort validates rank, plan digest, load-attempt generation, and
owner-allocation generation before touching state:

- vacant after a failed prepare/adoption consumes no native resource and can
  emit one cleanup acknowledgement;
- prepared and acknowledged states use their explicit synchronizing
  `abort_and_release` paths;
- resident state uses `CudaWeightArena::shutdown`; and
- any cleanup error changes the executor to terminal `CleanupFailed`, retains
  the attempt identity, and refuses a cleanup acknowledgement.

A successful abort clears the active identity. A repeated abort therefore
cannot forge a second acknowledgement. A mismatched abort does not consume
the valid state; dispatcher termination then drops the executor on its owner
thread.

`weights` is declared before `context`. Rust field-drop order consequently
drops the arena and its backend before destroying the context-owned stream.
The reader is also retained on that same owner thread for the executor
lifetime.

## Fail-closed execution boundary

Both `execute` and `execute_bound` reject nonresident or wrong-rank state as
an invariant failure. A resident state verifies owner-thread access and then
returns the explicit backend code reserved for “native target-layer program
not implemented.” It never invokes `cpu_output`, returns fabricated tokens,
or silently substitutes a CPU worker.

This makes the adapter useful for the checkpoint-load gate without
misrepresenting it as a serving executor.

## Error mapping

Factory/open failures retain the full typed reader, plan, or CUDA error in
`WorkerError::RankCheckpointLoad`. Once inside the existing transaction ABI,
rank lifecycle methods must return `LoadPlanError`; the adapter maps:

- plan errors unchanged;
- reader failures to `Reader`;
- topology and device-content validation failures to `Identity`;
- alignment and overflow failures to their matching variants; and
- remaining CUDA allocation, stream, DMA, ABI, shape, driver, and async
  failures to `Writer`.

This mapping is fail-closed but lossy. Preserving the exact native error code
inside the four-rank transaction result remains an observability hardening
item; this candidate does not claim that diagnostic fidelity.

## Exact local gate

At exact candidate `944d176fc28d46c386d6b39c4d38603032b94a7f`,
`scripts/local-checks.sh` passed:

- 332 Rust unit and integration tests with zero failures;
- workspace formatting and Clippy with warnings denied;
- CUDA-FFI host type checking and Clippy with warnings denied, including
  `native_worker.rs`;
- deterministic CPU, matrix, manifest, native-rank, engine, serving, and
  cache proof regeneration and byte comparison;
- all 84 then-current adversarial-review handoffs, with no configured result
  accepted by the local verifier; and
- C++ syntax parsing of `kernels/include/glmaxx_kernel.h`.

The external tokenizer proof was skipped because `GLMAXX_TOKENIZER_DIR` was
unset. The local host has no `nvcc`.

## Explicit exclusions

The host proof type-checks the feature-gated native adapter but cannot link or
execute it. No CUDA translation unit was compiled, no native library was
linked, no rank context was created, no rank file was opened through this
adapter, no pinned or device allocation was made, and no GPU work ran.

No CPU test instantiates `NativeCheckpointRankExecutor`; the existing fake
backend and transaction fault matrices prove its component contracts, not
the FFI composition. The runtime still needs a process-common way to obtain
the four worker-observed device identities before constructing
`RankSetLoadPlan`; callers currently must supply an already pinned plan.

No production or small checkpoint load, physical cleanup acknowledgement,
native execution, startup-health transition, SM120 correctness, capacity,
quality, or performance result is claimed. The next host implementation gate
is worker-observed identity discovery plus a single startup/load API. Native
compile/link and progressive checkpoint qualification require renewed cn4
operator authorization.
