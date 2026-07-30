# Online prefix publication v1 r2 amendment

Date: 2026-07-30

Status: corrective design candidate; adversarial review required before CPU
implementation

GPU evidence: none

## Purpose and precedence

This amendment closes the four MAJOR findings and the bounded follow-up
findings in `docs/reviews/fable-online-prefix-publication-v1.md`.

It amends `docs/online-prefix-publication-v1.md`; unchanged clauses in that
document remain normative. Where the two documents differ, this amendment
controls.

The on-disk authority, catalog, journal, checkpoint, relocation, deletion,
and recovery bytes are defined by `docs/direct-tier-durable-format-v1.md`.
This amendment does not create a second durable format. It defines the
runtime boundary that produces publication transactions and installs their
visible prefix records. The durable-format review and this review must both
pass before the online publisher is implemented.

No clause authorizes cn4 access, a GPU launch, or a production prefix-cache
claim.

## Closed findings

| Prior finding | Resolution |
|---|---|
| MAJOR-1 | Lease acquisition is one atomic live-allocation compare-and-acquire operation with an exact retired-versus-live mismatch matrix |
| MAJOR-2 | Sequence fork copies the committed-token cursor and partial tail byte-for-byte |
| MAJOR-3 | Runtime catalog snapshots use a structurally shared fixed-depth sparse Merkle tree; publication never clones or scans the estate |
| MAJOR-4 | Prefix namespace v2 explicitly binds target KV, target indexer, and combined draft-sidecar record ABIs |

The remaining sections also close the ten MINOR findings and answer all four
review questions.

## Prefix namespace v2

The original statement that the existing namespace remains unchanged is
withdrawn. The 132-byte target-indexer record and the combined 500-byte draft
record are independently behavior-bearing cache ABIs and must be explicit
namespace inputs.

`NamespaceInputs.v2` is:

```text
model_revision_sha256:            [u8; 32]
tokenizer_sha256:                 [u8; 32]
chat_template_sha256:             [u8; 32]
weight_policy_hash:               [u8; 32]
target_kv_record_abi_sha256:      [u8; 32]
target_indexer_record_abi_sha256: [u8; 32]
draft_sidecar_record_abi_sha256:  [u8; 32]
rope_parameters_sha256:           [u8; 32]
```

Every field must be nonzero. The draft-sidecar ABI digest covers the
token-major concatenation of the 368-byte draft KV record and 132-byte draft
indexer record, including field order, byte order, and padding rules. It is
not merely a digest of the draft KV portion.

The namespace is:

```text
SHA256-D(
  "glmaxx.prefix-namespace.v2\0",
  model_revision_sha256 ||
  tokenizer_sha256 ||
  chat_template_sha256 ||
  weight_policy_hash ||
  target_kv_record_abi_sha256 ||
  target_indexer_record_abi_sha256 ||
  draft_sidecar_record_abi_sha256 ||
  rope_parameters_sha256
)
```

`SHA256-D` means ordinary SHA-256 over the domain bytes followed by the
listed fixed-width fields without separators.

The page-key domain becomes `glmaxx.prefix-page.v2\0` and otherwise retains
the v1 construction:

```text
SHA256-D(
  "glmaxx.prefix-page.v2\0",
  namespace ||
  parent_page_key_or_zero ||
  valid_token_count_u16_le ||
  token_id_0_u32_le || ... || token_id_63_u32_le
)
```

Online publication accepts only full 64-token pages, so
`valid_token_count_u16_le` is exactly 64. The field remains hashed to keep
the page-key definition explicit.

Namespace v1 and page-key v1 are unsupported by the new durable store.
There is no online migration because the former estate cannot prove the
missing indexer ABI identity. Open fails closed and the cache is rebuilt.
This intentionally amends the namespace reference in
`docs/direct-tier-durable-format-v1.md`; it does not change any durable record
size or offset because namespace and page key remain 32 bytes each.

## Committed-token cursor and fork

The coordinator owns:

```text
CommittedTokenCursor.v1 {
    next_page_ordinal: u64,
    parent_page_key_or_zero: [u8; 32],
    tail_token_count: u8,       // stored range 0..=63
    tail_token_ids: [u32; 64],  // entries at and above count are zero
}
```

