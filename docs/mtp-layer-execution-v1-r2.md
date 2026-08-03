# Recurrent MTP execution v1 r2

Date: 2026-08-03

Status: corrective design candidate; adversarial acceptance required before
CPU implementation

GPU evidence: none

## Purpose and supersession

This contract resolves the withheld review of
`mtp-layer-execution-v1.md`. The reviewer accepted the shifted teacher
lineage, successor-slot sidecar, teacher-versus-recurrent state separation,
pipelined state machine, q-state bound, and gate order. This r2 retains those
sections byte-for-byte by reference and replaces only their incomplete ABI,
budget, manifest, retry, MTP0, determinism, and terminal-session boundaries.

The operative design is the conjunction of:

- `mtp-layer-execution-v1.md` at SHA-256
  `5ad5bf01cdbd5e183b5e50aa0940344b5aabc09bf05a90c57d58e3e5b28dd3a7`;
- this corrective r2;
- `step-execution-abi-v3.md` for the complete physical step transaction;
- `target-layer-execution-v1-r2.md` for target rows, tables, buffers, and the
  `StepPlan.v4`/`StepInput.v3` successors; and
- `sm120-w4a16-nf3-fused-moe-v1-r2.md` when the resident profile is the
  ModelOpt-W4A16/NF3 hybrid.

If any retained v1 sentence conflicts with this r2 or one of those explicitly
named successors, this r2 wins. No v1/v2 object may be mixed into one running
generation.

## Closed identity set

The coordinated amendment has these exact identities:

| Surface | Required identity |
|---|---|
| engine MTP semantics | `glmaxx.engine-mtp-semantics.v1.successor-slot` |
| draft sidecar semantics | `glmaxx.draft-sidecar.v1.successor-slot` |
| operation manifest | `glmaxx.glm52.operation.v2` |
| logical MTP program | `MtpProgram.v1` from the retained design |
| hybrid physical MTP program | `MtpProgram.v2` from the W4A16/NF3 r2 design |
| step plan | `glmaxx.step-plan.v4` |
| step input | `glmaxx.step-input.v3` |
| step output | `glmaxx.step-output.v2` |
| sampling counter/ticket | `SamplingCounter.v2` / `SamplingTicket.v2` |
| page transaction | `glmaxx.page-table-delta.v2` |
| prefix publication | `PrefixPublication.v2.successor-slot` |
| session export/completion | `SessionExport.v2` / `CompletionRecord.v2` |
| MTP tail oracle | `MtpProposalTail.v2` |
| memory plan | `glmaxx.system-memory-plan.v3` |

`StepPlan.v4` retains the exact 159-byte target-layer r2 record and binds its
layout-aware target program. `StepInput.v3` retains the step-ABI's fixed
480-byte sequence records and adds the target-layer r2 430-byte prefix and
three authenticated target tables. `StepOutput.v2` and `PageTableDelta.v2`
retain their exact step-ABI-v3 encodings. This r2 closes the previously
logical-only counter/ticket fields with the 48-byte encodings below. An old
domain, unknown enum, nonzero reserved byte, or predecessor schema fails
before allocation or queue admission.

## Exact output and pipeline ABI

The old `CommittedTokens` record is not an MTP execution result. Each rank
returns one 240-byte `StepOutput.v2` sequence record. Bytes `0..136` are
process-common and bytes `136..240` are rank-local:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | request ID |
| 8 | 4 | materialized target end after |
| 12 | 4 | emitted token end after |
| 16 | 1 | emitted token count |
| 17 | 1 | materialized token count |
| 18 | 1 | accepted proposal count |
| 19 | 1 | target kind |
| 20 | 1 | residual-fallback bit |
| 21 | 1 | model-terminal bit |
| 22 | 1 | next proposal count |
| 23 | 1 | next verify-row mask |
| 24 | 28 | emitted token IDs, seven `u32` slots |
| 52 | 4 | next pending target token |
| 56 | 8 | current bundle generation |
| 64 | 8 | next bundle generation |
| 72 | 8 | RNG counter before |
| 80 | 8 | RNG counter after |
| 88 | 8 | next DRAFT ticket begin |
| 96 | 8 | next DRAFT ticket end |
| 104 | 32 | sampling trace digest |
| 136 | 4 | local proposal-state bytes |
| 140 | 32 | local proposal-state digest |
| 172 | 32 | local retained-state digest |
| 204 | 32 | local page-write digest |
| 236 | 4 | reserved zero |

