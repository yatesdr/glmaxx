# Target graph physical-memory ABI v1

Date: 2026-08-04

Status: design candidate; adversarial acceptance required before CPU or CUDA
implementation

GPU evidence: none

## Purpose and boundary

`target-layer-execution-v1-r2.md` deliberately defines only logical buffer
lifetimes. It does not serialize a device arena, offset, capacity, or
consumer subrange, so neither eager target execution nor CUDA graph capture
can yet prove that every pointer is in bounds and nonaliasing.

This contract supplies that missing physical boundary. It is the required
successor between the logical target-layer contract and M3/M4 execution. It
does not change target math, row construction, the logical lifetime table,
the executor C ABI, or any operator workspace formula.

The operative design is the conjunction of:

- `target-layer-execution-v1.md` and its r2 amendment;
- `prefill-graph-profile-abi-v2.md`;
- `step-execution-abi-v3.md` and the target-layer `StepPlan.v4` successor;
- the complete SM120 rank-executor r1-r5 contract and native header;
- the accepted operator common plans selected by the target/MTP programs;
  and
- this physical-memory contract.

No implementation may infer physical storage from logical alias class,
`GraphEntry.maximum_scratch_bytes`, a kernel descriptor, or a CUDA pointer.
The exact records below are constructed and validated on the host before an
arena is allocated or a graph is captured.

## Closed identities

The exact new identities are:

```text
GraphMemoryPlan.v1          glmaxx.target-graph-memory-plan.v1
GraphArena.v1               glmaxx.target-graph-arena-table.v1
GraphClassSpan.v1           glmaxx.target-graph-class-span-table.v1
GraphBufferUse.v1           glmaxx.target-graph-buffer-use-table.v1
DeviceArenaBinding.v1       glmaxx.target-device-arena-binding-table.v1
GraphProfile.v3             glmaxx.graph-profile.v3
RankGraphMemoryReceipt.v1   glmaxx.rank-graph-memory-receipt.v1
```

All integers are little-endian. Unknown flags, enums, roles, classes,
nonzero reserved bytes, duplicate keys, noncanonical ordering, arithmetic
overflow, or a predecessor profile fails before native graph construction.

`GraphMemoryPlan.v1` is process-common and contains no native handle, device
address, rank, CUDA allocation ID, or implementation-selected padding.
`DeviceArenaBinding.v1` and `RankGraphMemoryReceipt.v1` are rank-local and may
contain different device addresses and generations.

## Logical arena table

One `GraphArena.v1` record is exactly 32 bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 2 | logical arena ID |
| 2 | 2 | executor arena role |
| 4 | 4 | flags |
| 8 | 8 | exact allocated bytes |
| 16 | 4 | required alignment |
| 20 | 4 | reserved zero |
| 24 | 8 | reserved zero |

Records are ordered by logical arena ID and contain exactly these seven
entries:

| ID | Executor role | Meaning | Flags |
|---:|---:|---|---:|
| 1 | 7 | immutable graph argument slab | `EXECUTOR_FIXED=1` |
| 2 | 8 | graph scratch | `EXECUTOR_FIXED=1` |
| 3 | 3 | target KV arena | `PERSISTENT_MODEL_STATE=2` |
| 4 | 4 | target indexer arena | `PERSISTENT_MODEL_STATE=2` |
| 5 | 5 | recurrent state, including pending logits and MTP state | `PERSISTENT_MODEL_STATE=2` |
| 6 | 9 | fixed collective spans | `EXECUTOR_FIXED=1` |
| 7 | 11 | completion and validation status | `EXECUTOR_FIXED=1` |

Exactly one flag bit is set. Alignment is a nonzero power of two and at least
256 bytes. `bytes` is the exact ceiling authorized by the accepted rank-set
resource budget and later charged by the final memory plan, not the number of
bytes touched by one step. Every arena is executor-owned and preexists graph
construction. Persistent model-state arenas may be shared by graph instances
only under their existing generation and transaction rules; no arena is owned
or freed by a graph.

The arena-table digest is:

```text
SHA256(
  "glmaxx.target-graph-arena-table.v1\0" ||
  u16_le(7) || seven ordered 32-byte records
)
```

