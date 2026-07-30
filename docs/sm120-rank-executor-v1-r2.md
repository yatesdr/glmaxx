# SM120 rank executor v1 corrective amendment r2

Date: 2026-07-30

Status: design candidate; adversarial re-review required before implementation

GPU evidence: none

Base contract: `docs/sm120-rank-executor-v1.md`

Native ABI contract: `docs/sm120-rank-executor-native-abi-v1.h`

## Scope and precedence

This amendment resolves every finding and question in the first adversarial
review of the Rust-owned SM120 rank executor. The base contract remains
normative except where this document replaces it. On conflict, this document
and its native ABI header take precedence.

This is a design correction, not a CUDA implementation or launch claim. It
does not authorize cn4 work.

The executor explicitly amends `spec/engine-v0.md` in two bounded ways:

1. `HEALTHY -> DRAINING -> CLOSED` are normal terminal lifecycle states.
   `FAILED` remains terminal and cannot transition back to `HEALTHY`.
2. collective native resources are created and memory-reconciled inside the
   `MEMORY_PLANNED` transition. `COLLECTIVES_VOTED` adopts or discards the
   already-created four-rank resource set and runs known-answer tests.

No other engine-v0 startup state or order changes.

## Versioned dependencies

Replace the unversioned dependency references in the base contract with:

- the reviewed successor `StepInput.v2`, including page-table-delta identity,
  emitted/materialized MTP boundaries, pending target token, proposal
  generation/count/mask, q-state digests, and next depth;
- `SamplingCounter.v2`, whose proposal-install and verifier draws use the
  reviewed ticket partition;
- `CollectiveSchedule.v2`, `CollectiveOp.v2`, and `StepOutput.v2`;
- the engine-v0 MTP successor-slot amendment; and
- this engine-v0 lifecycle/resource-creation amendment.

The names describe required contracts, not current implementation status. A
candidate or mixed-version dependency still fails startup.

## Corrected startup and memory order

### Owner-thread execution

The coordinator computes decisions and broadcasts commands. Only the four
rank-owner threads execute CUDA, peer, NCCL, upload, capture, or destruction
operations.

At `TOPOLOGY_VALIDATED`, the coordinator freezes one ordered peer decision.
Each owner thread applies its own outgoing peer edges and returns a receipt.
If any enable fails, every owner thread attempts to undo each edge it enabled.
Any undo failure is unrecoverable: the generation becomes `FAILED`, no later
CUDA call except diagnostic/cleanup calls is allowed, and the process exits
after recording the cleanup failure. There is no leak-and-continue path.

At `KV_READY`, the coordinator broadcasts the canonical empty-page-table
command. Each owner thread uploads its rank-local projection and returns the
device-visible digest.

### `MEMORY_PLANNED` transition

`MEMORY_PLANNED` is complete only after the following ordered subphases pass
on all four ranks:

1. measure free HBM after context creation and module load;
2. compute the accepted per-rank arena table and the following explicit
   bounded non-arena terms;
3. allocate all deterministic device and pinned-host arenas;
4. concurrently release all owner threads to create the one
   bootstrap-ID-bound NCCL communicator set and all custom collective route
   resources needed by the immutable route table;
5. bind every route to its fixed communication and scratch spans;
6. measure free HBM again and reconcile the collective-library delta;
7. hash the complete resource table, native handles excluded; and
8. accept four memory receipts or discard every candidate resource.

Therefore all collective communicators, buffers, and route handles exist
before `WEIGHTS_LOADED` and `GRAPHS_CAPTURED`. `COLLECTIVES_VOTED` never
creates them.

The memory plan contains distinct terms:

```text
context_and_module_resident_bytes
engine_device_arena_bytes
collective_library_internal_hbm_ceiling
graph_runtime_internal_hbm_ceiling
allocator_padding_bytes
emergency_escrow_bytes
```

Here `emergency_escrow_bytes` is unallocated free-HBM headroom. It is not an
arena. The base contract's `diagnostic escrow` arena name is replaced by a
bounded `diagnostic status` arena plus this separate unallocated escrow.

For rank `r`, checked arithmetic must prove:

```text
measured_context_module_residency[r]
+ engine_device_arena_bytes[r]
+ measured_collective_internal_hbm_delta[r]
+ graph_runtime_internal_hbm_ceiling[r]
+ allocator_padding_bytes[r]
+ emergency_escrow_bytes[r]
<= measured_pre_context_free_hbm_floor[r]
```

`measured_context_module_residency` is the checked decrease from the
pre-context free-HBM observation through the quiescent post-module
observation. The right side is therefore not a post-context value and the
context/module term is not counted twice.

The collective delta is:

