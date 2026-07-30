# HBM↔DRAM sealed-page transfer v1

Date: 2026-07-30

Status: design candidate; adversarial review required before CPU/native/CUDA
implementation

GPU evidence: none

## Scope

This contract defines the target-only path for moving one complete sealed
GLM-5.2 KV page between its owner rank's HBM arenas and bounded host DRAM. It
also defines the pinned bridge used by direct-tier NVMe restore/publication.

It freezes:

- actual GLM-5.2 page geometry and pitched-copy address derivation;
- HBM, pinned DRAM, and direct-I/O bridge ownership;
- target/indexer/draft atomicity;
- CUDA stream, event, digest, and buffer-generation ordering;
- four-rank prepare/receipt/commit behavior;
- cancellation, failure, cleanup, and decode-isolation boundaries; and
- CPU and SM120 evidence required before K02/K03/K04 can advance.

It consumes, but does not accept, the pending `sm120-rank-executor-v1-r2`
`RankTierCommand.v1` design. If that review changes the command or ownership
model, this contract must be reconciled and re-pinned before implementation.

This design does not implement a transfer, modify the pending native ABI,
authorize cn4, or claim HBM/DRAM/NVMe functionality.

## Why a dedicated contract is required

The existing CPU residency model owns bytes but labels no real device memory.
The direct-tier design correctly requires a later CUDA boundary. The accepted
review specifically requires:

1. copy completion recorded on the owning stream;
2. residency publication after that event;
3. host-buffer reuse gated on the event rather than stream idleness; and
4. a fence from host checksum completion to H2D submission.

The current native executor copy ABI is insufficient for the efficient page
route. GLM-5.2 stores:

```text
target_kv[layer][local_page][token][record]
indexer_k[group][local_page][token][record]
draft_kv[layer=0][local_page][token][record]
draft_indexer_k[layer=0][local_page][token][record]
```

A page is therefore contiguous within one layer/group but strided between
layers/groups. A 1-D-only ABI needs 99 calls for target or 101 for MTP. The
exact target route is two pitched copies; MTP adds two one-row copies. The
native ABI must add a checked 2-D operation before this path can be performant
or called complete.

## Fixed geometry

The constants are:

```text
page tokens                  64
KV record bytes             368
indexer record bytes        132
target layers                78
target indexer groups        21
draft layers                  1
draft indexer groups          1

one KV page row          23,552 = 64 * 368
one indexer page row      8,448 = 64 * 132
target KV piece       1,837,056 = 78 * 23,552
target indexer piece    177,408 = 21 * 8,448
draft combined piece     32,000 = 23,552 + 8,448
```

The host representation is exactly the accepted direct extent:

```text
target KV       [        0, 1,837,056)
zero padding    [1,837,056, 1,839,104)
target indexer  [1,839,104, 2,016,512)
zero padding    [2,016,512, 2,019,328)
draft KV        [2,019,328, 2,042,880)
draft indexer   [2,042,880, 2,051,328)
zero padding    [2,051,328, 2,052,096)
```

Target-only uses the first 2,019,328 bytes. MTP uses 2,052,096 bytes. Every
padding byte is zero before a D2H submission and after any host copy. Padding
is never copied into HBM.

At the currently budgeted serving shape, each rank has 266,688 target slots
and, when MTP is enabled, 266,688 draft slots:

```text
pages_per_rank             4,167
target KV source pitch    98,141,184 = 4,167 * 23,552
indexer source pitch      35,202,816 = 4,167 * 8,448
```

The formulas are derived from the accepted memory plan at startup; the
displayed values are required for that profile, not magic fallback constants.
The native rank refuses a different page count or pitch after memory-plan
adoption.

For arena base `B`, group/layer `g`, local page `p`, page count `P`, and page
row bytes `R`, the first byte is:

```text
B + (g * P + p) * R
```

All multiplication/addition is checked in Rust before conversion to a native
span. The final row must remain inside the exact adopted arena generation.

## Four physical copy planes

One operation has two or four fixed planes:

