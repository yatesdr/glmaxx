# Rust-owned SM120 rank executor v1

Date: 2026-07-29

Status: design candidate; adversarial review required before implementation

GPU evidence: none

## Purpose and ownership boundary

This contract defines the persistent device runtime that turns the reviewed
Rust control-plane objects into one four-rank SM120 execution generation. It
is deliberately GLM-5.2-only, TP4-only, DCP4-only, and SM120-only.

The engine, scheduler, memory plan, loader transaction, page tables, graph
registry, command queues, state transitions, error policy, and result
consensus remain Rust. CUDA translation units contain kernels and thin
CUDA/NCCL launch shims only. They do not own a scheduler, allocator policy,
model graph, request state, fallback decision, or serving loop.

The existing `NativeRankContext` proves device binding and stream ownership.
The existing `RankExecutor::execute(plan,schedule)` trait is not a production
executor ABI because it has no startup lifecycle, immutable row input,
page-table delta, device program identity, argument-slab generation, or
structured completion receipt. This design replaces that boundary after its
dependencies pass review; it does not reinterpret the current trait.

## Dependencies and nonclaims

Production implementation requires accepted versions of:

- checkpoint load transaction and strict four-rank manifest validation;
- target-layer and recurrent-MTP programs;
- `StepInput`, page-table transaction, and `CollectiveOp.v2`;
- distributed sampling and `StepOutput.v2`;
- graph profile and static memory plan; and
- target/draft cache ABIs.

The executor may be designed and CPU-mocked while those gates are pending.
It may not launch a target/MTP graph against a mixture of candidate ABIs.

This document does not authorize cn4, CUDA execution, a checkpoint upload,
kernel correctness, quality, capacity, concurrency, or performance.

## Process and thread topology

One process owns exactly:

```text
one Rust coordinator/dispatcher thread
four persistent Rust rank threads
one bounded completion channel per rank
one bounded command channel per rank
one supervisor/watchdog state machine
```

Rank-to-device binding is immutable:

```text
rank 0 -> visible CUDA device 0
rank 1 -> visible CUDA device 1
rank 2 -> visible CUDA device 2
rank 3 -> visible CUDA device 3
```

Exactly four devices must be visible. Each must report compute capability
12.0, nonzero memory, nonzero SM count, and the device identity pinned by the
load plan. MIG, MPS remapping, a fifth visible device, or a reordered UUID set
fails startup.

Every CUDA context, stream, event, graph handle, communicator, module, and
device allocation is created, used, and destroyed by its owning rank thread.
No CUDA handle is `Send` or `Sync`. Safe Rust wrappers contain the creator
`ThreadId`, rank, device UUID digest, generation, and nonzero native handle;
every method verifies all five before FFI.

A `RankExecutorFactory` is moved into each newly spawned rank thread and
constructs the executor there. Construction on the dispatcher followed by a
move is forbidden. All four rank threads return a startup receipt before the
dispatcher accepts a request. A thread-spawn, panic, or channel failure is
reported to the coordinator; it may not leave `spawn()` apparently healthy.

## Generation state machine

The normative process states are:

```text
CREATED
  -> HOST_VALIDATED
  -> CUDA_CONTEXTS_READY
  -> TOPOLOGY_VALIDATED
  -> MODULES_READY
  -> MEMORY_PLANNED
  -> WEIGHTS_LOADED
  -> GRAPHS_CAPTURED
  -> KV_READY
  -> COLLECTIVES_VOTED
  -> HEALTHY
  -> DRAINING
  -> CLOSED
```

Any failure before `CLOSED` moves all ranks to `FAILED`. `FAILED` is terminal
for the executor generation.

The current Rust mock order (`ContextReady`, `InventoryVerified`,
`WeightsLoaded`, then `MemoryProved`) is explicitly incompatible and must be
versioned out. The production coordinator holds a four-rank barrier at each
normative state. A rank may perform local work for the next state only after
receiving the coordinator's transition command for the same generation.

Every stage receipt contains:

```text
generation
rank
reached state
device identity SHA-256
load-plan SHA-256
memory-plan SHA-256
weight-policy SHA-256
kernel capability SHA-256
target-program SHA-256
MTP-program SHA-256
graph-profile SHA-256
collective-route-table SHA-256
local resource-layout SHA-256
last CUDA status
```

