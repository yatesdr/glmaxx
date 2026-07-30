# Serving page transaction v1

Date: 2026-07-29

Status: CPU coordinator subset implemented at `f480ef1`; rank-visible delta,
fixed-capacity hot path, and device integration remain pending review and
implementation

GPU claim: none

## Purpose

The scheduler currently proves batching and rank consensus, while
`SequencePageTable` separately proves bounded physical cache metadata. This
contract joins them without allowing a CUDA rank to allocate memory, infer a
page owner, or commit KV independently.

Production `ServingCoordinator` owns exactly one active page table constructed
from `SystemMemoryPlan.v2.cache_arena`. Every target, target-indexer, draft-KV,
and draft-indexer address used by a serving step is derived from its reviewed
rank-local physical page IDs.

The retained CPU implementation now makes the table mandatory, binds exact
restored attachments at admission, reserves every selected row before worker
submission, commits rank-consensus output counts, and removes terminal rows
before releasing prefix pins. Its proof is
`docs/serving-active-page-transaction-proof-v1.md`. It is still a
clone-on-step metadata oracle and does not yet construct a rank-visible
`PageTableDelta`.

## Admission transaction

Token admission occurs in this order:

1. retain the bounded prompt token bytes;
2. restore the longest capability-compatible sealed prefix;
3. derive one immutable `PrefixPageAttachment` from every restored tier
   record, binding namespace, key, generation, target hashes, and optional
   draft-sidecar hash after residency validation;
4. preflight all active target/draft page slots;
5. attach the complete prefix to `SequencePageTable`;
6. admit the identical prompt progress into `Scheduler`;
7. advance the sequence-table generation; and
8. publish the admitted event.

Steps 4–6 are one control-plane transaction. Failure releases all active page
references, residency pins, and prompt-byte reservations and leaves neither a
scheduler row nor an admission event. A target-only restored page is never
attached to an MTP-capable sequence.

The prevalidated admission API cannot claim cached tokens without exact page
keys and validated draft capabilities once the active table is mandatory.
The retained implementation now makes that bypass crate-private and returns
an opaque `RestoredPrefix` carrying exact validated attachments rather than
caller-supplied capability booleans.

An active shared page accepts only exact target identity, retains an existing
MTP sidecar on a target-only candidate, or upgrades target-only to MTP when
the target identity is unchanged and the MTP generation is newer. A target
hash collision, stale draft claim, or changed draft hash fails atomically.
This metadata relation does not replace the required rank payload-transfer
acknowledgment.

## Step reservation

No serving step may enter the four rank workers before all rows reserve their
maximum writable positions.

| Step | Per-row reservation | Successful commit |
|---|---:|---:|
| prefill | exact scheduled prompt-token count | exact scheduled count |
| MTP0 decode | 1 target position | 1 output token |
| MTPK verify | `K + 1` target and draft-capable positions | rank-consensus output count `1..K+1` |

Near the model limit, the scheduler clamps K before reservation. No
reservation may exceed 1,048,576 committed positions.

One batch reservation is atomic across every selected sequence and all four
DCP owners. It produces:

- the prior committed position for each row;
- the exact reserved logical position range;
- every target and optional draft physical page ID touched by that range;
- valid-token counts before execution;
- the new sequence-table generation; and
- a canonical reservation digest.

The CPU subset proves the prior committed position, exact logical reservation,
bounded owner-local physical allocation, and all-row atomicity. The explicit
changed spans, reservation generation, and canonical digest are requirements
for the next rank-delta implementation, not claims about `f480ef1`.

## Rank page-table delta

Rank execution needs the complete context-page mapping for attention, not
only the pages written by the current step. Each persistent rank therefore
owns a device-resident sequence page table and applies one canonical
`PageTableDelta` before launch.

The delta contains:

```text
schema                         glmaxx.page-table-delta.v1
generation_before              u64
generation_after               u64
sequence updates               0..64
owner-local page entries       bounded by the active arena
removed sequence IDs           0..64
canonical global digest        SHA-256
```

