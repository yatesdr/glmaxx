# Deterministic small-checkpoint runner v1 corrective amendment r3

Date: 2026-08-04

Status: corrective design candidate; adversarial acceptance required before
CPU/mock or CUDA implementation

GPU evidence: none

Base contracts:

- `docs/small-checkpoint-runner-v1.md`
- `docs/small-checkpoint-runner-v1-r2.md`

## Scope and precedence

This amendment is normative over conflicting M4 r1/r2 text. R2 corrects the
source/output, manifest, owner-thread, reset, and fatal-cleanup boundaries,
but still names an unspecified target-program v2 and a logical graph profile.
It predates the profile-specific target-program amendment, the ten-arena
physical graph plan, executor r5, and `GraphProfile.v3`.

Without those identities, a 533-tensor file can validate while its graph uses
a production target program, another module generation, an uncharged pending
logit span, or a stale page-table/weight address. This amendment supplies the
exact laboratory target-program and M4 execution record and binds every
graph-visible allocation. All r2 source, conversion, file, load, owner-thread,
fixture, numerical, reset, fault, evidence, and nonclaim requirements remain.

## Exact laboratory target program

M4 uses one separately typed target program. It is not the 58,794-binding
capacity-TR3 program, the 39,594-binding hybrid program, or either M3 replay
program. Its only target entry is layer 6 followed by the laboratory final
norm/head entry. There is no embedding or layer 0--5/7--77 entry.

The layer-6 entry contains exactly:

```text
19 protected/nonrouted bindings
256 combined gate/up NVFP4 bindings
256 down NVFP4 bindings
531 total layer bindings
```

The final-head entry contains exactly the final norm and rank-sharded
vocabulary head, for two bindings. All 533 bindings are byte-identical to the
accepted 192-byte laboratory semantic catalog projection and use the common
r3 16-byte encoding. Routed bindings permit only projection 4 with
`0x1202/0x1202` and projection 3 with the accepted `0x1201/0x1201` 1D/2D down
codec. Protected bindings have projection/layout zero.

The domains are:

```text
glmaxx.m4-target-program.layer6.v1.nvfp4-laboratory-layout-bound\0
glmaxx.m4-target-program.final-head.v1.nvfp4-laboratory-layout-bound\0
glmaxx.m4-target-program.v1.nvfp4-laboratory-layout-bound\0
```

The layer and final-head entry preimages retain the exact target-layer r2
scalar/hash field ordering and numerical/collective semantics, replace the
domain with the matching laboratory domain above, and serialize only their
closed r3 bindings. The final head uses the accepted distributed-sampling
composite:

```text
95fa7aa3b4b0b78a3f8313705d25e4c11682632fce6d8b8c2355b8130745f58c
```

The top-level target-program digest is:

```text
SHA256(
  "glmaxx.m4-target-program.v1.nvfp4-laboratory-layout-bound\0" ||
  laboratory_semantic_catalog_sha256 ||
  u16_le(1) || layer6_entry_sha256 || final_head_entry_sha256
)
```

The count is the number of layer entries, not the number of final-head
entries. The catalog is derived before the program and neither digest contains
the laboratory manifest or load-plan digest, avoiding a cycle. The manifest,
load plan, graph, and M4 program subsequently bind both catalog and target
program. An old target-program domain, production program, layer-6-only
program, 531-entry catalog, missing head, split projection, wrong layout, or
text digest fails before allocation.

## M4Program v1

