# Fable handoff: native TP4 checkpoint-load smoke v1

Date: 2026-07-30

Status: adversarial host implementation, teardown, budget, and evidence review
requested

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4, compile remotely, allocate a GPU, or
launch CUDA for this review

Review candidate commit:
`1770563713722685db26b0d3378f32e4ecf92519`

Required result path:
`docs/reviews/fable-native-checkpoint-load-smoke-v1.md`

Requested acceptance token, only if every blocker and major is resolved:
`native-checkpoint-load-smoke-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |
| `crates/glm-engine/Cargo.toml` | `1e4fafe2079b1f529a35b6153cdbfcc39420728fcba0cb72c5920715cf591401` |
| `crates/glm-cuda/Cargo.toml` | `90f62562daafa36a1e6a0926fdd962f23e6ec2c78946e10da70b70ae1b39899c` |
| `crates/glm-cli/Cargo.toml` | `cb88b0b20dd96a713a16bb38b07333c985e791f67cd43252ec42fa2ad562752c` |
| `crates/glm-engine/src/worker.rs` | `b8498639bb05ef84c2d06eb1e4650d8f7915eb1e3b306abdfd2cc0fb93b104fa` |
| `crates/glm-engine/src/native_worker.rs` | `5c98f827b0ca8a9dbf5f77b601f5c20588101ea222c20db6480cb497693f1150` |
| `crates/glm-engine/src/memory.rs` | `0ae657905a1b2091980c4904643e35a7a53b282ef112be44447362add89f023b` |
| `crates/glm-engine/src/checkpoint_load.rs` | `21fe9b2eb973ff6b446671e68002fafd3df975fe981a0f89ea97f0adcae8560f` |
| `crates/glm-engine/src/checkpoint_cuda.rs` | `6bceb0e8fa8c32e8d981bc3044163b03f5d2bcc5b3c3527ccfed215012240cc4` |
| `crates/glm-engine/src/lib.rs` | `e3a70f7906c7a0d33a6a43e8bf791e1de0daa47e4ab918825adb47ecd64fb4b9` |
| `crates/glm-cuda/src/ffi.rs` | `870ef7570d8f476cdaf32cea4fc36ac63ab4619b0db63d41262a258feb7d3663` |
| `crates/glm-cuda/src/lib.rs` | `63d7c260104dfb93baba9689251a3057b77e5d88cec31a14e0c5648ad48d0a6f` |
| `crates/glm-format/src/native_reader.rs` | `953a56702ba1ee000f508fe24cbbae7c6137d6496d104720f658fede5572699c` |
| `crates/glm-cli/src/main.rs` | `e486980e499bb861c5a34bd8d0b1b672ab1c60dacc1af8b0d02340683fc328ca` |
| `kernels/include/glmaxx_kernel.h` | `a7ddb56de39dbd22e25184be1a2a767dd43bc3ca5ecafd3dcc771aedebbdcf13` |
| `kernels/sm120/nvfp4_routed_fc1.cu` | `9786c68795c6e75f148192a49aec4a6845e81e3c5e1df1e561163da34204eb28` |
| `scripts/cn4-checkpoint-load-smoke.sh` | `fb4f82b76fe7d43dfa29168a1a9f9b3dab138a065f410a1dab31e26fe5ac5e36` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |
| `docs/sm120-rank-runtime.md` | `b2d5aceed8411402f512fc72889d756e178ca9de8bbcda54a208f0be9ea56b68` |
| `docs/native-checkpoint-load-smoke-proof-v1.md` | `2b6c5ef4cb3e37c25b2260ec165ffbc8a3c416d85d611a48b08c256e4fc6a3a2` |
| `docs/checkpoint-load-transaction-v1.md` | `184199144075e3f7c511b31002ea81c1d2dbaad1087e01fe7dd4533dc36a3c21` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-native-checkpoint-load-smoke-v1-handoff.md
cargo test --offline -p glm-engine
cargo clippy --offline --workspace --all-targets -- -D warnings
GLMAXX_KERNEL_LIB_DIR=/tmp cargo clippy --offline -p glm-cli \
  --features cuda-ffi -- -D warnings
bash -n scripts/cn4-checkpoint-load-smoke.sh
```

Also invoke `scripts/cn4-checkpoint-load-smoke.sh` without its authorization
variable and require exit 64 before any inventory, compiler, or CUDA command.

## Review boundary

This review covers:

- conversion of a completed measured profile budget into one validated,
  canonical, hash-bound system memory plan;
- operation-manifest, profile-budget, weight-policy, and linked
  codec-capability binding before native startup;
- the owner-thread live-free-HBM check;
- physical checkpoint-arena reconciliation with the memory-plan weight and
  immutable-metadata terms;
- the explicit four-rank normal weight teardown and terminal worker
  generation;
- the feature-gated CLI's pass-record timing and provenance;
- the cn4 wrapper's operator, topology, occupancy, source, toolchain,
  directory, symbol, cleanup, and evidence gates; and
- the CPU fault tests and proof exclusions.

It assumes earlier native reader, load-plan, CUDA-arena, rank-adapter,
transaction, and startup-composition reviews only at their typed boundaries.
It does not accept those candidates by implication.

It does not accept a CUDA compile, native link, cn4 execution, real checkpoint
load, target-layer execution, serving, quality, capacity-in-practice, or
performance result.