```text
free_before_collective_create - free_after_collective_create
```

and must not exceed its plan ceiling. An increase in reported free HBM is a
measurement failure rather than unsigned underflow.

No engine-directed `cudaMalloc`, `cudaHostAlloc`, module load, or communicator
creation occurs after `MEMORY_PLANNED`. The only permitted later opaque HBM
allocation is inside `cudaGraphInstantiate`, bounded by the
`graph_runtime_internal_hbm_ceiling`. Free HBM is sampled before and after
every instantiate; the checked aggregate delta must remain within that
ceiling. Graph-instantiation failure or ceiling excess destroys all candidate
graphs and fails the generation.

Engine-owned pinned host memory is one explicit process-wide plan:

```text
pinned_host_total
  = checkpoint_load_staging
  + sum_over_graph_profiles(
        argument_ring_slots * argument_mirror_bytes
      + completion_ring_slots * completion_mirror_bytes)
  + tier_in_staging
  + tier_out_staging
  + diagnostic_status_staging
```

Every factor and slot count is fixed in the accepted graph/memory profile.
`pinned_host_total` must be at most `pinned_host_process_cap_bytes`.
Engine-owned pinned memory can be created only as a
`GLMAXX_ARENA_HOST_PINNED` arena. Arbitrary pinned allocation is absent from
the production ABI.

After graph capture, KV initialization, maximum-workspace initialization, and
collective known-answer tests, but before `HEALTHY`, every owner thread
re-queries free HBM. The coordinator accepts only if:

```text
min(free_hbm_after_all_startup_work[r] for r in 0..4)
> emergency_escrow_bytes_per_rank
```

The initial per-rank escrow remains 1 GiB. The comparison is strictly greater,
matching engine-v0. The final four measurements and plan ceilings enter the
`COLLECTIVES_VOTED` receipt and local resource-layout digest.

### `GRAPHS_CAPTURED` and `COLLECTIVES_VOTED`

The graph-capture protocol in the base contract is retained, but every
collective node resolves an already-created immutable route handle. Capture
cannot create, replace, or choose a route.

At `COLLECTIVES_VOTED`, ranks:

1. vote to adopt the exact process-common communicator/route-table digest;
2. run every no-model control and known-answer operation;
3. run the maximum-scratch write/read check;
4. perform the post-startup free-HBM/escrow check above; and
5. atomically retain all resources or discard all resources.

A known-answer failure never causes rank-local route fallback.

## Resource ownership, unwind, and cleanup

The concrete Rust executor must use private `Option`/state-bearing resource
groups plus an explicit owner-thread `shutdown(&mut self)` implementation.
Declaration order is the reverse of dependency order:

```text
command admission and state
completion and argument rings
device page table
graph registry
collective route registry
adopted weights
arenas
events
streams
modules
enabled peer edges
native context
```

Normal shutdown takes each field exactly once in the displayed order.
`Drop` runs on the owner thread and invokes the same idempotent internal
take-and-destroy routine for any fields still present. This is the panic-unwind
path; it cannot depend on default field drop order.

Safe native handles are `!Send`, `!Sync`, contain their creator `ThreadId`,
and never expose a public raw handle. If a wrong-thread drop is nevertheless
reached because of unsafe-code corruption, Rust does not call the native
destructor from that thread: it records the invariant failure using
preallocated diagnostics and aborts the process. A destructor, peer undo,
communicator abort, or context cleanup failure similarly poisons the
generation and requires process exit. Silent leaking followed by continued
service is forbidden.

## Native ABI v1

`docs/sm120-rank-executor-native-abi-v1.h` is the complete design-level C ABI
for all eight native families named by the base contract. It freezes:

- every enumeration value, field order, width, struct size, and 16-byte
  alignment;
- owner-thread-affine opaque `uint64_t` handles;
- checked arena-relative spans rather than coordinator-supplied device
  pointers;
- exact constructor/destructor pairs;
- context/peer, module, arena/copy, stream/event, collective, graph/program,
  and validation/status entry points; and
- fixed 128-byte collective bootstrap IDs without exposing NCCL types.

The ABI uses the following mandatory rules:

1. Every input struct must have `abi_version == 1`, exact `struct_bytes`, zero
   reserved fields, known flags only, nonzero current-generation handles, and
   valid checked spans. Unknown input is rejected before a CUDA/NCCL call.
2. Every function returns only `glmaxx_executor_status_v1`. A nonnull,
   caller-owned 64-byte error object is overwritten on every call. Native CUDA
   or NCCL codes appear only in `native_code`; they never become undocumented
   function return values.
3. `NOT_READY` is legal only for event/device-status queries. Every negative
   status is fatal to the current startup transaction or executor generation,
   except a host request-validation error that occurs before native entry.
