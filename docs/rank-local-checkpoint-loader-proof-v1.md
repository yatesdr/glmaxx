# Rank-local checkpoint loader CPU proof v1

Date: 2026-07-30

Status: implementation candidate; adversarial review pending

GPU claim: none

## Candidate

The proved implementation is commit
`bc4b3d0cec9f8f8bc8ae368e2740377f923d5f6e`.

| Input | SHA-256 |
|---|---|
| `crates/glm-cuda/src/load.rs` | `89eb00dbf649b99355fb4a7ceb776e975d96a7f75240ade095ec5adbef0fecee` |
| `crates/glm-engine/src/checkpoint_cuda.rs` | `52a3f2823475174c8f91d3aa6681dbbf1378c20b2cbca6a2ea4ba6c5fc4dd984` |
| `crates/glm-engine/src/checkpoint_load.rs` | `736b8f7dd75e470a9fdae83376df40165245f6a0683fe6846f8df9f022a4e09f` |
| `crates/glm-engine/src/lib.rs` | `dfccf9816ea433d8a4be3480c3e1e46ae5f353cc0221193bf49361c6c391170c` |
| `crates/glm-engine/src/worker.rs` | `7791af31f91f8962fc21e3701d337c7e044d4844d88fc5b69e9926dbf59c66fd` |
| `crates/glm-format/src/native_reader.rs` | `953a56702ba1ee000f508fe24cbbae7c6137d6496d104720f658fede5572699c` |
| `crates/glm-serving/src/backend.rs` | `afcca1f6f50cb7699ef24cfc32e45fa005a6a8663efb7dc36806b2305ff295d4` |
| `crates/glm-serving/src/lib.rs` | `bc7eff0297e14b73df7eec5ade3352ad0f75ceabeaca1862c4866a51efb948e3` |
| `docs/checkpoint-load-transaction-v1.md` | `9b7625e0d8c2ca18f287303b9aaa831b442956e0f8b8c8617ab5c74649e29cf3` |
| `docs/cuda-checkpoint-arena-cpu-proof-v1.md` | `399c5b9a276870210d1e389269b602862091e3ef42ccc22a2175ad7589a09a76` |
| `docs/rank-set-load-coordinator-proof-v1.md` | `4076872eec5adcdb2f4a0445418ed58d695a188a1d8aeb4e95496ef7ec52196a` |
| `docs/sm120-rank-runtime.md` | `19638590ee3b42da32bfab7673986c26488da064649c635df895700838da5624` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

## Implemented load boundary

`PreparedCudaRank<B>::load` now composes the previously separate production
load pieces on the persistent rank thread:

1. it validates rank, conversion, file, manifest, descriptor, payload,
   policy, kernel ABI, model, tokenizer, template, operation manifest,
   profile budget, tensor contract, byte count, and tensor count against the
   immutable `RankSetLoadPlan`;
2. it allocates and zero-fills the exact planned weight and metadata arenas
   plus the fixed pinned staging ring;
3. `NativeRankReader::verify_and_stream` authenticates and semantically
   validates each file byte while `PlannedRankTensorSink` routes each metadata,
   primary, and auxiliary plane to the plan-derived offset;
4. it drains all H2D work, seals complete-arena expected hashes, reads both
   complete arenas back through bounded pinned chunks, and requires exact
   expected/observed SHA-256 equality;
5. it constructs typed load evidence and a prepared receipt only after all
   preceding checks pass; and
6. it returns a thread-affine `PreparedCudaRank`, which exposes no device
   pointer.

The typestate sequence is:

```text
PreparedCudaRank
    -- common PreparedRankSet --> AcknowledgedCudaRank
    -- final AdoptedRankSetReceipt --> CudaWeightArena
```

The first transition validates the common plan and rank-set receipt and emits
one rank acknowledgement. The second validates the final four-rank receipt
and consumes the lifecycle's unique execution permit. Only
`CudaWeightArena` exposes the immutable device pointers. Explicit aborts from
either quarantined state synchronize and release the physical arena.

## Canonical verification evidence

`RankLoadVerificationEvidence` is exactly 256 bytes and is hashed with domain
`glmaxx.rank-load-verification-evidence.v1\0`. Its canonical little-endian
record binds:

- magic, version, record length, rank, and verification mode;
- load-plan and device-identity SHA-256 values;
- nonzero allocation generation;
- authenticated file payload bytes and tensor count;
- exact uploaded metadata, primary, auxiliary, and total bytes;
- maximum ordinary reader scratch and fixed pinned-ring bytes;
- separately accumulated storage read, ordinary-to-pinned copy, H2D
  submission, H2D drain, and full-arena readback nanoseconds;
- the 208-byte CUDA full-arena readback subrecord digest; and
- a required nonzero software-provenance SHA-256.

Construction rejects a payload proof, upload category, rank, generation,
arena size, plan, readback chunk size, expected/observed device hash, timing,
or provenance mismatch. `PreparedRankReceipt::new` accepts this typed value,
not an arbitrary digest. The arbitrary constructor is compiled only for unit
tests. The canonical fixture digest is
`41839340fe94fc6dd7d5d9084497c25fe6748d8375da149c3c50bc553a82a46a`.

`NativeRankReader` measures only wall time spent inside its positional
`read_exact_at` operations and checked-adds the result. The arena separately
measures ordinary-to-pinned copies, asynchronous H2D submission, final H2D
drain, and full D2H verification. These fields separate storage, host copy,
submission, drain, and verification costs; they are evidence plumbing, not a
performance result on this host.

## Persistent rank ownership

`RankExecutor` no longer requires `Send`. A `RankExecutorFactory` is `Send`
only while crossing into a newly spawned rank thread; its `create` method
runs there and may return a non-`Send` executor. Consequently a future native
executor can create and retain its CUDA context, streams, graphs, arenas, and
collective handles on exactly one owner thread.

The startup handshake now reports factory creation failure to the dispatcher.
The pool becomes usable only after four distinct ranks initialize. Any
partial startup failure closes the command channels and joins every created
rank worker before returning. The existing transferred-executor API remains
available but requires `Box<dyn RankExecutor + Send>`, making the crossing
explicit.

A focused test constructs each executor with `Rc<()>`, proving it is
non-`Send`, records the creator thread, and rejects execution from any other
thread or rank. The normal persistent-worker, failure, bounded-queue,
page-mirror, and serving tests continue to pass through the stricter API.

## CPU proof

The candidate adds:

- exact encoding and negative tests for typed 256-byte load evidence;
- category-specific upload accounting derived from the plan;
- storage-read timing in the native reader proof;
- arena timing and device-readback binding;
- prepared and acknowledged CUDA-rank typestate tests that retain quarantine
  until the final global receipt; and
- a rank-thread factory test with a provably non-`Send` executor.

At exact candidate `bc4b3d0cec9f8f8bc8ae368e2740377f923d5f6e`,
`scripts/local-checks.sh` passed:

- 326 Rust unit and integration tests with zero failures;
- workspace formatting and Clippy with warnings denied;
- CUDA-FFI host type checking and Clippy with warnings denied;
- deterministic CPU, matrix, manifest, native-rank, engine, serving, and
  cache proof regeneration and byte comparison;
- all 82 then-current review handoffs, with no configured review result
  accepted by the local verifier; and
- C++ syntax parsing of `kernels/include/glmaxx_kernel.h`.

The external tokenizer proof was skipped because `GLMAXX_TOKENIZER_DIR` was
unset. The local host has no `nvcc`.

## Explicit exclusions

No CUDA translation unit was compiled, no native library was linked, no
pinned host or device allocation was made, and no GPU work ran. This
candidate makes no native CUDA, SM120, capacity, quality, or performance
claim.

The worker protocol does not yet carry prepare, adoption, finalization, abort,
or cleanup-ack commands. `PreparedCudaRank::load` is therefore implemented
but not invoked by `Tp4WorkerPool`. The native rank context's observed device
identity is not yet checked against the plan at this boundary. No full
production rank file was loaded; a valid production reader requires the
pinned 533-tensor contract, so the repository does not create a misleading
reduced file that production would correctly reject. The production-shaped
small checkpoint is the first honest end-to-end file fixture.

No four-rank physical cleanup acknowledgement, timeout, context integration,
small-checkpoint smoke, checkpoint execution, or startup-health transition
is claimed. Those are the next implementation and qualification gates.