Each concrete shape and eager/captured posture uses one exact 544-byte
`M4Program.v1` record. All integers are little-endian, all hashes are raw
32-byte values, and all reserved values are zero:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `G5M4PR1\0` |
| 8 | 2 | version, exactly `1` |
| 10 | 2 | record bytes, exactly `544` |
| 12 | 1 | mode: `1=DECODE`, `2=PREFILL` |
| 13 | 1 | rank count, exactly `4` |
| 14 | 1 | profile ID, exactly `3=NVFP4_LABORATORY_M4` |
| 15 | 1 | MTP depth, exactly `0` |
| 16 | 4 | real query rows |
| 20 | 4 | graph row bucket |
| 24 | 4 | sequence bucket, exactly `1` |
| 28 | 1 | attention transport |
| 29 | 1 | execution path: `1=EAGER`, `2=CAPTURED` |
| 30 | 2 | flags, zero |
| 32 | 32 | operation-manifest SHA-256 |
| 64 | 32 | four-rank laboratory-manifest-set SHA-256 |
| 96 | 32 | laboratory-domain `RankSetLoadPlan.v1` SHA-256 |
| 128 | 32 | exact laboratory target-program SHA-256 |
| 160 | 32 | `GraphProfile.v3` SHA-256 |
| 192 | 32 | exact `GraphMemoryPlan.v1` SHA-256 |
| 224 | 32 | executor target-only program-set SHA-256 |
| 256 | 32 | adopted module-set capability SHA-256 |
| 288 | 32 | accepted laboratory rank-set resource-budget SHA-256 |
| 320 | 32 | exact `CollectiveSchedule.v2` SHA-256 |
| 352 | 32 | M4 fixture SHA-256 |
| 384 | 32 | numerical-policy SHA-256 |
| 416 | 32 | route-table/topology SHA-256 |
| 448 | 32 | codec-capability SHA-256 |
| 480 | 32 | distributed-sampling composite SHA-256 |
| 512 | 32 | accepted NVFP4-laboratory M3 result SHA-256 |

The record digest is:

```text
SHA256("glmaxx.m4-program.v1\0" || exact 544-byte record)
```

The record has a 32-byte header and sixteen 32-byte identities. Every object
is supplied as a validated immutable typed value and its complete bytes are
reconstructed before accepting the digest. A caller cannot supply only a
hash, path, profile string, or native handle.

The predecessor field names the exact accepted NVFP4-laboratory M3 result for
the same source-control lineage, prompt/token IDs, layer-6 boundary input/cache,
topology route, and numerical policy. It does not import M3 weights, target
program, graph, or result type. A capacity/hybrid M3 result or design token
fails.

## Physical graph and executor binding

Each M4 decode/prefill graph uses:

```text
the exact laboratory target program
the executor target-only program set with no MTP member
the adopted target and device-validation module capabilities
one logical GraphProfile.v2 entry
one exact GraphMemoryPlan.v1
one GraphProfile.v3 binding
the exact collective schedule and topology route
the exact laboratory resource budget and final memory plan
```

The target module contains the layer-6, final norm/head, and diagnostic
distributed-greedy nodes required by the program. `mtp_program_present` is
zero and the MTP digest is all zero. Calling the runner MTP0 does not authorize
an MTP module or proposal node. Both fixture shapes have exactly one sequence;
the prefill graph evaluates the head only for its selected last row.

All ten graph-visible arenas exist and are charged once:

```text
1  immutable arguments
2  maximum graph scratch, including 154,880-byte rank-logit scratch
3  fixture target KV
4  fixture target indexer
5  recurrent state, including 309,760-byte CURRENT/NEXT pending logits
6  fixed collective spans
7  completion and device-validation status
8  1,982,245,376-byte laboratory weight payload
9  130,944-byte laboratory device codec metadata
10 fixture device page table
```

Class 30 and arena-5 pending-logit uses are present for both shapes. With the
sequence bucket fixed at `C=1`, CURRENT and NEXT are two disjoint 154,880-byte
rank-local vectors, exactly 309,760 bytes per rank. The selected head has one
simultaneously live row for both shapes, so target class 26 owns exactly
`L=1 * 154,880 = 154,880` bytes of rank-logit scratch in arena 2. The scratch
is not persistent state and cannot be placed in or charged to arena 5.
Proposal, q-state, draft-KV/indexer, boundary-hidden, and MTP bundle subranges
are zero because this is target-only MTP0. Those zero subranges are not
separate arenas, fake reservations, or permission to omit the nonzero
pending-logit allocation.

The truncated M4 distributed-greedy result is a diagnostic over the committed
laboratory head result. It does not implement production prefill/decode token
feedback: production prefill stores pending logits without emitting a token,
and production decode samples prior pending logits before embedding and
executing the sampled token. An M4 diagnostic token therefore cannot satisfy
or substitute for the full-checkpoint autoregressive smoke gate.