| Ordinal | Capability | HBM arena | Host offset | Width | Height | HBM pitch | Host pitch |
|---:|---|---|---:|---:|---:|---:|---:|
| 0 | target/MTP | target KV | 0 | 23,552 | 78 | `P * 23,552` | 23,552 |
| 1 | target/MTP | target indexer | 1,839,104 | 8,448 | 21 | `P * 8,448` | 8,448 |
| 2 | MTP only | draft KV | 2,019,328 | 23,552 | 1 | `P * 23,552` | 23,552 |
| 3 | MTP only | draft indexer | 2,042,880 | 8,448 | 1 | `P * 8,448` | 8,448 |

H2D swaps source/destination and their pitches. Plane order is fixed and
hash-covered. Target operations have exactly two planes and zero slots 2–3.
MTP operations have exactly four. A target operation cannot populate an MTP
page, and an MTP operation cannot omit either draft plane.

Startup queries the actual SM120 maximum pitch and rejects the profile unless
every adopted pitch is legal. No implementation may silently fall back to 99
or 101 individual copies in a serving profile. A scalar-copy route is a
diagnostic control only and is reported separately.

## Host memory classes

Version one has two explicit host classes.

### CUDA-pinned DRAM cache arenas

Each rank owns two optional, preallocated slot classes:

```text
target slot bytes = 2,019,328
MTP slot bytes    = 2,052,096
slot alignment    = 4,096
```

The rank's configured slot counts and byte totals are fixed in the system
memory plan and in `pinned_host_total`. They are not growable after
`MEMORY_PLANNED`.

The pending executor equation must therefore add:

```text
tier_dram_cache
  = sum_over_ranks(
        target_dram_slots[r] * 2,019,328
      + mtp_dram_slots[r] * 2,052,096)
```

as its own term in `pinned_host_total`; it cannot hide inside tier staging or
free system RAM. Startup checks the configured pinned-host process cap after
this addition.

One complete 1,048,576-token chain contains exactly 16,384 sealed pages,
4,096 per owner rank. Its slot allocation is:

```text
target-only total       33,084,669,952 bytes = 30.8125 GiB
target-only per rank     8,271,167,488 bytes =  7.703125 GiB
MTP-capable total       33,621,540,864 bytes = 31.3125 GiB
MTP-capable per rank     8,405,385,216 bytes =  7.828125 GiB
```

Those are allocated physical host bytes, including canonical padding.
Logical tenant charges remain 2,014,464/2,046,464 bytes per page. A production
DRAM profile claiming one full-chain capacity must reserve at least the
corresponding four per-rank terms plus transfer rings and process headroom; it
cannot quote only the aggregate free RAM.

The underlying mappings:

- are regular anonymous host mappings, not Rust `Vec` reallocations;
- use `MADV_DONTFORK` and `MADV_DONTDUMP`;
- are NUMA-bound according to the pinned rank/PCIe topology;
- are registered once with CUDA by their owner rank;
- are not io_uring-registered; and
- are zeroed before first use and before cross-tenant reuse.

This choice makes HBM↔DRAM a direct DMA path without a host memcpy. The
profile's pinned DRAM capacity is bounded explicitly; it is never inferred
from free system memory. A future pageable high-capacity cache would require
a measured bridge-copy amendment and may not masquerade as this path.

Each slot has:

```text
DramSlotId.v1 {
    rank: u8,
    class: TARGET | MTP,
    slot: u32,
    generation: u64,
}
```

Generation zero is invalid. Reuse increments before visibility. Overflow
retires the slot. A slot is keyed by complete catalog content identity and
entry digest while resident. Raw host addresses never cross the rank-owner
boundary.

An MTP-class slot may retain a target-only record under the normal capability
lattice; a target-class slot can never retain MTP. A target-to-MTP DRAM
upgrade reserves a new MTP slot and copies/verifies the complete new extent
before atomically replacing residency. It never appends draft bytes to the
target slot. Pool counts and observed target/MTP mix enter capacity evidence.

### Dual-registered direct-I/O bridge

The direct-tier fixed-buffer pool is partitioned by owner rank. Each maximum
2,052,096-byte mapping is:

- 4,096-aligned;
- CUDA-registered by exactly one rank owner;
- io_uring-registered by the process-wide I/O authority;
- charged for both memlock obligations;
- `MADV_DONTFORK | MADV_DONTDUMP`; and
- identified by the accepted full slot/buffer generation.

