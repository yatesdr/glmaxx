# Fixed-capacity page transaction v1

Date: 2026-07-29

Status: design candidate; adversarial review required before implementation

GPU claim: none

## Purpose

The retained `SequencePageTable` is a correctness oracle. It clones
`BTreeMap`, `BTreeSet`, and per-sequence `Vec` state before mutations, and
`PageTableDelta::between` normalizes complete active snapshots. That is not a
production serving hot path at 64 concurrent rows or a 1,048,576-token
sequence.

This contract replaces compute-step cloning with one preallocated
page-granular transaction journal. It preserves the generation, quarantine,
four-rank receipt, and fail-stop rules already implemented by
`StepInput.v1`, `PageTableDelta.v1`, the persistent four-rank mirrors, and
the generation-bound target/draft reuse quarantine.

Admission, terminal removal, and compute-step mutation have different bounds
and must not be hidden behind one allocation-prone generic transaction.

## Frozen production bounds

```text
MAX_ACTIVE_SEQUENCES       = 64
PAGE_TOKENS                = 64
MAX_MTP_DEPTH              = 6
MAX_VERIFY_TOKENS_PER_ROW  = 7
MAX_PREFILL_QUERY_ROWS     = 3072
MAX_STEP_ROW_UNDOS         = 64
MAX_STEP_PAGE_EDITS        = 174
MAX_STEP_RETIREMENTS       = 64
```

The prefill page-edit bound is exact:

```text
existing-tail plus new-page edits   <= 174
```

For one row, the worst initial posture is a mutable tail with 63 valid
tokens. Its edit count for `t >= 1` is
`1 + ceil((t - 1) / 64)`: the first token edits the tail and the second
immediately requires a new page. With `n` nonempty rows, two tokens per row
produce `2n` edits; every further edit costs 64 tokens. The maximum over
`1 <= n <= 64` and `sum(t_i) <= 3072` is therefore
`2*64 + floor((3072 - 2*64) / 64) = 174`. Empty/full-tail postures and fewer
rows cannot exceed it.

Decode and verify are smaller:

```text
existing tentative tails            <= 64
new pages for at most 7 tokens/row   <= 64
total changed page records           <= 128
```

A terminal 1M sequence may own 16,384 logical pages. It is intentionally not
placed in the 174-entry reversible step journal.

## Storage initialized before health

Every rank and the coordinator allocate these structures before production
health can be published:

1. 64 fixed row-undo slots;
2. 174 fixed page-edit/undo slots;
3. 64 fixed retirement slots for rejected speculative pages;
4. 64 fixed sequence-update descriptors;
5. 174 fixed `RankPageEntry` delta payload slots;
6. owner-rank target and draft free/quarantine bitmaps sized from the frozen
   arena;
7. active sequence slots with page-index capacity charged at admission; and
8. rank-mirror/device-table update staging with the same counts.

No compute step may grow a map, vector, queue, or page list. An impossible
capacity count is a startup/configuration error. Runtime count overflow is a
fatal invariant, not a fallback to allocation.

The CPU oracle may retain its owned containers for reference comparison.
Production serving must use the preallocated implementation explicitly; the
two types may not be selected by a rank-local decision.

## Step transaction record

One coordinator-owned `StepPageTransaction` has these states:

```text
Empty
  -> Reserved { generation, delta_digest }
  -> Executed { rank_receipts }
  -> Committed { successor_generation }
  -> Published

Reserved -> RolledBack { successor_generation }
any receipt/invariant failure -> WorkerGenerationRetired
```

Each row undo records:

- request ID and fixed row index;
- MTP posture;
- committed and tentative counts before reservation;
- page count before reservation;
- whether a mutable tail existed;
- the exact tail ordinal, target ID, optional draft ID, state, and valid
  count; and
- the first changed ordinal used by the canonical delta.

Each allocation record stores:

- row index and logical ordinal;
- owner rank;
- target local ID;
- optional draft local ID; and
- target/draft arena generation.

The journal never stores or copies KV payload bytes. CUDA writes remain
unreachable until the page-table successor is acknowledged and the compute
step is allowed to launch.

## Reservation algorithm

The coordinator performs all non-page preflight first: event capacity,
scheduler selection, graph entry, collective schedule, prompt slice bounds,
sampling fields, generation arithmetic, and transaction-slot counts.

It then performs one bounded page preflight without mutation:

1. verify every row's committed count against scheduler progress;
2. derive each row's first changed ordinal;
3. count tail edits and new target/draft allocations per owner;
4. verify all fixed journal/delta capacities;
5. verify all owner-rank arena free counts collectively; and
6. reserve exact IDs from the preallocated owner bitmaps.

