# Deterministic small-checkpoint runner v1 r2

Date: 2026-07-30

Status: corrective design candidate; adversarial review required before CPU
or CUDA implementation

Base contract: `docs/small-checkpoint-runner-v1.md`

GPU evidence: none

## Purpose

This amendment supersedes the implementation authority of the base M4
contract. Requirements in the base remain in force only where this amendment
does not replace them.

Re-deriving M4 against the laboratory manifest, fused routed-MoE r3,
checkpoint-load transaction, and persistent executor found six integration
defects:

1. the base conflates an accepted NVFP4 source with the native output even
   though FC1 may require an explicit `0x1201 -> 0x1202` permutation or a
   separately reviewed 2D-to-1D requantization;
2. it omits the protected header flag, separate file/device metadata terms,
   laboratory budget, catalog, and plan-hash domain;
3. it creates persistent rank workers after adoption, although those owner
   threads must already own contexts, modules, collectives, load streams,
   quarantined arenas, and the adoption command;
4. it says that no executor handle exists before adoption, which is too broad:
   non-executable owner-thread resources exist, while only the laboratory
   weight handle and model execution permit remain unavailable;
5. it does not define state reset and receipt generations tightly enough to
   make 100 warm repetitions independent rather than repeated reads of stale
   graph output; and
6. it requires successful resource cleanup even after owner-thread loss or
   failed DMA synchronization, contradicting the required process-fatal
   fail-safe boundary.

R2 corrects those defects without moving M4 earlier than M2/M3 or making a
full-model, service, quality, capacity, or performance claim.

## Immutable gate prerequisites

M4 construction is unavailable until all of these exact artifacts are
accepted and hash-bound:

1. the current native format and protected-header correction;
2. the M2 NVFP4 source/conversion and actual-shape operator result;
3. fused routed-MoE r3 plus its layout/policy/target-program CPU proof;
4. M3 layer-6 replay with the exact captured inputs, routes, reference
   values, numerical contract, and TP4/DCP4 collective route table;
5. the laboratory manifest, 192-byte catalog, budget, plan-domain, and
   type-state CPU proof;
6. the exact M4 graph profile and measured laboratory memory profile;
7. the persistent rank-executor and checkpoint-load CPU/mock gates; and
8. separate operator and collective SM120 evidence for every M4 row shape.

Every dependency is identified by its result SHA-256 and exact acceptance
token. A design token, source commit, successful CPU test, cubin, or prior
unmatched GPU run cannot substitute for a required result.

M4 remains after the complete M3 replay. It does not bootstrap M2 or M3 and
cannot provide their evidence.

## Source, conversion, and native-output identity

The base phrase “exact NVFP4 source checkpoint” is replaced by three distinct
identities:

```text
complete source checkpoint identity
selected source tensor identity
native M4 rank-set output identity
```

The laboratory manifest's `source` block binds the first two. Its
`conversion` block, per-tensor `conversion_route`, authenticated descriptor,
decoded codec metadata, and native plane digests bind the third.

The M4 fixture identity additionally contains:

```text
laboratory manifest schema and four manifest SHA-256 values
four native file UUIDs and payload SHA-256 values
conversion UUID
selected-source catalog SHA-256 and selected-source bytes
native tensor-contract SHA-256 per rank
rank-invariant 192-byte tensor-catalog SHA-256
conversion-policy SHA-256
target-program-v2 SHA-256
codec-capability SHA-256
laboratory-budget SHA-256
laboratory plan SHA-256
```

Only the closed laboratory conversion routes are legal:

```text
BYTE_EXACT_PROTECTED
NVFP4_1D_1201_TO_1202_PERMUTE
NVFP4_1D_1202_RETAIN
NVFP4_2D_TO_1D_REQUANTIZE_REVIEWED
NVFP4_DOWN_RETAIN
```

The last route name covers only down projections and retains the
authenticated 1D or 2D codec/layout selected by M2. The
`NVFP4_2D_TO_1D_REQUANTIZE_REVIEWED` route is unavailable unless the exact
conversion implementation, decoded-source/output hashes, and per-position
quality result are accepted. A filename, shape, checkpoint marketing name,
or desired kernel route is never evidence of a conversion.

