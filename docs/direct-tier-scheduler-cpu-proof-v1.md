# Direct-tier scheduler CPU proof v1

Date: 2026-07-30

Status: CPU implementation passed focused checks; adversarial review required

GPU claim: none

Storage claim: no syscall, filesystem, or device evidence

## Governing accepted design

The machine-accepted `docs/fable-direct-tier-io-v1.md` design review permits
the direct-format and pure CPU state-machine implementation to begin. It
requires:

- deterministic R0 resume reads, R1 admission reads, W0 publication writes,
  and W1 cleaner writes;
- read-reserved queue, buffer, descriptor, and completion capacity;
- an exact CQ bound independent of `IORING_FEAT_NODROP`;
- bounded advancement of already-accepted W0 work;
- an explicit policy for new-W0 admission under continuous reads; and
- W1 service only below read and publication low watermarks.

This candidate implements only that scheduling-policy slice in
`crates/glm-cache/src/direct_schedule.rs`. The retained extent, buffer,
descriptor, and restore-ticket CPU proofs remain separate.

## Fixed command and ordering surface

Every command carries:

```text
command_id
class
service_epoch
owner_id
page_ordinal
operation_ordinal
bytes
```

Within a class, the scheduler selects the lexicographically smallest:

```text
(service_epoch, owner_id, page_ordinal, operation_ordinal)
```

R0 is selected before R1. A duplicate command ID or duplicate in-class order
key is rejected before insertion. Zero IDs, zero epochs, zero owners, zero
bytes, queue overflow, byte overflow, and invalid class transitions fail
without partial insertion.

All five priority heaps and both membership tables fallibly reserve the
configured maximum command capacity in the constructor. The hard maximum is
65,536 queued commands, preventing an untrusted configuration from turning
construction into an unbounded allocation request. Successful insertion up
to the configured limit does not grow any collection. Hash-table iteration
never influences a decision; membership is the only hash-table operation
used by scheduling.

## Publication admission and service

New W0 work enters only through `offer_publication`. The caller must already
have settled capacity, catalog, tenant, and endurance eligibility; the
scheduler owns only deterministic read-pressure fairness and shared-resource
admission.

`PublicationAdmitted` is an admission receipt, not an execution decision. The
command remains in the accepted-W0 queue until a later `Service` decision.

An admission atomically subtracts:

```text
1 shared fixed buffer
1 shared descriptor
2 shared CQ entries
```

The two CQ entries cover the original completion and one possible
cancellation completion without relying on NODROP. Admission is refused
unless the resulting resource snapshot preserves every configured read
reserve. The caller must retain and refresh that sole-authority resource
snapshot through the eventual terminal publication path; this CPU policy
slice is not an independent buffer/descriptor ownership ledger.

The configuration requires:

```text
total_cq_entries = 2 * total_descriptors
read_reserved_cq_entries = 2 * read_reserved_descriptors
```

## Two independent starvation bounds

The scheduler has distinct immutable bounds:

```text
maximum_read_bytes_before_publication_admission
maximum_read_bytes_before_publication_service
```

While eligible W0 work waits above the normal read high watermark, serviced
R0/R1 bytes accrue toward the admission bound. At the first shared-capacity
opportunity at or before the next read would exceed the bound, exactly one
candidate receives a lease.

After admission, its service counter starts at zero. The accepted W0 command
is selected at or before the next read would exceed the independent service
bound. Therefore variable-size reads cannot overshoot either available
publication opportunity by one full extent.

If all shared resources are occupied and only the read reserves remain, the
eligible candidate remains queued and read work may continue. The byte
counter stays saturated at its bound. As soon as a shared slot is visible,
W0 admission precedes another read. W0 never consumes the read reserve to
manufacture a starvation guarantee.

## Cleaner isolation

W1 has no starvation guarantee. It runs only when:

```text
queued_read_bytes <= read_low_watermark_bytes
publication_candidates + admitted_publications
    <= publication_low_watermark_commands
```

R0, R1, or eligible/accepted W0 pressure prevents cleaner selection. This
module does not yet reserve W1 resources; that belongs to the single
io_uring authority.

## Focused proof

The focused test set contains 16 tests and passes:

```text
cargo test --offline -p glm-cache direct_schedule::tests -- --nocapture
cargo clippy --offline -p glm-cache --all-targets -- -D warnings
```

It covers:

- invalid scheduler geometry, allocation bound, and command values;
- exact R0/R1 priority and complete in-class order;
- duplicate ID/order and bounded-queue rejection;
- atomic u64 queued-byte overflow rejection;
- no collection-capacity growth after construction;
- normal W0 admission below the high watermark;
- continuous-read W0 admission at the exact byte bound;
- continuous-read accepted-W0 service at its separate exact byte bound;
- variable-size reads that would otherwise overshoot either bound;
- every `(current, next)` byte pair through a 300-byte boundary plus u64
  overflow;
- all 1,377 buffer/descriptor/CQ free-resource combinations around the
  configured reserves;
- inability to admit two W0 leases from one shared slot;
- W1 suppression above either low watermark;
- invalid external resource snapshots; and
- final zero queued commands and counters.

The complete `glm-cache` suite passes 113 tests and workspace Clippy passes
with warnings denied on these bytes.

`scripts/local-checks.sh` then passes with:

- 394 Rust tests and zero failures;
- workspace formatting and Clippy with warnings denied;
- CUDA FFI host checks;
- deterministic format/cache/engine/serving fixtures;
- review provenance for all 108 then-present handoffs, with 8/90 configured
  results accepted and zero withheld;
- the external tokenizer proof skipped because its configured directory was
  absent; and
- CUDA compilation skipped because this host has no `nvcc`.

## Acceptance boundary

Acceptance covers only the deterministic bounded CPU scheduling policy and
the named tests.

It does not accept or implement:

- an `io_uring` instance, SQE, CQE, registered file, registered buffer,
  `O_DIRECT`, fsync, async cancel, eventfd, or storage fault;
- the authoritative production resource ledger or terminal W0 resource
  replenishment;
- command cancellation after scheduling;
- the durable segment/journal/catalog/checkpoint codec or recovery;
- online prefix publication or cleaning;
- CUDA registration, HBM transfer, KV reconstruction, or attention;
- target storage behavior, decode isolation, capacity, latency, throughput,
  or production health; or
- cn4 access.

The direct-tier durable-format review, codec/recovery proof, Linux io_uring
feature/fault probe, target-storage qualification, and matched decode
isolation remain later gates.