Only after the complete C1/C64 batch passes does it mutate rows and physical
metadata. Every mutation writes its undo entry first. A local error replays
entries in reverse and leaves the authoritative generation unchanged.

The reservation delta is built directly from the row journal and final page
records. It must not snapshot unchanged sequences or unchanged context
prefixes. Each changed suffix begins at the journal's conservative
`first_changed_ordinal`.

## Execution, commit, and rollback

For host generation `G`:

1. reservation uses `R = G + 1`;
2. the immutable `StepInput` binds `R` and the reservation digest;
3. every rank installs `R` before executor entry and returns its existing
   exact receipt;
4. prefill treats `R` as final page state;
5. decode/verify applies commit generation `C = R + 1`; and
6. rollback after rank-visible reservation also uses a successor
   `B = R + 1`, never generation reuse.

Tentative commit changes accepted pages in place. At most one rejected new
page per row enters the generation-bound quarantine. The commit delta is
constructed from the retained row undo plus final page records.

If failure occurs before any rank installs `R`, reverse replay restores the
host table and reserved IDs directly. If all ranks installed `R`, rollback
first sends the exact `R -> B` delta and waits for all four receipts; only
then may host rollback release its quarantined IDs. Any missing or divergent
receipt retires the worker generation and its device arenas.

Scheduler state, prefix leases, prompt storage, events, and sampling/RNG
state publish only after the final page generation is acknowledged.

## Terminal removal and cache-only work

Terminal removal is acknowledge-before-destructive on the host:

1. preflight the complete sequence mapping and reference decrements
   read-only;
2. build a removal-only successor delta without copying the sequence pages;
3. apply it to all four ranks using the existing standalone `ApplyDelta`
   worker command;
4. verify all four exact receipts;
5. remove the host sequence and retire zero-reference target/draft IDs; and
6. release those IDs from quarantine against the already acknowledged
   removal generation.

The standalone `ApplyDelta` command is the production cache-only ABI. It has
no graph, model generation, CUDA compute launch, or collective. The legacy
`StepMode::CacheOnly` compute-plan shape must not be required for cleanup and
should be removed from non-test production routing after migration.

If the host's post-receipt destructive pass detects an invariant violation,
the process fails closed. It must not roll ranks back to resurrect a sequence
whose payload ownership is no longer trustworthy.

Cancellation during a compute step remains boundary-safe. Its removal may be
combined with the acknowledged commit successor when the complete removal
preflight has already passed; otherwise it uses a following standalone
delta.

## Admission

Admission is not a compute-step journal entry. It owns one bounded
`AdmissionPageTransaction` whose page-index storage is charged by the
admission permit before restore or allocation begins.

Admission:

- restores and validates exact prefix attachments;
- reserves all private and restored HBM IDs without publishing them;
- builds a full first-install delta from the request-owned page-index
  buffer;
- receives all four rank receipts; and
- atomically publishes scheduler, active-table, prefix-lease, sampling, and
  prompt state.

Failure releases the admission-owned slots because no published sequence can
resolve them. A partial rank application retires the worker generation.

## Required CPU proof

Before production integration, an independent fixed-array reference harness
must exhaust:

1. C1 and C64 at every tail occupancy 0–63;
2. MTP depths 0–6 and commit counts 1 through `K + 1`;
3. every prefill partition whose total is 1, 64, 65, 3071, and 3072;
4. the exact 174-edit prefill maximum;
5. owner-rank exhaustion on each allocation ordinal;
6. every undo failure point before rank submission;
7. reservation receipt, commit receipt, rollback receipt, and each missing
   or divergent rank;
8. terminal removal for empty, private, shared-prefix, MTP, and 16,384-page
   sequences;
9. cancellation before selection, during execution, after commit, and during
   standalone removal;
10. generation overflow and wrong-generation quarantine release;
11. allocation counters proving zero compute-step heap growth; and
12. byte-equivalent deltas and final snapshots against the retained clone
    oracle.

The integrated gate must retain the exact 1,048,576-position lifecycle,
multi-tenant batching, late publication failure, rank divergence, slow
consumer, and bounded-event regressions.

## Acceptance boundary

Acceptance of this design allows a CPU implementation of fixed-capacity
compute-step metadata. It does not accept CUDA table uploads, stream
visibility, payload arenas, tier movement, checkpoint execution, quality,
capacity under live model allocations, or performance.
