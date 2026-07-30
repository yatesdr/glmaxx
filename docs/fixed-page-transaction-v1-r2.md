# Fixed-capacity page transaction v1 r2

Date: 2026-07-30

Status: corrective design candidate; adversarial review required before
implementation

GPU claim: none

## Amendment scope and precedence

This document is a narrow normative amendment to
`docs/fixed-page-transaction-v1.md`. The base document remains normative
except where this amendment explicitly replaces it.

This amendment closes the two major findings in the first adversarial review:

1. an ordinary rank-mirror delta must not clone or validate the complete
   active mapping; and
2. a first install must not place a 16,384-entry sequence mapping in the
   174-entry compute-step staging area.

It also resolves every minor and question from that review. It does not
accept or implement `PageTableDelta.v2`, the coordinated execution ABI,
CUDA-visible tables, device hashing, cache payload movement, checkpoint
execution, model output, quality, capacity under live allocations, or
performance.

Where the older serving-page contract permits a complete-mapping resend or
requires a compute-plan `CACHE_ONLY` shape, this amendment supersedes those
routes:

- an ordinary production step accepts only a generation-proven bounded
  suffix delta;
- first install uses the bounded inactive-slot stream below;
- rebuilding an existing mapping uses acknowledged removal followed by a new
  admission and never a request-local resend fallback; and
- cache-only metadata work uses the standalone `ApplyDelta` transaction
  family, never a model `StepPlan`.

The production route is selected once, identically for the coordinator and
all four ranks, before health. The retained clone oracle is test-only. No
rank, request, tenant, or operation may choose between the oracle and
production routes.

## Frozen bounds and acceptance order

The base capacities remain:

```text
MAX_ACTIVE_SEQUENCES                 = 64
PAGE_TOKENS                          = 64
MAX_CONTEXT_PAGES_PER_SEQUENCE       = 16,384
MAX_OWNER_PAGES_PER_SEQUENCE         = 4,096
MAX_PREFILL_QUERY_ROWS               = 3,072
MAX_STEP_ROW_UNDOS                   = 64
MAX_STEP_PREFILL_PAGE_EDITS          = 174
MAX_STEP_DECODE_PAGE_EDITS           = 128
MAX_STEP_RETIREMENTS                 = 64
MAX_INSTALL_CHUNK_PAGE_ENTRIES       = 174
INSTALL_PIPELINE_SLOTS               = 8
```

`MAX_OWNER_PAGES_PER_SEQUENCE` is exact because owner rank is
`logical_ordinal mod 4`.

The 174 bound is conditional on acceptance of the 3,072-row prefill control
in `docs/prefill-graph-profile-abi-v2.md`. Acceptance and implementation
order is:

```text
prefill graph-profile ABI v2 design
  -> this fixed-page design
  -> fixed-page CPU implementation
  -> coordinated execution/page ABI implementation
```

Any change to `PAGE_TOKENS`, `MAX_ACTIVE_SEQUENCES`,
`MAX_PREFILL_QUERY_ROWS`, the maximum per-row reservation, or owner
partitioning must:

1. independently rederive the 174 and 128 maxima;
2. change the named constants rather than relying on a prose exception;
3. update compile-time assertions in the fixed implementation; and
4. pass a new adversarial review before production health.

The coordinated v3 proposal can reserve eight successor positions per row.
Eight positions still touch at most one existing tail and one new page per
row, so the 128 decode/verify edit bound remains exact and sufficient.

## Startup-owned production storage

Before any address or health state is published, the coordinator and every
rank allocate and fault in:

1. 64 fixed active/inactive sequence metadata slots;
2. a fixed page-index block arena sized by the adopted
   `SystemMemoryPlan`, with 64 page entries per block;
3. a fixed block-directory capacity of 256 host blocks and 64 owner-local
   blocks per sequence slot;
4. arena-indexed target and draft reverse-owner arrays, including active,
   reserved, and quarantine state plus arena generation;
5. the target and draft allocation/free/quarantine bitmaps;
6. a fixed request-ID-to-sequence-slot index and a fixed prefix-key index;
7. 64 row undo slots, 174 page edit/undo slots, 128 decode/verify edit
   slots, 64 rejected-page retirement slots, and 64 sequence-update
   descriptors;
8. the canonical delta, receipt, touched-page, and rollback buffers for each
   physical step;
9. a fixed pool of admission transaction descriptors, one for every
   simultaneously permitted admission;