## Required adversarial questions

1. Verify all candidate input hashes at review start and finish. Does the
   review stay pinned even if `main` advances?
2. Does the wrapper reject missing authorization on its first executable
   branch, before `nvidia-smi`, compilation, context creation, or any other
   GPU interaction?
3. Are the evidence and checkpoint directories required to be external, and
   are the exact four rank filenames, symlink posture, hard-link posture, and
   reader fingerprints fail-closed?
4. Does the CLI require a complete, conversion-approved, four-rank measured
   profile budget whose exact bytes match all four checkpoint manifests?
5. Can unknown JSON, false completion, absent per-rank post-context
   measurement, arithmetic drift, rank reordering, profile drift, or a
   one-rank fit failure enter the executable system plan?
6. Independently mutate every public aggregate/cache/rank field in a
   `SystemMemoryPlan`. Does `validate` reconstruct and reject the lie, and is
   the SHA-256 derived only from the validated canonical artifact bytes?
7. Can a caller substitute an unrelated memory-plan digest or an artificially
   small minimum-free value through `NativeCheckpointStartupConfig`, or are
   both derived inside the startup function from the typed plan?
8. Independently rederive the per-rank physical checkpoint allocation. Does
   startup require file payload bytes to equal the weight term and aligned
   device weight plus codec-metadata arenas to fit the combined weight and
   immutable-model-metadata terms? Is any physical arena byte hidden outside
   the required-HBM total?
9. Is `glmaxx_device_memory_info` a correct thin `cudaMemGetInfo` ABI on the
   already selected owner thread? Are null outputs, CUDA errors, zero free
   bytes, total-memory drift, and free-greater-than-total rejected?
10. Immediately before allocation, does each exact persistent rank compare
    its own live free bytes to its own validated required bytes? Can aggregate
    memory, rank-local fallback, a coordinator observation, or a stale
    pre-context observation substitute?
11. Does the codec-capability digest have a fixed-width, platform-independent
    canonical encoding? Does it validate the linked NVFP4 and all fixed EXL3
    projection ABI/workspace routes before advertising them, without claiming
    that a model kernel was launched?
12. Does the executable hash bind the actual running binary, and do compiled
    operation-manifest, capacity weight-policy, kernel ABI, checkpoint
    manifest, and profile-budget identities all meet before allocation?
13. Trace successful load teardown from resident arenas through one common
    plan/attempt command. Must every acknowledgement match rank, plan, attempt,
    and owner generation, with release exactly once?
14. Trace cleanup failure at each rank, timeout, channel close, malformed
    acknowledgement, repeated shutdown, and shutdown before load. Can any
    partial set become success or leave the worker generation usable?
15. Does `LoadedNativeCheckpoint::shutdown` consume the owner, wait for four
    acknowledgements, and join dispatcher/rank threads before returning?
    Does the proof accurately preserve the permanently-wedged-thread
    limitation?
16. Can `summary.json` or the pass verdict be written before full payload
    SHA-256, full device-arena readback, global finalize, four cleanup
    acknowledgements, and owner-thread joins?
17. Do the summary's per-rank file, device, arena, verification, finalize,
    HBM, and cleanup fields come from typed outcomes rather than caller
    labels? Are the elapsed intervals and `model_kernel_launched=false`
    truthful?
18. Audit every shell expansion and pipeline. Can whitespace, globbing,
    an existing evidence path, dirty worktree, stale CUTLASS, missing symbol,
    occupied GPU, non-SM120 device, command failure, or post-run live process
    be converted into a pass?
19. Is omitting a second raw 326-GiB `sha256sum` honest because the typed
    loader performs the full per-rank payload hashes and records them? Does
    the evidence still pin file stat identities and every authenticated
    payload digest?
20. Re-run the complete CPU fault matrix and exact local gate. Are 340 tests,
    86 handoffs, the authorization exit-64 check, absent nvcc/native link/GPU
    run, incomplete checked-in budget, no model kernel, and all downstream
    exclusions stated exactly?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- host preflight binds the exact checkpoint, binary, operation manifest,
  codec capability, completed budget, and validated system plan;
- every rank's physical arenas and live HBM are conservatively bounded;
- capability hashing and native ABI checks are canonical and non-overstated;
- normal teardown is four-rank exact, terminal, and non-forgeable;
- the pass record is impossible before successful load, readback, finalize,
  cleanup, and joins;
- the wrapper is operator-gated, shell-safe, occupancy-safe, and
  provenance-complete;
- the CPU tests prove every claimed normal-cleanup and failure position;
- proof claims and exclusions match the exact candidate; and
- this host artifact is ready for a separately authorized cn4 compilation and
  checkpoint-load qualification.

Only if all twenty answers are unqualified `YES`, end with the requested
token. Withhold it for stale provenance, pre-authorization GPU work, unsafe
path handling, incomplete/forgeable budget, mutable or unbound memory plan,
hidden arena bytes, wrong-rank/stale HBM, capability overclaim, identity gap,
partial cleanup success, reusable torn-down generation, premature pass
record, shell fail-open, unverifiable payload identity, incomplete fault
matrix, or evidence overstatement.

The token accepts only the host implementation and preparation command. It
does not authorize cn4 access or accept CUDA compilation/linkage, a real
checkpoint load, model execution, serving, SM120 correctness, capacity,
quality, or performance.