Target kinds are exactly `NONE=0`, `INITIAL=1`, `MTP0=2`,
`GREEDY_MISMATCH=3`, `RESIDUAL=4`, and `BONUS=5`. The output separately
states what became target-materialized and what became client-visible.
Accepted proposals may occur in both counts; a newly sampled target token may
be emitted while remaining the one pending, nonmaterialized token.

The common-prefix digest and four rank-ordered local suffix digests are formed
exactly as in `step-execution-abi-v3.md`. Local q, retained-hidden, and page
bytes are never compared as if TP shards were identical.

The six physical phases are `PREFILL`, `TARGET_DECODE`, `MTP_BOOTSTRAP`,
`MTP_VERIFY`, `MTP_PIPELINED_ZERO`, and `MTP_FLUSH`. Their row formulas,
eight-slot maximum reservation, `R+1` verifier target rows, up-to-seven
teacher rows, and current/next bundle rules are the step ABI's normative
rules. The target-layer r2 tables enumerate every valid row; bucket padding
cannot create a proposal, counter ticket, page write, or output token.

## SamplingCounter v2 and cross-step ownership

One request owns this canonical 48-byte counter state:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | seed |
| 8 | 8 | committed counter |
| 16 | 8 | current bundle generation |
| 24 | 8 | installed DRAFT ticket begin |
| 32 | 8 | installed DRAFT ticket end |
| 40 | 8 | reserved zero |

Every draw has this canonical 48-byte `SamplingTicket.v2` trace record:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | request ID |
| 8 | 4 | logical position |
| 12 | 1 | draft step, `0..6` |
| 13 | 1 | purpose |
| 14 | 2 | reserved zero |
| 16 | 8 | bundle generation |
| 24 | 8 | counter before |
| 32 | 8 | counter after |
| 40 | 8 | reserved zero |

Purposes are `TARGET=1`, `DRAFT=2`, `ACCEPTANCE=3`, `RESIDUAL=4`, and
`BONUS=5`. DRAFT, ACCEPTANCE, RESIDUAL, and BONUS require the nonzero bundle
generation they create or consume. TARGET uses the next bundle generation
for bootstrap/pipelined-zero and zero for target-only decode. Greedy records
retain the logical decision in the sampling trace but allocate no ticket and
do not advance the counter.

Bundle generations come from one checked, monotonic process-common `u64`
allocator. A generation reserved for a launched step is burned even if EOS,
clamping, cancellation, or stop prevents installation; it is never reused.
Overflow is process-fatal. Thus a terminal bootstrap TARGET ticket may name a
generation that never becomes current without creating generation ABA.

The immutable transaction identity is:

```text
(StepPlan.v4 step_id,
 StepInput.v3 canonical hash,
 PageTableDelta.v2 reservation generation,
 current bundle generation,
 committed counter)
```

The trace preimage binds that tuple, both bundle generations, counter
before/after, ticket count, and ordered ticket records. Installed DRAFT
tickets for bundle `g` end at the input committed counter. Verification then
allocates ACCEPTANCE records in proposal order and at most one
RESIDUAL/BONUS record for `g`; TARGET, if required by the phase, follows;
replacement DRAFT records for a distinct next generation `g'` are last.
`StepOutput.v2` publishes the
complete before/after range and the exact replacement DRAFT subrange.

The per-phase maximum advances remain:

```text
TARGET_DECODE                       1
MTP_BOOTSTRAP                       1 + next_depth
MTP_VERIFY                          current_depth + 1 + next_depth
MTP_PIPELINED_ZERO                  1
MTP_FLUSH / PREFILL                 0
```

A request-local failure before native launch may be retried only from the
unchanged transaction tuple. There is no same-generation retry after any
native launch. A launched failure publishes no counter, bundle, page, or
output state and retires the worker generation. Boundary replay is not a
sampling retry and cannot be used to salvage that generation.

## Exact memory homes

`SystemMemoryPlan.v3` adds four independently reported per-rank terms:

```text
mtp_proposal_state_bytes
mtp_recurrent_scratch_bytes
mtp_boundary_hidden_bytes
mtp_argument_completion_bytes
```

For maximum active sequences `C` and admitted maximum depth `D`:

```text
MASS proposal raw/allocated = C * D * 38,720 * 4
recurrent raw               = C * D * 500
recurrent allocated         = C * align_up(D * 500, 256)
boundary hidden             = C * 6,144 * 2
input/output rows           = C * (480 + 240)
```