The reference consumes the exact native output planes loaded by the device.
It never substitutes source values for a permuted or requantized output.

## Exact laboratory file and load identity

Every M4 rank file validates as:

```text
schema                         glmaxx.rank-manifest.nvfp4-laboratory.v1
profile                        nvfp4-laboratory-m4
header flags                   11
tensor count                   533
file payload bytes             1,982,245,376
file codec-metadata bytes      65,536
device weight-arena bytes      1,982,245,376
device metadata-arena bytes    130,944
uploaded plane/metadata bytes  1,982,310,912
```

The 533-tensor membership and primary/auxiliary plane arithmetic remain as
specified by the base and laboratory-manifest contracts. File codec metadata
is a packed control region. Device metadata independently aligns each of 512
nonempty 128-byte records to 256 bytes and does not round the final end:

```text
511 * 256 + 128 = 130,944
```

The four-rank physical plan preimage is:

```text
416 + 4 * 248 + 4 * 533 * 64 = 137,856 bytes
```

The encoding remains `RankSetLoadPlan.v1`, profile byte `1`, but its digest
is:

```text
SHA256(
  "glmaxx.rank-set-load-plan.v1.nvfp4-laboratory-v1\0"
  || 137,856-byte plan preimage
)
```

It uses the 192-byte laboratory semantic-catalog digest and one completed
`glmaxx.nvfp4-laboratory-budget.v1`. The capacity-EXL3 plan domain, production
manifest, and production budget are rejected. Mutating the profile byte,
schema, catalog version, budget identity, permission bit, or plan domain
cannot produce another valid plan.

M4 uses `FULL_SHA256`. FS-verity, cached trust, size/mtime/inode, or a previous
successful cycle are not accepted at this gate.

## Corrected owner-thread and resource order

The base load/execution steps are replaced by this exact process-common
transaction:

1. authenticate and strictly validate four laboratory rank files on the host;
2. derive and validate the exact 533-entry subset, four rank-local tensor
   contracts, one 192-byte semantic catalog, completed laboratory budget,
   target program v2, codec capability set, graph profile, collective route
   table, and laboratory-domain load plan;
3. create four persistent rank-owner threads;
4. on each owner thread, create and identify its CUDA context, load the exact
   modules, and return the pre-allocation identity/resource receipt;
5. validate the process-common device/topology decision, freeze the complete
   resource table, and allocate every deterministic device and pinned-host
   arena, including each non-executable weight/metadata arena, fixed
   collective span, stream, event, pinned ring, diagnostic span, and declared
   scratch/cache/output span;
6. create every M4 collective communicator and immutable route against those
   fixed spans;
7. reconcile measured context/module/collective residency, preserve the
   required unallocated cleanup escrow, and accept four exact
   laboratory-memory-plan receipts or discard all candidate resources;
8. stream every planned file plane through the fixed pinned ring, validate
   exact file and destination cursors after zero-filling both weight arenas,
   drain, seal, and full-readback-hash both device arenas;
9. collect four `PreparedRankReceipt.v1` records and issue one identical
   laboratory-domain adoption command;
10. move each sealed arena into a private owner-thread laboratory slot and
    validate four matching adoption acknowledgments;
11. construct one process-common `LaboratoryWeightHandle`; no production
    `WeightArenaHandle` is created;
12. capture or instantiate only graph keys bound to the exact target program
    v2, laboratory plan, routes, row shapes, and resource generations;
13. upload and independently hash-acknowledge the fixture page table, target
    KV, indexer keys/scales, immutable inputs, and expected route records;
14. issue an execution permit only after all four ranks acknowledge
    `GRAPHS_READY` and `FIXTURE_CACHE_READY`;
15. execute and compare the M4 matrix;
16. revoke execution, synchronize every recoverable in-flight generation, and
    destroy resources on their owner threads in reverse dependency order; and
17. accept success only after four exact destruction receipts and a
    process-common evidence finalization receipt.

Persistent rank threads, CUDA contexts, modules, collective resources,
quarantined arenas, and load control handles necessarily exist before weight
adoption. They are not model-executable.

