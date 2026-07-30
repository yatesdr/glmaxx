# Direct-tier buffer and restore state CPU proof v1

Date: 2026-07-30

Status: CPU-proof candidate; adversarial review required

Depends on:

- accepted `direct-tier-io-v1` design;
- candidate `direct-tier-extent-cpu-proof-v1`; and
- no GPU, NVMe, filesystem, or `io_uring` evidence.

## Scope

This proof implements the next deterministic CPU slice of the accepted
direct-tier contract:

- a fixed pool of maximum-size, 4,096-aligned host buffers;
- `DirectBufferId { slot, generation }` identity with generation zero
  invalid, increment-before-visibility, stale-handle rejection, and permanent
  retirement at overflow;
- the accepted read and write buffer-state transitions, fail-to-quarantine
  behavior, zero-on-release, and no access through a FREE handle;
- a bounded descriptor table whose 64-bit completion token resolves the full
  buffer ID and full operation generation;
- original/async-cancel completion tracking in either order without early
  descriptor or buffer reuse;
- the exact CQ bound
  `original + async_cancel + fsync <= descriptor_capacity * 2`, independent
  of `IORING_FEAT_NODROP`;
- one bounded shared restore-ticket table with deterministic waiter order,
  target/MTP capability checks, same- and cross-tenant deduplication, one
  ticket-scoped physical charge, and one logical charge per waiter;
- cancellation before submit, in flight, after the original CQE while
  hashing, after hash verification, and with or without async-cancel support;
  and
- catalog record binding that requires replan before submission but retains
  the immutable original record after submission.

The implementation is split across:

- `crates/glm-cache/src/direct_state.rs`; and
- `crates/glm-cache/src/direct_restore.rs`.

The deterministic proof command is:

```text
cargo run --release --offline -p glm-cli --bin glmaxx -- \
  direct-tier-state-proof /tmp/direct-tier-state-proof-v1.json
```

The canonical fixture is `fixtures/direct-tier-state-proof-v1.json`.

## Buffer ownership

`DirectBufferPool` allocates every slot once at the maximum MTP physical
extent size, 2,052,096 bytes. Each underlying allocation exposes one exact
4,096-aligned slice. No unsafe code is used.

CPU work reserves a free slot directly into `HASHING_FOR_WRITE` or
`READ_QUEUED`, matching the accepted CPU boundary. Before a slot is visible:

1. its generation is checked and incremented;
2. overflow retires the slot permanently;
3. the complete byte range is zeroed; and
4. the generation-bearing ID is returned.

Transition to FREE zeroes the entire maximum extent. A FREE, FAILED,
QUARANTINED, or RETIRED buffer cannot expose bytes. A stale generation cannot
read, mutate, transition, free, fail, or quarantine the new occupant.

The normal read path is:

```text
READ_QUEUED -> READ_INFLIGHT -> HASHING_FOR_READ -> HOST_READY
```

The CPU proof may release verified `HOST_READY` bytes after deterministic
delivery. The separate `release_abandoned_read` operation allows
`READ_INFLIGHT -> FREE` only after the restore authority proves that every
original/cancel completion is reaped. It is not part of the generic
transition table.

Any operational state may enter FAILED, then only QUARANTINED. Quarantined
and retired slots never reenter the free pool.

## Descriptor and completion identity

One descriptor entry contains:

```text
DirectDescriptorBinding {
    buffer: DirectBufferId { slot: u32, generation: u64 },
    operation_generation: u64,
    operation: READ | WRITE | FSYNC,
}
```

The 64-bit completion token is:

```text
bits 63..32  descriptor-slot generation, nonzero u32
bits 31..1   descriptor slot, u31
bit  0       ORIGINAL=0 | ASYNC_CANCEL=1
```

The token is only an ABA-resistant lookup key. The bounded table resolves it
to the full u64 buffer generation and full u64 operation generation. An
async cancel shares the logical descriptor with its original operation but
has a distinct token. The entry is not reusable until both pending
completions are consumed, in either order. Duplicate, wrong-role, unknown,
missing, and stale completions fail closed. Descriptor generation overflow
permanently retires the slot.

## Restore ticket and accounting

A ticket is bound to:

