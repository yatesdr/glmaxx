# Recurrent MTP execution v1 r3 integration amendment

Date: 2026-08-04

Status: corrective design candidate; adversarial acceptance required before
CPU or CUDA implementation

GPU evidence: none

## Scope and precedence

This amendment closes four integration gaps left after the MTP r2 candidate:

1. r2 names the retained sampling v1 mechanics but does not bind the accepted
   v1+r2 composite or its 32-byte TOP_K residual result;
2. r2 says pending logits live in target class 30 but gives their persistent
   double buffer no exact byte charge;
3. r2 gives verifier winner and proposal-state charges but not the fixed
   rank-logit scratch envelope needed by every target/draft sampling row; and
4. r2 predates GraphProfile v3, the physical class/use plan, executor r5's
   recurrent-state arena, and r4/r5's complete target-plus-MTP program-set
   binding.

R3 is normative over conflicting r1/r2 text. The retained shifted teacher
lineage, successor-slot sidecar, teacher-versus-recurrent separation, six
physical phases, bundle/counter ownership, exact StepOutput v2, terminal EOS,
prefix publication, and layer-78 source membership remain unchanged.

The operative design is the conjunction of:

- `mtp-layer-execution-v1.md`;
- `mtp-layer-execution-v1-r2.md`;
- this r3 amendment;
- distributed sampling v1 plus its r2 amendment;
- target-layer v1 plus its r2 amendment;
- the physical target-graph memory v1 design and GraphProfile v3;
- the SM120 rank-executor r1-r5 design and corrected native header; and
- the applicable accepted physical target/MTP program.

No r1/r2-only sampling, logical-only GraphProfile v2, draft-only executor
arena role, single-program validation digest, or uncharged pending state may
enter an r3 generation.

## Closed successor identities

R3 adds these exact requirements to r2's identity table:

| Surface | Required identity |
|---|---|
| distributed sampling | v1+r2 composite below |
| TOP_K residual wire | `TopKResidualResult.v2`, 32 bytes |
| sampling trace | `glmaxx.sampling-trace.v2` |
| graph profile | `glmaxx.graph-profile.v3` |
| physical graph memory | `glmaxx.target-graph-memory-plan.v1` |
| executor arena role 5 | `DEVICE_RECURRENT_STATE` |
| executor graph validation | `glmaxx.executor-graph-program-set.v1` |
| executor module memory ABI | SHA-256 of `glmaxx.target-graph-physical-memory-abi.v1\0` |
| memory plan | `glmaxx.system-memory-plan.v3`, with the r3 terms below |

The distributed-sampling composite is exactly:

```text
SHA256(
  "glmaxx.distributed-sampling-abi.v1-r2\0" ||
  383e328a527cc780ed553af0b78382cf200ad60f97afb26d96a2a1494b57c89b ||
  061903d0a0cf2a284f35b177da5f1c3484cb61dfd9020f627db7b5632a4f2b6b
)
  = 95fa7aa3b4b0b78a3f8313705d25e4c11682632fce6d8b8c2355b8130745f58c
```

The two 32-byte inner terms are raw SHA-256 bytes, not lowercase text. This is
the same composite consumed by the target-program final-head entry. A missing
or unaccepted sampling r2, changed inner file, v1-only digest, or rank-local
sampling identity fails before program or graph construction.

## Sampling result and StepOutput equality

R2's 240-byte `StepOutput.v2` remains exact. For a TOP_K rejection, rank zero
broadcasts the sampling amendment's exact 32-byte
`TopKResidualResult.v2`:

```text
SamplingResult.v2             16 bytes
selected_probability:f32       4 bytes
target_mass:f32                 4 bytes
residual_mass:f32               4 bytes
flags:u8                        1 byte
reserved_zero                   3 bytes
```

Flag bit 0 is `residual_fallback`; every other bit and trailing byte is zero.
All ranks validate token, purpose, draft step, counter, selected probability,
both mass folds, flag, and trace membership. A bare 16-byte sampling result is
invalid for a TOP_K rejection.

For a MASS rejection, the existing `MassSelection.v2.flags & 1` is the same
bit. The common `StepOutput.v2.residual_fallback` must equal the TOP_K result
or MASS selection bit on every rank. `target_kind` is `RESIDUAL` regardless
of whether the zero-residual fallback selected the target distribution; the
fallback bit and trace distinguish the numerical branch. Greedy mismatch,
ordinary target, accepted-EOS/no-target, and bonus paths require the bit zero.

