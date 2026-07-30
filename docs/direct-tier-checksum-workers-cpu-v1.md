# Direct-tier fixed checksum workers CPU proof v1

Date: 2026-07-30

Status: CPU-proof candidate; adversarial review required

Depends on:

- the machine-accepted `direct-tier-io-v1` design;
- the candidate direct-tier extent, restore-state, and checksum-authority CPU
  slices; and
- no GPU, NVMe, filesystem, `io_uring`, or performance evidence.

## Purpose

The checksum-authority candidate proved that only the restore authority can
construct a generation-bound verification result, but its proof called
`run_hash_job` synchronously. This candidate adds the fixed CPU worker
boundary required before real asynchronous reads can hand completed buffers
to checksum work.

The implementation has a fixed worker count and two bounded channels:

```text
authority                         fixed checksum workers
---------                         ----------------------
DATA_READY / ABANDONED
  -> capture ticket + buffer generation
  -> capture shared buffer handle
  -> capture current allocation address
  -> bounded command queue  ------> canonical extent decoder
                                     physical SHA-256
                                     piece SHA-256
                                     padding validation
                                     capability validation
  <------ bounded result queue ----- integrity + binding outcome
  -> revalidate ticket/generation/state
  -> HOST_READY or quarantine
```

Both channel capacities equal `maximum_hash_jobs`. The existing admission
rule reserves one hash job before read submission, so the number of tasks
that are queued, executing, or awaiting authority acknowledgement cannot
exceed either channel's capacity. The authority uses nonblocking dispatch and
poll operations; no unbounded queue or per-request worker is created.

Workers are named `glmaxx-direct-sha-N` and are created as one fixed group
before read admission. `worker_count` must be in
`1..=maximum_hash_jobs`. Starting a second group, stopping a group with live
hash reservations, restarting a retired group, and submitting reads after
retirement all fail closed.

## Exact-allocation hashing

Every `DirectBufferPool` slot owns one preallocated maximum-size,
4096-byte-aligned `DirectExtentBuffer`. A worker task contains:

- a private `DirectHashJob` with the ticket and complete
  `DirectBufferId { slot, generation }`;
- an `Arc` to the immutable extent record; and
- an `Arc`-backed read handle to that same preallocated buffer.

It does not contain a copied extent payload. The allocation is not replaced
when a slot is reserved, filled, verified, released, or reused.

Before dispatch, the authority reads the allocation address from the
generation-bound buffer handle. While holding the worker's read guard, the
worker checks both that address and the full buffer ID before decoding. An
address or generation mismatch is a pool-fatal `WorkerBinding` failure; it
cannot produce a successful result. Worker-produced results record that this
same-allocation check passed. The proof serializes only the boolean outcome,
not a process-specific address.

`copy_into_read_destination` is a bounded CPU fixture helper. It copies a
caller slice only while the exact ticket owns a submitted read and only
within `physical_length`. It replaces the earlier API that returned a mutable
buffer slice, so safe callers cannot retain mutable access while a worker
holds checksum ownership. This helper is not a zero-copy storage claim.

## Fault and shutdown behavior

The worker executes the canonical decoder inside a panic boundary.

- Digest, piece, padding, or capability failure returns an integrity-negative
  result. The authority quarantines that buffer and releases all associated
  accounting.
- A poisoned buffer lock fails the exact ticket and quarantines its buffer.
- A worker panic emits a failure completion and retires that worker. When the
  authority observes it, the entire checksum pool becomes failed and every
  reserved, queued, or running checksum ticket is failed and quarantined.
- Command or completion channel disconnection also fails the pool and all
  active checksum tickets.
- A failed pool rejects later dispatch and read submission. It cannot silently
  continue at reduced parallelism.

This whole-pool rule avoids rank- or request-local fallback after the fixed
execution posture changes. It also prevents an undispatched queued task from
leaking its pre-reserved hash capacity when a worker exits.

Normal shutdown first requires zero active hash jobs, closes the command
queue, and joins every worker. `Drop` also closes and joins the fixed group;
the result queue is sized to the maximum number of active jobs, so completed
workers cannot deadlock shutdown on a full result queue.

## Deterministic CPU proof

Run:

```text
cargo run --release --offline -p glm-cli --bin glmaxx -- \
  direct-tier-checksum-worker-proof \
  /tmp/direct-tier-checksum-workers-proof-v1.json
```

The canonical fixture is
`fixtures/direct-tier-checksum-workers-proof-v1.json`, SHA-256:

```text
6b07cc9497ff02c779fbe2243b0437f9dbfa52e12b6da18b86877fe44f6b715b
```

The proof:

1. starts two workers with two-slot command and completion queues before any
   ticket exists;
2. submits target-only and MTP extents, dispatches both before polling, and
   proves both are in `RUNNING`;
3. rejects the synchronous/manual execution path and rejects shutdown while
   work is live;
4. accepts both canonical extents irrespective of worker completion order,
   while proving both workers consumed the authority's exact allocations;
5. proves cancellation after `DATA_READY` waits for worker acknowledgement;
6. corrupts one real destination byte, rejects the extent, and quarantines
   its buffer;
7. joins the fixed workers, rejects restart and post-retirement read
   submission; and
8. finishes with zero tickets, waiters, active hash jobs, active buffers,
   descriptors, CQEs, and physical-byte charges.

The fixture sorts completed ticket IDs because fixed workers may finish in
either order. Debug and release output are byte-identical, and
`scripts/local-checks.sh` regenerates and compares the release artifact.

Focused unit tests additionally inject a worker panic while a second checksum
job remains queued. They prove both buffers are quarantined, all hash
capacity is released, new dispatch and polling are rejected, and shutdown
can still join the group.

## Nonclaims and next work

This proof does not implement or qualify:

- CPU affinity, NUMA placement, priority, work stealing, fused one-pass
  physical/piece hashing, or checksum throughput;
- registered or pinned host memory, fixed `io_uring` buffers/files, SQEs,
  CQEs, async cancel, short-I/O handling, fsync, or any filesystem device;
- CUDA host registration, HBM transfers, events, graphs, decode isolation,
  or cn4;
- durable write hashing, publication, recovery, relocation, or cleaning;
- DRAM-cache eviction/admission ownership; or
- checkpoint, model, quality, capacity, latency, or serving throughput.

After adversarial acceptance of the prerequisite CPU slices, the next
implementation boundary is the already-reviewed, off-by-default Linux
feature/fault probe. Its single issuer must fill these same fixed
generation-bound allocations, preserve the existing WAIT-before-work rule,
and hand exact completions to this fixed worker protocol. CPU placement and a
fused digest implementation must be measured before making a checksum
throughput or decode-isolation claim.
