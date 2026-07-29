# Online prefix publication v1

Date: 2026-07-29

Status: design candidate; adversarial review required before CPU
implementation

GPU evidence: none

## Scope

This contract defines how a full target page, mandatory target-indexer
sidecar, and optional combined MTP sidecar become a durable and immediately
reusable prefix after model execution seals the page.

It does not qualify HBM-to-host transfers, NVMe bandwidth, cache policy, or
SM120 execution. It freezes the ownership and failure boundary needed before
those paths can be implemented without page-ID reuse, restart, or
target/draft atomicity defects.

## Blocking gaps in the current implementation

The existing components prove offline publication and restore, but cannot be
connected as an online path:

1. `PhysicalPageId` has no allocation generation. An asynchronous copy can
   therefore read a local page ID after it has been freed and reused.
2. `TierRecord::generation` is used both like a durable content revision and
   like an HBM attachment generation. Those are different identity domains.
3. each rank restore worker opens its own `FileTierStore` and retains a
   private published-record map. It cannot observe a record published after
   worker startup.
4. `PrefixIndex` can insert only a complete prefix beginning at page zero. It
   cannot publish one newly sealed child page.
5. `PrefixIndex` cannot reconstruct its searchable estate from durable
   records after restart.
6. the coordinator releases prompt token IDs after prefill and has no
   incremental committed-token ledger from which to derive keys for later
   generated pages.
7. the linear `PageState` transition table models movement between tiers.
   Online publication is replication while the active HBM page remains
   readable, so publication state cannot replace the HBM state.
8. the store has no configured byte capacity, rolling write budget, or
   online catalog-update protocol.

These are correctness gaps, not performance polish.

## Identity domains

Three generations remain separate.

### Sequence-table generation

`sequence_table_generation: u64` orders rank-invariant page-table mutations.
It advances at admission, reservation, commit/rollback, cancellation, and
removal as defined by `serving-page-transaction-v1`.

### HBM allocation generation

Every target local page ID and every draft local page ID has a monotonically
increasing allocation generation:

```text
TargetAllocation {
    owner_rank: u8,
    local_page_id: u32,
    allocation_generation: u64,
}

DraftAllocation {
    owner_rank: u8,
    local_page_id: u32,
    allocation_generation: u64,
}
```

Generation zero is invalid. Reallocating the same local page ID increments
its generation before the ID becomes visible in a page-table delta. Overflow
is fatal. A copy, graph, or transfer descriptor names the complete allocation,
never only the local page ID.

### Durable content revision

`TierRecord::generation` is renamed conceptually to
`durable_revision: u64`. It orders durable revisions of one content-addressed
page key:

- exact deduplication retains the existing revision;
- a target-only record may be upgraded to an MTP-capable record at the next
  revision;
- an MTP-capable record is never downgraded; and
- unrelated HBM allocation reuse does not change a durable revision.

The durable revision is checked during catalog publication and restore. A
restored destination receives new HBM allocation generations; it does not
reuse the source durable revision as an HBM ABA guard.

`PageAttachments` and the pending page-table ABI must therefore carry
allocation generations separately from the restored source revision. The
current single-generation interpretation cannot be promoted unchanged.

## Content identity and incremental token ledger

The existing namespace and page-key hashes remain:

```text
namespace = H(model revision,
              tokenizer,
              chat template,
              weight policy,
              target/indexer/draft record ABIs,
              RoPE parameters)

page_key = H(namespace,
             parent_page_key or zero,
             valid_token_count = 64,
             token_ids[0..64])
```

DCP owner, writer rank, HBM IDs, allocation generations, graph ID, and a
byte-preserving kernel revision are excluded from the content key.

Each active sequence owns a bounded `CommittedTokenChain`:

```text
sequence_id
next_page_ordinal
parent_page_key or zero
tail_token_count: 0..63
tail_token_ids: [u32; 64]
```

Restored full pages initialize the parent key and next ordinal. Committed
prompt and generated tokens append exactly once after four-rank output
consensus. Tentative MTP tokens never enter the chain. When the tail reaches
64, the coordinator derives one child key, emits a seal ticket, advances the
parent/ordinal, and clears the tail.