The trace is exactly `SamplingTrace.v2` from sampling r2, including the
TOP_K residual record or MASS residual-summary digest and the exact
`no_target_reason`. R2's `sampling trace digest` field cannot summarize a
different payload set. Any result/trace/output disagreement is generation
fatal before page, token, counter, or bundle publication.

## Exact persistent pending-logit home

One pending logit is one complete rank-local physical vocabulary shard in
binary32:

```text
local physical vocabulary rows       38,720
bytes per binary32 logit                   4
bytes per rank-local pending vector 154,880
```

Every admitted live sequence reserves two independently generated vectors:

```text
CURRENT    committed input to the next decode/verify step
NEXT       tentative successor selected by the current physical step
```

They cannot alias. A launched step may fail after reading CURRENT and writing
NEXT; failure must leave CURRENT as the prior committed generation and make
NEXT unreachable. Four-rank consensus atomically swaps the selected NEXT
generation into CURRENT. Terminal cleanup may then retire both. An unused
terminal NEXT allocation remains charged by the fixed profile.

The exact per-rank persistent charge is:

```text
mtp_pending_logit_bytes = C * 2 * 38,720 * 4
```

At C64 it is exactly `19,824,640` bytes. One 154,880-byte vector is already
605 units of 256 bytes, so per-vector 256-byte alignment adds no padding.
The two vectors per sequence are ordered by `(sequence_slot, CURRENT, NEXT)`
inside target class 30, itself a nonaliasing subrange of executor recurrent-
state arena role 5. Slot/generation tables remain immutable class-27
arguments and contain no raw device pointer.

Target-only MTP0 has this same nonzero charge even though proposal state,
draft KV/indexer state, and MTP program membership are absent. Pipelined MTP0
also retains its nonzero bundle identity under r2. No implementation may use
zero role-5 bytes as a proxy for MTP0.

## Exact rank-logit scratch envelope

Target and recurrent heads produce binary32 rank logits before distributed
sampling. The fixed row stride is 154,880 bytes and is 256-byte aligned.
For a captured graph with maximum simultaneously live logit rows `L`:

```text
rank_logit_scratch_bytes = L * 154,880
```

The common DAG and `GraphBufferUse.v1` table derive `L`; a descriptor or rank
cannot choose it. Target class 26 owns target rank-logit scratch. MTP head
logits use the same span only when the physical-plan DAG proves their
lifetimes disjoint; otherwise the MTP use receives a separate class-zero
scratch interval. Aggregate scratch cannot authorize the reuse.

At the C64/MTP6 verifier ceiling, the target row bucket is 448 and the full
target envelope is exactly:

```text
448 * 38,720 * 4 = 69,386,240 bytes/rank
```

This is graph scratch, not persistent proposal state and not class-30 pending
state. It is charged once in the GraphMemoryPlan/GraphProfile-v3 maximum
workspace. The physical plan must also include every MTP proposal-head use;
the maximum of disjoint lifetimes or the aligned sum of overlapping lifetimes
is selected from the actual DAG, never from the smaller number desired by a
budget.

## Complete recurrent-state memory terms

`SystemMemoryPlan.v3` now reports these separate per-rank terms:

```text
mtp_pending_logit_bytes
mtp_proposal_state_bytes
mtp_recurrent_scratch_bytes
mtp_boundary_hidden_bytes
mtp_argument_completion_bytes
```

For active-sequence ceiling `C`, configured depth `D`, and one immutable
sampling class per graph profile:

```text
pending logits       = C * 2 * 154,880
GREEDY proposal      = 0
TOP_K proposal       = C * D * 2,048
MASS proposal        = C * D * 154,880
recurrent raw        = C * D * 500
recurrent allocated  = C * align_up(D * 500, 256)
boundary hidden      = C * 6,144 * 2
input/output rows    = C * (480 + 240)
```

At C64/MTP6:

| Term | Bytes |
|---|---:|
| pending logits | 19,824,640 |
| GREEDY proposal | 0 |
| TOP_K proposal | 786,432 |
| MASS proposal | 59,473,920 |
| recurrent scratch allocated | 196,608 |
| authoritative boundary hidden | 786,432 |
| sequence input/output rows | 46,080 |

