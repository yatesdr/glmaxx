# Fable handoff: native checkpoint rank adapter v1

Date: 2026-07-30

Status: adversarial host implementation and ownership review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4, compile
remotely, allocate a GPU, or launch CUDA for this review

Review candidate commit:
`b62325a47eaee6b78bd70ec29e3ea29cea48533e`

Required result path:
`docs/reviews/fable-native-checkpoint-rank-adapter-v1.md`

Requested acceptance token, only if every blocker and major is resolved:
`native-checkpoint-rank-adapter-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
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
| `docs/native-checkpoint-rank-adapter-proof-v1.md` | `ce55dbdb0eb12a60c891f945312899a0a32e783a0cf83bc7fb3e729210be5129` |
| `docs/sm120-rank-runtime.md` | `e52ecab7bd378d6aed2c9033ff6404a34c880ed860a3ac9e7ed4c8d4d0a11b02` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-native-checkpoint-rank-adapter-v1-handoff.md
cargo test --offline -p glm-engine checkpoint_cuda
cargo test --offline -p glm-engine worker
cargo clippy --offline --workspace --all-targets -- -D warnings
GLMAXX_KERNEL_LIB_DIR=/tmp cargo clippy --offline -p glm-cli \
  --features cuda-ffi -- -D warnings
```

## Review boundary

This review covers the feature-gated, thread-affine
`NativeCheckpointRankExecutor`; its factory integration; ownership of
`NativeRankContext`, `NativeRankReader`, and CUDA checkpoint typestates; exact
attempt validation; all prepared/acknowledged/resident abort routes; field
drop ordering; startup error retention; and the explicit fail-closed step
stub.

It assumes the earlier reader, CUDA arena, rank-local loader, and TP4
transaction reviews only at their typed boundaries. It does not accept those
candidates by implication.

It does not accept a worker-observed identity-discovery phase, automatic load
plan construction, CUDA link or execution, an actual checkpoint load,
physical cleanup evidence, target-layer execution, serving health, SM120
correctness, capacity, quality, or performance.

## Required adversarial questions

1. Verify every candidate input hash at review start and finish. Does the
   proof describe the exact candidate rather than moving `main`?
2. Does each factory closure retain exactly one rank path, and is the native
   executor constructed only after its persistent rank thread starts? Can a
   context, reader, backend, arena, or non-`Send` object cross threads?
3. On invalid rank, zero provenance, bind failure, reader-open failure, or
   reader-rank mismatch, are all constructed native objects dropped on the
   owner thread and is the exact startup error retained with its rank?
4. If any one factory fails after peers initialize, does the existing startup
   route close their channels, join them, and drop their contexts on their
   respective owner threads before returning?
5. At prepare entry, are rank, nonzero generations, plan membership, vacant
   state, and lack of another active attempt required? Is the exact attempt
   identity recorded before any backend/load failure that will require a
   common cleanup acknowledgement?
6. Does prepare call the real `NativeRankContext::checkpoint_load_backend`
   and `PreparedCudaRank::load` with the retained reader, exact plan, owner
   generation, and nonzero software provenance? Can any fake or CPU arena
   enter the native state?
7. Does acknowledgement bind the active plan and owner generation to this
   rank's receipt in the one common `PreparedRankSet`? On a consuming
   transition failure, is the quarantined arena cleaned while the attempt
   identity remains available for common abort?
8. Does finalization require the active plan and one common
   `AdoptedRankSetReceipt`? Is `CudaWeightArena` the only resident variant,
   and does the active identity remain after local success so a later
   cross-rank finalize failure can clean this rank?
9. Enumerate abort from vacant, prepared, acknowledged, resident, and
   cleanup-failed states. Are rank, plan, attempt, and owner generation
   validated before consumption? Does every success mean no resource remains?
10. Can a cleanup error or repeated/mismatched abort produce a false cleanup
    acknowledgement? Does a mismatched abort preserve the valid state until
    fatal executor drop?
11. Verify Rust field-drop order for `weights`, `context`, and `reader`. Can
    any explicit or implicit teardown destroy the context stream before the
    arena/backend synchronizes and releases its resources?
12. Follow all `mem::replace` consuming transitions through success, typed
    error, and panic. Is any state lost, double-freed, exposed early, or left
    with a reusable attempt identity?
13. Are full startup errors retained while in-transaction native errors are
    intentionally mapped to fail-closed `LoadPlanError` classes? Does the
    proof accurately disclose the diagnostic loss?
14. Do `execute` and `execute_bound` refuse wrong-rank, nonresident, and
    resident work without calling CPU output or constructing tokens? Is the
    resident error unmistakably “target-layer program absent” rather than a
    successful smoke?
15. Does `spawn_native_checkpoint_loaders` overclaim what it returns? Is the
    absence of worker-observed identity discovery and automatic plan
    construction accurately identified as the next host gate?
16. Are the exact 332-test result, host-only CUDA-FFI checks, absent native
    link/launch, and every checkpoint/GPU/serving exclusion accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- construction and destruction remain on one persistent rank thread;
- partial native startup cleans every successfully created peer;
- attempt identity is retained across every pre-publication failure;
- prepare invokes the real reader/backend/CUDA typestate composition;
- acknowledgement retains quarantine and binds the common prepared set;
- finalization alone creates the resident CUDA arena;
- locally resident ranks remain abortable after a peer finalize failure;
- every state has a truthful physical cleanup route;
- cleanup failure and repeated/mismatched abort cannot forge an ack;
- arena/backend drop precedes context-stream destruction;
- consuming transitions neither leak nor double-free safe Rust ownership;
- native error classification is fail-closed and its lossiness disclosed;
- both step methods contain no CPU or fabricated-output fallback;
- missing identity discovery and plan construction are disclosed;
- proof claims and explicit exclusions are accurate; and
- the boundary is ready for worker-observed identity/startup composition.

Only if all sixteen answers are unqualified `YES`, end with the requested
token. Withhold it for a stale candidate, cross-thread native state,
partial-startup leak, missing attempt identity, fake backend, adoption bypass,
premature resident pointer, unabortable finalized rank, false cleanup ack,
wrong drop order, unsafe consuming transition, recoverable native-error
misclassification, CPU fallback, hidden plan-construction gap, native/GPU
overclaim, or evidence overstatement.

The token accepts only this host-type-checked native checkpoint rank adapter.
It does not authorize cn4 access or accept identity discovery, automatic plan
construction, native compilation/linking, a checkpoint load, physical GPU
cleanup, target-layer execution, serving, SM120 correctness, capacity,
quality, or performance.
