# Resident-weight runtime generation v1

Date: 2026-08-03

Status: design candidate; implementation blocked on adversarial acceptance

## Purpose

GLMAXX must tune kernels and compatible runtime configuration without rereading
or retransferring model weights. Four long-lived Rust rank workers therefore
separate one immutable resident-weight generation from replaceable runtime
generations containing CUDA modules, graph instances, tuning tables, and
execution configuration.

This is not arbitrary live code replacement. A runtime generation cannot
change a source checkpoint, tensor membership, precision, native layout,
weight or metadata address, KV format or address space, collective route,
tokenizer, sampling semantics, target/MTP program, or quality posture. Any of
those changes requires a new cold model load and new resident identity.

## Identities

After the existing four-rank load transaction adopts its immutable arenas, the
coordinator constructs `ResidentWeightSetIdentity.v1` from:

```text
source-set SHA-256
four ordered native rank-manifest SHA-256 values
weight-policy SHA-256
operation-manifest SHA-256
target-program SHA-256
four ordered tensor-catalog SHA-256 values
four ordered load-plan SHA-256 values
four ordered allocation generations
four ordered weight/metadata arena byte counts
four ordered weight/metadata content SHA-256 values
native format and layout-policy SHA-256 values
```

Its identity is
`SHA256("glmaxx.resident-weight-set.v1\0" || the fixed fields above in the
displayed order)`, with integers little-endian. Device pointers are deliberately
not part of the portable digest; every rank separately retains its owner-local
base pointers and proves that they do not change across runtime generations.

A candidate `RuntimeGenerationManifest.v1` is GLMAXX canonical JSON: UTF-8,
lexicographically ordered object keys, no duplicate or unknown keys, no
whitespace outside strings, ordered arrays, and no floating-point numbers.
Values that originate as floats are represented by fixed-width integer bit
patterns. It binds:

- resident-weight-set identity;
- engine, step, kernel, graph, tensor-catalog, KV, collective, sampling, and
  MTP ABI versions;
- exact SM target `sm_120`, CUDA driver minimum/maximum, and four GPU UUIDs;
- ordered cubin/module SHA-256 values and required exported symbols;
- compile options, register/shared-memory resource ceilings, and dynamic
  shared-memory attributes per kernel;
- complete tuning-table and graph-profile SHA-256 values;
- supported MTP depths and batch/context graph buckets;
- exact pre-reserved graph, module, scratch, and host-pinned byte maxima;
- deterministic canary input and expected-output digests; and
- configuration, build-receipt, and independent-review SHA-256 values.

The generation ID is
`SHA256("glmaxx.runtime-generation.v1\0" || canonical manifest bytes)`. Every
artifact is a nonempty regular file under
`/home/derek/glmaxx/cache/runtime-generations/<generation-id>/`; symlinks,
mutable paths, unexpected files, tags without digests, and files outside the
GLMAXX root fail closed. A bundle contains no model tensor or KV payload.

## Fixed ownership and resources

Each persistent rank thread owns its CUDA context, step stream, collective
stream, setup stream, adopted weight/metadata arenas, KV arenas, collective
handles, graph/module slots, and generation state. No other thread calls the
CUDA driver for that context or mutates its generation state.

Before production health, every rank reserves resources for exactly:

```text
one active runtime generation
one secondary runtime generation, used first for prepare and then for rollback
the maximum reviewed graph/scratch/pinned-host bytes for both slots
driver-owned CUDA module/graph HBM escrow for both slots
```

User-owned graph scratch and pinned slabs are allocated for both slots before
health. CUDA owns the internal module and graph allocations, so GLMAXX cannot
place those bytes in its arena; instead the startup ledger withholds an
independently measured free-HBM escrow for the active and secondary slot.
Preparation samples free HBM immediately before and after every driver-owned
allocation and rejects an actual or declared high-water mark above the
reviewed slot ceiling.

Candidate preparation cannot borrow KV pages, uncharged free memory, the
active generation's allocations, or another rank's budget. No new prepare may
start while the secondary slot contains a rollback generation. After health,
a candidate whose exact resources exceed the secondary slot or escrow is
rejected rather than allocating an unbudgeted fallback. The startup HBM ledger
charges both slots and both driver escrows even while the secondary slot is
empty.

## Rank state machine

Rank-local states are:

```text
ACTIVE(generation, epoch)
PREPARING(active, candidate, epoch)
PREPARED(active, candidate, receipt, epoch)
QUIESCED(active, candidate, receipt, epoch)
ACTIVE_WITH_ROLLBACK(candidate, old, epoch+1, grace_remaining)
ACTIVE(candidate, epoch+1)
POISONED
```

Only the coordinator's common command may advance a state. Every command binds
the resident identity, current generation, current epoch, candidate generation,
command ordinal, deadline, and common four-rank digest. Duplicate commands are
idempotent only when every byte and prior state match; skips, stale epochs,
conflicts, or rank-local substitutions poison the transaction.

## Prepare, quiesce, and commit

Preparation may run while old-generation steps execute. On each owner rank it:

1. authenticates the bundle and every ABI/resource field;
2. checks resident, GPU, driver, KV, collective, MTP, graph, and tuning
   compatibility;
3. loads candidate CUDA modules against the secondary slot's reserved driver
   escrow;
4. resolves every required symbol and queries actual resource attributes;
5. validates tuning entries against the admitted graph/profile catalog; and
6. returns a rank receipt binding module handles, exact consumed bytes, resource
   attributes, and unchanged resident pointers/identity.

Preparation reads only generation-bundle bytes. The weight reader and
weight-H2D interfaces are not reachable from this state.

