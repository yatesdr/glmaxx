# Checkpoint load transaction v1

Date: 2026-07-30

Status: corrected r3 contract candidate; CUDA arena CPU proof implemented,
native CUDA qualification and integrated checkpoint load pending adversarial
review

GPU claim: none

## Purpose

The native rank reader can verify and stream four rank files without
materializing their payloads. This contract defines the next boundary: staging
those tentative bytes into four rank-local device arenas without making one
rank, tensor, or partially verified generation executable.

The engine loads exactly one weight generation in v0. Hot replacement and
runtime weight paging are outside scope.

## Normative startup order

The engine specification's startup state machine is authoritative:

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
```

The current Rust mock uses different names and places its memory proof after
weight loading. That mock is not the production ordering and must be replaced
or versioned before implementation. In particular:

- the four-file validator completes at `HOST_VALIDATED`, before any GPU load;
- modules and codec capability tables exist before arena planning;
- the measured per-rank memory plan is accepted before allocating weight
  arenas; and
- no request enters scheduling before the later graph, KV, collective, and
  smoke gates reach `HEALTHY`.

No implementation may silently map the current mock states onto the normative
states when their ordering differs.

## Immutable load plan

The coordinator constructs one `RankSetLoadPlan.v1` after host validation and
before device allocation. It contains:

```text
schema
conversion_uuid
verification_mode
serving_profile
weight_policy_sha256
kernel_abi_sha256
memory_plan_sha256
codec_capability_sha256
model_config_sha256
tokenizer_bundle_sha256
chat_template_sha256
operation_manifest_sha256
tensor_catalog_sha256
profile_budget_sha256
rank[4]
plan_sha256
```

Each rank entry binds:

```text
rank
CUDA device identity
file_uuid
manifest_sha256
descriptor_sha256
payload_sha256
rank_manifest_tensor_contract_sha256
tensor_count
file payload bytes
device weight-arena bytes
device metadata-arena bytes
arena-layout SHA-256
```

The plan SHA-256 is over a fixed-order binary encoding, not JSON text. The
plan rejects zero identities, duplicate/missing ranks, arithmetic overflow,
an arena interval overlap, a tensor absent from the fixed GLM-5.2 operation
manifest, a rank-entry tensor count different from the common header count,
and any descriptor-to-arena interval mismatch.

Every tensor layout entry binds its tensor ID, role, codec, primary,
auxiliary, and codec-metadata destination offsets and lengths. The complete
layout is fixed before streaming begins. A rank cannot infer a different
layout, codec route, alignment, or protection decision from local device
pressure.

Plane lengths and alignment are derived from the process-common compiled
GLM-5.2 contract and immutable weight policy. In particular, EXL3 projection
bytes come from the policy-fixed capacity plan. Every rank-file descriptor is
validated against those lengths; rank files never define or resize the
physical plan. A contiguous TP slice is legal only when the global extent is
exactly divisible by four, and validation requires
`rank_logical_extent * 4 == global_logical_extent`.

The codec capability table is process-common and hash-bound. A profile
containing EXL3 source payloads is rejected before allocation while the EXL3
device-load gate is closed. A direct-layout flag is a required property, not
permission to guess an unreviewed kernel route.

### Canonical plan encoding

All integers below are little-endian. Reserved bytes are zero. The plan
preimage is:

```text
416-byte RankSetLoadPlanHeader.v1
4 × 248-byte RankLoadEntry.v1, rank order
rank 0 TensorArenaEntry.v1 records, tensor-ID order
rank 1 TensorArenaEntry.v1 records, tensor-ID order
rank 2 TensorArenaEntry.v1 records, tensor-ID order
rank 3 TensorArenaEntry.v1 records, tensor-ID order
```

The 416-byte header is:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `G5LOAD1\0` |
| 8 | 2 | version `1` |
| 10 | 2 | header bytes `416` |
| 12 | 1 | verification mode: `1=FULL_SHA256`, `2=FS_VERITY` |
| 13 | 1 | profile: `1=nvfp4-laboratory`, `2=capacity-exl3`, `3=hybrid-serve` |
| 14 | 1 | rank count `4` |
| 15 | 1 | reserved |
| 16 | 4 | common tensor count |
| 20 | 4 | rank-entry bytes `248` |
| 24 | 4 | tensor-entry bytes `64` |
| 28 | 4 | reader chunk bytes `8,388,608` |
| 32 | 16 | conversion UUID |
| 48 | 32 | weight-policy SHA-256 |
| 80 | 32 | kernel-ABI SHA-256 |
| 112 | 32 | memory-plan SHA-256 |
| 144 | 32 | codec-capability SHA-256 |
| 176 | 32 | model-config SHA-256 |
| 208 | 32 | tokenizer-bundle SHA-256 |
| 240 | 32 | chat-template SHA-256 |
| 272 | 32 | operation-manifest SHA-256 |
| 304 | 32 | tensor-catalog SHA-256 |
| 336 | 32 | reviewed profile-budget SHA-256 |
| 368 | 4 | pinned staging-slot bytes; at least reader chunk bytes |
| 372 | 2 | pinned staging slots per rank; at least `2` |
| 374 | 42 | reserved |

Each 248-byte rank entry is:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 1 | rank |
| 1 | 7 | reserved |
| 8 | 32 | CUDA-device identity SHA-256 |
| 40 | 16 | file UUID |
| 56 | 32 | manifest SHA-256 |
| 88 | 32 | descriptor SHA-256 |
| 120 | 32 | payload SHA-256 |
| 152 | 4 | tensor count |
| 156 | 4 | reserved |
| 160 | 8 | file payload bytes |
| 168 | 8 | device weight-arena bytes |
| 176 | 8 | device metadata-arena bytes |
| 184 | 32 | arena-layout SHA-256 |
| 216 | 32 | rank-manifest tensor-contract SHA-256 |

Each 64-byte tensor entry is:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 4 | tensor ID |
| 4 | 2 | role ID |
| 6 | 2 | codec ID |
| 8 | 4 | descriptor flags |
| 12 | 8 | metadata destination offset |
| 20 | 8 | metadata bytes |
| 28 | 8 | primary destination offset |
| 36 | 8 | primary bytes |
| 44 | 8 | auxiliary destination offset |
| 52 | 8 | auxiliary bytes |
| 60 | 4 | required device alignment |

`plan_sha256 = SHA256("glmaxx.rank-set-load-plan.v1\0" || plan_preimage)`.
The file's names, logical/padded shapes, layer/expert identities, TP axes, and
codec semantics remain bound through the validated descriptor and tensor
catalog hashes rather than being duplicated in this physical layout table.
Each rank's manifest `tensor_contract_sha256` is recomputed from its complete
canonical tensor inventory and stored in that rank's entry. It is
intentionally not process-common: EXL3 codec metadata contains the rank, and
rank-local source component names and slice bounds differ.

The header's common tensor-catalog digest is instead derived independently
from rank-invariant semantic records defined below. All four ranks must derive
the same digest. The operation-manifest digest must match the compiled
GLM-5.2 manifest. A serving profile also requires the exact reviewed
profile-budget digest with `measurement_status=complete` and
`conversion_allowed=true`. The laboratory subset uses a separately identified
non-serving budget and cannot be promoted by changing only the profile byte.

### Rank-invariant tensor catalog

The common catalog does not erase a rank-local mismatch by simply ignoring
whole manifest objects. For each tensor, the loader first validates the full
rank-specific manifest record against that rank's descriptor, codec metadata,
source binding, and physical layout. It then projects exactly the fields below
into a 128-byte `TensorSemanticEntry.v1`.

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 4 | tensor ID |
| 4 | 2 | role ID |
| 6 | 2 | codec ID |
| 8 | 2 | signed layer ID |
| 10 | 2 | signed expert ID |
| 12 | 1 | signed TP shard axis |
| 13 | 1 | ndim |
| 14 | 1 | manifest tensor flags |
| 15 | 1 | source-binding kind |
| 16 | 2 | logical dtype |
| 18 | 2 | stored dtype |
| 20 | 4 | quantization-group elements |
| 24 | 16 | rank logical shape as four u32 values |
| 40 | 32 | global logical shape as four u64 values |
| 72 | 32 | SHA-256 of exact UTF-8 tensor name |
| 104 | 2 | reconstruction ID |
| 106 | 2 | collective-after ID |
| 108 | 2 | source-dtype ID |
| 110 | 1 | signed source axis |
| 111 | 17 | reserved |

Unused shape entries are one. The enumerations are fixed by the generated
GLM-5.2 operation manifest and the pinned checkpoint source contract; unknown
values fail rather than receiving a dynamic ID. The catalog preimage is
`little_endian_u32(tensor_count)` followed by entries in tensor-ID order:

```text
tensor_catalog_sha256 = SHA256(
    "glmaxx.rank-invariant-tensor-catalog.v1\0"
    || tensor_count
    || semantic_entry_0
    || ...
    || semantic_entry_N
)
```

Enumeration values are:

- source binding: `1=replicated`, `2=contiguous_tp_slice`,
  `3=explicit_rank_components`;
- reconstruction: `1=byte_exact_source_precision`,
  `2=exl3_tr3_trellis_v0`;
- collective-after: `0=none`, `1=tp_embedding_reduce`,
  `2=distributed_sampling`, `3=tp_all_reduce`; and
- source dtype: `1..22` in this fixed order:
  `BOOL,F4,F6_E2M3,F6_E3M2,U8,I8,U16,I16,U32,I32,U64,I64,F16,BF16,F32,F64,`
  `F8_E4M3,F8_E5M2,F8_E8M0,F8_E4M3FNUZ,F8_E5M2FNUZ,C64`;
  `0x8000=EXL3_TR3_COMPONENTS`.

No current pinned tensor uses an omitted safetensors dtype. Adding one requires
a catalog version change rather than reusing an ID.

Rank-local codec-metadata hashes, source component paths, source slice
start/end, padded shapes, plane byte counts, file offsets, device offsets,
payload hashes, and alignment gaps are excluded only from this common
projection. They remain mandatory in the rank manifest, descriptor,
rank-entry hashes, and physical arena tables. Projection never substitutes
for validation of the full rank-specific record.

`source_shape` is derivable in v1 and is therefore not repeated in the
semantic entry: it equals `global_logical_shape` for replicated and contiguous
TP sources and is empty for the pinned EXL3 component source. Any future source
kind for which that rule is false requires a catalog version change.

All canonical encodings in this contract are serialized field by field.
Implementations must not overlay a native `repr(C)` struct; several u64 fields
in `TensorArenaEntry.v1` are intentionally unaligned in the byte record.

## Quarantined arena ownership

Each persistent rank thread allocates its planned weight and metadata arenas
and returns a non-executable `QuarantinedRankArena`. Only that thread may
access its CUDA context, stream, events, or allocations.

The type state is:

```text
Allocated -> Staging -> Prepared -> Adopted
     \          \          \
      +----------+-----------> Aborted