The I/O authority selects the partition from page owner before submitting an
NVMe read. Another rank cannot copy from that buffer.

Startup order is mapping, NUMA policy, CUDA registration, then io_uring
registration. After zero outstanding native/CUDA/io_uring descriptors,
teardown is CUDA unregister on the owner rank, io_uring unregister on the I/O
authority, then unmap. Partial startup unwinds the acquired prefix in reverse.

Bridge buffers are staging, not DRAM cache capacity. Moving their verified
bytes into a DRAM cache slot requires a separate capacity transaction and
keeps the source generation owned until the host copy and destination hash
complete.

## HBM allocations

Target HBM allocations bind:

```text
owner rank
target local page ID
target allocation generation
target-indexer allocation generation
```

MTP adds:

```text
draft local page ID
draft allocation generation
draft-indexer allocation generation
```

Paired generations must match as specified by the page-attachment contract.
Local IDs and generations are resolved by the owner thread into the adopted
arena bases and pitches. Coordinator-provided device addresses are forbidden.

Restore reserves a fresh, unreachable HBM allocation before H2D. Publication
and demotion acquire an independent lease over an existing sealed allocation
before D2H. No allocation ID may be freed or reused until its transfer and
four-rank terminal transaction acknowledge release.

## Transfer identity

One canonical `TierTransfer.v1` contains:

```text
executor generation
scheduling epoch
command ID
operation ID
direction: H2D_RESTORE | D2H_DEMOTE | D2H_PUBLISH
capability: TARGET | MTP
owner rank
namespace
page key
durable revision
catalog epoch and entry SHA
HBM page IDs and allocation generations
host class, slot, and slot generation
four canonical plane descriptions
expected logical piece SHA-256 values
expected physical SHA-256 or zero for first publication
expected host row-digest-vector SHA-256 or zero for D2H
predecessor event ID/generation or zero
completion event ID/generation
absolute prepare/completion deadline
transfer SHA-256
```

For first publication, logical/physical hashes are outputs: the device and
host digests must agree, and those observed host digests become the candidate
durable record. For demotion or restore of an existing durable record, every
digest is an input bound to that exact catalog entry.

Operations are sorted by `(owner_rank, operation_id)`. Command identity and
all four-rank digests include unused zero slots. Rank-local completion timing
cannot change operation order or the page-table delta.

## Stream and event ownership

Every rank owner creates, before health:

- one nonblocking low-priority tier copy stream;
- one fixed transfer-completion event ring;
- one fixed device-digest status ring in pinned host memory; and
- immutable handles for the adopted four HBM arenas.

The copy stream priority and flags are part of the measured profile. Default
stream semantics are forbidden.

Each event slot has a u64 generation with zero invalid and overflow retirement.
One event generation belongs to one operation until the owner has:

1. observed terminal `cudaEventQuery`;
2. checked asynchronous CUDA status;
3. validated the digest status record;
4. emitted the rank receipt; and
5. received coordinator terminal acknowledgment.

Stream idle is never a substitute for the named event. A late event, status
record, command, cancellation, or receipt with a stale generation is fatal to
the executor generation.

The predecessor event is the exact last device writer of a D2H source. The
tier stream executes `cudaStreamWaitEvent` before the device digest or copy.
An H2D source becomes eligible only after host checksum verification has
completed and the I/O authority has transferred ownership to the rank. That
ownership handoff is the host memory fence; the rank never polls mutable
checksum-worker fields.

## Device/host integrity receipt

Copy completion alone is insufficient qualification. A target-only SM120
digest kernel computes one standard SHA-256 independently for each canonical
HBM row. Target has 78 KV plus 21 indexer row digests (99 total); MTP adds one
draft-KV and one draft-indexer digest (101 total). Plane order and increasing
group/layer order are fixed.

This avoids a serial 1.8-MiB device SHA chain while still detecting every copied
logical byte. The kernel reads the same pitched rows named by the copy
descriptor and writes one fixed 4,096-byte status slot containing the ordered
row digests, operation identity, allocation generations, count, status, and a
status SHA. Unused digest slots and remaining bytes are zero. Known-answer
inputs qualify every row against the Rust SHA-256 oracle.

Catalog piece SHA-256 and full physical SHA-256 remain host computations over
the canonical extent. The row vector is a transfer-integrity receipt, not a
replacement catalog digest.

