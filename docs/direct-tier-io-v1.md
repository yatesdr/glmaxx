# Direct DRAM/NVMe tier I/O v1

Date: 2026-07-29

Status: design candidate; adversarial review required before CPU
implementation

GPU evidence: none

## Scope

This contract defines the bounded host I/O path for full sealed GLM-5.2 KV
pages between the DRAM staging/cache tier and NVMe. It covers:

- direct-I/O record layout and aligned buffer ownership;
- one live catalog and one process-wide I/O authority;
- asynchronous restore, publication, cancellation, and deduplication;
- read/write scheduling that protects resident decode;
- durability, restart, capacity, segment cleaning, and error behavior; and
- CPU, fault, and measured-isolation gates.

It complements `online-prefix-publication-v1`. It does not qualify HBM↔DRAM
CUDA copies, model execution, or the NVMe hardware on cn4.

## Current blocking gaps

The existing CPU store and restore service prove basic crash visibility, but
are not a production tier path:

1. `FileTierStore` uses seek plus blocking `read_exact`/`write_all`.
2. each restore thread opens a private store and catalog snapshot.
3. each rank has only one blocking restore worker.
4. restore allocates one `Vec` per logical piece and hashes synchronously.
5. cancellation drops a response handle but does not cancel or safely abandon
   the underlying read.
6. a second request for a restoring key receives `Busy` instead of joining
   one physical transfer.
7. current per-piece lengths are not direct-I/O multiples even though their
   starting offsets are aligned.
8. the residency simulation labels host-owned `Vec` bytes HBM without a
   pinned-buffer or CUDA-event boundary.
9. there is no device capacity, queue-depth, bandwidth, endurance, or
   compaction policy.
10. publication syncs and restore reads have no explicit scheduling or
    decode-isolation rule.

The current path remains a CPU oracle. It cannot satisfy K03 or a live
HBM/DRAM/NVMe claim.

## Production posture

Version one is Linux-only and uses:

- an `io_uring` single-issuer service;
- `O_DIRECT | O_CLOEXEC` immutable data segments;
- registered aligned host buffers;
- registered data and journal descriptors where supported;
- no `SQPOLL` privilege requirement;
- one shared immutable catalog; and
- fixed-capacity command, completion, checksum, and catalog queues.

Startup probes the exact filesystem, kernel, mount, and device. Production
tier health is refused unless the qualified path supports the required direct
reads/writes, durability barrier, registered-buffer semantics, and completion
behavior.

The retained blocking store can generate fixtures and act as a matched CPU
control. It is never a silent production fallback. A fallback decision is
process-wide and made before healthy startup, not rank-local or per request.

## Exact direct-I/O extent

Logical pieces retain their existing byte order:

```text
target KV       1,837,056 bytes
target indexer    177,408 bytes
draft sidecar      32,000 bytes
```

One canonical physical extent starts each logical piece at 4,096-byte
alignment, fills every gap/tail byte with zero, and rounds the extent end to
4,096 bytes:

```text
target KV       [        0, 1,837,056)
zero padding    [1,837,056, 1,839,104)
target indexer  [1,839,104, 2,016,512)
zero padding    [2,016,512, 2,019,328)
draft sidecar   [2,019,328, 2,051,328)  MTP only
zero padding    [2,051,328, 2,052,096)  MTP only
```

Therefore:

```text
target-only logical bytes  2,014,464
target-only physical bytes 2,019,328 = 493 * 4,096
MTP logical bytes          2,046,464
MTP physical bytes         2,052,096 = 501 * 4,096
```

One `READ_FIXED` or `WRITE_FIXED` transfers the complete physical extent.
The file offset, userspace address, and length are all 4,096-byte aligned.
Piece SHA-256 values cover logical bytes only. A physical-extent digest covers
the complete padded extent. Restore verifies every padding byte is zero.

Capacity, endurance, and device bandwidth use physical bytes. HBM/DRAM
payload and quality accounting use logical bytes. Results retain both.

The durable metadata version adds:

```text
segment_id
physical_offset
physical_length
physical_sha256
piece offsets, lengths, and SHA-256 values
```

Target-only and MTP extents are immutable. An MTP upgrade writes a complete
new 501-block extent; it never appends a sidecar to the old extent.

## Buffer identity and lifecycle

