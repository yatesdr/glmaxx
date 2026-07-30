# Direct-tier checksum authority CPU proof v1

Date: 2026-07-30

Status: CPU-proof candidate; adversarial review required

Depends on:

- machine-accepted `direct-tier-io-v1` design;
- the candidate direct-tier extent codec and restore state machine; and
- no GPU, NVMe, filesystem, `io_uring`, or performance evidence.

## Purpose

The restore state machine previously accepted a caller-supplied
`complete_hash(ticket, bool)`. That was sufficient to model an abstract hash
acknowledgement, but it was not a safe implementation boundary: any caller
could publish unverified bytes by passing `true`.

This candidate removes that boolean boundary. Only the restore authority can
construct a `DirectHashResult`, and it constructs one by running the real
direct-extent decoder over the exact generation-bound buffer. The decoder
verifies:

- SHA-256 of the complete 493- or 501-block physical extent;
- SHA-256 of every logical piece;
- exact piece offsets and lengths;
- mandatory zero padding; and
- target-only versus MTP capability structure.

An integrity failure quarantines the buffer and releases ticket, waiter,
tenant, physical-byte, and hash-job accounting. It cannot publish
`HOST_READY`.

## Bounded admission

`DirectRestoreConfig.maximum_hash_jobs` is nonzero and no larger than the
fixed buffer pool. A hash slot is reserved before a read descriptor, CQE, or
`READ_INFLIGHT` transition is exposed:

```text
BUFFER_RESERVED
  -> reserve checksum capacity
  -> allocate descriptor
  -> reserve original CQE
  -> READ_SUBMITTED / READ_INFLIGHT
```

If the checksum bound is full, `submit_read` returns `HashWait` while the
ticket remains `BUFFER_RESERVED`. No descriptor, CQE, buffer transition, or
additional hash charge occurs. This is the accepted "WAIT before work
starts" rule, represented without an unbounded channel.

The reserved slot remains charged through read submission, queued hashing,
and running hashing. It is released only after one of:

- successful checksum publication;
- integrity/read failure and quarantine; or
- a logically abandoned read has reaped every required CQE and discarded
  the buffer.

`active_hash_jobs` is re-derived from every ticket during invariant
validation and must never exceed the configured maximum.

## Generation-bound worker protocol

The authority exposes the following deterministic protocol:

```text
complete_original(EXACT)
  -> hash state QUEUED
next_hash_job()
  -> DirectHashJob { ticket, DirectBufferId { slot, generation } }
  -> hash state RUNNING
run_hash_job(job)
  -> decode and hash the exact current buffer
  -> private DirectHashResult { job, verified }
complete_hash(result)
  -> revalidate ticket, slot, generation, and RUNNING ownership
  -> HOST_READY or quarantine
```

`DirectHashJob` and `DirectHashResult` fields are private. A consumer may
inspect their identities, but cannot construct a success result through the
safe public API. Replayed results, wrong ticket/buffer generations, results
outside `DATA_READY`/`ABANDONED`, and results after ownership release fail
closed.

`read_destination_mut(ticket)` exposes only the exact physical-length slice
and only while that ticket owns `READ_SUBMITTED`, the original completion is
pending, cancellation is not pending, and checksum capacity is reserved.
This lets the CPU proof model bytes written by an eventual fixed-buffer read
without exposing the whole maximum-size allocation.

The queue order is ticket-ID ascending because the bounded ticket table is a
`BTreeMap`. This is deterministic evidence behavior, not a production
fairness or worker-thread claim.

## CPU proof

Run:

```text
cargo run --release --offline -p glm-cli --bin glmaxx -- \
  direct-tier-checksum-proof \
  /tmp/direct-tier-checksum-authority-proof-v1.json
```

The canonical fixture is
`fixtures/direct-tier-checksum-authority-proof-v1.json`, SHA-256:

```text
431dc6143dad33bd271e4905ade9bad2149e2cf3be998d68128048b4b17c1fa9
```

The proof fixes `maximum_hash_jobs=1`, reserves two buffers, submits the
first read, and proves that the second read receives `HashWait` with one
descriptor and one CQE still outstanding. It then proves QUEUED and RUNNING
counts, hashes a canonical zero extent successfully, rejects a replayed
result, drains both tickets to zero resources, corrupts one physical byte in
a separate restore, rejects its physical SHA, and leaves one inactive
quarantined buffer.

Focused unit tests additionally construct a wrong buffer generation inside
the private module and prove `HashBinding`, then prove that a completed or
released job cannot be reused.

Debug and release proof output must be byte-identical to the fixture.
`scripts/local-checks.sh` regenerates and compares it on every complete local
gate.

## Nonclaims and next work

This proof does not implement or qualify:

- checksum worker threads, an authority thread, work stealing, CPU affinity,
  NUMA placement, or single-pass SHA throughput;
- `io_uring`, registered buffers/files, `O_DIRECT`, SQEs, CQEs, async-cancel
  syscalls, fsync, short I/O, or storage faults;
- CUDA host registration, HBM copies, CUDA events, decode isolation, or cn4;
- durable write hashing, publication, recovery, relocation, or cleaning;
- DRAM-cache ownership transfer, eviction, or admission; or
- checkpoint, model, quality, capacity, latency, or throughput evidence.

After adversarial acceptance, the next implementation boundary is the
reviewed Linux feature/fault probe. Its real completion authority must call
this bounded protocol, replace synchronous `run_hash_job` calls with fixed
workers, retain the exact generation token across the worker channel, and
measure a fused one-pass physical/piece digest implementation before making
any throughput claim.