10. for each admission transaction, eight global chunk slots of 174 entries
    and the corresponding fixed owner-local projections;
11. fixed incremental state-commitment nodes and scratch paths described
    below; and
12. fixed command, completion, and receipt queues for the coordinator and
    four rank owners.

The page-index arena is a startup allocation, not
`64 * MAX_CONTEXT_PAGES_PER_SEQUENCE` committed storage. Admission reserves
the exact number of already allocated blocks required by its declared final
page count. The admission permit owns those blocks and one inactive sequence
slot until commit or acknowledged abort. The same rule applies independently
to the host global index and each rank's owner-local index.

Consequently, a legal one-million-token sequence reserves:

```text
host blocks       = ceil(16,384 / 64) = 256
rank-r blocks     = ceil( 4,096 / 64) = 64, for each r in 0..4
```

The adopted plan fixes total block-arena capacity. Insufficient blocks or an
inactive-slot shortage rejects admission before restore/install begins. No
block vector, block directory, reverse index, hash tree, map, staging buffer,
or queue may grow after health.

The fixed request and prefix indexes use generation-tagged slots. Collision
handling and tombstones consume only their startup capacity. Probe exhaustion
is a pre-health sizing failure or a fail-closed runtime invariant; it cannot
fall back to a `BTreeMap`, heap allocation, or rank-local policy.

## Sequence-slot identity

Every production ordinary apply, active install commit, and removal commit
binds:

```text
request_id
sequence_slot in 0..64
sequence_slot_generation != 0
page_table_generation_before
page_table_generation_after
```

The coordinator assigns the inactive slot and its next nonzero generation.
All four ranks receive the same binding. A slot-generation mismatch,
request-ID collision, active destination on first install, inactive
destination on ordinary apply, zero/wrapped generation, or disagreement
between ranks is fatal before mutation.

The slot binding is part of the canonical operation digest. It does not rely
on rank-local hash-table placement and cannot be changed after a chunk has
been accepted. Install begin/chunk/abort records bind a separate nonzero,
nonreused admission-operation ID while the slot is inactive; only the active
install commit binds a global page-table predecessor and successor.

## Bounded ordinary mirror application

### Prohibited operations

For reservation, commit, and rollback of a physical step, production mirror
application must not:

- clone a mirror, sequence, prefix, page list, or reverse-owner table;
- iterate an unchanged prefix;
- iterate another active sequence;
- rebuild physical- or draft-ID maps;
- validate all live pages;
- allocate, resize, rehash, compact, or sort;
- send a complete mapping; or
- fall back to the retained `PageTableMirror::apply` oracle.

An ordinary update carries at most 64 sequence descriptors and the appropriate
174- or 128-entry changed-page bound. Removed sequences are not legal in an
ordinary physical-step delta.

### Preflight

Each rank preflights only:

1. the operation and predecessor generation/digest;
2. the exact active slot/request/slot-generation binding;
3. the sequence's current metadata;
4. `first_changed_ordinal <= page_count_before`;
5. `first_changed_ordinal <= page_count_after`;
6. the equality
   `changed_count = page_count_after - first_changed_ordinal`;
7. `old_suffix_count = page_count_before - first_changed_ordinal`, and both
   `old_suffix_count` and `changed_count` fit the phase's 174/128 bound;
8. the existing boundary record at `first_changed_ordinal - 1`, when one
   exists;
9. each changed or truncated page record;
10. each changed record's deterministic owner, arena bounds, target/draft
   coupling, state, and valid ends;
11. target/draft reverse-owner cells for the removed and replacement IDs;
12. fixed touched/undo/hash-path capacities; and
13. the exact resulting sequence metadata and incremental state commitment.

The trusted predecessor consists of the rank's active slot metadata,
generation, and previously acknowledged logical-state root. Preflight may
read the old suffix that this operation removes, but no other page. A
shrink-only rollback is legal:

```text
first_changed_ordinal == page_count_after
changed_count == 0
page_count_after < page_count_before
```

It validates and clears only the removed suffix.

Every fallible check completes before the first write. Application then has
an infallible, fixed-count write schedule. If an implementation cannot make
the post-preflight writes infallible, it must journal each touched cell before
the write and use the fixed undo area; it may not clone the table. One
page-operation slot covers the old and optional replacement value at one
ordinal, so storage is bounded by
`max(old_suffix_count, changed_count)`, not their sum.

### Incremental invariants

The following invariants are maintained from the accepted predecessor and
the touched records:

- ordinals are consecutive and owner rank remains `ordinal mod 4`;
- all non-final materialized pages are full;
- target/draft valid ends and page states match the sequence metadata;
- only private HBM state may contain a one-ahead draft sidecar;
- target and draft local IDs are in the adopted arenas;
- every target/draft ID has exactly one compatible reverse-owner binding;
- a replacement first releases only the predecessor binding named by the
  same slot and ordinal;
- unchanged prefix records and reverse-owner bindings remain immutable;
- the new page count and materialized/draft/reserved ends are exact; and
- the active sequence count remains at most 64.

The arena-indexed reverse-owner arrays make collision validation O(touched
pages). An ID cell contains at least slot, slot generation, logical ordinal,
page state, and arena generation. Reuse requires the existing
generation-bound quarantine receipt; an ordinary apply cannot manufacture a
free cell.

### Logical-state commitment

The production CPU mirror maintains a canonical incremental commitment to
the complete logical state. This is distinct from the incoming global and
rank-delta digests.

Each rank owns, for every sequence slot, a fixed 4,096-leaf binary tree over
owner-local ordinal `logical_ordinal / 4`. The host oracle comparison uses
the corresponding fixed 16,384-leaf tree. Inactive/absent leaves have
domain-separated canonical hashes. A present leaf hashes:

```text
TABLE_HASH_DOMAIN
rank_or_host_domain
sequence_slot
sequence_slot_generation
request_id
logical_ordinal
target_local_page_id
draft_local_page_id_or_absent
target_state
draft_state
target_valid_tokens
draft_valid_tokens
target_arena_generation
draft_arena_generation_or_absent
```

Internal nodes hash a domain tag, tree level, left child, and right child.
The sequence root hashes its page-tree root plus the complete sequence
metadata. A fixed 64-leaf top tree hashes sequence-slot roots in slot order;
the final table root hashes that top root and the page-table generation.
Every integer uses canonical little-endian bytes and every enum/absence value
has one frozen encoding. The CPU implementation must freeze these domain
bytes, field widths, enum codes, and absent encodings in a separately
reviewable codec before they can enter a production receipt.

An ordinary apply rewrites only:

- leaves for changed or truncated owner-local pages;
- their fixed-depth paths;
- the sequence metadata/root; and
- the fixed-depth path to the rank table root.

It never recomputes an unchanged leaf or scans a complete sequence. The
expected successor root is coordinator-bound in the operation and returned
in the rank receipt. A CPU implementation must also provide an independent
full recomputation oracle used only in tests, startup adoption, and explicit
offline validation.

This logical root proves deterministic transaction maintenance. It does not
claim to detect an arbitrary post-hash bit flip in untouched CUDA memory and
does not satisfy the pending coordinated ABI's full-device-memory hash gate.
That CUDA integrity and performance contract remains a later GPU design and
qualification item.

### Touched-work accounting

Every production apply returns counters for:

```text
sequence_descriptors_read
page_records_read
page_records_written
reverse_owner_cells_read
reverse_owner_cells_written
commitment_nodes_read
commitment_nodes_written
undo_cells_written
```

The counter increments at the primitive accessor boundary, not at the caller,
so a hidden validation scan is visible. Tests perform the same one-page and
two-page suffix updates at 64, 65, 524,288, and 1,048,576 prior positions.
For equal touched-page shapes, every counter must be identical regardless of
context length. A mutation that calls full-table validation, reads one
unchanged prefix page, or visits another sequence must fail this test even if
it performs zero heap allocations.

The implementation also retains allocator instrumentation proving no
post-health heap growth. Touched-work and allocation proofs are both
mandatory; neither substitutes for the other.

## Bounded first-install stream

### Ownership

Admission owns:

- one admission-operation quota permit;
- one inactive host slot and the same inactive slot on all four ranks;
- the exact host and rank page-index blocks reserved from startup arenas;
- all target/draft physical reservations and prefix/residency leases;
- one fixed eight-slot chunk pipeline; and
- its request, slot, generation, and final-mapping digest until terminal
  commit or acknowledged abort.

No full mapping exists in a temporary rank buffer. Chunks are written
directly into the reserved inactive final page-index blocks and incremental
commitment tree. The prepared sequence root is kept outside the active
64-slot top tree until commit, so streaming does not alter the active
page-table root or generation. An inactive slot is absent from request
lookup, prefix lookup, attention, allocation visibility, scheduler state,
and cache publication.

### Begin