The service preallocates `B` maximum-size 2,052,096-byte buffers aligned to
4,096. Production buffers are both io_uring-registered and CUDA-pinned once
the HBM transfer gate is integrated. CPU proof uses equivalently aligned,
nonpinned storage.

Every slot has:

```text
TierBufferId.v1 {
    slot: u32
    generation: u64
}
```

Generation zero is invalid. Reuse increments generation before a new
descriptor becomes visible. Overflow permanently retires that slot.

The buffer state machine is:

```text
FREE
  -> FILLING_FROM_HBM
  -> HASHING_FOR_WRITE
  -> WRITE_QUEUED
  -> WRITE_INFLIGHT
  -> FREE

FREE
  -> READ_QUEUED
  -> READ_INFLIGHT
  -> HASHING_FOR_READ
  -> HOST_READY
  -> COPYING_TO_HBM
  -> FREE

any non-FREE -> FAILED -> QUARANTINED
```

CPU-only work begins at `HASHING_FOR_WRITE` or `READ_QUEUED`. CUDA events
will own the adjacent HBM states after separate qualification.

An SQE user-data word resolves to a full buffer generation plus operation
generation in a bounded descriptor table. A late completion, async-cancel
completion, hash result, or CUDA event cannot release a reused slot.

Dropping a request handle never frees an in-flight buffer. Only the I/O
authority advances it after the terminal CQE and every dependent hash/copy
acknowledgment.

## One I/O authority

One process-wide `TierIoService.v1` owns:

- the io_uring;
- data segment and journal descriptors;
- physical offset/segment allocation;
- registered buffers;
- restore and publication tickets;
- shared catalog publication;
- durability state;
- segment cleaner state; and
- all queue-depth and byte counters.

Ranks and serving threads submit canonical commands to bounded queues. They
do not call filesystem APIs, allocate buffers, choose offsets, publish
catalog entries, or poll CQEs.

The service thread never performs SHA-256 over a 2 MiB record. A fixed
checksum worker pool owns hashing and padding validation. Hash jobs retain the
buffer generation and return through a bounded queue. The I/O service remains
the only state-transition authority.

All rank-facing restore plans and publication tickets are globally ordered
and digest-bound before submission. Rank-local pressure cannot select a
different storage route.

## Required io_uring features

Startup proves, on the actual store path:

- `IORING_OP_READ_FIXED`;
- `IORING_OP_WRITE_FIXED`;
- `IORING_OP_FSYNC` with data-only and full metadata barriers as required;
- registered buffers and data files;
- single-issuer operation;
- CQ sizing that cannot silently drop completions;
- `O_DIRECT` alignment and short-I/O behavior; and
- asynchronous cancellation or the defined logical-abandon path.

Optional cooperative task-run or submit-all features may be enabled only
under a pinned feature probe and matched tests. `SQPOLL`, fixed CPU affinity,
filesystem polling, GDS, and device-specific I/O priority are not required
for correctness.

If async cancel is unsupported or races with completion, logical abandonment
remains correct: the buffer and descriptor stay owned until the original CQE.

## Restore ticket and deduplication

Restore identity is:

```text
RestoreTicketKey.v1 {
    namespace
    page_key
    durable_revision
    required_capability: TARGET | MTP
}
```

An MTP ticket may satisfy target-only waiters. A target-only extent cannot
satisfy an MTP waiter.

The ticket contains:

```text
catalog epoch and record digest
segment/offset/length
buffer generation
physical and logical byte charges
bounded waiter slots
state and operation generation
```

The first waiter owns the global physical I/O reservation. Every waiter owns
its tenant logical quota charge. A concurrent compatible waiter joins without
another read. Waiter ordering is request ID ascending after coordinator
admission order has been frozen; map iteration order is irrelevant.

Restore states are:

```text
PLANNED
  -> BUFFER_RESERVED
  -> READ_SUBMITTED
  -> DATA_READY
  -> HASH_VERIFIED
  -> HBM_COPY_SUBMITTED
  -> RESIDENT
  -> RELEASED

PLANNED -> WAIT
any active state -> ABANDONED | FAILED
```

No read begins before quota, catalog epoch, destination-page, and buffer
reservations succeed. The eventual HBM copy publishes one shared physical
page plus N logical references, not N HBM copies.