```

There is no reverse transition and no public conversion from `Allocated`,
`Staging`, or `Prepared` to the executor's `WeightArenaHandle`.
`QuarantinedRankArena::drop` synchronizes any outstanding copies and frees
the generation. Its pinned ring slots, events, device allocations, and load
stream are released only after every recorded slot event has completed or the
owning load stream has synchronized, including on an early error, panic, or
process-wide abort. If synchronization itself fails, the implementation must
leak the pinned ring and every possibly referenced native resource and
terminate the process; it must never free memory that DMA may still reference.
Aborting one rank aborts all four.

Both complete device arenas are asynchronously zero-filled before staging.
Every declared destination interval is then overwritten exactly once from its
matching descriptor plane, so alignment gaps and unused tails have canonical
zero contents. File padding is verified by the reader but is not copied into
HBM. Codec metadata is copied into the immutable metadata arena.

## Streaming and asynchronous-copy lifetime

`NativeRankReader::verify_and_stream` owns an 8 MiB ordinary host buffer and
may reuse it immediately after a sink callback returns. A CUDA sink therefore
must not retain the callback pointer.

Each rank sink owns the plan's fixed-capacity pinned host staging ring and one
completion event per slot. For every callback it:

1. waits only when the next ring slot is still in flight;
2. copies the borrowed bytes into that owned pinned slot;
3. checks the planned tensor, plane, cursor, and destination bounds;
4. enqueues one host-to-device copy on the owner rank's load stream;
5. records the slot event; and
6. returns only after it no longer depends on the reader's borrowed bytes.

The final partial chunk is copied at its exact length. Ring slots are not
reused before their events complete. Reader and sink buffers, CUDA events,
and load streams are fixed before staging; the per-chunk path allocates
nothing.

The reader hashes the bytes it read. After all copy events complete and the
load stream synchronizes, the first `FULL_SHA256` load reads both entire
device arenas back through one fixed 8 MiB pinned slot. Each D2H chunk records
and synchronizes its event before ordinary host code reads the slot. The
loader hashes the full observed arenas, including zero gaps and tails, and
compares them with host-computed full-arena hashes from the planned zero-fill
and exact upload stream. A mismatch or any D2H/event error poisons the
generation and prevents adoption.

The bounded read-back subrecord is `CudaArenaVerificationEvidence.v1`.
Integers are little-endian and reserved bytes are zero:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 1 | rank |
| 1 | 7 | reserved |
| 8 | 32 | plan SHA-256 |
| 40 | 8 | owner allocation generation |
| 48 | 8 | device weight-arena bytes |
| 56 | 8 | device metadata-arena bytes |
| 64 | 4 | read-back chunk bytes, exactly `8,388,608` |
| 68 | 4 | reserved |
| 72 | 8 | total D2H chunk count across both arenas |
| 80 | 32 | host-expected weight-arena SHA-256 |
| 112 | 32 | observed weight-arena SHA-256 |
| 144 | 32 | host-expected metadata-arena SHA-256 |
| 176 | 32 | observed metadata-arena SHA-256 |

Its digest is
`SHA256("glmaxx.cuda-arena-readback.v1\0" || 208-byte subrecord)`.
The expected and observed hashes must be equal before the subrecord may enter
preparation evidence. A separately reviewed device-digest kernel may replace
the D2H pass later, but copy completion alone is never sufficient evidence.

## Full verification and preparation

The first load uses `FULL_SHA256`. All four readers run with bounded
concurrency, and each rank records:

- bytes read and uploaded;
- manifest, descriptor, metadata, auxiliary, tensor-plane, and complete
  payload verification results;
- maximum reader and pinned-ring scratch;
- storage-read, host-copy, PCIe-copy, and synchronization elapsed time; and
- its prepared-arena receipt.

An FS-verity restart route remains unavailable until a separate implementation
and evidence gate pins the root-digest provenance. Size, mtime, inode, CRC, or
a prior successful run cannot substitute for the first full proof.

After a rank's payload returns success, its sink drains every event, validates
exact primary/auxiliary/metadata byte totals, seals the allocations against
further writes, completes the mandatory arena read-back, and produces a
`PreparedRankReceipt.v1`. The receipt binds the rank-set plan hash, rank,
device identity, file UUID, payload hash, arena layout hash, allocation sizes,
verification mode, verified bytes, uploaded bytes, and a monotonically
increasing owner-thread generation.

The receipt is exactly 256 bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `G5PRP1\0\0` |
| 8 | 2 | version `1` |
| 10 | 2 | record bytes `256` |
| 12 | 1 | rank |
| 13 | 1 | verification mode |
| 14 | 2 | reserved |
| 16 | 32 | CUDA-device identity SHA-256 |
| 48 | 16 | file UUID |
| 64 | 32 | plan SHA-256 |
| 96 | 32 | payload SHA-256 |
| 128 | 32 | arena-layout SHA-256 |
| 160 | 8 | device weight-arena bytes |
| 168 | 8 | device metadata-arena bytes |
| 176 | 8 | verified file-payload bytes |
| 184 | 8 | uploaded plane/metadata bytes |
| 192 | 8 | owner-thread allocation generation |
| 200 | 32 | verification-evidence SHA-256 |
| 232 | 24 | reserved |

`prepared_rank_sha256` hashes the domain
`"glmaxx.prepared-rank-receipt.v1\0"` followed by the record bytes.

The receipt's `verification_evidence_sha256` is not an arbitrary digest. It
hashes a fixed 256-byte `RankLoadVerificationEvidence.v1` record:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `G5LVE1\0\0` |
| 8 | 2 | version `1` |
| 10 | 2 | record bytes `256` |
| 12 | 1 | rank |
| 13 | 1 | verification mode |
| 14 | 2 | reserved |
| 16 | 32 | plan SHA-256 |
| 48 | 32 | CUDA-device identity SHA-256 |
| 80 | 8 | owner allocation generation |
| 88 | 8 | verified file-payload bytes |
| 96 | 4 | tensor count |
| 100 | 4 | reserved |
| 104 | 8 | uploaded metadata bytes |
| 112 | 8 | uploaded primary bytes |
| 120 | 8 | uploaded auxiliary bytes |
| 128 | 8 | total uploaded bytes |
| 136 | 8 | maximum reader scratch bytes |
| 144 | 8 | pinned-ring bytes |
| 152 | 8 | storage-read nanoseconds |
| 160 | 8 | ordinary-host-to-pinned copy nanoseconds |
| 168 | 8 | H2D submission nanoseconds |
| 176 | 8 | H2D drain/synchronization nanoseconds |
| 184 | 8 | full-arena read-back nanoseconds |
| 192 | 32 | `CudaArenaVerificationEvidence.v1` digest |
| 224 | 32 | canonical software/runtime provenance SHA-256 |

Timing values come from the same monotonic clock and may be zero at clock
resolution; byte counts and scratch terms must validate exactly against the
plan and upload summary. The software/runtime provenance digest binds the
pinned Rust toolchain, CUDA toolkit/runtime, driver, native library hash, and
kernel ABI in canonical evidence JSON. The receipt field is:

```text
verification_evidence_sha256 = SHA256(
    "glmaxx.rank-load-verification-evidence.v1\0"
    || 256-byte RankLoadVerificationEvidence.v1
)
```

Any late reader, hash, sink, CUDA, byte-count, seal, or receipt failure drops
every quarantined arena. A prepared rank remains non-executable while another
rank is still staging.

## Four-rank adoption

Preparation and adoption are separate phases.

1. The coordinator collects exactly one prepared receipt from ranks 0–3.
2. It validates the common plan plus each rank's expected device, file,
   layout, byte counts, and nonzero owner allocation generation.
3. It derives `rank_set_receipt_sha256` as
   `SHA256("glmaxx.prepared-rank-set.v1\0" || rank0_prepared_sha256 || ... ||
   rank3_prepared_sha256)`.
4. It sends the identical `ADOPT(plan_sha256, rank_set_receipt_sha256)`
   command to all persistent rank threads.
5. Each rank rechecks its local receipt, moves its sealed arena into an
   executor-internal slot, and acknowledges the two hashes.
6. Only after four identical acknowledgments does startup enter
   `WEIGHTS_LOADED`.

Adoption does not open scheduling; the later startup gates still run. If an
adoption acknowledgment fails after another rank has moved its handle, the
worker generation becomes terminal and all four allocations are destroyed.
The process does not retry one rank or reuse an adopted subset.

This is process-atomic visibility rather than an impossible simultaneous
four-device instruction: no model step can observe the new generation unless
the coordinator has four matching adoption acknowledgments and later reaches
`HEALTHY`.

## Failure and fallback rules

The whole load fails on:

- a changed rank path or file descriptor identity;
- rank-set, manifest, tensor-catalog, profile, policy, ABI, or capability
  disagreement;
- a destination interval mismatch, overflow, overlap, short write, duplicate
  write, or unfilled interval;
- host allocation, CUDA allocation, copy, event, synchronization, or sealing
  failure;
- a rank receipt or adoption digest mismatch;
- a rank thread exit or timeout; or
- any attempt to expose a quarantined arena to execution.

There is no rank-local codec fallback, repack, smaller arena, verification
mode, retry, or device substitution. A coordinator may choose a different
predeclared profile only by starting a new process-wide load plan before any
arena allocation. That choice must remain within the reviewed quality and
capacity policy.

## Memory and performance accounting

The combined startup resource plan accounts for, per rank:

- exact weight and codec-metadata arenas including allocator alignment;
- CUDA module/context and graph-independent startup allocations;
- the load stream and events;
- the pinned host staging ring;
- reader control regions and payload-verification scratch; and
- temporary device verification workspace if the review requires it.

Only device-resident terms enter the HBM inequality; reader control memory and
the pinned ring are separate explicit host-memory terms with their own
process-wide cap. Weight load does not consume the serving escrow or KV floor.
The observed HBM high-water must fit the same rank independently; aggregate
free memory cannot rescue a failing rank.

Evidence reports the four rank timelines separately and the critical path.
Parallel rank loading may not hide storage contention: aggregate and per-rank
read throughput, host-copy throughput, PCIe throughput, wait time, and final
drain time are all retained.

## Required CPU/mock proof after review

Before a CUDA sink implementation, tests must cover:

- exact normative startup ordering and rejection of the old memory-after-load
  sequence;
- deterministic load-plan and four-rank receipt encodings;
- actual GLM-5.2 tensor counts, roles, codec memberships, and arena arithmetic;
- plain, NVFP4, EXL3, metadata, empty-auxiliary, and final-partial-chunk paths;
- late corruption on rank 3 after ranks 0–2 prepare;
- reader failure, sink failure, short/duplicate plane, bounds overflow,
  allocation failure, event failure, and final-drain failure on every rank;
- a prepared-receipt mismatch and an adoption failure after one prior
  acknowledgment;
- exactly-once abort/free behavior with no published handle;
- kernel ABI, capability, profile, policy, device, and memory-plan mismatch;
- fixed ring capacity and no per-chunk allocation;
- proof that no executor can receive a weight handle before four-rank
  adoption; and
- full evidence receipt serialization with no GPU or performance claim.

After that CPU proof, the next gate is a qualified CUDA upload sink and
small-checkpoint smoke. Full-checkpoint residency remains blocked by policy
fit, EXL3 device acceptance, quality gates, and measured capacity.
