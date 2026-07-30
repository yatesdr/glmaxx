# Native checkpoint startup composition host proof v1

Date: 2026-07-30

Status: implementation candidate; adversarial review pending

GPU claim: none

## Candidate

The proved implementation is commit
`83b8effb8f455b164d04f8f3c212fc1d31bc0849`.

| Input | SHA-256 |
|---|---|
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |
| `crates/glm-engine/Cargo.toml` | `1e4fafe2079b1f529a35b6153cdbfcc39420728fcba0cb72c5920715cf591401` |
| `crates/glm-engine/src/native_worker.rs` | `9f51a29ca14589adca8a530c36f41101948e68de123f662b0c2d088bf59a7f01` |
| `crates/glm-engine/src/worker.rs` | `ae3c4345bd41725516d773032669685c0a43bc72bc09c2ffdce48e04b59f97b0` |
| `crates/glm-engine/src/lib.rs` | `415bbd961b0788057dfab8db49ab0d622e7a69bc46899417fea712062a37a366` |
| `crates/glm-engine/src/checkpoint_cuda.rs` | `6bceb0e8fa8c32e8d981bc3044163b03f5d2bcc5b3c3527ccfed215012240cc4` |
| `crates/glm-engine/src/checkpoint_load.rs` | `736b8f7dd75e470a9fdae83376df40165245f6a0683fe6846f8df9f022a4e09f` |
| `crates/glm-cuda/src/ffi.rs` | `eee9a388c8c25b9d385ddf0159c7dc766d1a19560078f2915d22fe77d1642817` |
| `crates/glm-format/src/native_reader.rs` | `953a56702ba1ee000f508fe24cbbae7c6137d6496d104720f658fede5572699c` |
| `docs/native-checkpoint-rank-adapter-proof-v1.md` | `ce55dbdb0eb12a60c891f945312899a0a32e783a0cf83bc7fb3e729210be5129` |
| `docs/tp4-checkpoint-load-protocol-proof-v1.md` | `d536e281d694eb0cbdd123d2cea9527e0d7cb9348556c6c3be0c9da13323ccb7` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

## Worker-observed identity phase

`RankExecutor` now has a checkpoint-device-identity method. The native
implementation calls the retained `NativeRankContext` on its owner thread and
returns the UUID-bound digest captured by the successful SM120 bind.

`Tp4WorkerPool::checkpoint_device_identities` is a process-common dispatcher
operation:

1. it atomically claims the pool's exclusive permit;
2. it broadcasts one identity request to all four persistent rank threads;
3. it uses one common phase deadline to collect exactly four rank responses;
4. it rank-sorts and rejects missing, duplicate, or out-of-range responders;
5. it rejects any rank error, zero digest, or repeated digest; and
6. it releases exclusivity before returning the ordered four-digest array.

Any identity-phase failure is terminal for that worker generation. There is
no rank-local retry or device remapping. A successful identity phase is
nonterminal and leaves all four native owners intact for the subsequent load.

The same exclusive-permit primitive now also protects page-table
initialization and explicit page-delta application. Their former
check-then-send sequence could race a step or load between observing zero
outstanding operations and queuing the administrative mutation. The atomic
zero-to-exclusive transition closes that race; invalid deltas and all channel
failures release the permit through RAII.

## One-call native load composition

`load_native_checkpoint` accepts four ordered paths and one
`NativeCheckpointStartupConfig`. It performs:

```text
validate scalar startup configuration
  -> open four immutable readers on the coordinator thread
  -> require encoded ranks 0, 1, 2, 3
  -> build a nonpublishable preflight plan with unique synthetic identities
  -> spawn four persistent native rank owners
  -> collect identities from those exact workers
  -> rebuild the canonical plan with the observed identities
  -> execute Tp4WorkerPool::load_weights
  -> return only after four prepare, adoption, and finalize acknowledgements
```

