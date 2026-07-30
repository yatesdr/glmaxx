# Direct-tier durable format v1

Date: 2026-07-30

Status: design candidate; adversarial review required before codec, catalog,
journal, checkpoint, or cleaner implementation

GPU evidence: none

## Purpose and authority

This document is the exact durable-format amendment required by the accepted
`direct-tier-io-v1` review. It freezes:

- the store directory and immutable segment format;
- the durable catalog entry, shard, checkpoint, and control-slot encodings;
- publication, visibility, relocation, and retirement journal transactions;
- startup decisions for every complete and incomplete transaction;
- physical allocation, catalog epoch, endurance, and garbage reconstruction;
- the atomic schema-version boundary for the direct-tier runtime; and
- the fail-closed boundary from the retained blocking store.

It incorporates the five minor obligations and three requested fault cases in
`docs/reviews/fable-direct-tier-io-v1.md`.

This design does not implement storage, authorize cn4 access, qualify
`io_uring`, prove HBM transfers, or pass K03/K05. The next allowed step after
adversarial acceptance is a pure CPU codec/recovery proof. Cleaner
implementation remains forbidden until this amendment is accepted.

All integers below are unsigned little-endian. All byte ranges are half-open.
Every `reserved` field and all format padding must be zero when written and
must be rejected if nonzero when read. Sizes and offsets are decimal unless
prefixed by `0x`.

## Non-negotiable identities

The store has one nonzero 256-bit `store_id`, generated once at creation and
repeated in every top-level file. Tests inject a fixed ID. Production may use
an operating-system CSPRNG. No timestamp, host name, process ID, path, random
UUID, or rank-local value enters a content identity or deterministic fixture.

The durable catalog key is:

```text
(namespace: [u8; 32], page_key: [u8; 32])
```

The namespace and page key retain the definitions in
`online-prefix-publication-v1`. DCP posture and writer rank are not part of
either identity. A DCP posture change therefore does not discard otherwise
compatible NVMe records.

The three generation domains remain distinct:

- catalog epoch orders complete catalog mutations;
- durable revision orders content revisions for one catalog key; and
- HBM allocation, tier-buffer, descriptor, and operation generations are
  ephemeral ABA guards and are never reconstructed from durable revisions.

## Hash and checksum rules

`SHA256-D(domain, bytes...)` means SHA-256 over:

```text
u16_le(domain_utf8_length) || domain_utf8 || bytes...
```

The length prefix makes domain separation unambiguous. This document names
every domain string. SHA fields are computed with that SHA field and any
trailing CRC field zeroed unless a narrower covered range is stated.

CRC fields use CRC32C Castagnoli over the complete fixed-size object with only
the CRC field zeroed. CRC is a torn-write detector, not an authenticity
substitute. A SHA mismatch, CRC mismatch, nonzero reserved byte, or invalid
enum is corruption.

Logical-piece SHA-256 values use ordinary SHA-256 over only the logical piece
bytes. `physical_sha256` uses ordinary SHA-256 over the complete padded
physical extent. These two existing content digests do not use `SHA256-D`.

## Store directory

One store is one directory opened by one process-wide exclusive authority.
Every component is a regular file with link count one. Symlinks, hard links,
devices, FIFOs, sockets, unexpected names, and files that escape the opened
directory are rejected before write health.

The authority retains a nonblocking exclusive open-file-description write
lock over all bytes of `control.g5d` for its lifetime. Failure to acquire it
returns `AlreadyOpen`; it never opens a second private catalog. Read-only
inspection uses a separate explicitly offline command and cannot coexist with
a mutating authority merely by choosing a different journal descriptor.

```text
format.g5d
control.g5d
segments/seg-%016x.g5d
checkpoints/catalog-%016x-%016x.g5c
journals/journal-%016x.g5j
```

Hexadecimal components are lowercase and fixed-width. Segment IDs and journal
generations are nonzero. The two checkpoint components are catalog epoch and
transaction-ID high-water mark and may both be zero only for the bootstrap
checkpoint. Temporary files use the same directory, an
implementation-private `.tmp` suffix, `O_EXCL`, and a name that can never
parse as a committed file.

Unknown committed-looking files are corruption. Recognizable temporary or
orphan files are classified during recovery and are not read as live state.
They may be reclaimed only after the selected control/checkpoint/journal
state proves that neither valid control generation references them.

The retained `journal.log`/`data.bin` store and its `GLTJRNL2` records are
incompatible. Their presence in a direct-store directory refuses startup.
There is no in-place reinterpretation, mixed replay, or silent fallback. A
future migration tool must read the retained format and write a new direct
store through separately reviewed code.

## Atomic format descriptor

`format.g5d` is exactly 4,096 bytes and immutable after creation:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `GLDXFMT1` |
| 8 | 2 | format version, exactly 1 |
| 10 | 2 | descriptor bytes, exactly 4,096 |
| 12 | 4 | format epoch, exactly 1 |
| 16 | 32 | store ID |
| 48 | 8 | segment bytes, exactly 1,073,741,824 |
| 56 | 4 | direct-I/O alignment, exactly 4,096 |
| 60 | 2 | catalog shard count, exactly 256 |
| 62 | 2 | physical extent schema, exactly 1 |
| 64 | 8 | target physical bytes, exactly 2,019,328 |
| 72 | 8 | MTP physical bytes, exactly 2,052,096 |
| 80 | 8 | maximum buffer bytes, exactly 2,052,096 |
| 88 | 4 | catalog entry bytes, exactly 512 |
| 92 | 4 | journal record bytes, exactly 4,096 |
| 96 | 416 | thirteen ordered 32-byte schema digests |
| 512 | 32 | descriptor SHA |
| 544 | 3,548 | reserved zero |
| 4,092 | 4 | CRC32C |