The sequence ID is the map key and is not part of the copied value. A cursor
with `tail_token_count == 64` is never stored.

Appending a committed token performs these operations atomically with the
four-rank output commit:

1. write the token at `tail_token_count`;
2. if it is not the 64th token, increment the stored count and stop;
3. if it is the 64th token, derive and emit the full-page seal ticket,
   advance parent and ordinal, zero the entire tail, and store count zero.

A committed batch, including an MTP acceptance batch of at most seven
emitted tokens, repeats that operation in token order. It may emit multiple
pages and carries the final remainder into the zero-padded tail. Integer
overflow, nonzero tail padding, an ordinal inconsistent with the parent, or
a ticket-capacity overflow is engine-fatal before state publication.

`fork_sequence(source, destination)` copies the source cursor byte-for-byte
after the source sequence-table generation is stabilized and before the fork
becomes visible. The destination receives the same parent, ordinal, partial
tail, and zero padding. The destination sequence ID is distinct.

After a fork:

- identical continuations derive identical child keys and exact-deduplicate;
- divergent continuations derive distinct keys; and
- neither branch mutates the other's cursor.

Fork rollback removes only the destination cursor. Fork commit and cursor
creation are in the same fixed-capacity sequence-table transaction; a
visible destination without a cursor is forbidden.

## Allocation-generation ledger

Every `(owner_rank, allocation_kind, local_page_id)` has:

```text
AllocationGenerationState.v1 {
    next_generation: u64,
    live_generation: Option<u64>,
    live_sequence_id: Option<u64>,
    quarantine_generation: Option<u64>,
    retired_through_generation: u64,
    publication_lease_generation: Option<u64>,
}
```

`allocation_kind` distinguishes target KV, target indexer, draft KV, and
draft indexer even when their numeric local IDs coincide. Generation zero is
invalid. Generations increase monotonically and are never reused. Overflow
is engine-fatal.

`retired_through_generation` advances only after the exact four-rank
successor receipt releases the corresponding quarantine entry. A newer
generation cannot become live until the older generation has completed that
release. The fixed ledger survives local-page reuse for the executor
generation; it is not a durable cache record and is reset only by a complete
executor restart that also destroys every old device allocation.

## Atomic lease acquisition

One coordinator transaction evaluates every allocation named by a
`SealTicket.v1` and either acquires all publication references or acquires
none. It holds the allocation-ledger lock or equivalent single-owner
serialization across comparison and reference acquisition. No CUDA copy can
be submitted before this transaction commits.

For each named allocation:

| Observed state | Result |
|---|---|
| exact generation is live for the ticket sequence, is `HBM_SEALED`, has the expected kind/rank, and has no lease | eligible |
| exact generation already has the same canonical ticket hash leased | idempotent observation of the existing lease; do not acquire twice |
| ticket generation is at or below `retired_through_generation`, with no lease for it | `SKIPPED_RETIRED_BEFORE_LEASE` |
| ticket generation is the quarantined generation and legal sequence removal/cancellation is awaiting the successor receipt, with no lease | `SKIPPED_QUARANTINED_BEFORE_LEASE` |
| a newer live generation exists and the ticket generation is proven retired | `SKIPPED_RETIRED_BEFORE_LEASE` |
| exact generation is still live but sequence, state, kind, rank, attachment, or ticket identity differs | engine-fatal |
| exact generation is leased by a different ticket | engine-fatal |
| a newer generation exists without proof that the ticket generation retired through the legal quarantine path | engine-fatal |
| generation is absent, regresses, exceeds the ledger high-water mark, or has contradictory live/quarantine/retired state | engine-fatal |

If any allocation produces a skip, the whole ticket skips and no allocation
reference changes. If any produces a fatal result, the engine is poisoned.
Only an all-eligible result atomically installs the same ticket hash as the
lease on target KV, target indexer, and both optional draft allocations.

This matrix distinguishes the benign pre-lease cancellation race from an
ABA violation. Once any lease is acquired, generation mismatch is never a
skip.