Only one selected sampling-class proposal charge enters a concrete profile;
an engine supporting multiple classes must reserve their maximum or distinct
nonoverlapping profiles. The pending-logit charge is always present. The
class-11 winner maximum and class-26 rank-logit maximum remain graph scratch
and enter the single maximum-workspace charge; neither is added to recurrent
state. Target tables, proposal tokens, tickets, bundle descriptors, and
completion rows retain their prior exact argument homes. Digests never
replace allocated bytes.

The final physical resource order is acyclic:

```text
accepted operator/program formulas
-> rank-set resource budget
-> GraphMemoryPlan.v1
-> GraphProfile.v3
-> SystemMemoryPlan.v3
-> owner-thread arena materialization
```

Every term appears once in the resource budget and final memory plan. The
physical plan binds the pre-allocation budget, not the later memory-plan hash.

## Executor and graph generation binding

An MTP0 target-only graph has only the target program in executor r4/r5's
program-set digest. A verify graph with any layer-78 node has exact target and
MTP program digests. The first validation node binds that program set, the
explicit adopted validation module, the module-set capability digest, the
graph-memory ABI digest, GraphProfile v3, and all seven arena generations.

The applicable physical MTP program selects the immutable layer-78 codec and
layout policy. It cannot change by request, recurrence, sampling branch, or
rank. A hot reload may replace compatible modules/configuration only through
the executor r5 all-rank prepare/quiesce/commit/rollback transaction and
without moving or rereading resident weights.

No graph captures until the complete class/use plan includes pending logits,
proposal state, recurrent scratch, boundary hidden, target/draft rank logits,
winner lists, arguments, collectives, and completion/status spans. First-use
allocation, descriptor-supplied workspace, or implicit CUDA-library scratch
is forbidden.

## Coordinated CPU/mock gate

After all design tokens exist and before any CUDA work, one atomic proof must:

1. recompute the sampling composite from raw inner hashes and reject every
   predecessor or text-encoded hash;
2. encode/decode/corrupt TOP_K and MASS residual results, trace items, and the
   StepOutput fallback equality for every branch;
3. enumerate CURRENT/NEXT transitions through prefill, target decode,
   bootstrap, verify, pipelined zero, flush, EOS, clamp, cancellation,
   prelaunch failure, launched failure, and successful consensus;
4. allocate two distinct pending vectors for every C1/C64 sequence slot and
   reject alias, stale generation, wrong rank, wrong slot, and one-byte-short
   spans;
5. reproduce every formula above for D0..D6 and GREEDY/TOP_K/MASS, including
   the exact C64 known answers;
6. build physical M1/C64/MTP0/MTP3/MTP6 plans and validate all target and MTP
   head-logit uses, legal lifetime reuse, and overlapping-lifetime separation;
7. reconcile proposal, pending, winner, rank-logit, argument, recurrent,
   boundary, KV/indexer, collective, graph-runtime, and escrow terms exactly
   once in four rank memory receipts;
8. prove target-only MTP0 has recurrent-state pending logits but no MTP module
   or proposal/draft subrange;
9. prove every verify graph binds the exact target-plus-MTP program set and
   one common physical plan/module generation on all four ranks; and
10. inject every pre/post-launch and hot-reload failure without publishing a
    page, token, counter, bundle, pending vector, or mixed generation.

The proof remains CPU/mock, bounded, and synthetic. It does not authorize cn4
or accept a CUDA graph, model layer, checkpoint, quality result, KV capacity,
concurrency result, cold/hot-load result, or performance measurement.

## Gate effect and nonclaims

The r2 handoff is superseded and must not issue its token. R3 acceptance opens
only the coordinated CPU/reference implementation after sampling r2, target
layer r2, physical graph memory, executor r5, step/page/memory successors, and
the selected physical target/MTP program are each accepted.

That CPU implementation requires a separate adversarial token before the
retained SM120 gate order begins. Target-only MTP0 quality remains first;
MTP1..6 cannot be enabled to recover speed before matched MTP0 correctness.

This document is not an implementation, CUDA program, graph, layer replay,
checkpoint smoke, model-quality result, capacity result, concurrency result,
hot-reload result, or performance claim. It authorizes no cn4 work.