The thirteen schema digests, in order, are:

1. physical extent;
2. durable catalog metadata;
3. journal transaction;
4. checkpoint/control;
5. segment lifecycle;
6. tier buffer and completion descriptor;
7. restore ticket and state machine;
8. physical-versus-logical quota charges;
9. residency state set;
10. serving page-transaction types;
11. publication ticket and lease;
12. fixed metric names and enum label sets; and
13. the exact `io-uring` package plus relevant transitive package records
    from the qualified `Cargo.lock`.

Each digest is over a checked-in canonical schema byte string, not Rust debug
output. The descriptor SHA is:

```text
SHA256-D("glmaxx.direct.format.v1", bytes[0..512])
```

Changing any listed schema or dependency digest requires a new atomic format
epoch. A binary whose compiled schema vector differs refuses the store before
opening a segment. Ephemeral state is reset on restart, but its schema remains
part of this compatibility boundary so a mixed runtime cannot attach.

Bootstrap exclusively creates an 8,192-byte all-zero `control.g5d`, syncs it
and its directory, and acquires the lifetime lock before creating anything
else. It then creates and syncs, in order: the format descriptor, an empty
epoch-zero/high-water-zero checkpoint, journal generation one with only its
header, and control slot zero at generation one. Each renamed file's directory
is synced. Control slot one remains all zero and structurally invalid. The
bootstrap checkpoint has no segments or entries, `through_transaction = 0`,
`transaction_id_high_water = 0`, and `oldest_live_transaction = 1`.
Creation publishes no healthy store until every file is reread through the
same decoder used at restart.

## Canonical physical extent

The accepted direct-I/O extent is unchanged:

```text
target KV       [        0, 1,837,056)
zero padding    [1,837,056, 1,839,104)
target indexer  [1,839,104, 2,016,512)
zero padding    [2,016,512, 2,019,328)
draft sidecar   [2,019,328, 2,051,328)  MTP only
zero padding    [2,051,328, 2,052,096)  MTP only
```

Target-only records are 2,014,464 logical and 2,019,328 physical bytes
(493 blocks). MTP records are 2,046,464 logical and 2,052,096 physical bytes
(501 blocks). A target-to-MTP upgrade writes a complete new MTP extent.

## Segment files

Every segment file is exactly 1,073,741,824 bytes. Creation uses exclusive
creation, reserves the complete file capacity, sets the exact file length,
writes and syncs its header, renames it, and syncs `segments/` before a
`SegmentCreate` journal transaction can commit. A crash before
`SegmentCreate` leaves an orphan file, never a live segment.

The immutable 4,096-byte segment header is:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `GLDXSEG1` |
| 8 | 2 | version 1 |
| 10 | 2 | header bytes, 4,096 |
| 12 | 4 | flags, zero in v1 |
| 16 | 32 | store ID |
| 48 | 8 | nonzero segment ID |
| 56 | 8 | creation catalog epoch |
| 64 | 8 | fixed segment bytes |
| 72 | 1 | purpose: 1 publication, 2 relocation |
| 73 | 7 | reserved zero |
| 80 | 32 | format descriptor SHA |
| 112 | 32 | header SHA |
| 144 | 3,948 | reserved zero |
| 4,092 | 4 | CRC32C |

The header SHA is:

```text
SHA256-D("glmaxx.direct.segment-header.v1", bytes[0..112])
```

Segment IDs are process-global, monotonically increasing, nonzero, and never
reused. Recovery sets the next ID above every valid segment filename, header,
checkpoint entry, and journal reference, including orphans.

Physical extents begin at offset 4,096, are densely allocated in ascending
offset order, and never cross a segment boundary. `allocated_end` is the first
unallocated byte and is always 4,096-aligned. Segment state is journaled and
checkpointed, not rewritten into the immutable header.

There is at most one `PUBLICATION_ACTIVE` segment. A cleaner may additionally
own one `RELOCATION_BUILDING` destination, but new publications never append
to it and it accepts only offsets named by its committed relocation plan.
Sealed segments are read-only.

After its header, a segment can hold at most 531 target-only extents or 523
MTP extents. The residual bytes from `allocated_end` to the fixed file end
are tail slack. Tail slack is unspecified, outside all content digests, never
read, never copied, and never interpreted.

Capacity counts the complete fixed size of every allocated segment, including
an active relocation destination and files retained for crash fallback.
Cleaner admission requires capacity for a complete destination segment.

## Durable catalog entry

Every durable catalog entry is exactly 512 bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 32 | namespace |
| 32 | 32 | page key |
| 64 | 8 | nonzero durable revision |
| 72 | 1 | capability: 1 target, 2 MTP |
| 73 | 1 | advisory writer rank, 0 through 3 |
| 74 | 1 | piece count: 2 target, 3 MTP |
| 75 | 1 | flags |
| 76 | 4 | reserved zero |
| 80 | 32 | parent page key or all zero |
| 112 | 8 | page ordinal |
| 120 | 2 | valid token count, exactly 64 |
| 122 | 6 | reserved zero |
| 128 | 8 | nonzero segment ID |
| 136 | 8 | physical offset |
| 144 | 8 | physical length |
| 152 | 32 | physical SHA-256 |
| 184 | 64 | piece record 0 |
| 248 | 64 | piece record 1 |
| 312 | 64 | piece record 2 |
| 376 | 32 | entry SHA |
| 408 | 104 | reserved zero |

Flags are exactly one of:

```text
0x01 VISIBLE
0x02 PARENT_PENDING
```

Ordinal zero requires a zero parent and `VISIBLE`. A nonzero ordinal requires
a nonzero parent and exactly one of `VISIBLE` or `PARENT_PENDING`. No restore,
prefix hit, or residency registration may observe a pending entry. Pending
entries remain live durable content for capacity, deduplication, and cleaning.