At C64/MTP6 these are exactly:

| Term | Raw bytes | Allocated bytes |
|---|---:|---:|
| proposal state | 59,473,920 | 59,473,920 |
| recurrent sidecar scratch | 192,000 | 196,608 |
| authoritative boundary hidden | 786,432 | 786,432 |
| sequence input/output rows | 46,080 | 46,080 |

TOP_K may use a smaller reviewed fixed-support representation, but admission
charges the selected graph profile's complete fixed maximum. A digest never
replaces these bytes. Greedy and depth-zero have zero proposal-state bytes.

`WinnerList.v1` remains 16,392 raw bytes per teacher row. A 256-byte-aligned
row stride is 16,640 bytes, so the C64 seven-row maximum is 7,454,720 bytes
(7,343,616 raw). It lives in target buffer class 11 and is included in the
single maximum verifier workspace; it is not added again as retained state.
The recycled hidden ping/pong lives in classes 1/2. Only one authoritative
12,288-byte BF16 boundary hidden per live sequence crosses a physical-step or
prefill-chunk boundary, in the explicit boundary term above.

The one-ahead 500-byte teacher record occupies an already reserved private
draft-page slot. It is charged by draft tentative page capacity, not by a
second allocation. The StepInput/Output rows and bundle descriptors occupy
preallocated class-27 argument/completion slabs. The 46,080-byte term is the
MTP sequence-record portion; the target-layer r2 prefix and three target
tables remain in their existing graph-argument/table terms and are not charged
again. Each recurrent 500-byte stride reserves 368 KV bytes and 132 canonical
zero, nonaddressable indexer bytes because scratch rows never produce a
commit-capable key. Pending target logits retain their target-layer class-30
allocation. Every term is charged exactly once; none may hide in allocator
padding or emergency escrow.

For a production MTP3 profile, the same formulas use `D=3`. Supporting a
later MTP6 request requires an MTP6-charged graph/memory profile; a rank cannot
grow or select that posture locally.

## Complete layer-78 operation-manifest membership

`glmaxx.glm52.operation.v2` extends the structural source ranges for routed
gate/up, routed down, router weight, and router correction from layers
`3..77` to `3..78`. Layer 78 has index group 21 and the exact full-indexer,
attention, routed/shared-MoE, residual, and TP/DCP collective membership of a
full sparse target layer.

It additionally carries these protected BF16, replicated records:

| Role | Role ID | Source | Shape | TP rule |
|---|---:|---|---:|---|
| MTP embedding norm | `0x0801` | `model.layers.78.enorm.weight` | `[6144]` | replicated |
| MTP hidden norm | `0x0802` | `model.layers.78.hnorm.weight` | `[6144]` | replicated |
| MTP E/H projection | `0x0803` | `model.layers.78.eh_proj.weight` | `[6144,12288]` | replicated |
| MTP head norm | `0x0804` | `model.layers.78.shared_head.norm.weight` | `[6144]` | replicated |

Router weights/bias and all nonexpert layer-78 records remain protected
source precision. Routed layer-78 codec membership is not the old operation
manifest's unconditional NVFP4 string: it is bound by the accepted immutable
weight policy and physical MTP program. The hybrid profile requires all 256
draft experts to use ModelOpt-W4A16 codec `0x0102`; the capacity profile uses
its accepted EXL3 realization. No request or recurrence may change it.

## MTP0 pipeline identity

Bootstrap's first authoritative pending token has kind `INITIAL`. A live
`MTP_PIPELINED_ZERO` successor has kind `MTP0`. Both create or replace a
nonzero bundle generation even though proposal count, proposal token IDs,
q-state bytes, q-state digests, trace proposal range, and verify mask are
zero. That generation identifies the pending teacher record, authoritative
root hidden, pending logits, and retained-state digest; it is not a fake
proposal generation and consumes no DRAFT ticket.

Pipelined MTP0 therefore has configured depth nonzero, current/next effective
depth zero, one pending target, zero proposals, and nonzero current/next
bundle identities. Target-only decode has configured depth zero, equal ends,
no pending teacher state, and zero bundle identities. The two postures cannot
alias under a graph key or cache capability.

## Chunk boundaries, determinism, and prefix publication

Chunked MTP-capable prefill retains the final authoritative target hidden of
each nonfinal chunk in the boundary-hidden arena. The next chunk consumes it
for the first teacher transition. Restored prefixes reconstruct the same
boundary through the retained read-before-current-row replay; replay scratch
cannot overwrite or reread the cached current row as an extra self key.