Process-common identities must match across all ranks. The rank-local resource
layout may differ only where the accepted plan carries its four exact
digests. A zero, stale, missing, duplicate, or wrong-rank receipt poisons the
generation.

## Startup stages

### `HOST_VALIDATED`

No CUDA call has occurred. Rust has:

- opened and verified all four native rank files;
- accepted the complete production manifests and source identities;
- built the immutable rank-set load plan;
- accepted a fit-capable measured memory plan;
- compiled the exact target/MTP programs, graph profile, and route table; and
- allocated bounded host control objects.

### `CUDA_CONTEXTS_READY`

Each rank thread binds its exact device and retains the primary-context
generation. It creates no weight or KV allocation yet. Context overhead is
measured per rank and reconciled with the memory plan.

### `TOPOLOGY_VALIDATED`

Rust collects the complete ordered matrix:

```text
device UUID and PCI bus ID
PCIe ancestry/topology fingerprint
peer-access capability for every ordered pair
peer atomic capability if a route requires it
driver/runtime versions
NCCL version
```

The coordinator derives one topology digest and enables peer access only for
edges required by the accepted route table. Every rank receives the same
ordered decision. Failure on one edge causes all enabled edges to be undone
and the generation to fail; there is no local NCCL fallback.

### `MODULES_READY`

Each rank loads the exact SM120 module set and queries a versioned capability
table. The table binds:

- kernel ABI;
- module/cubin hashes;
- supported codecs and tensor roles;
- descriptor versions and sizes;
- graph-capture support;
- maximum row/bucket shapes;
- required dynamic/shared memory;
- device-validation ABI; and
- collective-shim ABI.

Every required kernel must contain an SM120 device image. PTX JIT fallback,
SM100/SM90 code, or a missing role fails. Modules are loaded before arena
planning so their measured HBM residency is an explicit term.

### `MEMORY_PLANNED`

The coordinator compares measured context/module/free bytes with the accepted
per-rank plan. Each rank then creates deterministic, separately owned arenas:

```text
immutable weight payload
immutable weight/codec metadata
target KV
target indexer
draft combined sidecar
device sequence page table
graph argument slabs
maximum graph scratch
collective communication
tier-transfer staging
completion/error/status
diagnostic escrow
```

Arena base, length, alignment, role, and generation form a canonical local
resource table. There is no general device allocator after this state.
Suballocation uses checked offsets fixed by the plan. A serving step performs
zero `cudaMalloc`, `cudaFree`, module load, graph instantiate, communicator
create, pinned-host allocation, or Rust heap growth.

Startup load staging is separately bounded and cannot consume serving KV
floors or diagnostic escrow.

### `WEIGHTS_LOADED`

The reviewed two-phase load transaction streams into tentative arena
intervals on rank-owned upload streams. No graph or kernel can resolve the
weight-generation handle before:

1. every tensor and plane finishes;
2. every final upload event completes;
3. each rank produces its prepared receipt;
4. the coordinator accepts all four receipts; and
5. all ranks acknowledge adoption of the same generation.

Abort frees or invalidates every tentative interval exactly once. Published
weight pointers never change for the lifetime of the generation.

### `GRAPHS_CAPTURED`

Every REQUIRED graph profile entry is instantiated on every rank. Capture is
a four-rank coordinator operation because NCCL or peer collectives at ordinal
`j` must be captured by all participants in the same order.

For each graph ID:

1. coordinator broadcasts the exact graph key and collective schedule
   template;
2. ranks reset their fixed argument and scratch slots;
3. all ranks acknowledge capture-ready;
4. coordinator releases ordinal-zero capture;
5. ranks capture every kernel/collective in schedule order;
6. ranks instantiate and locally validate the graph;
7. coordinator accepts four graph receipts; and
8. only then does the graph become addressable by execution.

Failure destroys every rank's candidate graph. Production never falls back to
eager execution. Eager graphs remain a separately identified correctness
route and cannot serve traffic.

### `KV_READY`