A 64-byte piece record is:

| Relative offset | Bytes | Field |
|---:|---:|---|
| 0 | 1 | kind: 1 target KV, 2 target indexer, 3 draft sidecar |
| 1 | 7 | reserved zero |
| 8 | 8 | offset relative to physical extent |
| 16 | 8 | logical length |
| 24 | 32 | logical SHA-256 |
| 56 | 8 | reserved zero |

Piece records are in ascending kind order and must exactly match the canonical
offsets and lengths. The third record of a target entry is all zero.
Capability, piece count, physical length, and piece table must agree.

The entry SHA is:

```text
SHA256-D("glmaxx.direct.catalog-entry.v1", bytes[0..376])
```

The entry SHA is the catalog record digest bound into restore tickets.
Relocation changes the entry SHA because it changes segment/offset, but not
the durable revision or logical piece hashes. Visibility changes only flags
and entry SHA. MTP upgrade increments durable revision and replaces the whole
physical extent.

## Catalog semantics

The durable catalog includes both visible and parent-pending records. The
rank/restore-facing immutable snapshot exposes only visible records.

Entries are sharded by `page_key[0]`. Within a shard they are sorted by
`(namespace, page_key)` bytewise ascending. Duplicate keys are corruption.
The catalog epoch starts at zero and increments exactly once for each:

- committed new publication;
- committed target-to-MTP upgrade;
- pending-to-visible transition; or
- committed catalog eviction; or
- committed relocation publication containing one or more mappings.

Segment create, seal, and retire events do not change catalog epoch. Exact
dedup performs no journal append, no extent allocation, no durable revision
change, and no catalog epoch change.

Catalog eviction is a cache operation, not sequence truncation. An entry may
be evicted only when it has no runtime snapshot/ticket/residency reference and
no visible or pending child names it as parent. Chains are therefore evicted
leaf-first. The removed extent becomes reclaimable only after old catalog
epochs drain. Reintroducing an absent key starts at revision one; safety does
not rely on lifetime-monotonic revision because every operation also binds
the catalog epoch and complete entry SHA. A still-live or older-epoch record
can never alias the new physical entry.

For new content, durable revision is one. An MTP upgrade is exactly the prior
revision plus one. Overflow is fatal. The accepted dedup/collision matrix is
applied against pending and visible entries alike. A visibility transition
does not change durable revision.

A new ordinal-zero record is visible. A new nonzero record is visible only if
its exact parent key is already visible in the same namespace at the prior
ordinal; otherwise it is committed as parent-pending. When a parent becomes
visible, pending direct children are made visible one at a time in
`(page_ordinal, namespace, page_key)` order. Each transition is its own
journal transaction and epoch. This makes out-of-order durable child
completion restartable without exposing a broken chain or requiring an
unbounded journal record.

The catalog root is:

```text
SHA256-D("glmaxx.direct.catalog-root.v1",
         catalog_epoch_le ||
         for shard 0..255:
             shard_id_le || entry_count_le || logical_shard_sha256)
```

An empty shard has count zero and an all-zero logical shard SHA. A nonempty
logical shard SHA is ordinary SHA-256 over its concatenated 512-byte entries.
The process publishes the new immutable top-level table only after the
corresponding journal transaction is durable.

## Journal record

Every journal file consists solely of 4,096-byte records. A short final record
is a torn tail. A complete record with any invalid field is corruption and is
never treated as a torn tail.

The common record is:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `GLDXJRN1` |
| 8 | 2 | journal version 1 |
| 10 | 1 | event kind |
| 11 | 1 | flags, zero in v1 |
| 12 | 2 | common-header bytes, exactly 120 |
| 14 | 2 | reserved zero |
| 16 | 8 | journal generation |
| 24 | 8 | record sequence within journal |
| 32 | 8 | transaction ID |
| 40 | 4 | event ordinal within transaction |
| 44 | 4 | payload bytes |
| 48 | 32 | previous full-record SHA |
| 80 | 32 | payload SHA |
| 112 | 8 | nondecreasing service hour |
| 120 | 3,972 | payload followed by zero padding |
| 4,092 | 4 | CRC32C |

The payload SHA is:

```text
SHA256-D("glmaxx.direct.journal-payload.v1",
         event_kind_u8 || payload[0..payload_bytes])
```

`previous_full_record_sha` is ordinary SHA-256 over all 4,096 exact bytes of
the previous record, including its CRC. The journal header record has an
all-zero predecessor. Sequence starts at zero and increments by one. No gaps,
duplicates, or hash-chain forks are allowed.

The journal header is record sequence zero, transaction zero, event ordinal
zero, kind 0. Its 160-byte payload is:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 32 | store ID |
| 32 | 8 | base checkpoint epoch |
| 40 | 8 | base checkpoint through-transaction |
| 48 | 8 | base transaction-ID high-water mark |
| 56 | 32 | base catalog root |
| 88 | 32 | format descriptor SHA |
| 120 | 8 | previous journal generation, zero at bootstrap |
| 128 | 32 | previous journal final-record SHA, zero at bootstrap |

All later transaction IDs are globally increasing and greater than the base
checkpoint's transaction-ID high-water mark. Transactions are contiguous and
cannot interleave. Event ordinals start at zero. A transaction type has one
exact terminal event; a nonterminal transaction may appear only as the final
transaction in a journal.

An error after a durable publication Begin or relocation PlanCommit
write-poisons the authority: no later transaction may be issued until
restart/recovery. This makes an incomplete durable transaction necessarily
the journal tail.

## Journal payloads and transaction shapes

Event kind numbers are fixed:

