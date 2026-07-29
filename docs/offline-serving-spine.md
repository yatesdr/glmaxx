# Offline serving spine

Date: 2026-07-29

Status: executable CPU/control-plane proof; no SM120 performance claim

The repository now has an executable serving path from multi-user admission
through a four-rank worker boundary. It is deliberately backend-neutral: the
CPU rank executors prove collective order, queue bounds, and lifecycle
semantics. `Tp4WorkerPool` owns four persistent, mutable, thread-affine
`RankExecutor` instances, so authorized SM120 executors can own one CUDA
context, stream/graph set, weight view, and collective handle per rank without
changing the scheduler or cache contracts.

## Implemented path

1. A continuous-batching scheduler selects separate captured prefill,
   MTP0-decode, or common-depth MTP1–6 verify batches.
2. The coordinator compiles the selected graph into one canonical `StepPlan`
   and collective schedule. DCP query, candidate, partial-LSE, TP reduction,
   and distributed sampling routes are explicit and graph-qualified.
3. One bounded dispatcher sends the identical plan to four rank workers.
   Every rank returns a fixed-capacity `StepOutput` in sequence-table order:
   at most 64 sequence records and at most seven committed token IDs per
   sequence. Plan, schedule, exact output, and canonical output-digest
   consensus are checked before the scheduler may commit the step. Padded
   language-head rows 154856–154879 are rejected at this boundary.
   Plan and collective-schedule validation occurs before the backend entry
   point. Each executor remains on its named rank thread across steps and may
   mutate only its own rank-local state. Any plan-order, malformed output,
   rank-backend, or consensus failure permanently closes that worker
   generation so no rank can enter a later collective schedule.
4. Serving consumes the exact rank-returned token IDs; it no longer derives
   mock tokens or acceptance counts from a digest. Verify steps commit a
   checked value from one through `depth + 1`, without crossing a request's
   generation limit. Each record carries an explicit accepted-draft count and
   target-token-presence bit, so an accepted draft EOS cannot be mislabeled as
   a target correction. EOS is legal only as the final committed token,
   terminates the scheduler before the length limit, and is reported as a
   `stop` rather than `length` finish.
5. Request events cover admission, prefill progress, tokens, finish,
   cancellation, and failure. Event and worker queues apply backpressure
   instead of growing without bound.
6. Prefix admission derives content keys internally, restores every matched
   durable page on its owner rank, validates namespace/generation/SHA-256,
   promotes it through the bounded residency manager, pins it for execution,
   and releases it at a terminal request state.

Prefix admission now has a nonblocking state machine. All missing rank-owned
pages are submitted to bounded per-rank restore workers in one admission
operation; callers can poll while decode scheduling continues or cancel and
roll every page back to its prior unpinned/NVMe state. `admit_tokens` retains
a blocking wrapper for deterministic command-line proofs, while production
integration uses `begin_admit_tokens` and `poll_admission`. The current HBM
promotion remains a byte-owning CPU residency proof, not a qualified CUDA
transfer. Tokenized prompts have a separate checked host-byte budget that
includes restore-pending and prefill-active requests. Fully restored prompts
release that reservation at admission; cold/partial prompts release it after
their final prefill chunk.

## Durable page store

`glm-cache::FileTierStore` writes exact target KV/indexer and optional MTP
sidecar pieces to an external `pages.dat`. Its fixed-size CRC32C journal
records begin, durable-piece, and publication events. Data is synchronized
before durability is recorded; all required pieces are durable before
publication. Recovery ignores incomplete transactions and a torn trailing
journal record. Restore rechecks every piece SHA-256.

The residency manager implements deterministic unpinned-LRU pressure:

```text
HBM -> DRAM when DRAM has room
HBM -> NVMe when DRAM is full
NVMe -> restoring -> HBM after validated asynchronous read
```

Pinned pages are never selected. If every possible victim is pinned, the
restore fails closed.

## Active sequence page table

`glm-cache::SequencePageTable` is the CPU metadata oracle for the active HBM
working set. It reserves independent target and optional MTP draft slots on
each DCP4 owner rank and exposes target-only and MTP-capable sequence limits
separately. With 4,096 target and draft pages per rank, one 1,048,576-token
MTP sequence consumes exactly 4,096 pages on every rank.

Only complete sealed prefix pages can acquire shared references. A prefix key
is bound to its logical page ordinal and therefore to one deterministic DCP4
owner. Session forks share sealed pages and allocate a private physical copy
for a mutable tail. MTP0–6 reservations transition target and draft
attachments together, including a cross-page verifier tail, and commit or
roll back atomically. Allocation failure restores the complete prior metadata
state.

This is not an HBM allocation or payload-transfer claim. The physical IDs are
bounded rank-local slot identities; the future CUDA rank executor must map
them to preallocated target, indexer, draft, and draft-indexer arenas without
changing these ownership or transaction rules.

## Reproducible proof

Run with an external directory:

```text
cargo run --release --offline -p glm-cli --bin glmaxx -- \
  serving-proof /tmp/glmaxx-serving-proof
```

The command writes real page and journal bytes under the supplied directory
and a deterministic summary. The golden report is
`fixtures/cpu-serving-proof-v1.json`; `scripts/local-checks.sh` regenerates
and compares it.

The proof currently covers two tenants, one real 64-token durable prefix
restore, cold prefill, MTP0 decode, MTP6 verify, eleven emitted tokens, and
clean completion through four consensus workers. The CPU executor deliberately
returns one target token per verify step, so this proof claims no synthetic
draft acceptance. A separate scripted-executor test proves the bounded
multi-token result and accepted-draft event path.

## SM120 handoff boundary

This closes missing CPU/control-plane work, not hardware qualification. The
next authorized cn4 work is:

1. reproduce the pinned toolchain and SM120 inventory without disturbing
   existing workloads or Docker assets;
2. run the existing NVFP4 correctness matrix on one idle SM120 GPU;
3. record actual-shape FC1 inclusive timings and counters;
4. replace the retained CUDA-core control with block-scaled MMA;
5. qualify the EXL3 source consumer;
6. implement `RankExecutor` with qualified CUDA rank state and connect it to
   this exact `StepPlan` boundary;
7. advance to TP4 one-layer replay only after the microbenchmark gate passes.

Until those gates pass, the executable reports its backend as
`four-rank-cpu-contract`.