This retains at most 256 token bytes per active sequence instead of keeping a
one-million-token vector solely for prefix publication. The prefill payload
still follows the separate `StepInput` retention contract.

## Seal ticket

After a step and its page-table commit succeed, the coordinator may emit a
canonical `SealTicket.v1`:

```text
sequence_id
page_ordinal
namespace
parent_page_key
page_key
token_ids[64]
target_allocation
target_indexer_allocation_generation
optional draft_allocation
optional draft_indexer_allocation_generation
sequence_table_generation_after_commit
mtp_capable
ticket_hash
```

The ticket is eligible only if:

- the page contains exactly 64 committed positions;
- the active table reports `HBM_SEALED`;
- no tentative position is present;
- target and target-indexer allocation generations match;
- an MTP ticket has both draft allocations at the same commit generation;
- the owner is `page_ordinal mod 4`;
- all four ranks acknowledged the commit and page-table delta digest;
- the token block derives the stated key from the stated parent; and
- no publication lease already exists for that allocation generation.

The globally ordered ticket list and its digest are rank-invariant. Only the
owner rank performs the payload copy. A rank-local decision may not change
which ticket or allocation is copied.

One step can seal multiple pages during prefill. The maximum ticket count is
derived from the admitted graph's prompt-row bound plus at most one prior
partial tail per row and is preallocated; it is not a growable hot-path
vector.

## Publication lease and ABA safety

Accepting a ticket acquires a publication reference on every named HBM
allocation. The reference is independent of active sequence and prefix
references.

An allocation cannot be freed, quarantined ID cannot be released, and a local
page ID cannot be reused until:

1. its final device read is ordered after the committing graph;
2. the device-to-host copy completes or is explicitly abandoned before
   submission;
3. the publisher records a terminal outcome; and
4. the owner rank acknowledges lease release in a later page-table or
   cache-only generation.

Request cancellation or normal termination does not revoke an accepted
publication lease. A ticket rejected before lease acquisition has no effect
on request correctness and may be counted as a skipped cache candidate.

## Orthogonal state machines

The active physical page remains `HBM_SEALED` while it is copied. Publication
uses a separate state machine:

```text
DISCOVERED
  -> LEASED
  -> COPY_SUBMITTED
  -> HOST_READY
  -> DURABLE
  -> CATALOG_VISIBLE
  -> RELEASED

DISCOVERED -> SKIPPED
LEASED | COPY_SUBMITTED | HOST_READY -> FAILED
```

The existing tier movement states continue to describe the active replica
selected for attention or restore. They do not encode the existence of a
durable replica. A page may therefore be `HBM_SEALED` while its catalog entry
is `NVME_RESIDENT`.

An accepted lease cannot transition to `SKIPPED`. Its terminal path is
`RELEASED` or engine-fatal `FAILED` for a device/integrity invariant. An
ordinary NVMe availability failure releases the lease after recording that no
new durable record became visible.

## Exact payload bytes

The owner rank produces these byte-preserving planes:

| Piece | Logical order | Bytes |
|---|---|---:|
| target KV | target layer, token, 368-byte record | 1,837,056 |
| target indexer | indexer group, token, 132-byte record | 177,408 |
| draft sidecar | token, 368-byte draft KV, 132-byte draft indexer | 32,000 |

Target-only logical bytes are `2,014,464`. MTP-capable logical bytes are
`2,046,464`.

With 4,096-byte starts for every piece, one target-only append reserves
`2,019,328` physical bytes and one MTP append reserves `2,052,096` physical
bytes, including tail and inter-piece padding. Capacity and write-endurance
accounting use physical appended bytes; transfer and payload metrics report
logical bytes separately.

The SM120 owner executor uses a fixed pack/copy descriptor and preallocated
device scratch if the kernel-facing layer-major slabs are not directly
copyable. It may not change record precision or arithmetic. Host SHA-256 is
computed over the exact staging bytes after the CUDA event completes.

