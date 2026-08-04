# SM120 rank executor v1 corrective amendment r5

Date: 2026-08-04

Status: corrective design candidate; adversarial review required before
CPU/mock or native implementation

Base contracts:

- `docs/sm120-rank-executor-v1.md`
- `docs/sm120-rank-executor-v1-r2.md`
- `docs/sm120-rank-executor-v1-r3.md`
- `docs/sm120-rank-executor-v1-r4.md`
- `docs/sm120-rank-executor-native-abi-v1.h`
- `docs/target-graph-physical-memory-v1.md`

## Scope and precedence

R1 requires persistent pending logits and proposal state, while r3 froze
device arena role 5 as only `DEVICE_DRAFT_SIDECAR`. Target-layer r2 then gave
pending target logits the external class-30 lifetime, including target-only
MTP0 operation. Calling that allocation a draft-only sidecar leaves a required
cross-step target state without an exact executor role.

R1-r4 also require device validation of every captured span but the module
capability record has no field proving that an adopted target, MTP, or
validation module implements the accepted physical graph-memory table ABI.
Its final 64 bytes remain reserved zero, so a module generation can advertise
the right family while interpreting a different arena/class/use table.

This amendment closes those two gaps. It is normative over conflicting r1-r4
text. The ABI version remains one because no executor ABI has been accepted or
implemented. Record sizes, alignments, function signatures, status values,
and all other r4 behavior remain unchanged.

## Recurrent-state arena role

Executor arena role 5 is exactly:

```c
GLMAXX_ARENA_ROLE_DEVICE_RECURRENT_STATE = 5
```

The old `GLMAXX_ARENA_ROLE_DEVICE_DRAFT_SIDECAR` spelling is removed, not
aliased. Role 5 owns every persistent cross-step model-state allocation that
is neither target KV/indexer, page table, nor completion metadata:

- target pending-logit slots and generations for MTP0 and MTP1..6;
- proposal distributions/support records and q state;
- authoritative boundary hidden state;
- recurrent teacher scratch retained across the physical-step transaction;
- successor-slot draft KV/indexer sidecars; and
- fixed MTP bundle/counter state that must be device resident.

Each subarena remains independently byte-specified by the accepted target/MTP
memory and physical graph plans. The broader role is not permission to merge
lifetimes, hide allocator padding, or replace a required byte allocation with
a digest. Target-only MTP0 may have zero proposal and draft-KV subranges while
retaining nonzero pending-logit state. A target-only graph can therefore use
role 5 without falsely claiming a draft program.

Every byte is charged once under recurrent state in the rank memory plan. A
legacy role name, draft-only validator, zero role-5 allocation with nonzero
class 30, or rank-local role interpretation fails before allocation.

## Physical graph-memory capability binding

The 192-byte `glmaxx_executor_module_capability_v1` record replaces its first
four reserved `u64` values at bytes `128..160` with:

```c
uint8_t graph_memory_abi_sha256[32];
```

The remaining `reserved[4]` at bytes `160..192` are zero. The required value
for TARGET_PROGRAM, MTP_PROGRAM, and DEVICE_VALIDATION is:

```text
SHA256("glmaxx.target-graph-physical-memory-abi.v1\0")
  = 68ac6d6113973e61f863980f1b42a7479466164fc795f91560f0a15a4614d3b8
```

It is nonzero and identical across every capability in one adopted module
set. R1-r4 named but did not serialize the capability digest construction.
R5 closes it. For one record:

```text
family_capability_sha256 = SHA256(
  "glmaxx.executor-module-family-capability.v1\0" ||
  exact 192-byte capability record with bytes 96..128 zero
)
```

Capability records are ordered by `(module_sha256,kernel_family)` and unique.
Every adopted module appears in at least one record. The set digest is:

```text
module_set_capability_sha256 = SHA256(
  "glmaxx.executor-module-set-capability.v1\0" ||
  SHA256(exact accepted native header bytes) ||
  u16_le(capability_count) || ordered exact 192-byte capability records
)
```

The complete records contain their module hashes and graph-memory identities,
so the digest changes for a different header, module image, family, descriptor
surface, row/bucket ceiling, codec/role mask, graph-memory ABI, or reserved
byte. The capability count must equal the closed target/optional-MTP/
validation family set from r3.

An unknown graph-memory identity, zero identity, mixed identities within the
module set, or a target/MTP/validation module that omits it fails module-set
adoption. The executor never selects an older parser, copies only a familiar
prefix, or falls back to host-only validation.

The binding means exactly that the module generation consumes the reviewed
`GraphMemoryPlan.v1`, `GraphClassSpan.v1`, `GraphBufferUse.v1`, and
`DeviceArenaBinding.v1` semantics, including all ten graph-visible arenas and
the immutable weight, codec-metadata, and page-table uses omitted by the
earlier draft. It does not let a module invent sizes:
the process-common tables are reconstructed from accepted programs/operator
plans and checked by Rust before allocation, then independently checked by
the first device-validation node before any data-dependent pointer use.

## Coordinated graph construction

R4's validation descriptor retains its exact 192-byte size and complete
target-plus-optional-MTP program-set digest. Its `arena_table` span now points
to the exact rank-local `DeviceArenaBinding.v1` table selected by the common
physical plan. The explicitly supplied validation module must advertise the
same graph-memory ABI digest as every target/MTP module used by that graph.

Graph construction fails before capture when any of these differ:

```text
GraphProfile.v3 physical-plan identity
GraphMemoryPlan.v1 digest
target/MTP executor program-set digest
module-set capability digest
graph-memory ABI digest
rank-set memory-plan digest
rank-local ten-arena binding generations and roles
```

The validation node still executes first. A validation latch can change only
values, never node count, pointer, byte count, route, participant mask, or
collective ordinal. All r4 generation, hot-reload, explicit-module,
owner-thread, synchronization, and fail-stop rules remain in force.

## Corrected CPU/mock gate

In addition to the complete r1-r4 matrix, the coordinated proof must:

1. compile the renamed role and revised capability layout under C11, C++17,
   and the independent Rust mirror with unchanged 192-byte size/alignment;
2. reject the removed draft-only enum spelling and every unknown, zero, or
   rank-divergent role-5 interpretation;
3. construct MTP0 with nonzero pending logits but zero proposal/draft
   subranges, plus MTP3/MTP6 with their exact recurrent-state terms;
4. prove every recurrent-state subrange is charged once and cannot overlap a
   simultaneously live subrange;
5. recompute the graph-memory ABI digest and mutation-test it in each module
   family, family-capability digest, module-set digest, physical plan, and
   validation input;
6. reject mixed old/new module generations even when their target program
   digests match;
7. prove the explicitly supplied validation module and every target/MTP node
   use the same accepted ten-arena table interpretation, with every
   weight/metadata/page-table pointer owner-derived and generation-bound; and
8. repeat hot-reload prepare, commit, rollback, and module retirement while
   keeping old and candidate graph-memory generations resident.

Only unqualified adversarial acceptance of r1-r5, the corrected header, and
the physical-memory design permits their coordinated CPU/mock implementation.
This amendment accepts no current Rust worker, native library, graph, CUDA
launch, checkpoint, KV capacity, model quality, concurrency, hot reload, or
performance result. It authorizes no cn4 work.