- the complete validated `DirectExtentRecord`;
- catalog epoch and catalog-record SHA-256;
- one capability-bearing physical extent;
- one buffer generation after reservation;
- ordered waiter records; and
- original/cancel descriptor ownership.

Only an exact record/epoch/digest match can join existing work. An MTP record
can satisfy target-only and MTP waiters. A target record can satisfy only a
target waiter. This is intentionally stricter than content-key-only lookup:
an upgraded or relocated catalog record does not silently join a differently
bound ticket.

The physical byte reservation belongs to the ticket and is charged exactly
once when its first waiter creates it. Every waiter is charged the logical
bytes of its own required capability to its tenant. Removing the first
waiter cannot release the physical charge while another waiter remains.
Waiter delivery is request-ID ascending from a `BTreeMap`, independent of
insertion or hash-map iteration order.

All ticket, waiter-per-ticket, physical-byte, tenant-logical-byte, buffer,
descriptor, and CQ limits return WAIT-style errors before the requested work
is exposed. Tests verify that rejected admissions change no charge or
membership.

## Cancellation

The CPU oracle proves:

- last waiter in PLANNED: remove the ticket and physical reservation;
- last waiter in BUFFER_RESERVED: zero/free the unsubmitted buffer, then
  remove the ticket;
- nonlast waiter in any active state: remove only that waiter and logical
  charge;
- last waiter in READ_SUBMITTED: mark ABANDONED and optionally issue exactly
  one async cancel;
- no async-cancel support or CQ capacity: retain the physical operation until
  the original completion;
- original then cancel and cancel then original: retain the descriptor,
  buffer, and physical charge until both are reaped;
- cancellation after original CQE: retain the hashing buffer until the hash
  worker acknowledges;
- cancellation after verified hash: zero/free HOST_READY; and
- read or integrity failure: clear waiter charges, quarantine the buffer, and
  retain any descriptor/physical charge until its final CQE.

No response-handle lifetime owns physical resources.

## CQ arithmetic

`DirectCqTracker` requires at construction:

```text
cq_entries == descriptor_capacity * 2
```

Every submit checks:

```text
outstanding_original
+ outstanding_async_cancel
+ outstanding_fsync
<= cq_entries
```

Original and fsync counts are individually bounded by descriptor capacity.
A full CQ returns `CqWait`; it never overcommits. The exact same trace passes
with `nodrop_present=false` and `true`, proving that NODROP is observable
metadata rather than a correctness dependency.

This arithmetic type is a CPU contract. It does not submit an SQE or consume
a kernel CQE.

## Deterministic result

The canonical fixture SHA-256 is:

```text
58f19d6b506e969c91561938eb45a509ce820d936b9bb4d901c9028a5ca17c75
```

It records:

- buffer generations 1 then 2;
- original/cancel user-data values 4,294,967,296 and 4,294,967,297;
- deterministic waiter order 10, 20, 30;
- one 2,052,096-byte MTP physical reservation;
- exact tenant logical charges 4,060,928 and 2,014,464 bytes;
- both completion orders and logical abandonment passing;
- replan-before-submit and pinned-after-submit catalog decisions; and
- final zero tickets, waiters, active buffers, descriptors, CQEs, and
  physical bytes.

Debug and release output must be byte-identical to the fixture.

## Nonclaims

This candidate does not implement or qualify:

- `mmap`, `mlock`, `MADV_DONTFORK`, io_uring registration, CUDA host
  registration, or their required teardown ordering;
- an `io_uring` instance, SQE submission, kernel CQE, async-cancel syscall,
  registered file, `O_DIRECT` read/write, fsync, short-I/O, or device-loss
  behavior;
- checksum worker threads or a concurrent authority thread;
- a durable binary record, journal, catalog, catalog shard, checkpoint, or
  restart replay;
- DRAM cache admission/eviction or ownership transfer from HOST_READY;
- HBM copy, CUDA events, ranks, model execution, or cn4 evidence;
- W0/W1 scheduling, starvation policy, segment allocation, relocation, or
  cleaning;
- K03, K05, checkpoint smoke, serving readiness, model quality, capacity, or
  performance.

The cleaner remains blocked on the separately reviewed relocation
journal/checkpoint amendment. The next safe implementation gate is the
durable record/catalog/journal contract and a nonproduction Linux io_uring
feature probe; neither is implied by this proof.