Executor r5 replaces the old C enum spelling `DEVICE_DRAFT_SIDECAR` with the
broader `DEVICE_RECURRENT_STATE` role. Both r5 and this design must be
accepted before implementation. The rename cannot be inferred from this
table alone; without accepted r5, arena 5 and every graph that needs class 30
remain nonconstructible.

## Physical class-span table

One `GraphClassSpan.v1` record is exactly 48 bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 2 | target logical slot class, `1..32` |
| 2 | 2 | logical arena ID |
| 4 | 4 | flags: `PRESENT=1`, `ALIAS_REUSE=2`, or zero for absent |
| 8 | 8 | byte offset in the logical arena |
| 16 | 8 | capacity bytes |
| 24 | 8 | maximum consumed bytes |
| 32 | 4 | required alignment |
| 36 | 2 | resolved phase-variant mask |
| 38 | 2 | reserved zero |
| 40 | 4 | first graph-node use ordinal |
| 44 | 4 | last graph-node use ordinal |

The table contains exactly 32 records in ascending class order. The arena
mapping is fixed:

```text
classes 1..26,31,32 -> arena 2 graph scratch
class 27             -> arena 1 graph arguments
class 28             -> arena 3 target KV
class 29             -> arena 4 target indexer
class 30             -> arena 5 recurrent state
```

The phase mask and first/last node ordinals must be derived from the accepted
`TargetBufferLifetime.v1` record, the complete captured DAG, the concrete
graph key, target program, index group, and layer ordinal. The last ordinal is
`u32::MAX` for classes 28..30 because their value remains live through the
external transaction boundary. These resolved values are repeated here so
validation cannot silently use a different lifetime interpretation. They are
conservative outer bounds; legality of reuse is additionally proved from all
individual buffer uses and graph dependencies.

An absent class has flags, offset, capacity, consumed bytes, alignment, phase
mask, and node ordinals all zero, but retains its fixed class and arena IDs.
A class is absent only when the exact target/MTP program and phase variants
contain no producer or consumer for it. In particular, dense layers have no
routed classes, shared-index layers have no index-production classes, and a
route without a candidate exchange has no candidate-exchange storage.

A present class has `PRESENT`, nonzero consumed bytes, power-of-two alignment
at least 16 bytes, and exact
`capacity = align_up(maximum_consumed_bytes, alignment)`. Its checked
`offset + capacity` is within its arena. `ALIAS_REUSE` is legal only for
scratch classes whose underlying byte intervals overlap another scratch
class. External and argument classes can never carry it.

The class-span digest is:

```text
SHA256(
  "glmaxx.target-graph-class-span-table.v1\0" ||
  u16_le(32) || 32 ordered 48-byte records
)
```

## Complete consumer table

Every pointer-like subrange consumed or produced by a captured node is
represented by one `GraphBufferUse.v1` record of exactly 80 bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 4 | graph node ordinal |
| 4 | 2 | use ordinal within that node |
| 6 | 2 | logical arena ID |
| 8 | 2 | target slot class, or zero for collective/status-only storage |
| 10 | 2 | access bits: `READ=1`, `WRITE=2`; at least one |
| 12 | 2 | applicable phase-variant mask |
| 14 | 2 | flags: `DYNAMIC_INDEXED=1`, otherwise zero |
| 16 | 8 | offset relative to the class span, or arena when class is zero |
| 24 | 8 | exact statically addressable byte envelope |
| 32 | 4 | required alignment |
| 36 | 4 | reserved zero |
| 40 | 32 | accepted operator/common-plan digest |
| 72 | 8 | reserved zero |

Records are strictly ordered by `(node_ordinal,use_ordinal)`. Node ordinals
are the exact captured DAG ordinals. Use ordinals start at zero and are
contiguous within a node. A node with no storage use has no record.

The selected target/MTP program, collective schedule, target phase template,
and accepted operator common plans must reconstruct the complete table.
Every descriptor pointer, target/MTP input/output/workspace, collective
send/receive/scratch span, validation arena table, and completion/status span
appears exactly once per node use. A caller-provided descriptor cannot add,
remove, resize, or redirect a use.