`InstallSequenceBegin` binds:

```text
request and sequence-slot identity
nonzero nonreused admission-operation ID
active page-table generation observed at begin
target-only or configured MTP posture
materialized_target_end
draft_prepared_end
reserved_end
final global page count <= 16,384
exact owner-local count for each rank <= 4,096
exact host/rank block reservations
expected ordered-entry stream digest
expected prepared host and rank sequence roots
expected chunk count = ceil(page_count / 174)
```

All arithmetic, quota ownership, block availability, physical-ID
reservation, prefix capability, and generation checks occur before begin is
sent. A rank writes only inactive transaction metadata after accepting begin.

### Chunks

`InstallSequenceChunk` contains:

```text
operation identity and slot generation
strictly increasing chunk ordinal
first logical page ordinal
1..174 consecutive canonical page entries
chunk digest
```

The empty-sequence case has zero chunks and is represented only by begin and
commit.

Every rank receives and verifies the same global chunk. It updates the
streaming ordered-entry digest, validates global ordinal/owner/state shape,
projects only its owner-local records, validates its arena reverse-owner
cells, and writes those records directly to the inactive final blocks.

The coordinator may have at most eight unretired chunks for one install.
Chunk slots are reused only after all four ranks return the exact chunk
ordinal and digest. Rank processing remains ordinal even if transport
completions arrive out of order. Pipeline saturation backpressures admission;
it never borrows compute-step buffers or allocates.

Duplicate, skipped, overlapping, reordered, wrong-owner, wrong-slot,
wrong-generation, wrong-digest, over-count, or post-terminal chunks fail the
whole admission transaction. There is no complete-delta fallback.

### Commit and publication

`InstallSequenceCommit` is legal only after:

1. every declared global page is consumed exactly once;
2. every owner-local count is exact;
3. the ordered-entry stream digest matches begin;
4. every page and reverse-owner binding is valid;
5. the prepared incremental sequence roots match begin; and
6. all chunk slots have exact four-rank receipts.

Multiple inactive installations may stream concurrently because their
operation IDs, slots, blocks, physical reservations, and prepared roots are
disjoint. Active publication is serialized. At commit, the coordinator
acquires the one page-table publication lane, binds the exact current active
generation/root as predecessor, derives its nonzero successor and the four
successor roots with this prepared slot inserted, and only then sends the
commit.

Each rank verifies that exact active predecessor, atomically changes its one
slot from `Installing` to `Active`, advances the page-table generation, and
returns the exact active-slot and successor-root receipt. Cross-rank
publication is a coordinator transaction, not a claim of hardware-atomic
four-GPU memory:

1. the coordinator sends the same commit to all four ranks;
2. it requires the exact rank set, generation, slot binding, and roots;
3. only then does it publish the host active slot, scheduler row, prefix
   lease, sampling state, prompt state, and admitted event.

If a failure occurs before any rank commits, the coordinator sends
`InstallSequenceAbort`; exact four-rank abort receipts clear inactive entries,
reverse-owner reservations, roots, and block ownership before physical IDs or
permits can be reused. A missing abort receipt, a partial rank commit, or a
divergent receipt retires the worker generation and keeps every uncertain
physical ID and block unavailable. Host state is never published for a
partial install.

## Existing-sequence rebuild and restart

Production never resends a complete active mapping because a suffix cannot be
proven. Generation, slot, length, predecessor-root, or suffix disagreement is
fatal for the current worker generation.

A deliberate rebuild is:

```text
acknowledged standalone removal
  -> slot-generation advance
  -> fresh bounded first-install admission
```

It cannot be selected as a rank-local or request-local recovery route.

Worker restart begins with an empty inactive table and generation zero. Its
startup coordinator may replay admitted sequences only through the same
bounded install stream before health. Startup replay has separately charged
admission slots and block ownership and cannot use a full-mapping delta.

## Compute transaction state correction

The base state machine is replaced by:

```text
Empty
  -> Reserved { generation, delta_digest }
  -> Executed { rank_receipts }
  -> Committed { successor_generation }
  -> Published

Reserved -> RolledBack { successor_generation }
Executed -> RolledBack { successor_generation }
any receipt/invariant failure -> WorkerGenerationRetired
```

The coordinator's page preflight through capacity and collective owner free
counts is read-only. Exact physical-ID reservation is the first journaled,
reversible mutation. Every later mutation is either proven infallible by that
preflight or writes its undo cell first.

## Terminal removal and cache-only work

