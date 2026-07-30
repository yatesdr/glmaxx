# Fable handoff: CUDA checkpoint arena CPU v1

Date: 2026-07-30

Status: adversarial implementation and corrected-contract review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4, compile
remotely, allocate a GPU, or launch CUDA for this review

Review candidate commit:
`c870454025ea9a401646155c010e1032b23659d8`

Required result path:
`docs/reviews/fable-cuda-checkpoint-arena-cpu-v1.md`

Requested acceptance token, only if every blocker and major is resolved:
`cuda-checkpoint-arena-cpu-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `Cargo.lock` | `7682aa97e756b9847f154630fedb54d5cdbc389c0b178c6a83cc9007002d56e6` |
| `crates/glm-cli/Cargo.toml` | `cb88b0b20dd96a713a16bb38b07333c985e791f67cd43252ec42fa2ad562752c` |
| `crates/glm-cuda/src/load.rs` | `89eb00dbf649b99355fb4a7ceb776e975d96a7f75240ade095ec5adbef0fecee` |
| `crates/glm-cuda/src/ffi.rs` | `57e97e8f98defcf9bbb6340fd059d6d3a7fdb2caa39e97d1f129f9157d3a4645` |
| `crates/glm-cuda/src/lib.rs` | `ee20a1c568c7347b8c0660096ce4c4fc4824411be3e9ecaa943cbeda05e3d5ae` |
| `crates/glm-engine/Cargo.toml` | `1e4fafe2079b1f529a35b6153cdbfcc39420728fcba0cb72c5920715cf591401` |
| `crates/glm-engine/src/checkpoint_cuda.rs` | `56c58a10ec5197753b07e52d4936fa55e134a630e88d54e734fb4439a1d700c3` |
| `crates/glm-engine/src/checkpoint_load.rs` | `73902c8c451f8208fd15609ac5800b0d4edf1f087409cf02ebdbc9fa39d0f07b` |
| `crates/glm-engine/src/lib.rs` | `ab94d22113b7f1f0f4122f134fffc682d5c2abfb4fdd6c38135f3e00dca91174` |
| `kernels/include/glmaxx_kernel.h` | `a96e58247db77f78d44b5763537bad20318c8dbaaec645ee96560bc9c6a82776` |
| `kernels/sm120/nvfp4_routed_fc1.cu` | `40feb292c455db8e14cb22411be42817fc2331ff0ef874857be6a28e3993ec8e` |
| `docs/checkpoint-load-transaction-v1.md` | `9b7625e0d8c2ca18f287303b9aaa831b442956e0f8b8c8617ab5c74649e29cf3` |
| `docs/cuda-checkpoint-arena-cpu-proof-v1.md` | `399c5b9a276870210d1e389269b602862091e3ef42ccc22a2175ad7589a09a76` |
| `docs/checkpoint-load-cpu-core-proof-v1.md` | `a3cbd93be0b7f131d98d996601c75e653764ec429839f19e2c26835fa4bd20c1` |
| `docs/rank-set-load-coordinator-proof-v1.md` | `4076872eec5adcdb2f4a0445418ed58d695a188a1d8aeb4e95496ef7ec52196a` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-cuda-checkpoint-arena-cpu-v1-handoff.md
cargo test --offline -p glm-engine checkpoint_cuda
cargo clippy --offline -p glm-cuda -p glm-engine --all-targets -- -D warnings
GLMAXX_KERNEL_LIB_DIR=/tmp cargo clippy --offline -p glm-cli \
  --features cuda-ffi -- -D warnings
clang++ -std=c++17 -fsyntax-only -x c++ kernels/include/glmaxx_kernel.h
```

## Review boundary

This review covers the CPU-proven generic checkpoint arena, native Rust/C ABI
surface, fail-closed ownership and teardown rules, mandatory bounded
full-arena read-back subrecord, and the r3 corrections to the governing load
contract.

It assumes the authenticated reader, deterministic plan, receipt/coordinator,
and adoption-permit contracts only at their typed boundaries. It does not
accept their separate candidates by implication.

It does not accept the not-yet-implemented 256-byte full-rank verification
evidence record, reader-to-arena rank command, prepared-receipt integration,
cleanup acknowledgements, native compilation/linking, real pinned/HBM
allocation, full-rank files, a checkpoint smoke, SM120 behavior, capacity,
quality, or performance.

## Required adversarial questions