For target classes, checked `relative_offset + bytes` must not exceed class
capacity. Class `maximum consumed bytes` equals the greatest checked end of
all of its uses, not an independently supplied estimate. `DYNAMIC_INDEXED`
is legal only for classes 28..30. Such a use covers the full statically
addressable class envelope; the exact active addresses and lengths are
independently reconstructed from the immutable page-write, indexer-write, or
pending-logit slot table before every launch. The dynamic table cannot expand
the envelope.

Class-zero records are legal only for recurrent-state, collective, and status
storage in arenas 5, 6, and 7 and are checked directly against the arena.
They cover MTP proposal/recurrent state and other program uses that have no
target-layer slot-class identity. Those arena-level intervals are included in
overlap validation against class 30 and every other arena-5 use. Every
nonzero operator digest must be one of the accepted common plans bound by the
adopted module-set capability digest. A missing plan, unknown digest, or
module/common-plan disagreement fails before capture.

The buffer-use digest is:

```text
SHA256(
  "glmaxx.target-graph-buffer-use-table.v1\0" ||
  u32_le(record_count) || ordered 80-byte records
)
```

## Graph memory plan

`GraphMemoryPlan.v1` is a fixed 480-byte record:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `G5GMPV1\0` |
| 8 | 2 | version, exactly 1 |
| 10 | 2 | record bytes, exactly 480 |
| 12 | 4 | graph ID |
| 16 | 1 | executor graph kind |
| 17 | 1 | attention transport |
| 18 | 1 | MTP depth, `0..6` |
| 19 | 1 | flags; bit 0 is `MTP_PROGRAM_PRESENT` |
| 20 | 2 | sequence bucket |
| 22 | 2 | arena count, exactly 7 |
| 24 | 2 | class-span count, exactly 32 |
| 26 | 2 | reserved zero |
| 28 | 4 | row bucket |
| 32 | 4 | token bucket |
| 36 | 4 | buffer-use count |
| 40 | 8 | reserved zero |
| 48 | 8 | exact graph-scratch bytes |
| 56 | 8 | exact graph-argument bytes |
| 64 | 8 | exact recurrent-state arena bytes |
| 72 | 8 | exact collective arena bytes |
| 80 | 8 | exact completion/status arena bytes |
| 88 | 32 | target-program SHA-256 |
| 120 | 32 | MTP-program SHA-256, or zero |
| 152 | 32 | executor program-set SHA-256 |
| 184 | 32 | logical `GraphProfile.v2` SHA-256 |
| 216 | 32 | adopted module-set capability SHA-256 |
| 248 | 32 | `CollectiveSchedule.v2` SHA-256 |
| 280 | 32 | `TargetBufferLifetime.v1` SHA-256 |
| 312 | 32 | accepted rank-set resource-budget SHA-256 |
| 344 | 32 | arena-table SHA-256 |
| 376 | 32 | class-span-table SHA-256 |
| 408 | 32 | buffer-use-table SHA-256 |
| 440 | 8 | reserved zero |
| 448 | 32 | plan SHA-256 |

The plan digest is:

```text
SHA256(
  "glmaxx.target-graph-memory-plan.v1\0" ||
  bytes 0..448 of the exact record
)
```

The five byte totals equal logical arenas 2, 1, 5, 6, and 7 respectively.
The target-KV and target-indexer allocation sizes remain bound through the
arena table and rank-set resource budget rather than being double-charged as
graph scratch. `GraphEntry.maximum_scratch_bytes` must equal the exact
arena-2 bytes, and `GraphEntry.argument_bytes` must equal arena-1 bytes.

The resource budget is an accepted pre-allocation ceiling record containing
the exact seven arena byte/alignment pairs and the nonarena context, module,
collective-library, graph-runtime, allocator-padding, and emergency-escrow
ceilings. It does not contain a physical-plan or GraphProfile-v3 digest. The
final rank-set memory plan is constructed afterward and binds the accepted
GraphProfile-v3 digest plus the same exact charges. This ordering prevents a
plan/profile/memory hash cycle while still making any byte drift fail closed.