The ticket invariant is mandatory:

```text
target_kv.owner_rank
  == target_indexer.owner_rank
  == optional_draft_kv.owner_rank
  == optional_draft_indexer.owner_rank
  == page_ordinal mod 4
```

An MTP ticket carries either both draft allocations or neither.

## Lease release and rank acknowledgement

The publisher outcome receipt is owner-produced, but it is not sufficient
to release a quarantined ID. These conditions are cumulative:

1. the owner proves the CUDA source read completed or was never submitted;
2. the publisher records a terminal lease outcome;
3. the coordinator includes that outcome digest in a rank-common
   `CACHE_ONLY` generation;
4. all four ranks acknowledge the successor page-table/cache generation; and
5. only then may quarantine release advance `retired_through_generation`.

Thus owner-only payload work and four-rank allocation reuse rules do not
conflict. A timeout or unknown owner outcome is lease-held, never free.

The lease forbids `HBM_SEALED -> INVALID` for its exact generation. Sequence
removal may remove logical visibility, but physical reuse waits for both the
publication terminal receipt and four-rank successor receipt.

## Shutdown

Graceful shutdown stops ticket discovery and applies this matrix:

| State | Shutdown action |
|---|---|
| discovered but not leased | record skipped |
| leased, copy not submitted | record released-unpublished; no device read existed |
| copy submitted | wait for the exact CUDA event |
| copy event succeeds and writer is healthy | either finish the already-reserved durable transaction or checksum then record released-unpublished |
| copy event succeeds and writer is read-only | checksum/retire the staging slot and record released-unpublished |
| copy event reports failure | engine-fatal |
| copy event does not finish by the executor shutdown deadline | process-fatal containment; do not report `CLOSED` and do not reuse the allocation |

`released-unpublished` is a terminal outcome code carried by the existing
`RELEASED` publication state; it does not install a catalog entry. The
publisher must drain or fatally contain every lease before native contexts
and pinned buffers are destroyed.

## Skip and re-offer policy

A pre-lease skip does not permanently forfeit an HBM-sealed page. The
coordinator may rediscover it while all named allocation generations remain
unchanged and live.

Re-offer is bounded by both:

- a fixed per-page next-eligible publication epoch; and
- a change in the relevant capacity/write-budget/queue epoch or expiry of a
  configured backoff.

There is at most one discovered or leased ticket per canonical ticket hash.
Re-offer uses the identical hash. It cannot create an unbounded retry queue.
Sustained skip rate and oldest unpublished sealed-page age are operational
SLOs, not model-correctness events.

## Structurally shared live catalog

The disk catalog remains the 256-shard sorted encoding in
`docs/direct-tier-durable-format-v1.md`. Its runtime snapshot is not a cloned
`BTreeMap` or a cloned vector of all entries.

The original durable-format catalog-root formula uses ordinary SHA-256 over
the concatenated entries in each changed shard. Because a publication must
name its planned post-root before `PublishBegin`, that formula would require
an O(shard entries) scan for every page and would reintroduce the exact
quadratic behavior this correction closes.

This amendment therefore changes only the logical catalog-root algorithm.
It does not change any file offset, record size, journal payload, catalog
entry, checkpoint field, or control field. The format descriptor's catalog
schema digest changes, so an estate created under the unamended candidate
fails descriptor validation.

Each catalog shard is a persistent fixed-depth binary sparse Merkle tree.
The shard is still selected by `page_key[0]`. Its 504-bit path is:

```text
namespace[0..32] || page_key[1..32]
```

The excluded page-key byte is exactly the shard ID. Bits are consumed
most-significant first within each byte.

Empty hashes are:

```text
empty[504] =
  SHA256-D("glmaxx.direct.catalog-empty-leaf.v1", "")

for depth in (0..504).reverse():
  empty[depth] =
    SHA256-D("glmaxx.direct.catalog-node.v1",
             depth_u16_le || empty[depth + 1] || empty[depth + 1])
```

A present leaf and internal node are:

```text
leaf =
  SHA256-D("glmaxx.direct.catalog-leaf.v1", catalog_entry[512])

node(depth, left, right) =
  SHA256-D("glmaxx.direct.catalog-node.v1",
           depth_u16_le || left || right)
```

An absent branch uses `empty[depth]`. Shard root is the depth-zero hash. The
amended catalog root is:

```text
SHA256-D("glmaxx.direct.catalog-root.v2",
         catalog_epoch_le ||
         for shard 0..255:
             shard_id_u16_le ||
             entry_count_u32_le ||
             sparse_merkle_shard_root)
```

The catalog epoch and shard entry counts retain the base durable-format
rules. The checkpoint's per-shard padded-payload SHA-256 remains an ordinary
file-integrity hash. On decode, the reader validates that ordinary hash,
parses and validates every sorted entry, reconstructs the sparse Merkle
shard roots, and then validates the amended catalog root. The former
`logical_shard_sha256` is no longer an input to the catalog root.

The persistent runtime representation is:

```text
CatalogSnapshot.v2 {
    epoch: u64,
    shard_roots: [Arc<MerkleObject>; 256],
    shard_entry_counts: [u32; 256],
    logical_root_sha256: [u8; 32],
}

MerkleObject =
    Branch {
        left: Arc<MerkleObject>,
        right: Arc<MerkleObject>,
        subtree_sha256: [u8; 32],
    }
  | Leaf {
        optional_entry: Option<Arc<CatalogEntry>>,
        leaf_sha256: [u8; 32],
    }
```

Canonical empty subtrees are shared by depth and allocate no per-snapshot
nodes. A leaf exists only at depth 504. Path compression and a
different-depth leaf are forbidden, so lookup/update work is independent of
estate size. The only public lookup returns an entry when its `VISIBLE` flag
is set; parent-pending leaves remain in the durable snapshot but cannot be
returned to rank or restore callers. A shard count overflow is engine-fatal
before journal publication.

Publishing, upgrading, deleting, relocating, or changing visibility:

1. validates the complete old snapshot and proposed entry;
2. allocates one replacement leaf and at most 504 replacement internal
   nodes;
3. reuses every untouched subtree by `Arc`;
4. clones the fixed 256-pointer/count top table;
5. computes the new logical root from the changed path and fixed top table;
6. atomically swaps one snapshot pointer; and
7. retires the old snapshot only after its reader epoch drains.

No operation clones or scans the catalog. Lookup and one-entry mutation
perform exactly 504 branch decisions, hence O(key bits) and O(1) with respect
to catalog entries. Computing the top root hashes exactly 256 fixed shard
descriptors. Allocation failure occurs before root swap and leaves the old
snapshot unchanged. An update uses bounded pre-reserved capacity for 505
tree objects and one top table; exhaustion stops new publications before
acquiring a lease.

Restore workers load the current snapshot once per lookup and may retain it
only for the bounded read operation. They never retain private catalogs.
Runtime root hash and entry semantics exactly match the amended durable
catalog root; the pointer representation is not serialized.

The CPU gate records node allocations and visited levels, proving that
updates at catalog sizes 1, 256, 65,536, and the configured maximum remain at
or below 505 tree objects, 504 branch decisions, and 256 top descriptors and
do not iterate untouched entries.

## Prefix lookup and residency bounds

Production does not retain a second full-estate `PrefixIndex` beside the
durable catalog. `PrefixIndex` becomes a lookup facade over the current
shared `CatalogSnapshot.v2` plus a separate bounded table of active
request/page references.

Lookup derives each chained page key from request token IDs and performs one
catalog lookup. It accepts only a visible entry with the exact namespace,
parent, ordinal, record roles, and minimum MTP capability. A miss or
parent-pending leaf terminates the prefix match without exposing the record.

The reviewed `insert_child` operation becomes the prevalidation phase of the
single authority's publication or visibility transaction. It validates
namespace, parent, ordinal, key, record relation, and reference overflow
before the one catalog snapshot swap. It may not mutate or clone a second
index. Failure performs no mutation.