Catalog revision or capability change before submission invalidates and
replans the ticket. Once submitted, the immutable catalog record and segment
remain pinned through completion even if a newer MTP revision becomes
visible.

## Cancellation

Cancelling one waiter removes only that waiter and its logical charges.

If waiters remain, physical work continues. If the last waiter disappears:

- a queued read releases its buffer without submission;
- an in-flight read is marked abandoned;
- the service may issue one async cancel;
- neither cancel success nor failure permits early buffer reuse; and
- the terminal read/cancel CQE pair drives the buffer to free.

If data has already verified, the page may enter an unpinned DRAM cache only
under the normal capacity policy; cancellation alone cannot create
unaccounted cache residency.

Request timeout, disconnect, shutdown, quota rollback, and fatal serving
cleanup all use the same idempotent waiter removal.

## DRAM tier ownership

A reusable DRAM page is one verified complete logical record in a registered
buffer or a separate bounded pinned DRAM arena. It is identified by content
key and durable revision, never an HBM local page ID.

DRAM states and bytes are real host allocations:

- `DRAM_READY` means all required pieces and hashes are present;
- `DRAM_PINNED` means at least one restore/HBM copy references it;
- eviction can discard a replica only if the catalog proves an equal or newer
  durable NVMe revision; and
- process restart discards every DRAM state and rebuilds from the durable
  catalog.

The online-publication staging pool is not counted as reusable DRAM cache.
Ownership may transfer from a completed publication buffer into DRAM cache
only through an explicit capacity transaction.

An HBM demotion whose page is already durable needs only D2H if a DRAM replica
is desired; otherwise it can drop HBM reachability and retain NVMe identity.
A newly sealed page cannot be dropped until publication durability or another
qualified replica is acknowledged.

Partial private tail pages are not NVMe records in v1. At most one per active
sequence remains in HBM or bounded DRAM until it becomes a full sealed page.
A later session-tail spill format requires its own review.

## I/O scheduling classes

Commands have fixed classes:

| Class | Work |
|---|---|
| R0 | resume a suspended request selected for immediate service |
| R1 | admission/prefix restore |
| W0 | accepted publication lease and durability journal |
| W1 | cleaner relocation and catalog checkpoint |

The scheduler reserves SQ and buffer capacity for reads. W1 never consumes
R0/R1 reserves. W0 accepted leases have a bounded completion path but new W0
leases stop before acquisition when read pressure crosses the configured
high watermark.

Within a class, ordering is:

```text
coordinator service epoch
request or ticket ID
page ordinal
piece/operation ordinal
```

No rank or completion timing changes it.

R0 has the lowest queue-latency target, but writes cannot starve indefinitely.
The immutable weighted service table specifies a maximum consecutive read
byte budget before one already-accepted W0 durability operation advances.
Cleaner W1 runs only below both read and publication low watermarks.

An fsync already issued cannot be preempted. Decode isolation therefore comes
from keeping decode-resident pages off the I/O path, bounded write issue,
reserved read/buffer capacity, and measured device behavior—not from claiming
an impossible NVMe preemption guarantee.

The I/O service never blocks the scheduler or rank worker. Full queues return
`WAIT` before work starts. A resident decode step has no dependency on tier
I/O completion.

## Publication durability

Publication retains the online-publication ordering:

1. reserve revision, extent, capacity, endurance bytes, and buffer;
2. append and `fdatasync` the Begin journal record;
3. issue one full-extent direct write and require exact byte completion;
4. `fdatasync` the data segment;
5. append and `fdatasync` each required durable-piece event;
6. append and `fdatasync` Publish;
7. atomically publish the new catalog epoch;
8. register residency/prefix visibility; and
9. release the publication lease and buffer.

Version one does not weaken the per-event durability barriers. A later
group-commit optimization requires crash-matrix evidence and a contract
amendment.

Zero padding is materialized and included in the physical digest before the
write. Short write, padding mutation, wrong extent, wrong buffer generation,
or post-write digest disagreement fails the publication and exposes no
catalog entry.

`O_DIRECT` does not replace the data/journal syncs. Completion means bytes
were accepted by the kernel/device path; only the required durability
barriers permit publication.

## Segments and bounded capacity

Data is stored in immutable 4,096-aligned segments with:

- monotonically increasing nonzero segment ID;
- configured fixed maximum segment bytes;
- no record crossing a segment boundary;
- one active append segment;
- sealed read-only segments; and
- physical live/garbage byte counters.