The graph kind, buckets, depth, transport, target/MTP programs, program-set
digest, schedule, and module set must agree with the executor graph and every
node. Prefill and MTP0 decode have an absent MTP program exactly as required
by executor r4. A verify graph with MTP nodes has a nonzero MTP program and
the exact target-plus-MTP program-set digest. Rank-local fallback or a graph
whose nodes name another program generation is invalid.

## GraphProfile v3 binding

`GraphProfile.v3` preserves every logical v2 entry and adds one exact physical
plan digest per graph:

```text
SHA256(
  "glmaxx.graph-profile.v3\0" ||
  graph_profile_v2_sha256 ||
  u32_le(entry_count) ||
  for entries in ascending graph_id:
    u32_le(graph_id) || graph_memory_plan_sha256
)
```

Each v2 graph ID appears exactly once, and there are no extra plans. The
physical plan binds the v2 digest, so this construction has no hash cycle.
Startup consensus, graph lookup, capture, eager execution, and hot reload use
the v3 identity. A logical v2 profile alone cannot launch.

## Physical overlap and one-byte-short validation

Validation is performed with checked integer arithmetic before allocation and
again against adopted native arena bindings before capture.

Two class spans whose physical byte intervals overlap are accepted only when:

1. both are present scratch classes in arena 2;
2. both carry `ALIAS_REUSE`;
3. both logical lifetime records have the same nonzero alias class;
4. for every phase variant admitted by the graph, their resolved live
   intervals are disjoint; and
5. no buffer-use interval from one remains live when a use from the other can
   execute, including collective/event dependencies.

Equality at a half-open endpoint is nonoverlap. Every other overlap is fatal.
Classes 27..30, arena-level collective/status uses, immutable descriptors,
tentative destinations, and any zero-alias logical class never overlap
another live span.

The validator independently checks each use against its class and each class
against its arena. Reducing any present class capacity, arena bytes, or
buffer-use bytes by one must reject the plan or its reconstruction. Aggregate
scratch equality cannot make an undersized class pass.

## Rank-local materialization

One `DeviceArenaBinding.v1` is exactly 48 bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 2 | logical arena ID |
| 2 | 2 | executor arena role |
| 4 | 4 | native arena ID |
| 8 | 8 | device base address |
| 16 | 8 | native arena bytes |
| 24 | 8 | native arena generation |
| 32 | 4 | native alignment |
| 36 | 1 | TP rank |
| 37 | 1 | flags, zero |
| 38 | 2 | reserved zero |
| 40 | 8 | reserved zero |

The table has seven records in logical-arena order. It is constructed only by
the persistent rank owner from adopted
`glmaxx_executor_arena_binding_v1` records. The rank-local binding digest is:

```text
SHA256(
  "glmaxx.target-device-arena-binding-table.v1\0" ||
  u8(rank) || u8(0) || u16_le(7) || four_zero_bytes ||
  seven ordered 48-byte records
)
```

Each record must match the common arena role, byte count, and alignment, the
accepted resource budget, and the final rank-set memory plan. Checked
base-plus-offset resolution creates
every native `glmaxx_executor_span_v1`; no coordinator or request supplies a
device address. The binding table is copied to the validation span and the
device-validation node checks its generation and digest before any
data-dependent load.

`RankGraphMemoryReceipt.v1` is exactly 288 bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `G5GMRR1\0` |
| 8 | 2 | version, exactly 1 |
| 10 | 2 | record bytes, exactly 288 |
| 12 | 1 | TP rank |
| 13 | 1 | flags, zero |
| 14 | 2 | reserved zero |
| 16 | 8 | graph ID |
| 24 | 8 | process graph generation |
| 32 | 8 | runtime generation |
| 40 | 8 | rank-local resource generation |
| 48 | 32 | common graph-memory-plan SHA-256 |
| 80 | 32 | rank-local arena-binding SHA-256 |
| 112 | 32 | common GraphProfile-v3 SHA-256 |
| 144 | 32 | common program-set SHA-256 |
| 176 | 32 | common module-set capability SHA-256 |
| 208 | 32 | rank-local resolved-native-span SHA-256 |
| 240 | 16 | reserved zero |
| 256 | 32 | receipt SHA-256 |