Arenas 8 and 9 are the four adopted laboratory generations and are read-only
inside the graph. Arena 10 is the exact reset fixture generation. Every
primary/auxiliary/metadata plane and every device page-table span appears in
the `GraphBufferUse.v1` table. The native executor derives all addresses from
ten owner-created `DeviceArenaBinding.v1` records; no M4 record, fixture, rank
manifest, or request contains a device address.

The resource budget is accepted before physical plans and contains the ten
exact byte/alignment pairs plus context/module, collective-library,
graph-runtime, allocator-padding, and emergency-escrow ceilings. The final
laboratory memory plan is constructed after `GraphProfile.v3`, binds its
digest and every concrete plan, and repeats the identical charges. The 533-
tensor manifest budget is not by itself the executor resource budget and
neither may be substituted for the other; the executor budget is derived from
and binds the completed laboratory budget.

Eager and captured controls share the same common plan, arena layout, native
span table, fixture generation, and schedule. They differ only in execution
path and program digest. No eager path is available to production serving.

## Corrected startup and execution order

R2's 17-step transaction remains, with these mandatory refinements:

1. the laboratory catalog, target program, load plan, executor resource
   budget, physical plans, and GraphProfile v3 are validated before owner
   allocation;
2. owner threads adopt the exact module set and expose graph-memory capability
   before building native spans;
3. the final laboratory memory plan charges all ten arenas and nonarena terms
   before any weight upload;
4. after four-rank weight adoption, each graph resolves and validates every
   use, captures with the device-validation node first, and returns a
   `RankGraphMemoryReceipt.v1`;
5. `GRAPHS_READY` requires four receipts for every required eager/captured
   shape and one common GraphProfile-v3 generation;
6. `FIXTURE_CACHE_READY` additionally requires the exact arena-3/4/10
   generations and reset digest; and
7. every execution permit binds `M4Program.v1`, the four rank-local arena
   binding digests, and the current argument/cache/output generations.

No module, allocator, workspace, descriptor route, graph instance,
communicator, or address is chosen on first execution. A one-rank fallback or
mixed module/program/plan generation is process-invalid.

## Corrected CPU/mock gate

After this amendment and every predecessor design are accepted, the
coordinated Rust proof must extend r2 to:

1. compile the 531-binding layer entry, two-binding final-head entry, and
   exact 533-binding laboratory target program from the semantic catalog;
2. independently serialize and mutation-test every target-program preimage,
   all 544 M4-program bytes, and every hash domain;
3. reject production, M3, old target-program, wrong sampling, 320/480-byte
   replay, and cross-profile objects before allocation;
4. reconstruct decode and prefill physical plans, all 32 class records, every
   buffer use, and all ten exact arenas from accepted operator plans;
5. prove class 30 and the exact 309,760-byte arena-5 CURRENT/NEXT pending-logit
   span are present, class 26 owns the exact 154,880-byte arena-2 rank-logit
   scratch span, and every proposal/draft subrange and MTP program is absent;
6. materialize four rank-local address/generation tables and prove all
   descriptor spans are owner-derived and within the accepted laboratory
   arenas;
7. subtract one byte from every nonzero use, class, arena, load-plan span, and
   budget term and prove rejection;
8. execute r2's two-shape native CPU reference, distributed-greedy,
   repetition, poison/reset, recoverable-fault, process-fatal, cleanup, and
   bounded-child matrix under the exact program identities; and
9. prove every laboratory handle, program, graph, result, and token is unable
   to reach production weight/profile types, `HEALTHY`, HTTP, scheduling,
   prefix publication, quality, or performance paths.

The implementation result must pin the two known M4-program encodings, target
entry/program encodings, all table and receipt hashes, source/test commit,
synthetic fixtures, and full test output. Model bytes and raw evidence remain
outside Git.

## Gate effect and nonclaims

Acceptance opens only the corrected M4 CPU/mock implementation after every
named predecessor is accepted. Actual M4 remains behind profile-matched M2,
accepted NVFP4-laboratory M3 device evidence, the laboratory
load/physical-memory CPU
proofs, exact module qualification, and a fresh cn4 occupancy check.

This design is not a checkpoint smoke result. A future M4 pass remains a
bounded 533-tensor truncated layer-6-to-head control; it does not prove a full
TR3 or hybrid checkpoint, autoregressive decoding, model quality, KV
capacity, concurrency, cold boot, hot reload, or throughput.