For D2H:

1. wait for the predecessor event;
2. hash the immutable HBM source;
3. copy the tiny device digest status D2H;
4. issue the two/four pitched D2H data copies;
5. record the completion event;
6. after the event, hash the host rows, logical pieces, and full zero-padded
   extent;
7. require every device-row digest equal its host-row digest; and
8. for a durable demotion, also require the catalog digests.

For H2D:

1. require the bridge/DRAM slot is already host-hash verified;
2. issue the two/four pitched H2D copies;
3. hash the destination HBM rows;
4. copy the tiny digest status D2H;
5. record the completion event; and
6. after the event, require every device-row digest equal the already
   verified host-row digest vector.

The completion event is after the status copy, so successful query makes both
data and receipt visible. Host padding is checked but has no HBM counterpart.
Digest mismatch quarantines destination bytes and is never a cache miss.

The device SHA path is part of correctness in v1, not a benchmark-only debug
mode. Any later cheaper integrity primitive requires independent corruption
coverage and a reviewed amendment.

## Required native ABI amendment

The pending executor ABI must add a 16-byte-aligned v2 descriptor and owner-
thread function equivalent to:

```text
copy_2d_async(
    source checked arena span,
    destination checked arena span,
    source_pitch: u64,
    destination_pitch: u64,
    width_bytes: u64,
    height: u32,
    direction,
    stream,
    sequence)
```

It enqueues no implicit completion event. Rust issues all canonical planes,
the digest/status work, and then explicitly records the one operation event.
Width, height, pitches, last-row bounds, arena generations, direction, pinned
host role, and reserved fields are validated natively before the first CUDA
call.

The existing 112-byte 1-D copy descriptor remains checkpoint-load control
only. Repeating 99/101 calls or recording one completion event per plane is
not an accepted serving implementation.

The native ABI must also accept an already-created, plan-sized host mapping
for owner-thread CUDA registration and return an opaque checked
`HOST_PINNED` arena. The registration call verifies mapping address, length,
alignment, role, generation, and resource digest. This is required because
the current `arena_create` interface cannot prove Rust's NUMA placement,
`MADV_DONTFORK`, or shared io_uring registration. Arbitrary post-health host
registration remains forbidden.

The native module must additionally expose one fixed-capability tier-digest
entry point. It accepts arena IDs/local page IDs and the adopted geometry, not
arbitrary device pointers. Its descriptor/status byte layouts, kernel
resource counts, module hash, and `sm_120f` cubin identity require their own
ABI proof before launch.

## H2D restore transaction

The process-wide restore ticket first reserves:

- one catalog entry/epoch pin;
- one bridge or DRAM source slot;
- one fresh owner-rank target allocation and optional paired draft
  allocation;
- one event/status slot;
- global physical and tenant logical quota; and
- four-rank command/receipt capacity.

No H2D starts until source physical/piece/padding hashes pass and the complete
rank command has passed four-rank prepare.

Only the owner rank submits data work. Nonowner ranks validate the identical
operation list and proposed page-table delta and return an explicit no-data
prepared receipt. After owner event/digest success, all four ranks bind the
same destination attachment generations and post-apply table digests.

The coordinator publishes HBM residency and releases restore waiters only
after:

1. the owner transfer receipt is valid;
2. every nonowner receipt agrees on command/operation/delta identity;
3. the page-table delta commits on all four rank mirrors; and
4. the serving/cache transaction commits.

Before that point, model graphs cannot resolve the fresh local page ID.

An MTP restore installs target, target indexer, draft KV, and draft indexer in
one attachment transaction. A target waiter may share an MTP restore. No
target-only restore can satisfy an MTP waiter.

## D2H demotion transaction

A demotion is allowed only for a complete sealed HBM page with a valid
allocation-generation lease and no admitted graph that can write it.

The destination DRAM slot is reserved and zeroed before prepare. D2H follows
the device/host integrity sequence. A durable catalog record must match all
observed logical hashes; a nondurable sealed page uses the publication path
instead.

The DRAM entry becomes `DRAM_READY` only after host hashes and the four-rank
receipt agree. The page-table/residency transaction may then remove HBM
reachability and release the HBM allocation if no remaining HBM pin exists.
Failure preserves the old HBM mapping and quarantines the destination; it
never partially demotes target without indexer or draft.