The synthetic identities are used only to run the existing strict plan
builder before native startup. They prove rank-set consensus, the pinned
59,585-entry tensor contract, arena arithmetic, profile, manifest, and all
non-device environment fields. The preflight plan is immediately discarded
and is never sent to a rank.

After the persistent contexts exist, the canonical plan is rebuilt from their
four observed identities and the same retained readers. The plan builder
rechecks each reader's file/path fingerprint at both builds. Each native rank
also independently opens the path and later verifies every payload byte
against that exact plan, so replacement or mutation cannot inherit the
preflight identity.

`LoadedNativeCheckpoint` retains:

- the live `Tp4WorkerPool` that owns all four resident arenas;
- the immutable canonical `Arc<RankSetLoadPlan>`;
- the complete typed `WeightLoadOutcome`; and
- the ordered worker-observed device identity digests.

It exposes read-only accessors and an ownership-preserving `into_parts`.
Dropping the wrapper drops the worker pool and its rank-owned arenas.

## Configuration and error ownership

The startup config binds:

- maximum outstanding operations;
- full-SHA verification mode and checkpoint profile;
- memory-plan and codec-capability digests;
- staging slot size and count;
- software-provenance digest;
- load-attempt generation;
- four owner-allocation generations; and
- one phase timeout.

Zero queue capacity, software provenance, load attempt, owner generation, or
timeout fails before opening a reader or starting native workers. The
preflight plan rejects the remaining malformed environment and file inputs
before native startup.

`NativeCheckpointStartupError` distinguishes configuration, rank-indexed
reader open, plan construction, worker startup/identity, and transactional
load failure. If plan rebuilding or load fails after native startup, normal
Rust scope teardown drops the pool, closes the rank channels, and joins the
owner threads. The existing process-level limitation remains: a permanently
wedged native call can make that join unbounded.

## CPU fault matrix

Four new worker tests prove:

1. a successful handshake returns rank-ordered unique identities, releases
   exclusivity, and leaves the generation able to execute a later step;
2. an identity error at each of ranks 0, 1, 2, and 3 is reported with the
   exact rank and `DeviceIdentity` phase and is terminal;
3. a zero identity at each rank, a duplicate identity, and a common-deadline
   timeout all fail closed; and
4. a delayed successful identity phase owns exclusive pool capacity and
   rejects a concurrent step until it completes.

Existing tests continue to cover abandoned step quota, administrative
saturation, rank startup cleanup, all checkpoint phase failures, and partial
cleanup acknowledgement.

## Exact local gate

At exact candidate `83b8effb8f455b164d04f8f3c212fc1d31bc0849`,
`scripts/local-checks.sh` passed:

- 336 Rust unit and integration tests with zero failures;
- workspace formatting and Clippy with warnings denied;
- CUDA-FFI host type checking and Clippy with warnings denied, including the
  native composition;
- deterministic CPU, matrix, manifest, native-rank, engine, serving, and
  cache proof regeneration and byte comparison;
- all 85 then-current adversarial-review handoffs, with no configured result
  accepted by the local verifier; and
- C++ syntax parsing of `kernels/include/glmaxx_kernel.h`.

The external tokenizer proof was skipped because `GLMAXX_TOKENIZER_DIR` was
unset. The local host has no `nvcc`.

## Explicit exclusions

The host proof does not link or execute the feature-gated composition. No
CUDA translation unit was compiled, no native library was linked, no context
or reader was opened through `load_native_checkpoint`, no allocation was
made, and no GPU work ran.

The one-call API currently has no CLI command or durable success/failure
evidence writer. Its returned pool still rejects inference because the
native target-layer program is absent. Normal successful shutdown has no
four-rank cleanup-evidence transaction; implicit owner-thread RAII is
fail-stop on cleanup error.

No small or production checkpoint load, native cleanup evidence, checkpoint
execution, startup-health transition, SM120 correctness, capacity, quality,
or performance result is claimed. The next host gate is a reproducible
feature-gated smoke CLI and evidence record. Native compile/link and
progressive checkpoint qualification require renewed cn4 operator
authorization.