Terminal removals never consume the 174/128 page-edit areas or the 64
rejected-page retirement slots. All zero-reference target/draft IDs enter the
arena-sized generation-bound quarantine owned by the standalone removal
transaction.

Production does not combine terminal removal with a compute-step successor,
even when cancellation is observed during execution. The physical step first
commits or rolls back. A following standalone `ApplyDelta` removal then:

1. preflights the complete sequence and prefix-reference decrements;
2. binds the active slot and predecessor root;
3. removes the request from active lookup;
4. walks only that sequence's owner-local final blocks on each rank;
5. clears its reverse-owner cells and commitment leaves;
6. advances generation and returns exact four-rank roots/receipts;
7. permits host destructive removal only after all receipts; and
8. releases quarantine only against that acknowledged successor.

This operation is O(pages in the terminal sequence), but it is not a physical
compute-step apply and allocates no sequence-sized staging or undo buffer.
Shared prefix pages remain live while referenced. A 16,384-page removal
cannot overflow a compute journal because it never enters one.

`ApplyDelta` here names the standalone page-control transaction family, not
the retained `PageTableDelta.v1` owned-vector implementation and not a
`CACHE_ONLY` model phase. The pending production device ABI must provide
stream-visible table update and receipt semantics without constructing a
graph, running model kernels, or entering a collective. Until that ABI is
implemented and accepted, the existing CPU command is evidence only.

## Required CPU proof

The accepted implementation must compare the fixed production route with the
retained clone oracle while keeping the route process-global and
startup-fixed. In addition to the base matrix, it must cover:

1. exhaustive 174/128/64 capacity rederivation using the accepted 3,072-row
   and eight-successor controls;
2. all tail occupancies 0..63 at C1 and C64;
3. equal one- and two-page suffix changes at 64, 65, 524,288, and
   1,048,576 prior positions with identical touched-work counters;
4. mutations that add a full clone, full validation, one unchanged-prefix
   read, another-sequence read, dynamic allocation, or complete resend, each
   of which the gate must distinguish and reject;
5. grow-only, tail-rewrite, grow-plus-tail, shrink-plus-tail, and explicit
   shrink-only rollback deltas;
6. target-only and MTP-capable records, including separate target/draft valid
   ends and one-ahead private draft state;
7. every fallible preflight point and every bounded undo point;
8. reverse-owner collision, stale arena generation, wrong deterministic
   owner, and early quarantine reuse;
9. incremental-root equality with independent full recomputation after every
   operation, plus leaf/internal/domain/endianness mutations;
10. admission page counts 0, 1, 64, 65, 173, 174, 175, 4,096, 4,097, and
    16,384;
11. chunk pipeline depths 1 and 8, slot reuse only after four receipts, and
    duplicate/skipped/reordered/overlapping/wrong-digest chunks;
12. failure and cancellation at begin, every chunk boundary, final chunk,
    each rank commit, host publication, and abort;
13. exact block, quota, physical-ID, prefix-lease, inactive-slot, and chunk
    ownership after success and each failure;
14. partial rank commit and missing abort receipt causing worker retirement
    without allocator-visible uncertain IDs;
15. restart replay through bounded chunks only, including a 1M mapping;
16. process-global route mutation tests that attempt rank-, request-,
    tenant-, and operation-local oracle selection;
17. empty, private, shared-prefix, MTP, and 16,384-page standalone removals;
18. cancellation during execution proving compute completion precedes the
    standalone removal and terminal IDs never enter step retirement slots;
19. request/prefix fixed-index collision, tombstone, probe exhaustion, and
    slot-generation wrap;
20. exact byte-equivalent successor snapshots/deltas against the retained
    oracle; and
21. allocation instrumentation proving zero post-health growth in step,
    install, abort, restart replay, and removal paths.

The proof records the touched-work counters and allocator counts per case,
not only pass/fail totals.

## Acceptance boundary

Acceptance permits implementation of:

- fixed host and rank CPU page-index arenas;
- bounded suffix-only production mirror application;
- arena-indexed reverse ownership;
- incremental logical-state roots;
- bounded streamed first install and restart replay;
- standalone terminal removal; and
- the required independent CPU/mutation proofs.

Acceptance does not accept the current clone implementation as production,
does not accept a full-mapping resend, and does not accept any CUDA table,
device-memory hash, stream event, graph, collective, KV payload, tier,
checkpoint, model, quality, capacity, latency, or throughput result. It does
not authorize cn4 access.