| Kind | Name |
|---:|---|
| 0 | JournalHeader |
| 1 | SegmentCreate |
| 2 | SegmentSeal |
| 3 | PublishBegin |
| 4 | PublishExtentDurable |
| 5 | PublishPieceDurable |
| 6 | PublishCommit |
| 7 | VisibilityCommit |
| 8 | RelocationPlanBegin |
| 9 | RelocationMap |
| 10 | RelocationPlanCommit |
| 11 | RelocationPublish |
| 12 | SegmentRetire |
| 13 | CatalogDelete |
| 14 | RelocationAbandon |
| 15 | PublishAbort |

Fields below are packed in listed order with no implicit alignment. Any
remaining payload area is record padding and zero.

### Segment lifecycle

`SegmentCreate` is one complete transaction, ordinal zero:

```text
segment_id:u64
creation_catalog_epoch:u64
segment_bytes:u64
purpose:u8
initial_state:u8
reserved:[u8;6]
segment_header_sha:[u8;32]
allocated_end:u64                 // exactly 4,096
```

Initial state is `PUBLICATION_ACTIVE` or `RELOCATION_BUILDING`. The named file
and synced header must already exist.

`SegmentSeal` is one complete transaction:

```text
segment_id:u64
allocated_end:u64
purpose:u8
reserved:[u8;7]
segment_header_sha:[u8;32]
```

It is appended only after every allocated extent is durable. A sealed file is
never appended again.

### Publication

A successful publication transaction is exactly:

```text
PublishBegin(ordinal 0)
PublishExtentDurable(ordinal 1)
PublishPieceDurable(ordinal 2 .. 1 + piece_count)
PublishCommit(ordinal 2 + piece_count)
```

`PublishBegin` payload:

```text
expected_catalog_epoch:u64
new_catalog_epoch:u64             // expected + 1
expected_previous_entry_sha:[u8;32] // zero for a new key
candidate_entry:[u8;512]
pre_catalog_root:[u8;32]
planned_post_catalog_root:[u8;32]
data_endurance_charge:u64         // physical_length
```

Before Begin, the authority validates capacity, revision, dedup/collision,
parent state, extent allocation, quotas, and rolling endurance. It reserves
the fixed extent and buffer. Begin is appended and `fdatasync`ed before any
extent byte may be written.

`PublishExtentDurable` payload:

```text
candidate_entry_sha:[u8;32]
observed_physical_sha:[u8;32]
exact_completed_bytes:u64
```

It is appended only after one exact full-extent direct write completes and
the data segment `fdatasync` succeeds. The journal event is then
`fdatasync`ed.

Each `PublishPieceDurable` payload is:

```text
candidate_entry_sha:[u8;32]
piece_kind:u8
reserved:[u8;7]
observed_piece_sha:[u8;32]
```

Pieces occur in ascending kind order, and every event is individually
`fdatasync`ed.

`PublishCommit` payload:

```text
candidate_entry_sha:[u8;32]
expected_previous_entry_sha:[u8;32]
previous_catalog_epoch:u64
new_catalog_epoch:u64
pre_catalog_root:[u8;32]
post_catalog_root:[u8;32]
```

Commit is `fdatasync`ed before the in-memory catalog epoch changes. Each
journal append/barrier is a strict dependency; concurrently outstanding
journal appends across one of these barriers are forbidden.

The candidate's `VISIBLE` or `PARENT_PENDING` flag is fixed at Begin.
Publication Commit always installs the durable entry and advances the durable
catalog epoch. Only a visible candidate enters the rank-facing prefix
snapshot.

An exact dedup is decided before Begin and writes nothing. A same-key content
collision is engine-fatal before Begin.

If a crash or durability failure leaves a final publication transaction after
its synced Begin but before Commit, recovery must append and sync
`PublishAbort` as that same transaction's next event before write health can
resume:

```text
candidate_entry_sha:[u8;32]
segment_id:u64
physical_offset:u64
physical_length:u64
data_endurance_charge:u64
last_valid_event_kind:u8
durable_piece_mask:u8
reason:u8                         // 1 crash, 2 data I/O, 3 durability
reserved:[u8;5]
```

The fields must match the validated Begin and any later valid events. Abort is
the terminal event for this failed publication. It does not install an entry,
release or reuse its extent, or refund endurance; the bytes remain allocator
garbage. If Abort cannot be durably appended, the store remains read-only.

### Pending child visibility

`VisibilityCommit` is one complete transaction:

```text
expected_catalog_epoch:u64
new_catalog_epoch:u64
old_entry_sha:[u8;32]
new_entry:[u8;512]
pre_catalog_root:[u8;32]
post_catalog_root:[u8;32]
```

The old entry must be parent-pending. The new entry must differ only by
changing flags to visible and recomputing entry SHA. Its parent must be
visible at the immediately preceding ordinal in the same namespace. The
record is synced before the new catalog snapshot is published. Startup rolls
forward a durable VisibilityCommit.

### Catalog eviction

`CatalogDelete` is one complete transaction:

```text
expected_catalog_epoch:u64
new_catalog_epoch:u64
old_entry_sha:[u8;32]
namespace:[u8;32]
page_key:[u8;32]
reason:u8                         // 1 capacity, 2 tenant, 3 policy
reserved:[u8;7]
pre_catalog_root:[u8;32]
post_catalog_root:[u8;32]
```

Before append, the old entry must match, have no current child and have no
runtime reference. The record is synced before removal from the in-memory
catalog. Recovery always rolls a durable deletion forward. Physical bytes
remain protected by older epoch references and valid recovery generations;
the journal event never authorizes immediate segment deletion.

### Relocation plan

Relocation uses a completed plan transaction followed later by an independent
single-record publication transaction. The journal is therefore not held
open while up to one segment of data is copied, and ordinary publications may
continue during the copy.

A plan transaction is:

```text
RelocationPlanBegin(ordinal 0)
RelocationMap(ordinal 1 .. mapping_count)
RelocationPlanCommit(ordinal mapping_count + 1)
```

`RelocationPlanBegin` payload:

```text
pinned_catalog_epoch:u64
source_segment_id:u64
destination_segment_id:u64
mapping_count:u32
reserved:u32
mapping_list_sha:[u8;32]
source_catalog_root:[u8;32]
```

Every `RelocationMap` payload is exactly 696 bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 4 | zero-based mapping ordinal |
| 4 | 4 | reserved zero |
| 8 | 32 | namespace |
| 40 | 32 | page key |
| 72 | 8 | durable revision |
| 80 | 8 | old segment ID |
| 88 | 8 | old offset |
| 96 | 8 | new segment ID |
| 104 | 8 | new offset |
| 112 | 8 | physical length |
| 120 | 32 | physical SHA-256 |
| 152 | 32 | old catalog entry SHA |
| 184 | 512 | complete proposed new catalog entry |

The new entry must differ from the old entry only in segment ID, physical
offset, and entry SHA. It preserves visibility, capability, revision, logical
piece metadata, and physical SHA.

`mapping_list_sha` is:

```text
SHA256-D("glmaxx.direct.relocation-map-list.v1",
         concatenated 696-byte RelocationMap payloads)
```

Maps are ordered by old offset, then namespace, then page key. Their old
extents are exactly the pinned epoch's live/pending records in the selected
source segment, and their destination extents are dense, aligned,
nonoverlapping, and within the destination segment.

`RelocationPlanCommit` payload:

```text
plan_begin_full_record_sha:[u8;32]
mapping_list_sha:[u8;32]
mapping_count:u32
reserved:u32
destination_allocated_end:u64
destination_header_sha:[u8;32]
source_header_sha:[u8;32]
data_endurance_charge:u64         // sum of mapped physical lengths
```

No destination extent write may start until the complete plan, including its
terminal marker, is `fdatasync`ed. Thus a plan transaction without
RelocationPlanCommit has no associated data and is ignored on recovery.

Once the plan commits, its complete data endurance charge is consumed
conservatively even if a later copy stops early. The cleaner copies each full
physical extent through a qualified aligned buffer, verifies physical SHA,
piece hashes, and padding, then syncs the destination segment. It seals and
syncs the destination before relocation publication.

### Relocation publication

`RelocationPublish` is one complete transaction:

```text
plan_transaction_id:u64
pinned_catalog_epoch:u64
expected_current_catalog_epoch:u64
new_catalog_epoch:u64
mapping_count:u32
reserved:u32
mapping_list_sha:[u8;32]
pre_catalog_root:[u8;32]
post_catalog_root:[u8;32]
source_segment_id:u64
destination_segment_id:u64
```

Before append, every current catalog entry named by the plan must still match
its old entry SHA. Unrelated catalog changes since the pinned epoch are
allowed. A changed planned entry aborts publication: the destination remains
an orphan and no catalog mutation occurs.

The record replaces all mappings in one copy-on-write catalog epoch. It is
`fdatasync`ed before the new in-memory snapshot is published. A durable
RelocationPublish whose catalog was not published before a crash is always
rolled forward at startup; discard is forbidden.

The old source remains readable until all runtime readers of every older
epoch, restore tickets, and checksum jobs drain.

If a committed plan cannot be published because a mapped entry changed, a
copy or sync fails, or shutdown elects not to finish it, the authority appends
one complete `RelocationAbandon` transaction:

```text
plan_transaction_id:u64
source_segment_id:u64
destination_segment_id:u64
mapping_list_sha:[u8;32]
destination_header_sha:[u8;32]
reason:u8                         // 1 stale, 2 I/O, 3 shutdown
reserved:[u8;7]
```

It is legal only when no RelocationPublish exists. After it is synced, the
source remains authoritative and the destination is a fully garbage orphan.
The destination may be unlinked and its directory synced; a crash between
marker and unlink completes that action during recovery. Segment IDs and
endurance charges are not recovered or reused.

### Segment retirement

`SegmentRetire` is one complete transaction:

```text
source_segment_id:u64
plan_transaction_id:u64           // zero only for a fully garbage segment
relocation_publish_transaction_id:u64 // zero only for fully garbage
catalog_epoch_after_relocation:u64
source_header_sha:[u8;32]
```

It may commit only when the current catalog has no entry in the source and all
runtime epoch/ticket/checksum references have drained. It marks the source
`RETIRE_PENDING`; it does not itself make unlink immediately safe.

A segment file may be unlinked only after:

1. SegmentRetire is durable;
2. neither structurally valid control slot's checkpoint-plus-journal replay
   can require the segment;
3. no current catalog entry or runtime reference names it; and
4. the directory is synced after unlink.

Because two control generations are retained, physical deletion can require
two checkpoint rotations after relocation. The cleaner may request those
rotations but may not invalidate the previous recovery generation. A crash
after a retirement marker but before eligible unlink causes startup to
complete the same eligibility check and unlink. A missing source without a
durable retirement marker is corruption.

## Checkpoint file

A checkpoint is immutable. Its layout is:

```text
4,096-byte checkpoint header
segment table, padded to 4,096
16,384-byte shard directory (256 * 64)
nonempty shard payloads, each independently padded to 4,096
```

For an empty segment table, padded table bytes and table SHA are all zero and
the shard directory begins at offset 4,096. Otherwise its byte length is
`segment_record_count * 128`, rounded up to 4,096 with zero padding, and the
stored SHA is ordinary SHA-256 over that complete padded table.

