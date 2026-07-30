# CUDA checkpoint arena CPU proof v1

Date: 2026-07-30

Status: implementation candidate; adversarial review pending

GPU claim: none

## Candidate

The proved source and corrected governing contract are commit
`6e156600f0c35293875c353594a61a77ac95c635`.

| Input | SHA-256 |
|---|---|
| `crates/glm-cuda/src/load.rs` | `89eb00dbf649b99355fb4a7ceb776e975d96a7f75240ade095ec5adbef0fecee` |
| `crates/glm-cuda/src/ffi.rs` | `57e97e8f98defcf9bbb6340fd059d6d3a7fdb2caa39e97d1f129f9157d3a4645` |
| `crates/glm-cuda/src/lib.rs` | `ee20a1c568c7347b8c0660096ce4c4fc4824411be3e9ecaa943cbeda05e3d5ae` |
| `crates/glm-engine/src/checkpoint_cuda.rs` | `56c58a10ec5197753b07e52d4936fa55e134a630e88d54e734fb4439a1d700c3` |
| `crates/glm-engine/src/checkpoint_load.rs` | `73902c8c451f8208fd15609ac5800b0d4edf1f087409cf02ebdbc9fa39d0f07b` |
| `crates/glm-engine/src/lib.rs` | `ab94d22113b7f1f0f4122f134fffc682d5c2abfb4fdd6c38135f3e00dca91174` |
| `kernels/include/glmaxx_kernel.h` | `a96e58247db77f78d44b5763537bad20318c8dbaaec645ee96560bc9c6a82776` |
| `kernels/sm120/nvfp4_routed_fc1.cu` | `40feb292c455db8e14cb22411be42817fc2331ff0ef874857be6a28e3993ec8e` |
| `docs/checkpoint-load-transaction-v1.md` | `9b7625e0d8c2ca18f287303b9aaa831b442956e0f8b8c8617ab5c74649e29cf3` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

## Implemented boundary

`CudaQuarantinedArena<B>` is the rank-thread-owned physical writer behind
`PlannedRankTensorSink`. Its backend trait has no `Send` requirement, and both
the generic arena resources and native backend carry `PhantomData<Rc<()>>`,
so neither can move to another rank thread.

Allocation creates:

- one weight arena and one metadata arena at their exact planned sizes;
- one nonblocking load stream;
- a fixed pinned staging ring with at least two plan-sized slots; and
- one completion event per slot.

Both complete device arenas are zero-filled on the load stream before any
payload copy. A successful write validates monotonic destination order,
checked offset arithmetic, capacity, and staging-slot size before copying the
reader's borrowed bytes into owned pinned memory. It waits only before reusing
an in-flight slot, enqueues H2D, records that slot's event, and returns without
retaining the reader buffer. The upload path allocates no per-chunk memory.

The writer hashes the logical contents of both complete arenas as it stages:
zero gaps, exact payload bytes, and zero tails. `drain_and_seal` waits every
in-flight event, synchronizes the load stream, closes both full-arena hashes,
and makes further writes impossible.

`verify_device_contents` then reads both complete arenas back in bounded
8,388,608-byte D2H chunks through one existing pinned slot. Every chunk is
event-synchronized before it is copied into one ordinary fixed-size host
buffer. The observed full-arena SHA-256 values must equal the independently
constructed expected values. A copy, event, arithmetic, or digest failure
poisons the arena. The evidence digest is pinned by the unit fixture to
`e863578068c1fd64bfde6ab11682e4d4770fd1d1f5aebff712fbe232e66dec87`.

No device pointer is available from `CudaQuarantinedArena`. Adoption consumes
the exact rank/plan/allocation-generation `WeightArenaExecutionPermit` issued
only after four-rank coordinator completion. It also requires sealing,
successful device read-back, and an unpoisoned generation. Only the resulting
`CudaWeightArena` exposes immutable weight and metadata pointers.

## Teardown and failure behavior

Every explicit abort or shutdown synchronizes the load stream before
destroying events, freeing pinned slots, freeing device allocations, or
destroying the stream. An asynchronous event-record failure leaves the slot
owned and the stream synchronizable; the arena is poisoned and the abort path
still performs the ordered teardown.

If stream synchronization fails, cleanup discards all native handles without
freeing them. This deliberate leak prevents DMA use-after-free. The explicit
cleanup API returns the fatal CUDA error to the process coordinator. If the
same failure occurs during implicit `Drop`, there is no caller to promote it,
so the process aborts.

The native backend maintains owner-thread registries for every allocation,
pinned pointer, stream, and event. It validates exact ownership and all
device/pinned ranges before invoking the thin C ABI. The native ABI additions
are `cudaHostAlloc`, `cudaFreeHost`, and `cudaMemsetAsync`; existing H2D, D2H,
stream, and event operations supply the rest of the path.

## CPU fault proof

The mock backend stores actual device and pinned byte arrays rather than only
recording calls. Five focused tests prove:

1. the third write waits for slot zero's event before ring reuse;
2. full read-back reproduces both expected arena hashes, the canonical
   evidence digest, and exactly two chunks for the small fixture;
3. a changed byte in a zero-filled device gap is detected and prevents
   adoption;
4. event-record failure poisons the generation, while abort synchronizes
   before every free;
5. failed cleanup synchronization frees or destroys nothing; and
6. zero-fill covers both allocations, post-seal writes fail, and permit
   rank/plan/allocation identity is exact.

The final item shares tests with the preceding cases; the focused suite has
five test functions.

At exact candidate `6e156600f0c35293875c353594a61a77ac95c635`,
`scripts/local-checks.sh` passed:

- 323 Rust unit and integration tests with zero failures;
- workspace formatting and Clippy with warnings denied;
- CUDA-FFI host type checking and Clippy with warnings denied;
- deterministic CPU, matrix, manifest, native-rank, engine, serving, and
  cache proof regeneration and byte comparison;
- all 81 then-current review handoffs, with no configured review result
  accepted by the local verifier; and
- C++ syntax parsing of `kernels/include/glmaxx_kernel.h`.

The external tokenizer proof was skipped because `GLMAXX_TOKENIZER_DIR` was
unset. The local host has no `nvcc`.

## Explicit exclusions

No CUDA translation unit was compiled, no native symbol was linked into an
executable, no pinned host or HBM allocation was made, and no GPU operation
ran. This candidate therefore makes no native CUDA, SM120, capacity, quality,
or performance claim.

The arena is not yet wired into a persistent rank worker or a four-file
`NativeRankReader` load attempt. The canonical 256-byte full-rank verification
evidence record specified by the corrected contract is not implemented, and
no `PreparedRankReceipt` is produced from the read-back subrecord. Allocation
failure positions, a real CUDA asynchronous error, implicit-drop process
abort, full-rank byte counts, small-checkpoint smoke, and physical timing
evidence remain for the native qualification and integration gates.

The next implementation boundary is the typed full-rank evidence record and
persistent-rank load command that connects authenticated readers, the planned
sink, this arena, prepared receipts, four-rank adoption, and cleanup
acknowledgements. Native CUDA compile and smoke remain blocked until renewed
cn4 authorization.