Ranks initialize target/indexer/draft arenas, page-table storage, free-page
metadata, and both argument-slab generations. The coordinator uploads a
canonical empty page-table generation and all ranks acknowledge its
device-visible digest.

### `COLLECTIVES_VOTED`

The executor initializes all resources in the immutable route table, then
performs:

- four-rank adoption vote;
- no-model control collectives for every route family and payload band;
- byte-preserving packed-record known answers;
- fixed-order sampling and LSE known answers; and
- a maximum-scratch escrow write/read check.

All resources adopt or all discard. A route cannot remain listed if one rank
failed its local setup.

### `HEALTHY`

One deterministic small kernel smoke passes on all ranks and its output
digest reaches consensus. Only then may admission and scheduling start.

## Rust resource model

The production executor is conceptually:

```text
Sm120RankExecutor {
    identity
    generation
    native_context
    streams
    events
    modules
    arenas
    adopted_weights
    graph_registry
    collective_registry
    device_page_table
    argument_ring
    completion_ring
    last_step_id
    last_page_generation
    last_collective_ordinal
    state
}
```

All fields are private move-only RAII objects. Drop runs only on the owner
thread and in reverse dependency order:

```text
stop new commands
drain or poison compute
destroy graphs
destroy collective resources
destroy events/streams
invalidate/free arenas
unload modules
release peer access
release context
```

If device loss makes cleanup calls fail, Rust records each failure but does
not double-free or continue the generation. Process exit is the final
containment boundary for an unrecoverable CUDA/NCCL state.

## Stream and event DAG

Each rank owns:

```text
control stream       page/argument uploads and tiny D2H receipts
compute stream       captured target/MTP graphs and captured collectives
tier-in stream       DRAM-to-HBM restores
tier-out stream      HBM-to-DRAM evictions/publication
load stream          startup only; destroyed before HEALTHY
```

All streams are nonblocking. Default-stream semantics are forbidden.

The step dependency is:

```text
page/argument H2D on control
  -> control_ready event
  -> compute waits
  -> graph launch on compute
  -> compute_done event
  -> bounded output/status D2H on control
  -> completion event
  -> rank receipt
```

The rank thread polls completion with bounded backoff or the reviewed event
integration; it does not call device-wide synchronization. A device-wide
synchronize is allowed only in startup qualification, fatal diagnostics, and
shutdown evidence.

Tier streams may overlap compute only after the residency transaction proves
that their page ranges do not alias any graph-readable/writable page. Their
events and byte ranges are coordinator commands. A rank never evicts from
local pressure.

## Argument and completion rings

Each graph profile owns at least two fixed device argument slabs and two
pinned-host mirrors. A slab state is:

```text
FREE -> HOST_FILLING -> UPLOADING -> DEVICE_READY
     -> EXECUTING -> DOWNLOADING -> COMPLETE -> FREE
```

The generation counter increments on every reuse; overflow is fatal. A graph
reads only its selected immutable slab. Rust cannot mutate the host mirror
after upload begins.

The slab contains or points to:

- `StepPlan` and `CollectiveSchedule.v2`;
- immutable `StepInput` rows and prompt payload;
- page-table generation/delta digest and owner-local device entries;
- target/MTP program digests;
- graph masks, positions, token IDs, route/compaction tables;
- sampling parameters, counters, pending bundles, and q-state pointers;
- output/status destinations; and
- arena bounds/generations used by device validation.

Pointers are offsets into adopted arenas, not unchecked process addresses in
coordinator-owned records. Rust resolves checked `(arena_id,offset,length,
generation)` spans on the rank thread and writes native pointers only into the
rank-local device slab.

The completion block is fixed-capacity and contains:

```text
generation, rank, step_id
plan/input/schedule/page/program hashes
graph_id and route-table hash
last entered/completed collective ordinal
device validation bitset
CUDA/kernel/collective status
bounded StepOutput.v2
sampling/proposal trace digest
page-write digest
completion checksum
```

The device writes status before output becomes observable. Rust validates
every field and checksum before producing a rank receipt.

## Device validation

Every graph begins with one descriptor-validation kernel. It checks:

- SM120 capability marker and kernel ABI;
- all arena IDs, generations, offsets, lengths, and alignments;
- program/graph/route IDs;
- real rows, buckets, masks, and row order;
- page owners, local IDs, valid ranges, and table generation;
- nonaliasing of immutable, output, scratch, and tentative spans;
- q-state and output capacity; and
- expected first/last collective ordinal.

The validation kernel sets a device error latch. Every later graph node checks
the latch before dereferencing a step-derived pointer or writing output. A
validation failure therefore turns later nodes into no-ops while preserving
the captured collective order and the exact scheduled native counts. Fixed
communication buffers are initialized to the route's neutral records before
validation; a local latch may change payload values but never count,
participant mask, pointer, or ordinal. If a route cannot safely enter with
neutral fixed-capacity payload after validation failure, validation must occur
in a separate prelaunch graph and complete with four-rank host consensus
before the main graph. One rank may not decide this locally.

Host validation remains mandatory. Device validation is defense in depth, not
permission to accept malformed host input.

## Collective registry

`CollectiveRouteTable.v1` is selected before graph capture from the exact
topology fingerprint. It contains NCCL controls and any qualified custom
routes:

```text
NCCL all-reduce/all-gather control
direct one-shot peer reduction
ring
tree
two-pair hierarchy
byte-preserving packed-record gather
fixed-rank sampling gather/broadcast
fixed-rank partial-LSE merge
```

NCCL is a correctness/performance control, not an automatic fallback. A
custom route is enabled only after matched correctness and timing on the
actual PCIe layout.

Associative hidden-state TP reductions may use a qualified NCCL/custom route
with the precision membership frozen by the target program. Operations whose
numerical ABI requires rank order—sampling mass/CDF, top-k tie merge,
candidate order, and partial LSE—use explicit `0,1,2,3` merge kernels or an
independently proven equivalent. An opaque reduction tree cannot replace
fixed rank order.

Each `CollectiveOp.v2` is looked up by:

```text
route ID
layer/phase/group
participant mask
real rows and graph bucket
logical and wire bytes
graph/eager flag
```

Lookup yields fixed buffer slots, peer spans, native handles, and launch
parameters. Missing or mismatched lookup fails before launch. Empty owners
use the record defined by the target/sampling contract; they do not skip an
ordinal.

## Step command and execution

The dispatcher sends the same immutable `Arc<RankStepCommand>` to all ranks.
The object contains:

```text
executor generation and scheduling epoch
StepPlan plus plan hash
StepInput plus input hash
CollectiveSchedule.v2 plus hash
PageTableDelta plus global and expected local digests
target/MTP program hashes
graph-profile and route-table hashes
argument-slab generation per rank
maximum completion deadline for supervision
```

Rank-specific slab IDs are fixed in the command's four-entry table and
covered by the global command hash. A rank cannot select another free slab,
graph, or route.

Before enqueue, every rank proves:

- state is `HEALTHY`;
- generation/epoch/step ID are strictly current;
- all common hashes match adopted startup state;
- page generation is exactly the expected successor;
- graph and slab are compatible and free;
- command shapes fit fixed capacities; and
- all collective records resolve.

The dispatcher releases the four ranks only after four prepare
acknowledgments. Each rank uploads, launches, and completes independently on
its owner thread, but collective order is the immutable schedule.

After completion, Rust validates each receipt and `StepOutput.v2`. The
coordinator requires identical plan/input/schedule/program/output/trace
digests and the exact rank set. Only then may the serving page/RNG/token
transaction commit.

`CACHE_ONLY` carries no compute graph or GPU collective. It may upload a page
table removal and submit coordinator-approved tier operations. Its receipt
still participates in four-rank generation consensus.

## Watchdog and failure semantics

Request-local validation errors occur before the four-rank prepare barrier and
launch nothing.

Generation-fatal errors include:

- rank thread panic or channel disconnect;
- CUDA/NCCL/peer error;
- module, graph, route, arena, page, or generation mismatch;
- device validation latch;
- nonfinite kernel status where the operator forbids it;
- collective ordinal disagreement or timeout;
- asynchronous launch/completion error;
- malformed, stale, or divergent receipt/output; and
- inability to prove that all ranks will avoid future ordinals.