The 4,096-byte header is:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `GLDXCP01` |
| 8 | 2 | version 1 |
| 10 | 2 | header bytes, 4,096 |
| 12 | 2 | shard count, 256 |
| 14 | 2 | flags, zero |
| 16 | 32 | store ID |
| 48 | 8 | checkpoint catalog epoch |
| 56 | 8 | through-transaction |
| 64 | 8 | oldest-live-transaction |
| 72 | 8 | catalog entry count |
| 80 | 4 | segment record count |
| 84 | 4 | active publication segment count, zero or one |
| 88 | 8 | segment-table offset, 4,096 |
| 96 | 8 | padded segment-table bytes |
| 104 | 8 | shard-directory offset |
| 112 | 8 | shard-directory bytes, 16,384 |
| 120 | 8 | first shard-payload offset |
| 128 | 32 | padded segment-table SHA-256 |
| 160 | 32 | shard-directory SHA-256 |
| 192 | 32 | catalog root |
| 224 | 32 | format descriptor SHA |
| 256 | 32 | checkpoint-header SHA |
| 288 | 8 | endurance base service hour |
| 296 | 192 | twenty-four `u64` data-write buckets |
| 488 | 8 | segment-ID high-water mark |
| 496 | 8 | transaction-ID high-water mark |
| 504 | 3,588 | reserved zero |
| 4,092 | 4 | CRC32C |

Header SHA is:

```text
SHA256-D("glmaxx.direct.checkpoint-header.v1",
         complete 4,096-byte header with header SHA and CRC zero)
```

The full checkpoint ordinary SHA-256 and exact length are stored in the
control slot.

Each segment-table record is 128 bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | segment ID |
| 8 | 8 | creation catalog epoch |
| 16 | 8 | allocated end |
| 24 | 1 | state |
| 25 | 1 | purpose |
| 26 | 6 | reserved zero |
| 32 | 32 | segment header SHA |
| 64 | 8 | relocation plan transaction or zero |
| 72 | 8 | relocation publish transaction or zero |
| 80 | 8 | retirement catalog epoch or zero |
| 88 | 40 | reserved zero |

States are `PUBLICATION_ACTIVE`, `RELOCATION_BUILDING`, `SEALED`,
`RETIRE_PENDING`, and `RETIRED_TOMBSTONE`. Checkpoint creation is forbidden
while any relocation destination is building or while a plan is committed but
neither published nor durably abandoned, so `RELOCATION_BUILDING` cannot
appear in a completed checkpoint. Segment lifecycle records are sorted by
segment ID. A retired tombstone may remain after its file is deleted and may
be omitted only when neither valid control generation can require the old
segment. No live/garbage/free counters are stored.

Checkpoint creation has no open or unresolved transaction, so
`oldest_live_transaction` is exactly `transaction_id_high_water + 1`;
overflow is fatal. `through_transaction` is the greatest transaction whose
terminal event was materialized into the checkpoint. The high-water mark also
covers transaction IDs observed in a safely truncated, unsynced partial
relocation plan and is never smaller than `through_transaction`. The explicit
fields make truncation and nonreuse verifiable rather than inferred. The two
control generations retain the journals each needs. Runtime-reader epochs are
intentionally not durable and do not change this field.

The segment-ID high-water mark is at least every assigned segment ID,
including deleted segments and omitted tombstones. It prevents reuse after
all files and lifecycle records for an old segment are gone.

Each 64-byte shard descriptor is:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 2 | shard ID |
| 2 | 2 | reserved zero |
| 4 | 4 | entry count |
| 8 | 8 | payload offset |
| 16 | 8 | logical payload bytes |
| 24 | 8 | padded payload bytes |
| 32 | 32 | padded payload SHA-256 |

Empty shards have zero offsets, lengths, and SHA. Nonempty logical length is
`entry_count * 512`; padded length is rounded up to 4,096. Padding is zero.
Payload offset and padded length are 4,096-aligned. The descriptor SHA field
is ordinary SHA-256 over the complete padded payload. The separately defined
logical shard SHA used by the catalog root is ordinary SHA-256 over only its
logical entries and is recomputed while decoding.

Catalog entries in the checkpoint include parent-pending records. The decoder
validates sorting, uniqueness, entry digests, extent geometry, capability,
parent/ordinal state, shard assignment, all top-level hashes, and the catalog
root before returning a snapshot.

## Two-slot control file

`control.g5d` is exactly 8,192 bytes containing two independent 4,096-byte
slots. Slot index equals physical slot. A slot is:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `GLDXCTL1` |
| 8 | 2 | version 1 |
| 10 | 1 | slot index, 0 or 1 |
| 11 | 1 | flags, zero |
| 12 | 2 | slot bytes, 4,096 |
| 14 | 2 | reserved zero |
| 16 | 8 | nonzero control generation |
| 24 | 32 | store ID |
| 56 | 8 | checkpoint catalog epoch |
| 64 | 8 | checkpoint through-transaction |
| 72 | 8 | checkpoint transaction-ID high-water mark |
| 80 | 8 | journal generation |
| 88 | 8 | exact checkpoint file length |
| 96 | 32 | full checkpoint SHA-256 |
| 128 | 32 | full journal-header-record SHA-256 |
| 160 | 8 | segment bytes |
| 168 | 4 | shard count |
| 172 | 4 | format epoch |
| 176 | 32 | format descriptor SHA |
| 208 | 32 | control-slot SHA |
| 240 | 3,852 | reserved zero |
| 4,092 | 4 | CRC32C |

Slot SHA is:

```text
SHA256-D("glmaxx.direct.control-slot.v1",
         complete 4,096-byte slot with slot SHA and CRC zero)
```

At startup both slots are structurally validated independently. The
structurally valid slot with the greatest control generation is selected.
Equal nonzero generations are corruption. If the selected slot then names a
missing, truncated, or invalid checkpoint/journal, startup fails; it does not
silently fall back to an older structurally valid slot. A structurally torn
new slot may fall back to the prior slot because the writer accepts no new
transaction until the new slot has been reread and validated.