`recover_namespace` validates the selected durable snapshot and constructs
only the bounded active-reference table. It does not rebuild a duplicate
estate-sized map. The current clone-and-swap `PrefixIndex` remains a
nonproduction oracle until replaced atomically with this facade.

The restore-facing residency manager contains only:

- allocated HBM pages;
- capacity-accounted DRAM pages;
- in-flight restores/demotions; and
- bounded pinned references.

It never registers the complete NVMe catalog. Its maximum entries are
derived from fixed HBM slots, fixed DRAM slots, and fixed operation slots.
Eviction candidates are maintained in an indexed generation heap with
O(log R) insert/remove/touch and deterministic `(last_use_generation,
namespace, page_key)` ordering. A scan of all durable entries or all
residency entries per restored page is forbidden.

Publishing a durable record changes the durable catalog and prefix index.
It registers residency only if the record already has a capacity-accounted
HBM or DRAM replica.

## Parent-pending records and healing

The durable store has a separate configured
`max_parent_pending_catalog_entries`, no larger than
`max_catalog_entries`. Reservation checks it before allocating an extent or
acquiring a publication lease. Pending and visible entries both consume
catalog-entry and physical-byte capacity.

An index keyed by `(namespace, parent_page_key, page_ordinal)` names pending
children. When a parent becomes visible, the writer performs deterministic
`VisibilityCommit` transactions for directly unblocked children in
`(page_ordinal, page_key)` order. Each committed child then unlocks its own
children. Work per transaction is bounded; a continuation is queued if one
parent has more than the configured visibility batch.

The same healing runs:

- after a new parent publication;
- after exact dedup finds an existing parent-pending record;
- during startup replay; and
- during an explicit bounded retry.

An identical re-offered child exact-deduplicates and may retrigger healing.
A different payload for the same key remains engine-fatal. Healing never
depends on accepting nondeterministic bytes.

Metrics add current parent-pending entries, oldest pending age, visibility
continuations, and pending-capacity skips.

## Capacity, deletion, and cleaning

Reaching catalog, parent-pending, physical-byte, or rolling-write limits
stops new publications before lease acquisition. Publication itself does
not choose victims implicitly.

The separate reviewed eviction controller may issue `CatalogDelete` under
the durable-format reference rules. A successful delete creates catalog
capacity only after its journal transaction and snapshot swap commit.
Segment cleaning and relocation reclaim physical bytes only under the
durable-format epoch and retirement rules.

MTP upgrade initially makes the old target-only extent garbage. Physical
capacity continues to count both extents until relocation and retirement
actually reclaim the old bytes. No logical catalog update may pretend those
bytes were reclaimed.

## I/O barriers and hashing budget

The v1 durable format retains individually ordered and synced Begin, data,
piece, and Commit barriers. Online publication may coalesce independent
transactions only where the durable-format contract explicitly permits it;
it may not group away an intra-transaction barrier.

The implementation has fixed publisher and SHA-256 worker counts, fixed
input/output queues, fixed buffers, and explicit CPU-time/queue-delay
metrics. A full queue causes a pre-lease skip. Hashing never runs on a native
rank owner thread or blocks model compute.

Any future piece-event group commit is a separate durable-format amendment
and fault proof. It is not assumed by this candidate.

## Failure classification r2

Engine-fatal:

- any post-lease allocation-generation mismatch;
- any live pre-lease allocation mismatch not proven legally retired;
- contradictory allocation-ledger state;
- ticket/key/token/namespace/owner/attachment mismatch;
- CUDA copy/event failure;
- same content key with different logical piece hashes;
- durable checksum mismatch;
- contradictory catalog metadata;
- parent/ordinal corruption; or
- inability to contain a submitted copy during graceful shutdown.

Publication-local:

- a ticket generation proven retired or quarantined before lease;
- queue, staging, node-reserve, catalog, pending-parent, physical-byte, or
  write-budget pressure before lease;
- filesystem failure before durable Commit;
- bounded post-Commit registration retry; and
- request termination after a lease safely pins the physical source.

After a data or journal sync failure, the writer is permanently read-only
until restart revalidates the entire selected control/checkpoint/journal
chain. Already visible records remain readable if their validation succeeds.
No later write in the same process may clear write poison.