CUDA and NCCL work is not assumed cancellable. On timeout the supervisor
atomically stops admission, closes all rank command channels, records the last
known ordinal/events, and terminates the executor generation. It never lets
healthy-looking ranks enter a later step.

A worker error rolls back host page/token/RNG metadata for the current step.
Tentative device bytes remain unreachable by generation. Recovery creates a
new process/executor generation and reloads the checkpoint; in-process rank
replacement is outside v1.

The completion deadline is observability, not proof that GPU work stopped.
Cleanup and process termination remain required after a timeout.

## Backpressure and serving concurrency

The command queue is bounded. Only one dependency-linked compute step may be
executing per generation because target KV, pending logits, proposal state,
and collective ordinals are sequential. The coordinator may compile the next
step, fill a free host argument slab, run prefix/tier I/O, and tokenize other
requests concurrently.

Continuous batching is achieved by changing rows inside C1..C64 graphs, not
by running two independent TP4 model graphs at once. Prefill/decode overlap is
enabled only by a separately reviewed MIXED graph and matched ITL evidence.

Slow HTTP clients never hold an argument/completion slab. Output is committed
to the bounded serving layer before the slab returns to `FREE`.

## Native ABI additions

The C header remains a plain C ABI consumed through Rust `extern "C"`.
Required families are:

```text
device/context and peer capability
module capability query
arena allocation/free and checked async copies
stream/event lifecycle
collective communicator/route lifecycle
graph capture/instantiate/launch/destroy
target/MTP graph construction
device validation and status query
```

Each family has one ABI version and a capability digest. Handles are opaque
nonzero `uint64_t` values only inside the safe Rust wrapper. Native code may
not retain a pointer to movable Rust memory or invoke Rust callbacks from a
CUDA stream.

The existing microbenchmark launchers remain laboratory APIs. They are not
the production target/MTP graph ABI.

## Required CPU/mock proof

After adversarial acceptance and before CUDA implementation:

1. exact normative startup order and four-rank barriers;
2. factory construction on owner threads and cross-thread-handle rejection;
3. all startup failures at every rank/stage with reverse-order cleanup;
4. deterministic resource tables and no arena overlap/overflow;
5. two-phase weight adoption and exactly-once abort;
6. graph capture voting, partial capture failure, and no eager fallback;
7. argument/completion ring transitions and generation ABA rejection;
8. identical command broadcast with rank-specific slab projection;
9. page-delta upload dependency and expected-local digest mismatch;
10. every collective-v2 lookup, dependency, empty-owner record, and missing
    ordinal;
11. device-latch simulation proving later nodes preserve safe collective
    behavior;
12. C1/C64 PREFILL, DECODE, VERIFY, MTP0–6, and CACHE_ONLY command shapes;
13. completion receipt checksums, bounded output, and four-rank consensus;
14. timeout, rank panic, malformed output, and asynchronous-error fatal drain;
15. pending MTP token/proposal state across step success and failure;
16. bounded queue/slab backpressure with concurrent host preparation;
17. shutdown from every state with no double free; and
18. proof that the hot step path performs no device allocation, module load,
    graph instantiate, communicator creation, or unbounded host allocation.

The mock uses fake CUDA handles/events and a deterministic collective oracle.
It is not GPU evidence.

## Later cn4 qualification

With renewed authorization and all prerequisite tokens:

1. reproduce exact device/topology/toolchain/module identities;
2. pass context, peer, arena, stream, event, and reverse-cleanup tests;
3. upload/adopt a small four-rank fixture through the real transaction;
4. cooperatively capture one no-model graph with each route family;
5. run fixed-rank collective known answers for every payload band;
6. inject prelaunch/device-latch/async/timeout failures;
7. execute one target layer and one layer-78 teacher/scratch replay;
8. execute full required graph buckets against a small checkpoint;
9. retain separate upload, argument, kernel, collective, D2H, and framework
   timings; and
10. prove no GLMAXX context, communicator, allocation, or process remains
    after shutdown.

Only a later full checkpoint smoke, quality gate, capacity run, concurrent
service run, and matched benchmark can promote this runtime.