The catalog names segment ID and extent. Segment capacity, total capacity,
catalog entries, and rolling physical write bytes are checked before a
publication lease.

Superseded revisions and unreachable pages become garbage only after no
catalog snapshot, restore ticket, DRAM replica, or publication transaction
can reference them.

Cleaner relocation is:

1. select a sealed segment by deterministic garbage ratio, then segment ID;
2. pin its catalog epoch;
3. copy still-live complete extents through registered buffers;
4. verify logical pieces, padding, and physical digest;
5. sync destination data;
6. append and sync relocation metadata;
7. publish one new catalog epoch;
8. wait for all readers of the old epoch; and
9. unlink the old segment and sync the directory.

A crash at any point exposes either the old catalog/segments or the fully
durable new mapping. It never combines an old extent with new metadata.

Cleaner bytes count against endurance and W1 bandwidth. If capacity cannot
admit relocation safely, new writes stop and the store becomes read-only;
published reads continue. The service never deletes a live extent merely to
make progress.

The exact relocation journal encoding and startup recovery enter the durable
format version reviewed with this contract.

## Catalog visibility

The process exposes one immutable `CatalogSnapshot` identified by a
monotonically increasing epoch and digest. Restore planning retains an epoch
reference. Publishers and cleaners are the only writers.

Updates use copy-on-write bounded catalog shards so publishing one page does
not clone a 16,384-page or multi-tenant estate. The top-level shard table and
changed shard are replaced atomically after durability.

All restore workers consult this shared snapshot. No worker opens a private
published map. Reader epoch references protect segment extents through I/O
completion and checksum validation.

Startup replays journal/catalog checkpoints, validates parent-before-child
prefix metadata, rebuilds snapshots, classifies incomplete transactions as
orphans, and opens no service health until every referenced segment/extent is
present, aligned, nonoverlapping, and within file bounds.

## Failure classes

Engine-fatal integrity failures:

- content key, namespace, revision, capability, owner, or catalog digest
  mismatch;
- piece or physical SHA-256 mismatch;
- nonzero padding;
- overlapping/out-of-range/misaligned extent;
- buffer/operation generation ABA;
- impossible CQE, duplicate completion, or accounting underflow;
- published parent/child contradiction; or
- catalog pointing to missing/truncated durable data.

Tier-degraded operational failures:

- NVMe read `EIO`, device disappearance, or filesystem unavailability;
- io_uring service/thread failure;
- repeated timeout without completion;
- registered-file/buffer invalidation; or
- durability barrier failure.

A tier-degraded transition is global. Existing HBM/DRAM-resident requests may
continue if they need no failed tier work, but the service cannot report full
production health or admit a request whose context depends on NVMe. There is
no rank-local fallback.

Write-local fail-closed outcomes before durability:

- queue/buffer/capacity/endurance saturation before lease;
- `ENOSPC`;
- publication or cleaner write failure; and
- insufficient safe relocation space.

These stop new writes and preserve already published readable records.
Integrity failures are not downgraded to cache misses.

## Shutdown

Graceful shutdown:

1. reject new restore/publication/cleaner commands;
2. cancel or abandon unsubmitted work;
3. allow in-flight reads needed by active requests until the grace deadline;
4. complete or safely fail accepted publication durability transactions;
5. submit logical/async cancellation for remaining operations;
6. reap every original and cancel CQE;
7. release catalog epochs, buffers, waiters, and quota charges;
8. sync journal/catalog state;
9. unregister buffers/files; and
10. close the ring and prove zero outstanding descriptors.

Forced process termination relies on journal recovery, never on destructors
making in-flight data durable.

## Observability

Fixed-cardinality metrics include:

- commands, waits, cancellations, dedup joins, and completions by class;
- logical and physical bytes by read/write/cleaner;
- queue, SQ, CQ, buffer, checksum-worker, and waiter high water;
- submit, device, checksum, durability, HBM-copy, and end-to-end latency;
- direct-I/O short operations and async-cancel outcomes;
- DRAM ready/pinned bytes and entries;
- segment live/garbage/free bytes and cleaner write amplification;
- catalog epoch, entries, shards, and oldest reader age;
- device read/write bandwidth and latency;
- publication endurance budget;
- tier degraded/read-only transitions; and
- checksum, padding, generation, alignment, and recovery failures.