## Bounded staging and backpressure

Each rank owns a fixed number `Q` of staging slots, each sized for the maximum
`2,046,464` logical bytes. Total pinned host staging is exactly:

```text
4 * Q * 2,046,464 bytes
```

For example, `Q=8` reserves `65,486,848` bytes. The configuration also fixes:

- maximum discovered tickets;
- maximum device copies in flight per rank;
- maximum host-ready pages awaiting I/O;
- maximum logical and physical NVMe bytes;
- maximum rolling physical write bytes per 24-hour window; and
- maximum catalog entries.

No device worker waits for a staging slot, filesystem operation, journal
sync, or catalog lock. Before a lease is acquired, saturation, capacity, or
write-budget exhaustion skips/defer-drops the cache candidate and increments
a bounded reason counter. It does not fail model execution.

Once a lease is acquired, the publisher must drive it to a terminal state.
New NVMe writes stop after an I/O error or budget exhaustion; already durable
records and read-only prefix hits remain valid.

## Runtime service ownership

There is one process-wide tier I/O authority:

- one journal/data append authority assigns durable revisions and offsets;
- one immutable published-catalog snapshot is shared by all restore workers;
- restore workers use independent read handles or asynchronous reads but do
  not retain private catalogs;
- a successful durable publish atomically replaces the catalog snapshot
  before any online prefix registration; and
- no rank opens or mutates the journal independently.

The implementation may later replace the first Rust I/O worker with
`io_uring` or GDS under matched evidence. The ownership and visibility
contract does not change.

## Durable transaction and visibility

For a new or upgraded page:

1. reserve physical append bytes and a durable revision under the single
   writer;
2. append and sync the `Begin` journal record;
3. write every required piece and sync data;
4. append and sync one durable-piece event per piece;
5. append and sync `Publish`;
6. update the shared published-catalog snapshot;
7. register the record with the owner-rank residency manager; and
8. make the child visible in the prefix index.

Steps 1–5 use the existing all-pieces durability rule. Target/indexer alone
is a valid MTP0 record. An MTP-capable record becomes visible only with the
single combined draft sidecar.

A crash after step 5 but before step 8 is safe: startup rebuilds the shared
catalog, residency registrations, and prefix index from durable records.
Runtime registration failure leaves a recoverable durable record and queues a
bounded retry; it never exposes a nondurable index entry.

Children may copy and become durable out of order, but a child cannot become
catalog-visible until its parent is visible. Startup performs the same
parent-before-child validation. A record with a missing parent remains an
invisible recoverable orphan.

## Incremental and restartable prefix catalog

The durable catalog metadata must add:

```text
parent_page_key or zero
page_ordinal
valid_token_count = 64
advisory writer_rank
```

The journal/container version changes because the current `TierRecord` does
not carry these fields. The page key remains the primary content identity.
Writer rank is diagnostic and does not control restore ownership.

`PrefixIndex` gains two reviewed operations:

```text
insert_child(parent, token_ids[64], record)
recover_namespace(records)
```

`insert_child` derives and checks the child key, requires the parent for
nonzero ordinals, and applies no partial mutation on failure.

`recover_namespace` validates namespace, record shape, full-page count,
parent/ordinal continuity, revision monotonicity, and MTP capability without
requiring original token IDs. Lookup still derives the exact chained key from
request tokens. Duplicate keys with contradictory metadata are corruption.

## Deduplication and MTP upgrade

The single writer compares logical piece hashes before appending:

| Existing durable record | Candidate | Required result |
|---|---|---|
| none | target-only or MTP | append new revision |
| target-only | same target/indexer, target-only | exact dedup; no write |
| target-only | same target/indexer plus draft | append MTP upgrade |
| target-only | different target/indexer | fatal content collision |
| MTP | same target/indexer, target-only | exact dedup; retain MTP |
| MTP | all same pieces, MTP | exact dedup; no write |
| MTP | different target/indexer or draft | fatal content collision |