Each sequence update carries its request ID, committed position, MTP posture,
and complete ordered `(page ordinal, owner rank, target local page ID,
optional draft local page ID, state, valid-token count)` mapping whenever it
is first attached. Later updates may carry a consecutive changed suffix only
when both host and device prove the same prior generation and sequence length.
Otherwise the complete mapping is resent.

All four ranks hash the same global delta. A rank uploads only its owner-local
entries plus the rank-invariant sequence metadata, but acknowledges the
global digest after its stream dependency makes the upload visible. The graph
launch depends on that acknowledgment.

The reservation generation and global delta digest become part of canonical
step execution input. This requires adding an explicit
`page_table_delta_digest` field to the pending `StepInput.v1` candidate, or a
reviewed equivalent binding; the current candidate cannot be promoted
unchanged. A missing, excess, stale, or wrong-owner span is fatal before any
collective.

## Commit and rollback

After four-rank output consensus:

- prefill commits its exact reserved count;
- MTP0 commits its one target result;
- MTPK commits the consensus output count and makes every rejected suffix
  unreachable before reuse;
- accepted target, draft-KV, and draft-indexer positions advance together;
  and
- the sequence-table generation advances again before another step compiles.

Commit preserves the same physical IDs that the kernel wrote. It must not
free and deterministically reacquire pages as its production mechanism.

Any worker, collective, output-shape, vocabulary, EOS-placement, or consensus
failure rolls every batch row back to its exact prior physical pages and
valid-token counts. A failed worker generation is already terminal, but the
CPU metadata still rolls back so cleanup and evidence are deterministic.

Cancellation requested during execution is applied only after the current
step commits or rolls back. Terminal cleanup removes the active sequence
before releasing the corresponding residency pins. Shared sealed pages remain
reachable by other sequences; private tails return both target and draft
slots.

Freed physical IDs are quarantined until every rank acknowledges the removal
generation. If no compute step is ready to carry that delta, the coordinator
submits a `CACHE_ONLY` page-table update with no CUDA graph or collective.
An ID may not be reused while any rank could still resolve it to the prior
sequence.

## Fixed-capacity hot-path API

The production API must not clone the full page table or loop once per prompt
token. It uses:

- one preflight pass over at most 64 rows;
- page-granular arithmetic for prefill;
- a fixed-capacity undo log bounded by the batch row and touched-page limits;
- one allocation/free set per owner rank;
- no heap allocation after graph/arena initialization; and
- no device allocation in a serving step.

The existing clone-on-error CPU oracle remains useful for proof but is not the
serving hot-path implementation.

## Generation and failure rules

The global generation advances after every visible page-table mutation:

- admission;
- batch reservation;
- commit or rollback;
- cancellation cleanup; and
- terminal removal.

The CPU subset advances one host-visible generation for admission and each
published successful or terminal mutation. It intentionally does not claim
the separate reservation/commit generations until ranks consume and
acknowledge a canonical delta.

Overflow is fatal. Ranks never choose a fallback from local capacity. If one
row or owner cannot reserve the canonical batch, the entire batch remains
unlaunched and returns to scheduling or admission according to one
coordinator decision.

The following are fatal invariants:

- scheduler progress differs from committed page-table positions;
- a prefix key is attached at a different ordinal;
- target and draft generation or valid-token counts diverge;
- a rank consumes a page it does not own;
- a commit references an unreserved token;
- a rollback changes a shared sealed prefix; or
- a rank acknowledges a different reservation digest.

## Required CPU proof

The retained coordinator now covers cold/fully/partially cached admission,
target-only and draft-capable attachment, bounded prefill/decode/verify
reservation, late capacity failure before rank submission, worker and output
failure cleanup, cancellation cleanup, accepted draft EOS, exact MTP0 and
MTP6-capable 1,048,576-position accounting, and dynamic MTP tail clamping.

Before a CUDA executor consumes the complete boundary, the remaining CPU ABI
proof must cover:

- fixed-capacity C1 and C64 undo records at MTP0 through MTP6;
- page-granular prefill without a per-token loop;
- every tail occupancy 0–63 through the serving coordinator;
- the canonical rank delta and reservation digest;
- rank acknowledgment, removal quarantine, and `CACHE_ONLY` cleanup; and
- generation/digest disagreement across four rank consumers.