Only the private `LaboratoryWeightHandle` exists after process-wide adoption.
Only a later exact execution permit can launch the M4 graph. Neither type can
convert to a production handle, startup `HEALTHY`, HTTP service, serving
prefix namespace, production scheduler, or full-checkpoint route.

## Exact M4 execution boundary

The M4 program consumes two immutable, real M3 boundary fixtures:

- one M=1 decode row; and
- one accepted prefill row bucket.

Each fixture binds its captured layer-6 hidden rows, absolute positions,
sequence lengths, DCP owners, page-table projection, target KV, indexer
keys/scales, router reference, target-program v2 digest, graph key, and
collective schedule. It also binds the expected current-row target-KV and
indexer writes, their physical owner/destination records, and the exact
post-step page-table/cache digest. The fixture contains no embedding output
presented as a token-feedback loop.

For each shape the program performs exactly:

1. layer-6 input norm;
2. absorbed MLA/indexer production, current-row target-KV/indexer writes, and
   the reviewed DCP attention route;
3. attention output projection, TP reduction, and residual;
4. post-attention norm;
5. router/shared/routed expert execution, compaction, SwiGLU, FC2, TP
   reduction, and residual;
6. comparison and generation-bound publication of the fixture-owned
   target-KV/indexer destinations and page-table successor;
7. final norm applied directly to that layer-6 output;
8. four vocabulary-head shards; and
9. distributed greedy candidate merge without a full-vocabulary gather.

The reference deliberately performs the same truncated layer-6-to-head
program over the loaded native planes. It is not a claim about layers 0–5 or
7–77, embedding, recurrent decoding, or full-model logits.

All four ranks receive one identical immutable step/program/route identity.
There is no rank-local codec, layout, collective, DCP, graph, or numerical
fallback.

## Repetition isolation and cold/warm meaning

Each execution has a monotonically increasing process step generation and
rank-local argument, page-table, input, output, graph-launch, collective, and
completion generations. Every output receipt binds all of them. A completion
from an earlier generation is rejected even if its values match.

Before each repetition, the runner:

1. restores the immutable fixture bytes into a fresh logical fixture
   generation, including the pre-step cache snapshot and page table;
2. overwrites every required output/candidate/partial/error slot with a
   generation-specific poison pattern while setting contract-declared
   padding and unused slots to their canonical empty values;
3. uploads and acknowledges exact argument and page-table records;
4. clears only contract-declared scratch; and
5. establishes a four-rank barrier before launch.

The run passes only when every required output byte is overwritten under the
current generation and all unused/padded bytes retain their required
canonical value. The post-step target-KV, indexer, and page-table digest must
also equal the fixture successor. Reusing stale graph output, cache state, or
a prior receipt therefore fails.

Definitions:

- `cold-load`: a fresh child process with no adopted M4 generation;
- `cold-graph`: first instantiation and launch in that child process;
- `cold-fixture`: first fixture upload/restore in that child process;
- `warm`: adopted weights, graph instances, modules, collective routes, and
  immutable fixture residency retained, with a new logical execution
  generation and reset output/scratch state.

M4 runs five fresh child-process cold-load/run/destroy cycles. Inside each
successful child it runs one eager and one captured cold-graph control per
shape followed by 100 warm captured repetitions per shape. Eager and captured
controls consume byte-identical immutable input/cache snapshots and distinct
output generations.

Determinism applies to plans, routes, inputs, output values, comparison
classification, and receipts. Durations and physical addresses are recorded
but are not required to be byte-identical.

## Corrected failure and cleanup matrix

Faults are classified before injection:

### Recoverable test-generation failures

These include validation, hash, semantic, budget, plan, allocation-before-
publication, upload with successful synchronization, receipt, adoption with
all owner threads alive, graph/profile, route divergence detected before
launch, malformed input/partial/router output, sampling rejection, and
collective-safe cancellation.

They require:

- zero accepted M4 output;
- one common abort identity;
- four exact owner-thread cleanup receipts;
- no live laboratory handle or execution permit;
- zero engine-owned live allocation/handle counts; and
- successful child-process exit reporting the expected rejected fault.