## Revised CPU gate

After both design tokens, one CPU implementation candidate must prove:

1. all namespace-v2 inputs, domains, byte order, zero rejection, and v1
   fail-closed open;
2. every seal-ticket field, exact owner equality, and all invalid
   target/indexer/draft combinations;
3. zero-padded cursor append for batch sizes 1 through 7 across every tail
   count, multi-page prefill, ordinal overflow, and fixed ticket capacity;
4. fork before and after every tail count, identical continuation dedup,
   divergent continuation, rollback, and branch isolation;
5. every atomic lease-acquisition matrix row, including all-or-none
   multi-allocation acquisition and pre-lease cancellation skip;
6. allocation reuse only after publisher terminal plus exact four-rank
   successor receipt;
7. graceful shutdown in every publication state, including successful,
   failed, and never-completing submitted-copy events;
8. re-offer backoff/capacity epochs with at most one live ticket per hash;
9. exact target-only/MTP logical and physical byte arithmetic;
10. all deduplication and MTP-upgrade cells, including physical garbage
    accounting;
11. two concurrent candidates for one key and write-poison behavior;
12. crash points before and after every durable-format write, sync, catalog
    swap, visibility transition, delete, checkpoint, and control update;
13. out-of-order children, pending bound, chained healing, missing parent,
    retry, and restart;
14. persistent sparse-Merkle concurrent readers and mutations at 1, 256,
    65,536, and maximum entries, asserting at most 505 replacement tree
    objects, 504 branch decisions, and 256 top descriptors per operation;
15. no-partial-mutation `insert_child`, branched lookup, and a 16,384-page
    chain through the shared catalog without a duplicate full-estate map;
16. residency cardinality never exceeding fixed HBM + DRAM + operation
    capacity and O(log R) deterministic eviction operations;
17. shared live visibility from every restore worker without private maps;
18. queue, buffer, byte, entry, pending, endurance, and worker saturation;
19. DRAM/NVMe corruption, torn records, retry, write-poison, and recovery
    containment;
20. rank-invariant ticket/outcome digests and collective-safe lease release;
21. repeated-prefix byte identity across every qualified graph bucket and
    batch shape; and
22. fixed CPU/hash worker budgets under a deterministic publication burst.

The CPU candidate must use mock device events and mock durable fault points.
It authorizes neither a GPU launch nor a claim that real transfers are
asynchronous.

## Device gate retained

Only after the CPU gate may the reviewed HBM-to-DRAM descriptor and pack path
enter SM120 qualification. Device acceptance requires:

- exact target/indexer/draft record bytes from actual graph buckets;
- event-ordered source reads and lease release;
- repeated-prefix byte identity across batch shapes;
- D2H/DRAM/NVMe overlap and decode-isolation evidence;
- cold restore and warm reuse through a real checkpoint; and
- all four ranks agreeing on ticket and release generations.

If byte identity cannot be proved, prefix reuse remains disabled and the
namespace/execution-identity decision returns to design review.

## Required acceptance decisions

The adversarial reviewer must answer:

1. Does namespace v2 completely bind all three durable record roles without
   changing fixed record sizes?
2. Does the allocation-ledger matrix distinguish legal pre-lease retirement
   from masked ABA without a third ambiguous case?
3. Are lease acquisition and release all-or-none across target, indexer, and
   optional draft allocations?
4. Are fork and multi-token cursor semantics complete through page
   boundaries?
5. Does the sparse-Merkle amendment make both the runtime snapshot and the
   durable pre/post catalog-root computation O(1) in estate size and safe for
   concurrent readers?
6. Can parent-pending healing finish without an estate scan, unbounded queue,
   or visibility deadlock?
7. Do catalog, prefix-index, and residency ownership avoid duplicate
   full-estate maps?
8. Is shutdown fail-closed for every submitted-copy outcome?
9. Do separate deletion/cleaning rules avoid false capacity reclamation?
10. Is the 22-row CPU gate sufficient to begin implementation after both
    design tokens?

Withhold the implementation token for any BLOCKER or MAJOR.
