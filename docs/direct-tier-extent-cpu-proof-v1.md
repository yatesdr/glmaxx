# Direct-tier extent CPU proof v1

Date: 2026-07-30

Status: CPU-proof candidate; adversarial review required

GPU evidence: none

## Scope

This proof implements only gate 2's canonical physical-extent codec from
`direct-tier-io-v1`. It establishes the byte layout that a later blocking
cross-reader and `io_uring` service must consume:

- target-only and MTP logical pieces occupy the exact accepted offsets;
- each physical extent and its userspace buffer are 4,096-byte aligned;
- every gap and tail byte is zero;
- one SHA-256 covers the complete padded extent;
- one SHA-256 per piece covers logical bytes only;
- decode validates metadata, alignment, physical digest, padding, and piece
  digests before returning borrowed logical views; and
- the retained blocking-store record is rejected with
  `MigrationRequired`, because it has no segment identity or complete
  physical-extent digest.

The implementation is in `crates/glm-cache/src/direct.rs`. The deterministic
CLI proof is:

```text
cargo run --release --offline -p glm-cli --bin glmaxx -- \
  direct-tier-proof /tmp/direct-tier-extent-proof-v1.json
```

Its canonical fixture is
`fixtures/direct-tier-extent-proof-v1.json`.

## Exact arithmetic

The implementation reuses the existing logical page-length constants from
`glm-cache::tier` and fixes only the accepted physical offsets:

```text
target KV       [        0, 1,837,056)
padding         [1,837,056, 1,839,104)
target indexer  [1,839,104, 2,016,512)
padding         [2,016,512, 2,019,328)
draft sidecar   [2,019,328, 2,051,328)  MTP only
padding         [2,051,328, 2,052,096)  MTP only
```

The resulting sizes are:

| Capability | Logical bytes | Physical bytes | 4,096-byte blocks |
|---|---:|---:|---:|
| target | 2,014,464 | 2,019,328 | 493 |
| MTP | 2,046,464 | 2,052,096 | 501 |

The CPU allocator overallocates a Rust `Vec<u8>` by at most 4,095 bytes and
exposes one aligned subslice of the exact physical length. It uses no unsafe
code. `DirectExtentBuffer` is intentionally not `Clone`: copying its private
allocation and retaining the old alignment displacement would not preserve
the alignment invariant.

## Fail-closed behavior

`DirectExtentRecord::validate` rejects:

- the wrong format version;
- zero namespace, page key, revision, or segment identity;
- the wrong physical length or a missing physical digest;
- a misaligned physical file offset;
- a missing, extra, reordered, moved, resized, or digest-less logical piece;
  and
- arithmetic overflow or a piece outside the physical extent.

`decode_direct_extent` additionally rejects:

- a misaligned userspace address;
- a short, long, or non-4,096-multiple input;
- a whole-extent digest mismatch;
- any nonzero byte in any padding range; and
- a logical-piece digest mismatch.

Tests re-sign the physical digest after mutating padding, proving that the
zero-padding check is not merely a consequence of the physical SHA. Tests
also re-sign the physical digest after mutating each logical piece, proving
that every per-piece SHA remains an independent boundary.

The retained blocking format cannot be losslessly reinterpreted as this
format. `try_from_blocking_store` validates the old record and then always
returns `MigrationRequired`; a future migration tool must read the legacy
logical pieces and write a newly encoded direct extent.

## Deterministic fixture

For the fixed proof inputs, the canonical JSON SHA-256 is:

```text
eb5efc3faefc67a932ed4b86e1af29bee89b53cf0483b6a39c373c938b047d6c
```

The target physical SHA-256 is:

```text
dc4a7e39978017ec424bd076ae7e2545561ff18126eaedd666332feeded77c9e
```

The MTP physical SHA-256 is:

```text
d3821fa7fe515afa07e8594b6134ccdff760e7fe396db44898ece260ca7c358a
```

The local gate regenerates the release fixture and byte-compares it with the
checked-in file. Debug and release commands emit identical bytes.

## Review findings carried forward

The accepted design review's nonblocking findings are binding on the next
implementation stages:

- CQ capacity must satisfy
  `original + async_cancel + fsync <= CQ entries`, with CQ entries fixed to
  twice descriptor capacity; `IORING_FEAT_NODROP` is not a correctness
  dependency.
- Physical restore reservation is ticket-scoped and only seeded by the first
  waiter; removing that waiter cannot release it.
- Future registered and CUDA-pinned buffers must budget double memlock,
  receive `MADV_DONTFORK`, and tear down in the order CUDA unregister,
  io_uring unregister, then unmap after zero outstanding descriptors.
- Segment tail slack outside a record is unspecified and must never be read.
- Publication-lease starvation policy must be explicit before implementing
  W0 admission.
- Fault proof must include data and journal fsync failure, CQ overflow with
  and without NODROP, and registered file/buffer invalidation.

The segment cleaner remains blocked on the preimplementation relocation
journal/checkpoint amendment required by the accepted review.

## Nonclaims

This proof does not implement or qualify:

- a durable binary record, journal, checkpoint, catalog, or restart replay;
- segment allocation, rollover, cleaning, relocation, or tail handling;
- `O_DIRECT`, `io_uring`, registered files, or registered buffers;
- buffer-slot generations or asynchronous ticket/cancellation state;
- checksum worker pools, quotas, scheduling classes, or admission policy;
- CUDA-pinned memory, HBM transfer, CUDA events, or GPU execution;
- cn4's filesystem, NVMe device, throughput, latency, endurance, capacity, or
  decode isolation; or
- K03, K05, checkpoint smoke, model quality, or serving readiness.

Those remain separate gates in the accepted sequence.