An MTP upgrade rewrites a complete three-piece generation so recovery never
combines pieces from different durable revisions.

Two concurrent candidates for the same key serialize under the writer and
produce one append or one append plus one verified dedup. A mismatch for the
same namespace/key proves nondeterministic cache bytes or corruption and is
engine-fatal. Actual graph buckets and batch shapes must therefore prove
bit-identical tier payloads for repeated identical prefixes before prefix
reuse is qualified.

## DRAM and NVMe relationship

Pinned staging is not advertised as DRAM cache capacity. A later bounded DRAM
replica may become reusable after its complete target/indexer/draft piece set
is present and checksummed, but process restart discards it.

The v1 correctness path may publish from staging directly to NVMe. This does
not close the mandatory DRAM tier or HBM-to-DRAM performance gate. Adding a
DRAM replica changes neither the content key nor durable transaction.

## Failure classification

Engine-fatal:

- ticket/key/token/owner/generation mismatch;
- HBM allocation ABA or lease violation;
- CUDA copy or event failure;
- target/indexer/draft attachment disagreement;
- same content key with different logical piece hashes;
- durable checksum mismatch;
- contradictory catalog metadata; or
- parent/ordinal corruption in a committed catalog.

Publication-local and fail-closed:

- queue or staging saturation before lease;
- configured NVMe capacity or write-budget exhaustion;
- filesystem unavailable before a durable publish;
- runtime registration allocation failure after durability; or
- request termination after a lease has safely pinned the page.

Publication-local failures preserve active HBM correctness, expose no new
prefix, release every acquired lease, and leave the service read-only if
continued writes are unsafe.

## Observability

All labels use fixed enums. Required metrics include:

- tickets discovered, leased, skipped, durable, visible, deduplicated,
  upgraded, and failed;
- skips by saturation, capacity, write-budget, and policy;
- queue depth and staging slots used/free per rank;
- pack, D2H, queue, data-sync, journal-sync, catalog, and total latency;
- HBM-to-host logical bytes;
- NVMe logical and physical bytes and write amplification;
- current durable/catalog bytes and entries;
- rolling write-budget used/remaining;
- collision, checksum, orphan, retry, and read-only-transition counts; and
- publication leases and oldest lease age.

These host and transfer metrics are not kernel or model throughput evidence.

## Required adversarial and CPU gates

Adversarial review must resolve the generation split, durable metadata
version, parent visibility, duplicate mismatch policy, and publication-local
failure classification before implementation.

After acceptance, the CPU proof must cover:

1. every seal-ticket field and invalid combination;
2. exact target-only/MTP logical and physical byte arithmetic;
3. allocation reuse before and after lease release;
4. all deduplication/upgrade matrix cells;
5. two concurrent writers for the same key;
6. target-only, MTP, and target-to-MTP restart recovery;
7. crash points before/after every journal sync and catalog update;
8. out-of-order child completion and missing-parent recovery;
9. incremental insertion on branches and a 16,384-page chain;
10. shared live-catalog visibility from every restore worker;
11. cancellation, sequence removal, and engine shutdown with copies in flight;
12. queue, staging, byte-capacity, catalog-capacity, and rolling-write limits;
13. DRAM/NVMe corruption and retry containment;
14. rank-invariant ticket digests and wrong-owner rejection; and
15. repeated-prefix payload identity across every qualified graph bucket.

Only after the CPU gate may CUDA transfer descriptors and pack kernels enter
SM120 qualification. Device acceptance additionally requires actual records,
CUDA-event ordering, profiler evidence, overlap measurements, and cold/warm
prefix replay through a checkpoint.

## Dependencies

- the allocation-generation and lease release enter the pending active
  page-table transaction contract;
- the rank-invariant seal-ticket digest enters `StepInput`/commit consensus;
- the durable record metadata change requires a new journal/container
  version and a fail-closed migration decision;
- online catalog sharing replaces the current private restore-worker maps;
  and
- publication metrics extend `serving-observability-v1`.

No part of this document authorizes cn4 access, a GPU launch, or a production
prefix-cache claim.