After four matching prepare receipts, the scheduler stops selecting new
physical steps but continues bounded request admission and queueing. Already
selected work completes entirely on the old generation. Each rank then
synchronizes step, setup, and collective streams, proves zero outstanding graph
or collective work, creates candidate graph instances using the unchanged
resident/KV pointers, and runs the manifest's deterministic canaries in the
candidate slot's private scratch. A canary cannot modify live KV, scheduler
state, prefix state, RNG counters, or response buffers.

Only four matching `QUIESCED` receipts allow commit. The coordinator issues one
common commit command; each owner atomically changes its active generation and
epoch, retaining the old generation in the secondary slot as rollback. The
scheduler publishes the new epoch only after all four commit acknowledgements.
A step is bound to one epoch in its immutable step input, so target decode, all
MTP draft and verify work, sampling, and collectives for that physical step use
one generation on every rank.

Any failure before common commit unloads the candidate and resumes the old
generation without changing its epoch. Timeout while work is outstanding is a
failed transaction, not permission to destroy resources still visible to CUDA.

## Grace and rollback

The rollback generation remains resident until sixteen successful physical
steps have completed and their four-rank output consensus is published. During
that grace period, another prepare is forbidden. Any candidate failure detected
before output publication triggers a common rollback barrier: finish or cancel
only candidate-private work, synchronize all rank streams, prove no candidate
output or KV successor was published, swap all ranks back to the old
generation, and advance to a new epoch. Candidate modules/graphs are then
quarantined and unloaded.

If an asynchronous error prevents synchronization, any candidate result or KV
state may have become externally visible, a rank cannot acknowledge the common
rollback, or failure occurs after output publication, the process is poisoned
and fails every active/queued request. Rank-local rollback and continuing on
three ranks are forbidden.

After sixteen successes, all ranks receive one common retirement command and
unload the rollback generation. Epoch and command-ordinal overflow are
process-fatal before reuse; identifiers never wrap.

## Compatible configuration surface

A hot generation may change only:

- cubin/module implementations behind unchanged reviewed ABIs;
- tuning-table choices among already admitted shapes/layouts;
- CUDA graph instances for already admitted batch/context/MTP buckets;
- launch geometry and resource attributes within pre-reserved maxima; and
- scheduler selection weights that do not alter fairness, capacity, precision,
  cache, MTP, sampling, or collective semantics.

Rust engine logic, resident formats, tensor catalogs, graph key definitions,
KV addressing, request schemas, and model semantics are part of the cold binary
and cannot be hot-reloaded in v1.

## Zero-weight-traffic proof

The resident loader owns monotonic per-rank counters for model file opens,
model bytes read, tensor bytes verified, weight staging bytes, and weight H2D
bytes. Only the cold load capability can increment them. Every prepare,
quiesce, commit, rollback, and retirement receipt records before/after values,
all of which must be bit-identical.

The same receipts record weight/metadata base pointers, allocation generations,
byte counts, catalog identity, and full-arena content digests. Pointer/content
sampling alone is not the authority: zero traffic requires the capability and
counter proof as well as unchanged identity. Cubin/config/module reads and
module/graph allocations are reported separately.

## Cold start boundary

Cold start remains a separate measured path. It authenticates an already-native
rank image, streams it once into fixed arenas with bounded pinned staging, and
overlaps storage verification and rank-local H2D where the admitted storage
topology permits. It never source-unpacks NF3/EXL3 or re-quantizes NVFP4 at
service startup.

Evidence splits container/process setup, identity/index validation, storage
read, hashing, staging, H2D, readback verification, module load, collectives,
graphs, KV allocation, and health publication. Page-cache, prefix-cache, model
residency, and previous-process posture are explicit. Five GLMAXX samples and
five separately authorized immutable vLLM controls are retained; GLMAXX never
reuses or mutates the ongoing production vLLM resources.

## CPU proof and target evidence

The CPU implementation uses four persistent mock rank owners, fixed generation
slots, fixed resource ledgers, and injectable failure at every transition. It
proves:

- every legal prepare/commit/grace/retire and prepare/rollback path;
- all single-rank failures, receipt mutations, stale/duplicate commands,
  timeouts, and every transition pair;
- no step, including MTP3, can mix generations or ranks;
- queued requests survive bounded quiesce while selected steps finish old;
- zero model opens/reads/H2D and unchanged arena identity over at least ten
  successful reloads including rollback;
- fixed user-owned memory and host accounting plus bounded driver-owned
  module/graph escrow with no uncharged post-health growth;
- safe pre-publication rollback and process-fatal post-publication failure; and
- epoch/ordinal overflow and owner-thread violations fail closed.

After implementation review, SM120 evidence runs ten compatible reloads with
real resident weights, at least one injected candidate failure, eager and graph
canaries, and concurrent queued requests. It reports reload downtime, request
latency impact, module/graph bytes, all zero-weight-traffic counters, unchanged
HBM pointers, and every rank receipt. Performance tuning may claim a win only
after the complete correctness and quality gates remain matched.

## Gate sequence

1. adversarial design acceptance;
2. CPU state machine, resource ledger, capability counters, and exhaustive
   failure proof;
3. adversarial implementation/proof review;
4. SM120 module/graph prepare and canary without model execution;
5. real resident-weight MTP0 reload and rollback;
6. MTP3 generation-atomic reload;
7. ten-reload latency/zero-traffic evidence; and
8. matched cold-start measurement.

This design does not implement reload, accept a runtime generation, prove zero
weight traffic, authorize cn4, accept a cold-start claim, or establish quality,
capacity, latency, or throughput.