If the page is already durably recoverable and no DRAM replica is desired, a
separate metadata-only HBM eviction may release it without D2H. That operation
still uses a four-rank page-table transaction but is not labeled transfer
bandwidth.

## D2H publication transaction

Publication starts from the accepted `SealTicket`/lease and copies into the
owner rank's direct-I/O output bridge. Request cancellation does not revoke
the lease.

After device/host digest agreement, ownership moves from the rank to the
single I/O authority as `HASHING_FOR_WRITE`/host-ready publication input. The
rank cannot reuse the bridge generation. The durable Begin/data/piece/Commit
order is governed by `direct-tier-durable-format-v1`.

A successful durable Commit and shared catalog publication allow:

- release of the publication lease while HBM remains resident;
- explicit copy into a separately reserved DRAM slot; or
- later HBM eviction while retaining durable identity.

Durability failure never invalidates the active sealed HBM source. It
write-poisons/degrades the tier according to the durable contract and
releases HBM only through a later safe residency transaction.

## DRAM and NVMe bridge paths

The legal composite paths are:

```text
HBM -> pinned DRAM slot
HBM -> output bridge -> NVMe
NVMe -> input bridge -> HBM
NVMe -> input bridge -> pinned DRAM slot
pinned DRAM slot -> HBM
pinned DRAM slot -> output bridge -> NVMe
```

Host-to-host moves are exact full physical extents with destination zeroing,
piece/physical hash verification, and independent source/destination
generations. They are CPU-copy time, not PCIe or NVMe time.

There is no direct HBM↔NVMe/GDS route in v1. Adding one cannot bypass catalog,
ticket, digest, quota, event, or four-rank transaction semantics.

## Four-rank coordination and overlap

Every tier command is selected once by the coordinator and delivered
identically to all ranks. Rank-local fallback, operation deletion, reordering,
or capability downgrade is fatal.

Host hashing, NVMe I/O, and filling a free bridge slot may overlap model
compute. Device transfer may overlap a graph only when the reviewed executor
proves:

- the source/destination HBM ranges are disjoint from every range that graph
  may write;
- a D2H source is immutable and has an exact predecessor event;
- an H2D destination is unreachable by that graph;
- status/event/host slots do not alias graph argument/completion rings; and
- the immutable collective schedule is unaffected.

A command that cannot prove disjointness waits rather than guessing. No
next-step argument or page-table H2D is thereby authorized before the
executor's current v1 release point.

Publication and restore work are bounded by per-rank stream/event/bridge
slots. Resident decode never waits synchronously on a transfer. Requests that
need a restore remain prefix/admission-pending; full queues return `WAIT`.

## Cancellation and abandonment

Cancellation before native submission releases reservations atomically.
After any CUDA work is submitted:

- the host/HBM/event/status generations remain owned until the named event;
- async transfer is not cancelled through unsafe stream destruction;
- the owner still validates device status and emits a terminal receipt;
- a cancelled restore publishes no request reference;
- a completed unreferenced page may enter cache only through normal capacity
  policy; and
- publication continues under its lease.

Generation timeout or owner-thread failure triggers the accepted whole-
executor generation replacement. Resources that cannot be proven quiescent
are leaked/quarantined until context destruction rather than reused.

## Failure classification

Engine/executor-generation fatal:

- arena/local-page/generation mismatch;
- wrong owner rank or wrong plane geometry;
- device/host piece digest disagreement;
- durable expected digest mismatch;
- nonzero padding;
- stale/duplicate event or status generation;
- CUDA asynchronous error or impossible event state;
- page-table delta/receipt divergence; or
- rank-local operation/fallback difference.

Tier/read-write degraded:

- io_uring/NVMe failure after host transfer;
- bridge registration invalidation;
- durability barrier uncertainty; or
- host memory mapping/NUMA policy loss after health.

Admission-local `WAIT` before mutation:

- no HBM/DRAM/bridge/event/status slot;
- tenant/global quota refusal;
- transfer queue saturation;
- deadline too close before prepare; or
- overlap cannot yet be proven safe.

