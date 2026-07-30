# Fable handoff: rank-local checkpoint loader v1

Date: 2026-07-30

Status: adversarial CPU implementation and native-ABI review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4, compile
remotely, allocate a GPU, or launch CUDA for this review

Review candidate commit:
`9c345421557a0a4e290831c61afcf65cf3f53a10`

Required result path:
`docs/reviews/fable-rank-local-checkpoint-loader-v1.md`

Requested acceptance token, only if every blocker and major is resolved:
`rank-local-checkpoint-loader-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |
| `crates/glm-cuda/Cargo.toml` | `90f62562daafa36a1e6a0926fdd962f23e6ec2c78946e10da70b70ae1b39899c` |
| `crates/glm-cuda/src/load.rs` | `3ffa499c9c985954fc9847d4d75a7356a8be33ded6b77afd6fc593f2e5833392` |
| `crates/glm-cuda/src/ffi.rs` | `eee9a388c8c25b9d385ddf0159c7dc766d1a19560078f2915d22fe77d1642817` |
| `crates/glm-engine/Cargo.toml` | `1e4fafe2079b1f529a35b6153cdbfcc39420728fcba0cb72c5920715cf591401` |
| `crates/glm-engine/src/checkpoint_cuda.rs` | `6bceb0e8fa8c32e8d981bc3044163b03f5d2bcc5b3c3527ccfed215012240cc4` |
| `crates/glm-engine/src/checkpoint_load.rs` | `736b8f7dd75e470a9fdae83376df40165245f6a0683fe6846f8df9f022a4e09f` |
| `crates/glm-engine/src/lib.rs` | `dfccf9816ea433d8a4be3480c3e1e46ae5f353cc0221193bf49361c6c391170c` |
| `crates/glm-engine/src/worker.rs` | `7791af31f91f8962fc21e3701d337c7e044d4844d88fc5b69e9926dbf59c66fd` |
| `crates/glm-format/src/native_reader.rs` | `953a56702ba1ee000f508fe24cbbae7c6137d6496d104720f658fede5572699c` |
| `crates/glm-serving/src/backend.rs` | `afcca1f6f50cb7699ef24cfc32e45fa005a6a8663efb7dc36806b2305ff295d4` |
| `crates/glm-serving/src/lib.rs` | `bc7eff0297e14b73df7eec5ade3352ad0f75ceabeaca1862c4866a51efb948e3` |
| `kernels/include/glmaxx_kernel.h` | `e98f4362af98dd3e23849d62f2ef1b47cb75d3d7a71439c5f9f5577c421f1d07` |
| `kernels/sm120/nvfp4_routed_fc1.cu` | `52bb5b1a91d54cb11125084513032a56957f051d22b5d583717d599f9f0bf51f` |
| `docs/checkpoint-load-transaction-v1.md` | `184199144075e3f7c511b31002ea81c1d2dbaad1087e01fe7dd4533dc36a3c21` |
| `docs/cuda-checkpoint-arena-cpu-proof-v1.md` | `399c5b9a276870210d1e389269b602862091e3ef42ccc22a2175ad7589a09a76` |
| `docs/rank-set-load-coordinator-proof-v1.md` | `4076872eec5adcdb2f4a0445418ed58d695a188a1d8aeb4e95496ef7ec52196a` |
| `docs/sm120-rank-runtime.md` | `e52ecab7bd378d6aed2c9033ff6404a34c880ed860a3ac9e7ed4c8d4d0a11b02` |
| `docs/rank-local-checkpoint-loader-proof-v1.md` | `62fd339b8f8d6b45a72af0c3e4e13af2eddf64baad7ec722f964e4e0089ed0ea` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-rank-local-checkpoint-loader-v1-handoff.md
cargo test --offline -p glm-engine checkpoint_cuda
cargo test --offline -p glm-engine checkpoint_load
cargo test --offline -p glm-engine worker
cargo test --offline -p glm-format native_reader
cargo clippy --offline --workspace --all-targets -- -D warnings
GLMAXX_KERNEL_LIB_DIR=/tmp cargo clippy --offline -p glm-cli \
  --features cuda-ffi -- -D warnings
clang++ -std=c++17 -fsyntax-only -x c++ kernels/include/glmaxx_kernel.h
```

## Review boundary

This review covers the CPU-verifiable composition of an authenticated
production `NativeRankReader`, deterministic load plan, planned tensor sink,
quarantined CUDA arena, full device-readback proof, canonical 256-byte rank
evidence, prepared receipt, adoption typestates, and a persistent rank-thread
factory capable of owning a non-`Send` native executor.

It also covers the corrected device-observation boundary: the native C ABI
returns the CUDA UUID, Rust defines the canonical observed-device digest,
`NativeRankLoadBackend` retains it on the context owner thread, and
`PreparedCudaRank::load` must compare it with the planned rank before
allocation.

It assumes the earlier reader, load-plan, arena, and four-rank coordinator
reviews only at their typed boundaries. It does not accept those candidates
by implication.