The receipt digest is
`SHA256("glmaxx.rank-graph-memory-receipt.v1\0" || bytes 0..256)`.
The resolved-span digest is
`SHA256("glmaxx.target-resolved-native-spans.v1\0" || u32_le(use_count) ||
records)`, where each ordered record is exactly
`node_ordinal:u32_le, use_ordinal:u16_le, reserved_zero:u16_le,
native_arena_id:u32_le, reserved_zero:u32_le, offset:u64_le, bytes:u64_le,
generation:u64_le` (40 bytes). It uses native arena IDs rather than opaque
handles or base addresses.

Four receipts must agree on graph ID, process graph/runtime generations,
plan, profile, program-set, and module-set fields. Rank, arena-binding,
resource generation, and resolved-span digests are rank-local and enter the
ordered rank-set receipt. No graph becomes executable until all four receipts
pass consensus.

## Lifecycle, accounting, and hot reload

The creation order is:

1. accept all source contracts and CPU common plans;
2. accept the acyclic rank-set resource budget;
3. construct and validate the common physical plan and GraphProfile v3;
4. charge its seven exact arenas in the final rank-set memory plan;
5. create owner threads, contexts, modules, native arenas, and fixed routes;
6. construct and validate the seven rank-local binding tables;
7. build descriptors only from checked class/use resolution;
8. capture or instantiate the graph with the validation node first;
9. collect four rank receipts; and
10. publish one executable graph generation.

Allocation, resizing, pointer discovery, descriptor-selected capacity, or
first-use workspace creation inside capture or launch is forbidden. Arena
bytes are charged once by executor role. Logical class spans and uses are
views and add no second HBM charge.

A compatible hot reload may reuse resident weights and arenas only when the
new accepted physical plan fits every already allocated arena and all ranks
prepare the same new GraphProfile v3, module-set, program-set, plan, and
binding generations. Commit occurs at a common step boundary. Failure keeps
the old graph generation executable and destroys the candidate generation on
owner threads. No weight read or weight H2D is permitted by this operation.

## Required CPU/mock proof after review

Before any M3/M4 or production CUDA launch, one coordinated Rust proof must:

1. encode/decode and mutation-test the 32-, 48-, 80-, and 480-byte records
   and every hash domain;
2. reconstruct exact tables for actual GLM-5.2 M1/C1, C64/MTP0,
   C64/MTP3, C64/MTP6, and M3072 prefill graph keys;
3. cover dense/sparse, FULL/SHARED, query/CKV, target-only, and target-plus-MTP
   programs with their accepted operator workspace plans;
4. prove all 32 classes have the exact presence, arena, lifetime, capacity,
   and maximum-use result;
5. subtract one byte from every nonzero use, class, and arena in turn and
   prove fail-closed rejection;
6. exhaust legal disjoint-lifetime reuse and every prohibited live,
   external, argument, collective, and status overlap;
7. prove every native descriptor span is derived from an owner-created
   binding and no raw address can enter through the coordinator or request;
8. materialize four different rank-local address tables while preserving one
   common plan/profile/program/schedule identity;
9. reject stale arena, module, program, graph, page-table, argument, and
   runtime generations before launch;
10. simulate candidate hot reload success and rollback with unchanged weight
    read/H2D counters; and
11. reconcile every arena byte once against laboratory and production memory
    ledgers with no aggregate-scratch substitution.

The proof must use bounded synthetic allocations and mock addresses. It does
not authorize cn4 or imply that a CUDA graph, model layer, checkpoint, KV
capacity, quality gate, or performance target has passed.

## Gate effect and nonclaims

Acceptance opens only the CPU/mock implementation above. That implementation
must receive its own adversarial token before SM120 compilation or capture.
M3 and M4 must bind accepted GraphProfile-v3/physical-plan result hashes and
remain behind every earlier operator, collective, executor, and checkpoint
gate.

This design is not a CUDA ABI implementation, graph, kernel, collective,
layer replay, checkpoint smoke, serving path, MTP result, capacity result,
quality result, hot-reload result, or performance claim. It authorizes no cn4
work.
