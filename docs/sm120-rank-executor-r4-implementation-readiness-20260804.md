# SM120 rank-executor r4 implementation readiness

Date: 2026-08-04

Status: static implementation map; no r4 acceptance token, CPU/mock executor,
native executor, CUDA launch, or model result

## Current executable boundary

The existing `NativeCheckpointRankExecutor` already constructs one thread-
affine `NativeRankContext` per persistent rank worker, opens and validates a
native rank image, uploads an immutable weight arena, participates in the
four-rank adoption transaction, and tears the arena down through acknowledged
rank-owner cleanup. Its step path deliberately returns
`NATIVE_PROGRAM_NOT_IMPLEMENTED` after proving that the arena and owner context
are resident.

The current `glm-cuda` native boundary is the earlier narrow launch/load API.
It exports device bind, raw allocation, pinned allocation, streams, events,
copies, individual NVFP4/EXL3 controls, and profiler hooks. It does not
implement any of the 35 `glmaxx_executor_*_v1` functions, a module registry,
NCCL communicator, immutable route, cooperative graph builder, validation
module, device-status record, or generation-aware handle table. Those raw
handles therefore cannot be relabeled as the r4 executor ABI.

The normative header compiles as C11 and C++17 in the local gate. That proves
layout syntax only; it is not a Rust mirror or an implementation.

## Gate-controlled implementation cut

Only the exact token
`sm120-rank-executor-v1-r4-design-accepted` may open the coordinated CPU/mock
proof below. R4 acceptance still does not open a native library or CUDA work.

The CPU/mock change should be split into four reviewable pieces:

1. Add a private Rust ABI module with exact `#[repr(C, align(16))]` mirrors for
   every r4 enum, handle, and structure plus all 35 function signatures. Add
   compile-time size/alignment/offset assertions and a generated C11/C++17/
   Rust signature manifest. No mock behavior belongs in this piece.
2. Add an owner-thread mock resource table. Each nonzero handle records its
   kind, owner thread, context identity, generation, live/poisoned state, and
   borrow count. Implement context, peer, module/capability, arena/span,
   stream/event, and copy validation without allocating CUDA resources.
3. Add process-common mock collective coordination and owner-local immutable
   routes, graph builders, graph instances, validation-module binding,
   asynchronous completion/status, and fatal shutdown. Preserve explicit
   rank-common decisions while every resource handle remains rank-local.
4. Add one deterministic proof fixture exercising four persistent mock rank
   owners, both resident module generations, all normal and fatal destruction
   orders, and every r1-r4 rejection. Keep the production
   `NativeCheckpointRankExecutor::execute*` fail-closed.

A separate implementation review must accept those bytes before a native ABI
library begins. Target/MTP program gates must then accept their own program
descriptors before the native checkpoint executor may stop returning
`NATIVE_PROGRAM_NOT_IMPLEMENTED`.

## Rust ownership model

The ABI mirror contains plain C-compatible values only. Safe wrappers own all
semantics and carry `PhantomData<Rc<()>>`, making contexts, modules, arenas,
streams, events, communicators, routes, graph builders, graphs, and status
views `!Send` and `!Sync`. Only immutable rank-common plans, digests, and the
existing factory closure cross into a worker thread.

The mock registry uses a monotonically increasing nonzero handle serial and
never reuses a retired serial. Lookup checks, in order:

```text
nonzero -> known -> expected kind -> live -> owner thread -> context -> generation
```

Failure performs no partial transition. A poisoned context permits only the
contract's synchronization, abort, query, and destruction operations. The
mock must not use host pointer identity or registry iteration order in a
rank-common digest.

Module adoption is an explicit immutable generation record containing the
ordered module handles and the accepted module-set capability digest. The r4
validation-node operation takes the exact adopted validation-module handle.
Graph instantiation converts every builder module borrow into an executable-
graph borrow; destroying the builder cannot release an executable borrow.
Module unload fails while either borrow count is nonzero. Old and candidate
generations may coexist, but no operation scans them or selects the newest.

## Native follow-on boundary

After the CPU/mock implementation is accepted, the native library should be a
new target under the GLMAXX build and symbol namespace, not aliases layered on
the existing ad-hoc exports. Its opaque handles need type, owner, context,
generation, and lifecycle validation before dereferencing native objects.

The existing CUDA checkpoint loader can remain as the first weight-ingest
backend while executor arenas and copies are qualified. Migration is complete
only when one rank owner holds one r4 context and all weight, KV, graph,
collective, status, and staging resources are children of that context. A
temporary process containing both APIs may not pass raw handles between them.

The native implementation requires, at minimum:

- CUDA driver module load and exact capability-table query;
- one nonblocking owner stream plus explicit auxiliary streams/events;
- deterministic arenas for every role in the r4 header;
- NCCL communicator construction and immutable route implementations;
- cooperative graph construction with explicit target, MTP, collective,
  validation, and status-finalization nodes;
- a dedicated device-validation kernel exported by the explicitly supplied
  validation module;
- asynchronous status polling with common fatal synchronization; and
- reverse-topological cleanup that refuses false success after any native
  destruction failure.

The first native qualification remains actual SM120 only. No A6000, SM100,
mock, or header-layout result can stand in for it.

## Deterministic CPU/mock matrix

The post-token proof must cover every operation in the 35-function manifest
and at least these state boundaries:

1. exact ABI version, structure bytes, alignment, reserved-zero fields, enum
   values, flags, handles, addresses, and complete signatures in C11, C++17,
   and Rust;
2. four distinct owner threads and contexts, all cross-thread/cross-context
   substitutions, stale generations, wrong kinds, zero/unknown handles, and
   serial exhaustion;
3. module load, capability census, duplicate/missing/wrong families, adoption,
   coexistence of old/candidate generations, graph borrows, and unload order;
4. every arena role/kind, checked span bounds and alignment, copy direction,
   alias rule, generation, zeroing, and destruction dependency;
5. stream/event record, wait, not-ready, async failure, poisoned state, normal
   synchronization, fatal synchronization, and teardown;
6. four-rank unique-ID and communicator agreement, all route families,
   participant masks, topology/route digests, ordinal bounds, byte ceilings,
   abort, and destroy ordering;
7. graph/profile/bucket/node sequencing, dedicated validation-node insertion,
   generic validation-node rejection, explicit validation-module generation,
   instantiate, launch, completion, status checksum, and destruction;
8. rank-local failure before any collective, inside every collective ordinal,
   after the last collective, and during cleanup, proving one common terminal
   outcome without a rank-local fallback; and
9. deterministic fixture bytes in debug/release and 100 fresh-process guarded
   fatal schedules with no leaked handle or live mock owner.

## Shortest post-acceptance path

The first useful deliverable is not a large native rewrite. It is the exact
Rust mirror plus four-owner mock state machine and fixture. That provides an
executable specification for the native implementation, lets Fable attack
resource lifetime and hot-generation behavior without a GPU, and prevents the
native code from inventing semantics during CUDA bring-up.

Until that implementation review passes, the current native checkpoint loader
may prove load/adoption only and the execution path must continue to fail
closed. This record does not authorize code from an unaccepted design.