4. All C++/CUDA definitions are `noexcept`, wrap their complete bodies in
   `try/catch (...)`, translate exceptions to `INTERNAL_ERROR`, and never let
   an exception or unwind cross `extern "C"`.
5. A native constructor writes a zero output handle before doing work and
   publishes a nonzero handle only after success. The Rust owner must invoke
   the matching destructor exactly once. A device address returned in an
   arena binding is borrowed from that arena and is never freed separately.
6. Module image bytes are copied or consumed synchronously before
   `module_load` returns. Native code retains no host pointer from an input
   struct. Async copies refer only to native-owned pinned arenas whose lifetime
   extends through their completion event.
7. Graph construction copies every host descriptor before returning. Captured
   graphs retain only native handles and adopted device spans.
8. A communicator owns library-internal state; route handles borrow the
   communicator and fixed arena spans. Routes are destroyed before the
   communicator. `communicator_abort` is owner-thread-only and idempotently
   poisons the communicator and all of its routes. `destroy` follows abort.
   If abort or destroy fails, the process-exit containment rule applies.
9. Rust capability hashing covers this header's complete bytes, all module
   hashes, and the ordered queried capability records. A size, alignment,
   signature, status, or digest change requires ABI v2.

For a target or MTP graph node, `native_object` is the adopted module/program
handle. For a collective node it is the immutable route handle. For
status-finalize it is zero. Validation nodes use the dedicated validation
entry point; `graph_node_add` rejects `GLMAXX_NODE_DEVICE_VALIDATE`.

The header's C++ static assertions are part of the contract. The CPU proof
must independently mirror every struct in Rust with `#[repr(C, align(16))]`
and assert the same sizes, alignments, offsets, and enum values before any FFI
implementation is admitted.

## Route validation classification

Every route-table entry carries an immutable validation class. The complete v1
classification is:

| Route family | v1 class | Failure-latch behavior |
|---|---|---|
| NCCL all-reduce | `NEUTRAL_IN_GRAPH` | fixed count/pointers; reduce neutral hidden-state record |
| NCCL all-gather | `NEUTRAL_IN_GRAPH` | fixed count/pointers; gather neutral fixed-size record |
| direct one-shot peer reduction | `NEUTRAL_IN_GRAPH` | fixed peer spans; write/read neutral record |
| ring | `NEUTRAL_IN_GRAPH` | fixed four-rank hops/counts; forward neutral record |
| tree | `NEUTRAL_IN_GRAPH` | fixed nodes/counts; merge neutral record |
| two-pair hierarchy | `NEUTRAL_IN_GRAPH` | fixed pair/inter-pair order; merge neutral record |
| packed-record gather | `NEUTRAL_IN_GRAPH` | contract-defined byte-preserving empty-owner record |
| sampling gather/broadcast | `NEUTRAL_IN_GRAPH` | fixed-rank empty-mass record and invalid token |
| partial-LSE merge | `NEUTRAL_IN_GRAPH` | fixed-rank `m=-inf`, `l=0`, zero numerator record |

The device-validation node is first, and communication buffers are fixed,
in-bounds registry spans initialized to the listed neutral records before
launch. A latch can alter values only; count, pointer, participant mask, and
ordinal remain unchanged.

`PRELAUNCH_REQUIRED` is reserved for a future reviewed route family that
cannot satisfy those rules. No such route is admitted by v1. An unknown,
variable-count, step-derived-pointer, or locally selected route fails route
table compilation rather than being classified at runtime.

## Step, tier, and deadline commands

The bounded rank command enum has two production variants:

```text
RankCommand.v1 =
    STEP(Arc<RankStepCommand.v1>)
  | TIER(Arc<RankTierCommand.v1>)
```

`RankStepCommand.v1` carries the base contract's fields with
`StepInput.v2` and `SamplingCounter.v2`. It never represents `CACHE_ONLY`.

`RankTierCommand.v1` is the only carrier for cache-only device work:

```text
executor generation and scheduling epoch
monotonic command_id
PageTableDelta plus global and four expected local digests
operation_count: u8, 0..=64
64 fixed slots of TierOperation.v1
tier-residency transaction SHA-256
absolute prepare/completion deadline
command SHA-256
```

Each occupied `TierOperation.v1` contains:

```text
operation_id
RESTORE | EVICT | PUBLISH | REMOVE
page content identity and page generation
rank owner and physical page ID
source tier and destination tier
source arena/offset/length/generation
destination arena/offset/length/generation
expected byte digest
required predecessor event and completion event
```