### Process-fatal safety failures

These include owner-thread loss, failed CUDA/load-stream synchronization,
failed communicator abort, ambiguous in-flight DMA, a wedged rank past the
absolute supervisor deadline, a cleanup receipt mismatch, or loss of
exclusive resource ownership.

They require:

- zero finalized/accepted M4 output;
- no forged cleanup receipt;
- no free of any resource that may still be referenced by CUDA or DMA;
- a terminal fatal record emitted from preallocated evidence storage when
  possible; and
- nonzero child-process termination observed by the parent supervisor.

Leaking possibly referenced resources until process exit is correct in this
class. Continuing the process, retrying one rank, or claiming exactly-once
cleanup is forbidden.

The matrix runs every rank-local recoverable fault for ranks 0–3, adoption
failure after each possible prior acknowledgment count, and every declared
process-fatal class in an isolated child. The parent validates exit class,
accepted-output absence, and bounded timeout without treating child-process
termination as a successful cleanup receipt.

Global free-HBM equality is not a cleanup oracle because driver/JIT/library
caches may persist. Recoverable cleanup is proved by the engine-owned
allocation/handle ledger plus owner-thread destruction receipts. Cold-child
termination supplies the stronger isolation boundary for process-fatal
cases.

## Evidence amendments

In addition to the base fields, every M4 record retains:

```text
laboratory schema/profile/catalog/budget/plan-domain identities
header flags and all four native file/control-region identities
selected-source versus native-output identities and conversion route
file payload, file metadata, device weight, device metadata, and upload bytes
target-program-v2 and layout-source/quant-policy digests
owner/context/module/collective/arena/graph/cache/execution generations
fixture boundary and full page-table/KV/indexer digests
cold input and post-step KV/indexer/page-table successor digests
cold-load, cold-graph, cold-fixture, warm, eager, and captured labels
output poison/overwrite validation
recoverable versus process-fatal fault class
abort, cleanup, destruction, fatal, and parent-supervisor receipts
engine-owned allocation/handle ledger before and after recoverable cleanup
```

Kernel, launch, transfer, collective, framework, and end-to-end durations use
the same pinned monotonic clock and explicit event boundaries. They remain
diagnostic. M4 cannot produce a speedup claim.

## Required CPU/mock proof after review

Before a CUDA M4 run, one coordinated Rust CPU/mock proof must:

1. build the exact 533-entry subset from the operation manifest;
2. validate all header/schema/profile/source/conversion/catalog/budget/plan
   identities and reject every cross-domain substitution;
3. reproduce all file, device, upload, and plan-preimage byte arithmetic;
4. exercise the exact owner-thread order and prove no laboratory execution
   permit exists before four-rank adoption, graph, and fixture receipts;
5. prove the laboratory handle is not convertible to any production type;
6. execute both truncated layer-6-to-head shapes against an independent
   reference using the same native plane bytes;
7. mutation-test every codec/layout/projection, route, graph, tensor,
   page-table, cache, input, output-generation, and receipt field;
8. prove stale output and completion receipts are rejected across all warm
   repetitions;
9. exhaust the recoverable fault matrix with exact cleanup;
10. run each process-fatal class in a child and prove no forged cleanup or
    accepted output;
11. keep every allocation, queue, evidence record, and test deadline bounded;
    and
12. regenerate only synthetic small fixtures in Git.

CPU/mock acceptance does not open cn4 or establish checkpoint, model,
quality, capacity, or performance evidence. Actual M4 remains behind all
accepted prerequisites and fresh operator authorization.

## Exit and nonclaims

R2 is accepted only if it closes the six defects above and remains
substitution-resistant across source/output, production/laboratory, CPU/GPU,
recoverable/fatal, and truncated/full-model boundaries.

The base nonclaims remain in force. In particular, neither this design nor a
future M4 pass proves EXL3, a fit-capable full checkpoint, target-only
autoregressive decoding, MTP1–6, serving, concurrency, prefix/tier operation,
1M context, quality, capacity, or a performance advantage.