Page key, namespace, prompt hash, request ID, tenant ID, buffer generation,
segment path, and raw error strings are forbidden labels.

Cold NVMe, warm DRAM, and warm HBM restores are separate result rows. Linux
page-cache warmth is not called a warm NVMe cache under `O_DIRECT`.

## Required CPU and fault proof

Before any HBM integration, tests cover:

1. exact target/MTP logical and physical layout plus zero padding;
2. address, offset, and length alignment rejection;
3. registered buffer allocation/reuse/generation exhaustion;
4. target-only, MTP, and target-to-MTP upgrade;
5. one physical restore with same-tenant and cross-tenant waiters;
6. MTP restore satisfying target waiter but not the reverse;
7. cancellation before submit, in flight, after CQE, during hash, and as last
   versus nonlast waiter;
8. original/cancel CQEs in every order with no early reuse;
9. catalog revision change before versus after submission;
10. queue, SQ, CQ, buffer, checksum, waiter, capacity, and endurance limits;
11. short read/write, `EINTR`, `EAGAIN`, `EIO`, `ENOSPC`, and device loss;
12. bit flip in every piece and nonzero byte in every padding range;
13. crash before/after each journal/data/relocation sync and catalog publish;
14. 16,384-page prefix catalog restart and lookup;
15. live catalog visibility during concurrent reads and publication;
16. segment rollover at every record boundary;
17. cleaner with old-epoch readers, concurrent upgrade, and crash at every
    relocation stage;
18. safe read-only transition when relocation space is exhausted;
19. DRAM admission, pin, hit, eviction, and restart loss;
20. partial private tail refusal;
21. deterministic class/order selection across repeated runs;
22. no write starvation under continuous reads and no cleaner interference
    above watermarks;
23. shutdown with queued, reading, hashing, host-ready, writing, syncing, and
    cleaning operations;
24. final zero buffers, SQEs, CQEs, waiters, epochs, permits, and leases after
    every success/failure schedule; and
25. retained blocking-store cross-read of every direct-format fixture or an
    explicit fail-closed migration boundary.

## Required performance and isolation evidence

CPU/device qualification on the actual NVMe path reports:

- single and queued-depth physical read/write bandwidth for 493- and
  501-block extents;
- p50/p95/p99 submit-to-CQE and checksum latency;
- registered-buffer and direct-I/O CPU cost;
- cold restore throughput for target-only and MTP pages;
- warm DRAM promotion throughput;
- publication throughput including every required sync;
- cleaner useful and physical bandwidth/write amplification;
- mixed R0/R1/W0/W1 scheduling at configured watermarks;
- CPU utilization, RSS, pinned bytes, context switches, and queue highs; and
- resident-decode matched control with no I/O versus saturated restore,
  publication, and cleaner traffic.

Decode isolation passes only under a predeclared ITL/throughput regression
tolerance with identical model, context, batch, clocks, power, HBM posture,
and graph route. The exclusive ledger reports NVMe, checksum, host scheduling,
CUDA copy, and model time separately.

No result from another NVMe device, buffered page cache, tmpfs, or synthetic
small record qualifies cn4 production storage.

## Gate order and dependencies

1. adversarial review of this contract with online publication, quota,
   page-transaction, observability, and benchmark contracts;
2. direct-format CPU codec and blocking cross-reader;
3. io_uring feature/alignment/fault proof on a nonproduction filesystem;
4. shared catalog, dedup ticket, cancellation, and segment cleaner proof;
5. measured DRAM/NVMe path on the target storage under explicit operator
   authorization;
6. separately reviewed HBM↔DRAM CUDA descriptors and events;
7. cold/warm prefix replay and suspend/resume through a checkpoint;
8. sustained cache-thrash and decode-isolation qualification.

Implementation changes required after acceptance include:

- durable record/journal/catalog version;
- shared store ownership instead of four private `FileTierStore` maps;
- generation-bearing registered buffers and restore tickets;
- resource-ledger I/O/pin charges;
- residency states backed by real DRAM/HBM allocations;
- online-publication staging transfer;
- segment cleaner and restart recovery; and
- tier observability.

No part of this document authorizes cn4 access, a GPU launch, destructive
cache migration, or a production tier claim.
