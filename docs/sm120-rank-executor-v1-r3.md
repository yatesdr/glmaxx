# SM120 rank executor v1 corrective amendment r3

Date: 2026-08-04

Status: corrective design candidate; adversarial review required before
CPU/mock or native implementation

Base contracts:

- `docs/sm120-rank-executor-v1.md`
- `docs/sm120-rank-executor-v1-r2.md`
- `docs/sm120-rank-executor-native-abi-v1.h`

## Scope and precedence

This amendment closes three pre-review defects in r2's native ABI and one
related enumeration gap. It is normative over conflicting r1/r2 text. All r2
startup, memory, ownership, rank-consensus, route, tier, deadline,
backpressure, MTP-failure, proof, and nonclaim requirements remain in force.

The ABI version remains one because no implementation or accepted v1 ABI
exists. Any implementation must use the header bytes pinned by this r3
candidate; the r2 header is superseded.

## Exact C11 and C++17 layout

The header must apply 16-byte type alignment in both accepted language modes:

```text
C++17: alignas(16)
C11 on target Clang/GNU: __attribute__((aligned(16)))
```

Any other C compiler fails preprocessing instead of silently dropping the
alignment. The header performs all 18 size and all 18 alignment assertions in
both languages. C uses `_Static_assert` and `_Alignof`; C++ uses
`static_assert` and `alignof`.

The CPU proof compiles the header as both C11 and C++17, then independently
emits every field offset, size, alignment, enum value, and function pointer
type from both modes. It compares both records byte-for-byte with a separate
Rust `#[repr(C, align(16))]` record. A C-only empty alignment macro, a
C++-only assertion, implicit enum value, or signature mismatch is fatal.

## Frozen flags, families, and arena roles

Every v1 `flags` field, including capability/status outputs and the raw flags
arguments to stream/event creation, must equal
`GLMAXX_EXECUTOR_FLAGS_V1_NONE`. Native code creates nonblocking CUDA streams
unconditionally; a caller cannot request default-stream behavior through a
flag. Any nonzero bit requires ABI v2 and is rejected before a CUDA or NCCL
call.

`required_compute_capability` and reported `compute_capability` use the exact
integer `120` for SM120. They are not `(major,minor)` packed bytes, CUDA
version numbers, or hexadecimal encodings.

The only v1 module capability families are:

```text
1 TARGET_PROGRAM
2 MTP_PROGRAM
3 DEVICE_VALIDATION
```

Capability records are strictly ascending and unique by family. The adopted
module set has exactly one target and one validation family. It has exactly
one MTP family when the immutable production profile supports MTP1..6, and no
MTP family only for an explicitly target-only MTP0 profile. Unknown, duplicate,
missing, or posture-incompatible families fail before graph construction.

The arena-role enumeration is exact:

| IDs | Kind | Roles |
|---:|---|---|
| 1..12 | device | weights, codec metadata, target KV, target indexer, draft sidecar, page table, graph argument, graph scratch, collective, tier transfer, completion/status, diagnostic/status |
| 13..18 | pinned host | checkpoint staging, argument mirror, completion mirror, tier-in, tier-out, diagnostic/status |

The header freezes the individual numeric values. Device roles cannot use a
host-pinned kind and host roles cannot use a device kind. The rank memory plan
contains one or more uniquely identified arenas for the required roles, but
each `arena_id` occurs exactly once and every byte is charged to exactly one
role. A missing required role, unknown role, kind/role mismatch, duplicate
arena ID, uncharged span, or role change after planning fails startup.

## Module handle is the only program native object

`glmaxx_executor_graph_node_v1.native_object` has one exact meaning by node
kind:

```text
TARGET_PROGRAM  -> adopted module handle with TARGET_PROGRAM capability
MTP_PROGRAM     -> adopted module handle with MTP_PROGRAM capability
COLLECTIVE      -> adopted immutable route handle
STATUS_FINALIZE -> zero
```

There is no program-handle type, hidden program constructor, symbol handle,
or implementation-selected alternative in v1. The module may provide more
than one capability record, so the same module handle may legally back target
and MTP nodes. `node_kind` selects the one fixed native entry family;
`program_sha256` binds the accepted rank-common target- or MTP-program semantic
identity copied into the graph node. The module-set capability digest binds
the exact header, module image hashes, and ordered capability records.

The generic graph-node entry rejects validation nodes. The dedicated
validation-node entry is the only construction route for
`DEVICE_VALIDATION`, and it binds the adopted validation capability. A module
without the exact node-family capability cannot be used even if its image
exports a similarly named symbol.

## Explicit context synchronization

The ABI adds:

```c
int32_t glmaxx_executor_context_synchronize_v1(
    glmaxx_executor_handle_v1 context,
    struct glmaxx_executor_error_v1* error) noexcept;
```

It is owner-thread-only and legal only in these already permitted states:

1. startup qualification before health;
2. generation-fatal diagnostics after new commands are closed and all
   communicators have been ordered to abort; or
3. final shutdown after admission is closed.

It is forbidden in healthy step, tier, prefix, reload, and profiling hot
paths. Ordinary completion uses the fixed event DAG and `event_query`.

Normal shutdown first closes command admission and proves every accepted
operation terminal through its completion event. It then calls context
synchronize once before destroying any graph, route, event, stream, arena,
module, peer edge, or context. This supplies the final no-outstanding-device-
borrow proof needed before native-owned pinned or device arenas are released.

On a fatal generation, communicator abort is issued first. The owner then
attempts context synchronization for bounded diagnostic/cleanup evidence. An
out-of-band process supervisor owns the absolute fatal-shutdown deadline
because a native driver call may not return after device loss. If
synchronization fails or the supervisor deadline expires, Rust records the
preallocated terminal evidence when possible, does not free any arena or
other resource that may still be borrowed by device work, and terminates the
process. Leaking into process termination is allowed; leaking and continuing
service is forbidden.

`context_synchronize` overwrites the required 64-byte error record on every
call and returns only the frozen executor status taxonomy. CUDA error values
remain confined to `native_code`; no exception may cross the C boundary.

## Corrected CPU/mock gate

In addition to the complete r2 matrix, the independent CPU/mock proof must:

1. compile and record identical C11/C++17 layouts, including all field
   offsets and function pointer signatures;
2. mutation-test all flag fields, compute-capability encodings, module-family
   IDs, arena-role IDs, and kind/role combinations;
3. reject missing, duplicate, unknown, or posture-incompatible module
   capability records;
4. prove each graph node resolves exactly the native-object meaning above and
   that no program handle can be constructed or supplied;
5. reject validation through the generic graph-node entry and a target/MTP
   node backed by the wrong capability;
6. prove normal shutdown synchronizes before the first resource destruction;
7. prove healthy hot paths never call context synchronization; and
8. inject synchronization failure and a nonreturning fatal call, showing that
   no possibly borrowed resource is freed and the supervisor terminates the
   process instead of permitting recovery.

Only unqualified adversarial acceptance of r1+r2+r3 and the corrected header
permits the coordinated CPU/mock executor implementation to begin. It does
not accept current Rust workers, a native library, cn4 execution, checkpoint
loading, graph capture, collectives, target/MTP execution, quality, capacity,
concurrency, or performance.