It does not accept the not-yet-implemented worker prepare/adopt/finalize/abort
wire protocol, four physical cleanup acknowledgements, a production rank-file
load, native compilation/linking, real pinned/HBM allocation, checkpoint
smoke, SM120 execution, capacity, quality, or performance.

## Required adversarial questions

1. Verify every declared candidate hash twice. Are the proof and handoff
   pinned to the bytes actually reviewed, including the CUDA UUID ABI
   correction?
2. Is the C/CUDA/Rust `glmaxx_device_bind` signature ABI-consistent? Does it
   return all 16 UUID bytes only after successful device binding and property
   lookup, reject null outputs, and avoid returning a partially valid
   identity?
3. Independently reconstruct `NativeDeviceIdentity::identity_sha256`. Is the
   domain and little-endian preimage injective over visible count, visible
   index, compute capability, SM count, memory bytes, and UUID? Can a
   reordered device set or another visible index reuse a digest?
4. Does `RankLoadBackend::device_identity_sha256` enforce the same owner
   thread as every CUDA operation? Does `PreparedCudaRank::load` reject zero
   or mismatched observed identity before allocating any resource?
5. Enumerate every reader/plan comparison. Can conversion, file, manifest,
   descriptor, payload, policy, kernel ABI, model, tokenizer, template,
   operation, budget, tensor contract, byte count, rank, or tensor count
   drift while a prepared receipt is emitted?
6. Follow ownership through every failure point after allocation: planned
   sink construction, reader validation/streaming, H2D, sealing, D2H
   readback, evidence construction, receipt construction, and lifecycle
   preparation. Does RAII safely synchronize and release the arena without
   pointer exposure or a double free?
7. Recompute the 256-byte `RankLoadVerificationEvidence` field offsets and
   fixture digest
   `41839340fe94fc6dd7d5d9084497c25fe6748d8375da149c3c50bc553a82a46a`.
   Are reserved bytes canonical and every integer little-endian?
8. Can production code forge a successful full-arena readback subrecord or
   pass an arbitrary evidence digest to `PreparedRankReceipt::new`? Do
   test-only constructors stay absent from non-test builds?
9. Are metadata, primary, auxiliary, and total upload counts independently
   derived from the plan and exactly compared? Can overlap, gaps, zero tails,
   or a category swap retain the same accepted evidence?
10. Are storage read, ordinary-to-pinned copy, H2D submission, H2D drain, and
    full readback measured at the named boundaries with checked arithmetic?
    Does the proof correctly avoid treating host wall-clock fields as GPU
    performance evidence?
11. Prove the `PreparedCudaRank -> AcknowledgedCudaRank -> CudaWeightArena`
    sequence. Can any safe path obtain a device pointer before the final
    four-rank receipt or adopt with a stale plan/allocation generation?
12. Is `RankExecutorFactory` the only object crossing the thread boundary in
    the factory path? Can it create a genuinely non-`Send` executor on the
    rank thread, and do all later calls and destruction remain there?
13. On one factory failure or partial thread-spawn failure, does startup close
    all channels and join every created worker before returning? Can the pool
    appear healthy with fewer than four distinct initialized ranks?
14. Does removing `Send` from `RankExecutor` introduce any unsound transfer
    through the old spawn API, serving adapters, response channels, panic
    cleanup, or trait-object coercions?
15. Are the absence of a reduced “production” native file, the 533-tensor
    requirement, and the production-shaped small-checkpoint next gate
    accurate? Is any untested composition being mislabeled as executed?
16. Are the exact 327-test result, host/FFI type checks, lack of `nvcc`, and
    every GPU, checkpoint, integration, capacity, quality, and performance
    exclusion accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- observed CUDA identity is UUID-bound, canonical, and checked before load;
- reader and plan identities are exhaustively bound;
- every partial load failure retains safe arena ownership;
- the 256-byte evidence encoding and pinned digest are exact;
- upload categories and full-arena readback cannot be forged in production;
- timing categories match their claimed boundaries;
- quarantine prevents pointer exposure before final four-rank adoption;
- the factory creates and retains non-`Send` state on one rank thread;
- partial startup failure joins every created worker;
- the old transferred-executor API remains explicitly `Send`;
- proof claims and explicit exclusions are accurate; and
- the boundary is ready for the four-rank worker load protocol.

Only if all twelve answers are unqualified `YES`, end with the requested
token. Withhold it for a stale candidate, device-identity replay, UUID ABI
mismatch, allocation before device validation, missing reader/plan binding,
unsafe partial cleanup, forgeable readback/evidence, pointer exposure before
global adoption, cross-thread native state, partial healthy startup, hidden
GPU/checkpoint claim, or evidence overstatement.

The token accepts only this CPU-proven rank-local loader and native identity
contract. It does not authorize cn4 access or accept the four-rank wire
protocol, a production checkpoint load, a CUDA compile, a GPU launch, SM120
execution, capacity, quality, or performance.