Unused slots are all zero and hash-covered. The rank validates all ranges,
digests, generations, operation IDs, and the reviewed residency transaction
before any transfer. At most 64 operations are admitted; larger work is split
into later coordinator transactions. Tier transfer ranges that overlap any
page or span readable/writable by the current or admitted graph are rejected
before the four-rank prepare barrier.

A tier command launches no model graph and no GPU collective. Its page-table
delta becomes visible only after every transfer completes and all four
rank-local receipts reach consensus. The existing `ApplyDelta` control path is
a CPU/mock predecessor, not the production tier command ABI.

The deadline is one absolute monotonic timestamp created before the dispatcher
sends prepare commands. It covers bounded-queue wait, rank preparation, the
four-rank prepare barrier, upload, launch/transfer, D2H completion, and receipt
consensus. A rank wedged in prepare therefore trips the same supervisor and
generation-fatal path as a rank wedged after launch.

## H2D overlap decision

Version one allows host filling of a free next-step mirror during current
compute, but forbids every next-step device H2D until:

1. the current step has four valid completion receipts;
2. its serving page/token/RNG/MTP transaction has committed;
3. the next command has passed four-rank preparation; and
4. the coordinator releases that exact next command.

This includes argument-slab and page-table-delta uploads. It avoids exposing a
speculative successor generation through a shared device page table. The
latency cost must be reported. A later overlap amendment may permit a distinct
immutable slab and double-buffered page table with explicit per-slab events;
an implementation may not make that optimization under v1.

## Serving backpressure and fatal request ownership

Output publication uses the accepted nonblocking transport policy:
`try_push` into bounded, precharged completion queues. A full queue or
exhausted output-byte permit marks the request a slow consumer, sends one
idempotent backend cancel, releases queued network bytes, and closes the
connection. The coordinator never waits for socket progress and returns the
argument/completion slab immediately after the internal serving transaction
and nonblocking publication attempt.

If generation failure occurs after an MTP target token was emitted to the
client but before that token was materialized into target state, the serving
coordinator owns termination of the request. It records the terminal cause,
issues idempotent cleanup, and never resubmits or resumes that request in the
replacement executor generation. Client-visible output is not rewound.

## Corrected CPU/mock proof

After adversarial acceptance and before CUDA implementation, the CPU/mock
proof must cover all base items plus:

1. collective resource creation before graph capture and adoption/KAT only at
   `COLLECTIVES_VOTED`;
2. collective-internal and graph-internal HBM deltas below, equal to, and one
   byte above their ceilings, including underflow rejection;
3. process-wide pinned-host arithmetic at the exact cap and one byte above;
4. the strict post-startup `free_hbm > escrow` gate at greater/equal/less;
5. every native struct size, alignment, field offset, enum value, reserved
   field, status mapping, constructor failure, and exactly-once destructor;
6. native exception translation, wrong-thread access, wrong-thread drop
   containment, and communicator abort/destroy failure;
7. peer-enable partial failure, successful reverse undo, undo failure, and no
   rank-local fallback;
8. manual panic-unwind cleanup from every partially constructed resource
   group, in exact reverse dependency order;
9. every route family's neutral-latch path and rejection of every
   prelaunch/unknown/variable-count route;
10. tier restore/evict overlap rejection against current and admitted graph
    spans, plus a nonoverlapping transfer/consensus path;
11. zero, one, 64, and 65 tier operations; stale operation/event/page
    generations; digest mismatch; and partial-transfer failure;
12. deadline expiry in queue wait, prepare, barrier, upload, launch, D2H, and
    consensus;
13. proof that next-step host fill may overlap but every next-step device H2D
    is rejected before current commit;
14. a full serving completion queue proving nonblocking cancellation,
    immediate slab release, and progress by at least 63 peer requests; and
15. an emitted-but-unmaterialized MTP token followed by generation replacement,
    proving termination without retry or client-visible rewind.

The proof also retains the base contract's complete C1/C64
PREFILL/DECODE/VERIFY, MTP0-6, transaction, receipt, failure, and no-hot-path-
allocation matrix. It remains CPU/mock evidence only.

## Re-review boundary

Re-review should decide whether:

1. collective resources now exist before every graph that captures them;
2. all post-plan HBM and engine-owned pinned allocations are bounded,
   measured, reconciled, and followed by the strict escrow gate;
3. the attached ABI is concrete, internally consistent, unwind-safe, and has
   complete ownership/abort semantics;
4. all nine first-review minor findings are closed;
5. all three first-review questions are answered without an implementation
   choice remaining; and
6. the expanded CPU/mock matrix is sufficient to begin implementation.

Only an unqualified adversarial acceptance permits the CPU/mock executor
implementation to start. GPU qualification remains a later, separately
authorized gate.