A selected checkpoint may tolerate a missing `RETIRE_PENDING` segment only
after recovery has also validated the other structurally valid control
generation and both replay states contain a durable retirement that makes
that segment unnecessary. `RETIRED_TOMBSTONE` never requires a file. Every
other selected segment state requires the exact file and header.

## Checkpoint and journal rotation

Rotation requires the single writer, no open transaction, no write-poison,
and no unresolved relocation plan. The exact order is:

1. sync the current journal and capture its final-record SHA;
2. write a complete checkpoint temporary file, sync it, rename it to its
   derived final name, and sync `checkpoints/`;
3. write a new journal temporary file whose header binds that checkpoint and
   previous journal tail, sync it, rename it, and sync `journals/`;
4. write one complete new control slot into the lower-generation physical
   slot, `fdatasync(control.g5d)`, reread it, and validate the selected
   checkpoint and journal through that slot;
5. only then switch the authority to the new journal and accept work; and
6. reclaim files referenced by neither structurally valid control
   generation, subject to SegmentRetire and directory-sync rules.

The new control generation is prior maximum plus one; overflow is fatal.
Journal generation and transaction IDs are likewise monotonic and
nonreusable. Checkpoint creation contains the current durable catalog and
segment lifecycle after replay through `through_transaction`.

Both valid control generations and every file required to replay either are
retained. This is why segment deletion may lag catalog relocation. Temporary
checkpoint/journal files created before control publication are orphans.

## Restart algorithm

Only the process-wide exclusive authority may recover or mutate the store.
Startup performs these steps before reporting tier health:

1. open the directory without following links and acquire the exclusive
   store lock;
2. validate `format.g5d` and the compiled thirteen-schema vector;
3. scan committed-looking filenames, validate syntax, and establish
   nonreusable ID/generation high-water marks;
4. structurally validate both control slots and select the highest;
5. validate the selected checkpoint and journal header against the control
   slot;
6. decode the checkpoint catalog, segment table, endurance buckets, and
   parent state;
7. replay complete journal records in sequence/hash-chain order;
8. classify the final transaction using the rules below;
9. validate every catalog extent is aligned, has an allowed length, lies
   within `[4,096, allocated_end)`, and does not overlap another extent in
   that segment;
10. validate every referenced segment file/header/size and every segment
    lifecycle transition;
11. rebuild visible/pending catalog shards, allocator high-water marks,
    endurance buckets, capacity, and garbage counters;
12. classify safe orphans without reading them as content; and
13. initialize all ephemeral buffers, descriptors, tickets, quotas,
    residency states, page transactions, leases, and CQ counters empty.

Physical and piece hashes are verified on every restore before bytes become
host-ready. Startup does not claim a full multi-terabyte payload scrub; it
validates metadata, bounds, nonoverlap, headers, and file presence. An
optional background scrub is separately reported and cannot turn corruption
into a cache miss.

The final transaction decisions are:

| Final state | Recovery decision |
|---|---|
| short final record | ignore/truncate only after authority opens writable |
| complete invalid record | fatal corruption |
| partial relocation plan without PlanCommit | truncate to the transaction boundary, retain its transaction-ID high-water, and reuse no destination or transaction ID; no data was permitted |
| PublishBegin without Commit | reserve its extent/high-water and endurance charge, keep entry invisible, and durably append PublishAbort before write health |
| publication through data/piece events without Commit | same orphan rule; validate the observed phase and append PublishAbort; never synthesize Commit |
| complete PublishCommit | roll forward catalog entry and epoch |
| complete PlanCommit without RelocationPublish | destination extents are orphan/reserved; source catalog remains authoritative |
| complete RelocationPublish | roll forward every mapping and epoch |
| complete RelocationAbandon | retain source mapping and reclaim destination under its marker |
| complete VisibilityCommit | roll forward visibility and epoch |
| complete CatalogDelete | roll forward catalog removal and epoch |
| complete SegmentRetire | retain or unlink only under the two-control-slot eligibility rule |

An incomplete nonfinal transaction, an event after write-poison, or a later
transaction following an incomplete one is corruption. The two explicitly
allowed recovery normalizations are truncating an unsynced partial relocation
plan and terminally aborting the final begun publication. Neither reuses a
transaction ID.

Parent validation runs over the durable catalog after replay. A visible child
without its exact visible parent at the prior ordinal is corruption.
Parent-pending entries remain invisible and may later transition when their
parent becomes visible. Contradictory metadata for one key is corruption.

No in-flight I/O, waiter, runtime epoch reference, or HBM/DRAM residency
survives process restart.

## Garbage and capacity reconstruction

Garbage counters are never persisted or trusted. At startup, for each segment:

```text
allocated_bytes = allocated_end - 4,096
current_live_bytes = sum physical_length of all catalog entries naming segment
startup_garbage_bytes = allocated_bytes - current_live_bytes
tail_slack_bytes = segment_bytes - allocated_end
```

The sums use checked arithmetic and nonoverlap has already been proven.
Parent-pending records count live. Tail slack is not garbage and is never
read. At runtime, extents retained by old catalog epochs, restore tickets, or
checksum jobs are additionally nonreclaimable until those references drain.

Physical store capacity is the fixed size of all allocated segment files, not
only `allocated_end`. Catalog capacity counts pending and visible entries.
Insufficient capacity for a relocation destination makes new writes
read-only; it never permits deletion of a referenced source.

Cleaner source selection remains deterministic: greatest reclaimable-garbage
ratio, then lowest segment ID. A segment is ineligible while active, building,
already retirement-pending, or referenced by an undrained runtime epoch.