CUDA copy or digest failure is never downgraded to a cache miss.

## Metrics and evidence split

Fixed-cardinality metrics include:

- operations/bytes by H2D restore, D2H demote, and D2H publish;
- target versus MTP capability;
- source/destination host class;
- prepare wait, predecessor wait, device hash, DMA submit, event wait, host
  hash, consensus, and total latency;
- logical and physical bytes;
- pitched-copy count and scalar-control copy count;
- event/status/bridge/DRAM/HBM slot high water;
- host/device row-digest mismatch, catalog-digest mismatch, and CUDA errors;
- cancellation phase and quarantined bytes;
- NUMA node and pinned profile ID, not page/request/tenant identifiers; and
- resident decode ITL/throughput isolation deltas.

Kernel/device-hash time, DMA time, host hash time, host copy time, NVMe time,
and framework/consensus time are reported separately. A composite route is
not called faster if its DRAM, prefix, capability, or digest posture differs.

## Required CPU/mock proof after design acceptance

The CPU/mock gate must cover:

1. exact 23,552/8,448 row arithmetic and every host extent boundary;
2. address derivation for page zero, page 4,166, every plane, and one-past
   failures;
3. exact 98,141,184/35,202,816 serving pitches and adopted-profile drift;
4. two-plane target and four-plane MTP round trips against a flat oracle;
5. mutation of every width, height, pitch, offset, arena, rank, page ID,
   capability, and generation;
6. target/MTP slot class allocation, zeroing, reuse, overflow retirement, and
   cross-tenant cleanup;
7. bridge rank partitioning and CUDA/io_uring registration/unwind order;
8. checksum fence before H2D and predecessor event before D2H;
9. event/status generations, late/duplicate receipts, and no reuse before
   coordinator terminal acknowledgment;
10. device-digest mock versus host digest for every piece and padding;
11. H2D prepare/owner receipt/nonowner receipt/four-rank commit at every
    failure point;
12. D2H demotion preserving HBM on every destination failure;
13. publication lease survival through cancellation and durable handoff;
14. last-waiter cancellation before submit and after every mocked CUDA phase;
15. target/MTP atomic attach, no target-to-MTP inversion, and shared waiter
    behavior;
16. all HBM/DRAM/bridge/event/status/quota/receipt saturation paths;
17. allowed versus refused graph overlap from exact span sets;
18. host-to-host bridge paths with source/destination generation mutation;
19. owner-thread/context failure with leak/quarantine rather than reuse; and
20. final zero descriptors, events, statuses, bridge slots, leases, waiters,
    quota, and runtime epoch references.

The mock must model pitched memory, asynchronous event visibility, and stale
generations. A synchronous byte copy alone is not an adequate proof.

## Required SM120 qualification

After CPU/mock and native ABI acceptance, authorized cn4 evidence must:

1. pin source, cubin, driver/runtime, firmware, clocks, power, topology, NUMA,
   memory plan, and commands;
2. prove the digest kernel on adversarial actual-shape pages;
3. compare two/four pitched copies against 99/101 scalar-copy controls;
4. cover target and MTP H2D/D2H at queue depths 1 through the configured
   maximum;
5. verify event ordering, cancellation, injected digest corruption, and
   asynchronous CUDA errors;
6. report cold and warm DRAM, bridge, and NVMe composites separately;
7. measure every PCIe/NUMA layout available on the four-GPU target;
8. run resident decode with no transfer, saturated restore, saturated
   publication, and mixed transfer traffic under a predeclared isolation
   tolerance;
9. prove bounded pinned/HBM/DRAM bytes and zero resource drift across a
   sustained multi-user run; and
10. retain raw per-operation timing/evidence outside Git with hashes in the
    results index.

No A6000 result is acceptance evidence for SM120.

## Gate and claim boundary

Required order:

```text
this design review
-> CPU/mock transfer proof
-> native 2-D copy and digest ABI review
-> SM120 actual-shape correctness
-> HBM/DRAM/NVMe composite fault proof
-> cold/warm prefix checkpoint smoke
-> matched decode-isolation benchmark
```

K02 remains open through native SM120 correctness. K03 remains open through
qualified NVMe composite and decode isolation. K04 remains open through real
online publication and restart reuse. This document alone advances none of
them.