The base engine may qualify numerically equivalent target kernels across
different buckets without requiring bit-identical GEMM output. Prefix
publication is stricter. A target page and its successor-slot sidecar may be
sealed only after evidence proves byte-identical target, indexer, draft KV,
and draft indexer payloads for every admitted prefill chunk, graph bucket,
page split, and boundary-replay route that can produce that namespace. Until
then the page stays private and cannot enter prefix, DRAM, NVMe, or exported
session state. The same content key with different bytes remains
engine-fatal. This is `PrefixPublication.v2.successor-slot`.

## Terminal EOS and session visibility

`CompletionRecord.v2` binds request ID, `StepOutput.v2` digest,
materialized end, emitted end, model-terminal bit, and the optional final
unmaterialized target token/kind. A terminal pending EOS may therefore be
client-visible with `emitted_end = materialized_end + 1` while having no
target KV or durable teacher sidecar. An accepted draft EOS is materialized
and has equal ends.

`SessionExport.v2` contains only the equal, materialized target/draft prefix.
A nonterminal export or suspension first executes `MTP_FLUSH`. A terminal
completion with one unmaterialized EOS is not resumable as though that EOS
had KV: import stops at the materialized end and the completion transcript
records the EOS separately. Prefix keys, page valid counts, and session token
counts never include that unmaterialized terminal position.

## Coordinated amendment set

Acceptance opens one coordinated CPU amendment, not partial implementation:

1. engine and format successors adopt teacher/scratch successor slots and the
   six physical phases;
2. operation-manifest v2 adds complete layer-78 structural membership;
3. StepPlan v4, StepInput v3, StepOutput v2, SamplingCounter/Ticket v2, and
   PageTableDelta v2 are implemented atomically;
4. PrefixPublication v2 and SessionExport/CompletionRecord v2 adopt the
   determinism and terminal rules above;
5. SystemMemoryPlan v3 exposes every added MTP arena term and charges the
   class-11 maximum once; and
6. `glm-cache` replaces `SpeculativeTail` with `MtpProposalTail.v2`, covering
   materialized/emitted ends, pending state, current/next bundles, ticket
   ranges, one-ahead storage, and all six depths.

No CUDA code consumes an incomplete subset. Every predecessor identity fails
closed at the boundary.

## Withheld-review closure

| Finding | Corrective section |
|---|---|
| MAJOR-1.1 output ABI | Exact 240-byte output and materialized-versus-emitted invariants |
| MAJOR-1.2 counter fields | Exact 48-byte state/ticket records, transaction tuple, trace, counter partitions, and retry rule |
| MAJOR-1.3 memory surfaces | Four `SystemMemoryPlan.v3` terms, exact C64/MTP6 formulas, class-11 ownership, and tentative-page charge |
| MINOR-1 layer-78 tensors | Operation-manifest v2 ranges, index group 21, four protected roles, and policy-bound expert codecs |
| MINOR-2 retry/chunk boundary | No post-launch retry plus the explicit per-sequence authoritative boundary-hidden arena |
| MINOR-3 pipelined MTP0 | `INITIAL`/`MTP0` kinds and nonzero pending-state generations with zero proposal state |
| QUESTION-1 sealed determinism | Publication requires byte identity across every admitted bucket/chunk/replay route |
| QUESTION-2 terminal EOS | Completion v2 records the unmaterialized terminal token; SessionExport v2 excludes it |

## Gates and nonclaims

The retained sixteen-item CPU gate remains mandatory and gains exact known
answers for the two 48-byte sampling records, four explicit memory terms,
layer-78 operation membership, both MTP0 postures, chunk-boundary hidden
retention, publication bucket invariance, and terminal session import. It
must inject failure before and after every native-launch and publication
boundary and prove no same-generation launched retry.

Only after MTP r2, step ABI, target layer, sampling, page transaction, memory
plan, and resident physical program designs are accepted may their coordinated
CPU proof begin. Only after that implementation is separately reviewed may
the retained SM120 gate order run: one-row lineage replay, slot 63/64 replay,
C1/C64 graphs, scratch nonpublication, target-only versus pipelined MTP0,
then MTP1 through MTP6 quality and timing.

This document is not an accepted ABI, implementation, CUDA program, layer
replay, checkpoint smoke, model-quality result, capacity result, concurrency
result, or performance claim. It authorizes no cn4 work.