## Rolling data-write endurance

The configured 24-hour limit is explicitly a logical-host physical-extent
write budget, not a claim about flash translation-layer NAND writes.
Publication charges one complete physical extent at durable Begin.
Relocation charges the sum of all mapped physical extents at durable
PlanCommit. Both are conservative when a crash or I/O failure occurs before
all bytes reach the device.

The journal common `service_hour` is `floor(unix_seconds / 3,600)` from the
qualified host clock. It is clamped nondecreasing during one authority
lifetime. A backward clock does not reopen a bucket. A configured implausible
forward jump degrades write health until operator resolution; a legitimate
restart more than 24 hours later expires old buckets normally. Deterministic
tests inject fixed service hours.

The checkpoint stores 24 consecutive buckets beginning at
`endurance_base_service_hour`; replay adds charges from PublishBegin and
RelocationPlanCommit. Admission rotates expired buckets, checks the complete
planned charge before lease/plan acceptance, and uses checked arithmetic.

Journal, checkpoint, control, and segment-header bytes are reported
separately as metadata writes but are not mislabeled as extent payload or
NAND wear. A later device-health wear budget requires measured device
telemetry and a contract amendment.

## Runtime invariants bound by the format

Although buffers and tickets are not serialized, the atomic format descriptor
binds these accepted rules:

- fixed buffers are 2,052,096 bytes and 4,096-aligned;
- generation zero is invalid and generation overflow retires the slot;
- descriptor user data resolves to full buffer and operation generations;
- `CQ entries = descriptor_table_capacity * 2`;
- at startup and every submit,
  `original + async_cancel + fsync <= CQ entries`;
- submission returns `WAIT` before violating that inequality;
- `IORING_FEAT_NODROP` is probed but never required for correctness;
- the physical restore charge belongs to the ticket and survives departure of
  the first waiter; every waiter separately owns logical quota;
- registered plus CUDA-pinned buffers budget both memlock charges and use
  `MADV_DONTFORK`; and
- teardown after zero descriptors is CUDA unregister, io_uring buffer
  unregister, then unmap.

Registered-file or registered-buffer invalidation, impossible/overflow CQ
state, data-segment fsync failure, journal fsync failure, and control/
checkpoint fsync failure are explicit fault-injection cases. Durability
barrier uncertainty globally degrades the tier; it is not a local retry or
cache miss.

## W0 lease starvation rule

Read reserves remain unavailable to writes. New W0 publication leases normally
pause above the read high watermark, but eligible W0 work cannot be deferred
forever by continuous reads.

The immutable scheduler configuration includes
`max_read_bytes_before_w0_admission`. Once an eligible W0 has waited while
that many R0/R1 bytes are serviced, the next shared-capacity service
opportunity admits exactly one W0 lease. Startup requires at least one
non-read-reserved buffer, descriptor, and CQ allowance capable of that lease.
Capacity, catalog, tenant, or endurance refusal makes the candidate
ineligible rather than starved. Accepted W0 work retains its bounded
completion path. W1 cleaner work receives no analogous guarantee and runs
only below read and publication low watermarks.

The CPU scheduler proof must demonstrate the exact byte bound under continuous
R0/R1 arrivals and must separately show that the periodic W0 does not consume
read reserves.

## Required CPU proof after review

After this design receives its exact acceptance token, one CPU proof must
cover:

1. byte-exact encode/decode and mutation rejection for every fixed object;
2. deterministic store bytes with injected store ID and service hour;
3. all 256 catalog shards, empty shards, sorting, duplicate rejection, and a
   16,384-page chain;
4. visible versus pending child recovery and deterministic visibility
   cascading;
5. every exact dedup, MTP upgrade, collision, and revision-overflow cell;
6. segment rollover at every target/MTP boundary and exact 531/523 maxima;
7. capacity accounting including fixed files, tail slack, pending entries,
   orphans, and relocation destinations;
8. publication crash injection before and after every write and sync;
9. relocation plan, every mapped extent, destination sync/seal, publish,
   epoch drain, two-control-generation retention, retire, unlink, and
   directory-sync crash points;
10. concurrent upgrade/visibility change invalidating a relocation plan;
11. both control slots torn/corrupt/missing-reference matrices and selection
    behavior;
12. checkpoint rotation and journal hash-chain continuity;
13. physical overlap, truncation, missing segment, wrong header, reused ID,
    bad reserved byte, bad CRC, and every SHA layer;
14. exact startup decisions for every incomplete transaction;
15. garbage reconstruction from catalog with no persisted counter trust;
16. 24-hour bucket rotation, backward/forward clock behavior, and
    publication/cleaner conservative charges;
17. legacy retained-store and mixed-format refusal;
18. CQ overflow with and without NODROP, fsync failures, and registration
    invalidation;
19. periodic W0 admission under continuous reads without consuming read
    reserves; and
20. final zero ephemeral buffers, descriptors, CQEs, tickets, waiters,
    quotas, leases, and runtime epoch references after every schedule.

The CPU proof uses ordinary files and the reference extent codec. It does not
claim `O_DIRECT`, io_uring, filesystem durability, device bandwidth, CUDA
pinning, HBM transfer, model execution, or decode isolation.

## Subsequent gates

The allowed order after acceptance is:

1. pure CPU codec, catalog, replay, checkpoint, and relocation state proof;
2. adversarial review of that executable proof;
3. nonproduction io_uring feature/fault probe on an authorized Linux host;
4. target-store direct-I/O, durability, crash, and cleaner qualification;
5. registered/pinned HBM transfer integration and adversarial review;
6. cold/warm restore plus publication through a checkpoint; and
7. matched resident-decode isolation under R0/R1/W0/W1 pressure.

No result before the final two steps passes K03, K05, or the production
prefix-cache requirement.