1. Are `NativeRankLoadBackend`, `CudaArenaResources`, the quarantined arena,
   and adopted arena actually thread-affine and non-`Send`? Can any safe API
   use a rank's CUDA handles from another thread?
2. Walk every partial allocation point. Does RAII retain enough ownership to
   synchronize and release every successfully created stream, allocation,
   pinned slot, and event without double free? Can duplicate native handles
   overwrite a live registry entry?
3. Does every upload copy the reader's borrowed bytes into an owned pinned
   slot before returning? Is a slot synchronized before reuse, is H2D ordered
   after whole-arena zero-fill, and is the steady upload path free of
   per-chunk allocation?
4. Do monotonic offsets, checked arithmetic, arena capacity, and exact byte
   hashing cover every payload byte plus every zero gap and tail? Can the
   planned tensor order legally violate the writer's monotonic-plane rule?
5. Does sealing wait every in-flight event and synchronize the stream before
   finalizing expected hashes? Can any post-seal, post-poison, empty, overlap,
   or out-of-order write change the expected evidence?
6. Independently reconstruct the D2H loop. Does it read every byte of both
   arenas in bounded 8 MiB chunks, record and synchronize before host access,
   handle a partial final chunk, avoid stale pinned bytes, and poison every
   asynchronous or digest failure?
7. Recompute the exact 208-byte read-back subrecord and pinned fixture digest
   `e863578068c1fd64bfde6ab11682e4d4770fd1d1f5aebff712fbe232e66dec87`.
   Is the encoding injective and domain-separated?
8. Is successful read-back required before adoption? Can a caller forge
   `content_verified`, construct the evidence directly, reveal device
   pointers from quarantine, or use a permit for another
   rank/plan/allocation generation?
9. On event-record failure, explicit abort, normal shutdown, free failure,
   and stream-synchronization failure, is teardown ordered safely? Does a
   synchronization failure free nothing, and does implicit drop terminate
   rather than continue after an unreported fatal error?
10. Does the native backend validate owner thread, exact handle ownership,
    all device and pinned ranges, nonzero byte counts, and live streams/events
    before every unsafe FFI call? Is there any context-rebind or backend
    lifetime hole that the future persistent-rank integration must close?
11. Are the thin C ABI additions correct for 64-bit Linux CUDA, and do the
    Rust declarations, C header, definitions, feature forwarding, and
    ownership assumptions agree exactly?
12. Does the memory-faithful fake backend actually model zero-fill, H2D, D2H,
    pinned reuse, corruption, and freeing rather than merely asserting call
    logs? Are allocation failures, implicit-drop abort, and native
    asynchronous failure accurately excluded rather than silently claimed?
13. Do the r3 contract amendments fully resolve the r2 review's two MAJORs:
    pinned-ring lifetime under every teardown and mandatory end-to-end HBM
    content proof? Are its canonical zero-gap, common plane-length,
    TP-divisibility, tensor-count, field-serialization, source-shape, and
    verification-evidence corrections internally consistent?
14. Are the proof's exact candidate, 323-test result, pinned digest, native
    typecheck claim, lack of `nvcc`, and all GPU/integration/performance
    exclusions accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- rank-local native ownership is thread-affine and fail-closed;
- partial allocation and normal teardown are RAII-safe;
- the fixed pinned ring closes the borrowed-buffer lifetime;
- whole-arena expected hashing includes canonical zero gaps and tails;
- bounded D2H verification proves the staged device contents before adoption;
- the read-back evidence encoding and pinned digest are exact;
- quarantined pointers cannot become executable before global adoption;
- failed synchronization cannot cause pinned-memory or device-memory
  use-after-free;
- the thin native ABI and feature wiring are internally consistent;
- the r3 contract resolves every r2 blocker/major for this boundary; and
- proof claims and explicit exclusions are accurate.

Only if all eleven answers are unqualified `YES`, end with the requested
token. Withhold it for a stale candidate, cross-thread handle path, partial
allocation leak hidden by the proof, borrowed-buffer retention, slot reuse
race, unverified gap/tail, incomplete D2H coverage, forgeable adoption state,
pointer exposure before global adoption, unsafe failed-synchronization free,
ABI mismatch, unresolved r2 major, or evidence overstatement.

The token accepts only this CPU-proven arena/native-ABI contract. It does not
authorize cn4 access or accept a native CUDA compile, checkpoint smoke, full
rank load, SM120 execution, capacity, quality, or performance.
