# Hybrid serving weight policy, rank manifest, and load plan v1

Date: 2026-07-30

Status: design candidate; adversarial review required before CPU
implementation

GPU evidence: none

## Purpose

This contract supplies the missing fit-capable `hybrid-serve` boundary for a
full GLM-5.2 checkpoint. It coordinates:

- expert-atomic weight policy v2;
- strict hybrid rank manifests;
- source/conversion and per-position quality identity;
- a layout-bound rank-invariant semantic catalog;
- exact variable tensor/file/device arithmetic;
- a profile-specific 1M/MTP6 serving budget;
- a domain-separated four-rank load plan; and
- production type state that cannot be confused with capacity EXL3 or the M4
  laboratory subset.

The current production schema `glmaxx.rank-manifest.v0.2.2` and
`WeightPolicy.v1` remain capacity-EXL3-only implementation paths. They must
not be widened or reinterpreted.

The hybrid path uses:

```text
weight policy       glmaxx.weight-policy.v2
rank manifest       glmaxx.rank-manifest.hybrid-serve.v1
profile budget      glmaxx.hybrid-serve-budget.v1
```

This design selects no actual tensor membership, authorizes no conversion,
and makes no fit, quality, capacity, or speed claim.

## Preconditions

Hybrid conversion remains unavailable until exact accepted artifacts exist
for:

1. current native format and protected-header semantics;
2. direct EXL3 source execution and every selected source representation;
3. fused routed-MoE r3 and NVFP4 1D/2D variants;
4. target-program v2 layout binding;
5. complete target and recurrent draft execution;
6. per-position candidate quality evidence;
7. matched actual-SM120 performance evidence;
8. full rank-local physical-byte derivation;
9. graph/workspace/collective/load-staging measurements;
10. exact 1M target/draft/indexer/page-table budget; and
11. current-tree converter, reader, loader, and production-startup reviews.

A design token, kernel cubin, average KLD, nominal HBM total, aggregate free
memory, or model marketing name cannot satisfy a prerequisite.

## Fixed full-model inventory basis

The full MTP-capable serving profile includes:

```text
target sparse layers            75  (3..77)
draft sparse layer               1  (78)
experts per sparse layer       256
expert instances N          19,456
capacity-EXL3 routed records 58,368  (3 * N)
protected records            1,217
capacity tensor total       59,585
```

Draft weights are always present. Runtime MTP0–6 is a request/graph posture,
not a weight-policy mutation.

For the hybrid policy define:

```text
F   expert instances whose gate/up pair is combined NVFP4 1D
D1  expert instances whose down projection is NVFP4 1D
D2  expert instances whose down projection is NVFP4 2D
D   D1 + D2
```

Bounds are:

```text
0 <= F,D1,D2 <= N
D1 + D2 <= N
```

Every remaining gate, up, or down projection is direct EXL3 source.

Hybrid requires at least one NVFP4 physical projection and at least one EXL3
physical projection. The completed budget, not this weak membership minimum,
enforces how much EXL3 is required to retain 1M capacity.

## WeightPolicy v2

`WeightPolicy.v1` is not accepted by the hybrid manifest. V1 has three
independent logical assignments, aliases NVFP4 1D/2D, permits mixed gate/up
backends, and charges two records where combined FC1 has one.

V2 contains one 256-byte record for every `(layer,expert)` in ascending
layer/expert order. Backend IDs are:

```text
gate_up_backend
  1 EXL3_SPLIT_SOURCE
  2 NVFP4_1D_COMBINED

down_backend
  1 EXL3_SOURCE
  2 NVFP4_1D
  3 NVFP4_2D
```

The 256-byte `ExpertPhysicalPolicy.v2` record is:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 2 | layer ID, `3..78` |
| 2 | 2 | expert ID, `0..255` |
| 4 | 1 | gate/up backend |
| 5 | 1 | down backend |
| 6 | 2 | flags; zero |
| 8 | 8 | gate/up rank payload bytes |
| 16 | 4 | gate/up file metadata bytes |
| 20 | 2 | gate/up codec ID |
| 22 | 1 | gate/up representation: `1=split pair`, `4=combined` |
| 23 | 1 | gate/up physical descriptor count: `2` or `1` |
| 24 | 2 | gate/up value-layout ID |
| 26 | 2 | gate/up scale-layout ID |
| 28 | 2 | down codec ID |
| 30 | 1 | down projection ID, exactly `3` |
| 31 | 1 | down descriptor count, exactly `1` |
| 32 | 8 | down rank payload bytes |
| 40 | 4 | down file metadata bytes |
| 44 | 2 | down value-layout ID |
| 46 | 2 | down scale-layout ID |
| 48 | 32 | gate per-position quality-evidence SHA-256 |
| 80 | 32 | up per-position quality-evidence SHA-256 |
| 112 | 32 | down per-position quality-evidence SHA-256 |
| 144 | 32 | gate/up conversion/source-route SHA-256 |
| 176 | 32 | down conversion/source-route SHA-256 |
| 208 | 32 | physical-realization semantic SHA-256 |
| 240 | 16 | reserved; zero |

Legal physical values are exact:

| Representation | Payload | Metadata | Codec | Value/scale layouts |
|---|---:|---:|---|---|
| EXL3 split gate+up | 2,385,928 | 192 | `CODEC_EXL3_SOURCE` | `0/0` |
| NVFP4 combined gate/up | 3,538,944 | 128 | `CODEC_NVFP4_1D` | `0x1202/0x1202` |
| EXL3 down | 1,192,964 | 96 | `CODEC_EXL3_SOURCE` | `0/0` |
| NVFP4 1D down | 1,769,472 | 128 | `CODEC_NVFP4_1D` | `0x1201/0x1201` |
| NVFP4 2D down | 1,769,472 | 128 | `CODEC_NVFP4_2D` | `0x1201/0x1201` |

Every quality and route digest is nonzero. EXL3 and NVFP4 1D/2D route
identities are distinct even when plane lengths match.

The physical-realization digest in bytes `208..240` is:

```text
SHA256(
  "glmaxx.expert-physical-realization.v2\0"
  || record bytes 0..48
)
```

Here `0..48` is the half-open byte range ending before the quality digests.
The manifest's `expert_policy_record_sha256` is ordinary SHA-256 over the
complete 256-byte record. Neither digest includes itself.

The 256-byte `WeightPolicyHeader.v2` is:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `G5WTP2\0\0` |
| 8 | 2 | version `2` |
| 10 | 2 | header bytes `256` |
| 12 | 1 | profile, exactly `3=hybrid-serve` |
| 13 | 1 | rank count, exactly `4` |
| 14 | 1 | maximum MTP depth, exactly `6` |
| 15 | 1 | reserved; zero |
| 16 | 4 | expert-policy records, exactly `19,456` |
| 20 | 4 | protected records, exactly `1,217` |
| 24 | 4 | `F` |
| 28 | 4 | `D1` |
| 32 | 4 | `D2` |
| 36 | 4 | routed physical descriptor count |
| 40 | 4 | complete tensor count |
| 44 | 4 | reserved; zero |
| 48 | 8 | rank tensor-plane bytes |
| 56 | 8 | rank file metadata bytes |
| 64 | 32 | source-set SHA-256 |
| 96 | 32 | operation-manifest SHA-256 |
| 128 | 32 | protected semantic-inventory SHA-256 |
| 160 | 32 | deterministic selection-receipt SHA-256 |
| 192 | 32 | quality-policy SHA-256 |
| 224 | 32 | matched-performance-policy SHA-256 |

The policy digest is:

```text
SHA256(
  "glmaxx.weight-policy.v2\0"
  || 256-byte header
  || 19,456 ordered ExpertPhysicalPolicy.v2 records
)
```

The canonical policy body after the domain is 4,980,992 bytes. The complete
digest input additionally includes the displayed domain prefix. The body is
rank-invariant. Rank-local source ranges, native plane hashes, global scales,
file offsets, and arena offsets are excluded and remain mandatory in each
rank manifest.

The policy does not bind the budget digest, avoiding a policy/budget hash
cycle. The budget binds the finalized policy.

## Source and conversion identity

Hybrid may consume multiple authenticated source checkpoint forms, so it
uses a canonical sorted `source_set` rather than pretending that every
physical projection came from one file tree.

Each source-set entry binds:

```text
source_id
repository and revision
model revision and configuration digest
complete file-inventory digest
source index/manifest digest
codec/source-format identity
logical-base provenance digest
```

The set identifies exactly one protected-tensor authority. Every selected
output tensor binds one source ID, source tensor/component identity, byte
range, dtype/shape, and source hash. A source slice cannot supply two output
tensor IDs.

All source entries must trace to the same pinned logical GLM-5.2 base through
an accepted provenance/equivalence result. Shape or model name alone is
insufficient. Protected tensors come only from the protected authority; a
second checkpoint cannot silently replace them.

The conversion block binds source commit, container, Rust/CUDA/CUTLASS
toolchains, format/engine/operation manifests, target program, exact
WeightPolicy.v2, codec capability, quality/performance selection receipt, and
converter implementation.

Per-tensor conversion routes are closed:

```text
BYTE_EXACT_PROTECTED
EXL3_SOURCE_RETAIN
NVFP4_1D_1201_TO_1202_PERMUTE
NVFP4_1D_1202_RETAIN
NVFP4_2D_TO_1D_REQUANTIZE_REVIEWED
NVFP4_1D_DOWN_RETAIN
NVFP4_2D_DOWN_RETAIN
```

The 2D-to-1D route remains unavailable without its exact accepted conversion
and per-position quality artifact. No runtime repack or conversion exists.

## Deterministic policy selection

The hybrid manifest validates an immutable membership; it does not choose one
from current load, request mix, or available HBM.

The selection receipt consumes:

```text
all logical gate/up/down candidate quality records
joint gate/up conversion quality where applicable
per-position full-vocabulary values and classifications
actual-shape inclusive SM120 timing by backend and row/context band
observed hot-expert distribution for the pinned workload corpus
physical file/device bytes and workspace/graph effects
the immutable hybrid budget-constraint envelope
quality thresholds and objective/tie-break policy
```

The constraint envelope fixes required context, MTP depth, escrow, and every
term that the final budget must measure. It is an input to selection rather
than the finalized budget artifact, so the finalized policy and budget do
not form a hash cycle.

Gate and up remain separately scored for quality but are selected
expert-atomically. The optimizer cannot choose an NVFP4 gate with EXL3 up,
use an average KLD to hide a failing position, substitute a different row
mix, or credit an unmatched kernel time.

The deterministic objective, constraints, candidate ordering, and
tie-breaks are hash-bound. The output is the complete ordered policy record
stream, not an allowlist interpreted at runtime. The selection-receipt
artifact commits its inputs and that selected record stream, but excludes the
WeightPolicy header and final policy digest; the header then commits the
receipt. Re-running the same inputs must produce byte-identical receipt and
policy bytes without a self-reference.

A membership may enter conversion only when:

- every protected and routed logical role has accepted quality evidence;
- every selected physical route has accepted actual-SM120 correctness and
  inclusive timing;
- all quality gates pass per position;
- the completed budget fits independently on all four ranks;
- 1,048,576 target positions plus the MTP6-capable draft/indexer terms fit;
  and
- no unresolved measurement blocker remains.

## Strict hybrid rank manifest

The manifest schema is:

```text
glmaxx.rank-manifest.hybrid-serve.v1
```

Its profile is:

```text
name                         hybrid-serve
scope                        full-glm-5.2-target-and-draft
serving_allowed              true
production_health_allowed    true
http_allowed                 true
prefix_namespace_allowed     true
maximum_mtp_depth            6
weight_policy_schema         glmaxx.weight-policy.v2
```

The top-level strict JSON contains exactly:

```text
schema
rank
tp_degree
profile
model
tokenizer
source_set
conversion
review
integrity
tensor_count
rank_tensor_plane_bytes
file_payload_region_bytes
file_codec_metadata_bytes
device_weight_arena_bytes
device_metadata_arena_bytes
weight_policy
tensor_contract_sha256
tensors
```

Unknown or duplicate fields, noncanonical JSON, timestamps, random UUIDs,
host paths, credentials, free-form metadata, and alternate numeric spellings
fail.

The manifest profile binds the immutable
`budget_constraint_envelope_sha256`; it does not contain the finalized
hybrid-budget digest. The finalized budget can therefore bind all four
manifests without a hash cycle. The later load-plan header binds that
finalized budget digest.

Every file contains protected, EXL3, and NVFP4 tensors and uses header flags:

```text
NVFP4 | EXL3 | PROTECTED | HYBRID = 30
```

The direct-layout bit is clear because `spec/format-v0.md` requires every
file containing source EXL3 to clear it. Missing or extra membership bits
fail. Existing files produced before the protected-header correction are
invalid rather than accepted under a second spelling.

Tensor records include every production v0.2.2 field plus:

```text
projection_id
value_layout_id
scale_layout_id
representation_source_sha256
quant_policy_sha256
expert_policy_record_sha256
source_binding
conversion_route
primary/auxiliary/metadata hashes and bytes
```

The engine derives the expected physical tensor inventory from the compiled
operation manifest, protected inventory, and WeightPolicy.v2. Manifest names
or tensor counts never define membership.

Physical routed names are closed:

```text
EXL3 gate   model.layers.{layer}.mlp.experts.{expert}.gate_proj.weight
EXL3 up     model.layers.{layer}.mlp.experts.{expert}.up_proj.weight
NVFP4 FC1   model.layers.{layer}.mlp.experts.{expert}.gate_up_proj.weight
all down    model.layers.{layer}.mlp.experts.{expert}.down_proj.weight
```

`layer` and `expert` use canonical unsigned decimal with no leading zero.
Protected names remain the compiled protected inventory. A source alias or
alternate combined-FC1 spelling is not a physical tensor name.

## Tensor count and physical byte arithmetic

The routed representation counts are:

```text
EXL3 descriptors    = 3N - 2F - D
NVFP4 descriptors   = F + D
routed descriptors  = 3N - F
tensor_count T      = 1,217 + 3N - F
                    = 59,585 - F
```

The capacity-EXL3 protected tensor-plane content is independently fixed at:

```text
protected tensor planes per rank = 11,959,396,352
```

Hybrid tensor-plane content bytes per rank are:

```text
11,959,396,352
+ (3N - 2F - D) * 1,192,964
+ F * 3,538,944
+ D * 1,769,472

= 81,590,319,104
  + F * 1,153,016
  + D *   576,508
```

File codec metadata bytes are:

```text
(3N - 2F - D) * 96 + (F + D) * 128

= 5,603,328 - 64F + 32D
```

The current native-file and arena ordering gives each EXL3 projection a
1,193,216-byte slot (1,192,964 plane bytes plus 252 bytes before the next
plane); NVFP4 planes are already multiples of 256. Protected planes have no
hidden alignment term, and the final protected tensor ends the ordered
inventory without a rounded tail. Therefore the native file payload region
and device weight arena are equal:

```text
file_payload_region_bytes
  = device_weight_arena_bytes

device_weight_arena_bytes
  = 11,959,396,352
    + (3N - 2F - D) * 1,193,216
    + F * 3,538,944
    + D * 1,769,472

  = 81,605,027,840
    + F * 1,152,512
    + D *   576,256
```

The implementation must also derive this by ordering every descriptor plane;
the closed form is a cross-check, not file-layout or allocation authority.
`rank_tensor_plane_bytes`, `file_payload_region_bytes`, and
`device_weight_arena_bytes` are distinct manifest fields even though the last
two are equal under this exact ordering. Equating tensor-plane bytes with
either aligned total fails.

Let:

```text
R = 3N - F
L = byte length of the final metadata-bearing descriptor in tensor-ID order
    (96 for EXL3, 128 for NVFP4)
```

Every routed descriptor has one nonempty metadata record, each starts on a
256-byte device boundary, and the final arena end is not rounded:

```text
device_metadata_arena_bytes = (R - 1) * 256 + L
```

The manifest records `L` only indirectly through the authenticated final
descriptor. A caller cannot choose it.

All arithmetic is checked. Every rank has the same counts and byte totals but
different authenticated payload/file identities. A one-byte-short plane,
arena, metadata region, or total fails before allocation.

## Rank-invariant hybrid semantic catalog

Hybrid uses a separate 224-byte semantic entry. It does not reuse reserved
bytes in the production 128-byte or laboratory 192-byte catalogs.

Offsets 0 through 110 (111 bytes) retain the production semantic fields. The
extension is:

| Offset | Bytes | Field |
|---:|---:|---|
| 111 | 1 | projection ID |
| 112 | 2 | value-layout ID |
| 114 | 2 | scale-layout ID |
| 116 | 32 | representation-source SHA-256 |
| 148 | 32 | quantization-policy SHA-256 |
| 180 | 32 | expert-policy-record SHA-256 |
| 212 | 12 | reserved; zero |

Protected records have zero extension bytes. EXL3 records have projection
`1`, `2`, or `3`, zero layout IDs, the accepted EXL3 source/mapping digest,
zero quantization-policy digest, and their expert-policy-record digest.
NVFP4 records have projection `4` or `3`, exact r3 layout IDs, layout-source
and quantization-policy digests, and the same expert-policy-record digest for
their `(layer,expert)`.

The catalog digest is:

```text
SHA256(
  "glmaxx.hybrid-serve-tensor-catalog.v1\0"
  || tensor_count:u32_le
  || T entries in tensor-ID order
)
```

Tensor IDs are ascending exact UTF-8 name in the complete policy-derived
physical set. They are profile-local; a capacity tensor ID cannot be reused
without the hybrid catalog and plan identities.

All four ranks derive byte-identical semantic entries. Rank-local source
ranges, payload/global-scale hashes, file offsets, and arena offsets remain
outside the common catalog and mandatory in rank-local records.

## Hybrid serving budget

The budget schema is:

```text
glmaxx.hybrid-serve-budget.v1
```

It binds the exact WeightPolicy.v2, four manifests/catalogs, native
library/modules, graph profile, route/collective table, cache ABIs, page-table
ABI, target/draft program, serving configuration, and measurement commands.
The four manifests bind the common constraint envelope rather than this
finalized artifact, so this one-way binding is acyclic.

For each rank it records and independently validates:

```text
measured pre-context and post-context usable HBM
weight and metadata arenas
context/module resident bytes
graph runtime and graph-resident bytes
maximum mutually exclusive workspace
collective buffers and library-internal delta
load staging/readback and pinned-host terms
target KV committed/slack bytes at 1,048,576 positions
target indexer committed/slack bytes
draft KV and draft indexer committed/slack bytes
target/draft page tables and active-transaction journals
model/program/manifest metadata
allocator padding and fragmentation
unallocated emergency escrow
required total and observed headroom
```

Host DRAM, pinned-host, and NVMe reservations are separate terms. They cannot
hide in HBM or rescue an active 1M request that the profile claims remains in
HBM.

The executable state requires:

```text
measurement_status              complete
conversion_allowed              true
serving_allowed                 true
production_health_allowed       true
maximum_total_positions         1,048,576
maximum_mtp_depth               6
unmeasured_blockers             []
quality_gate                    PASS
kernel_gate                     PASS
layer_replay_gate               PASS
checkpoint_preconditions        PASS
```

The minimum rank headroom decides. Aggregate free HBM cannot compensate for a
failing rank. Runtime `cuMemGetInfo` may reject a completed budget but cannot
expand it.

## Hybrid rank-set load plan

Hybrid reuses the physical `RankSetLoadPlan.v1` encoding with:

```text
profile byte                    3
verification mode               FULL_SHA256 on first load
rank count                      4
tensor count                    T = 59,585 - F
tensor catalog                  224-byte hybrid domain
profile budget                  completed hybrid budget
```

The plan preimage size is:

```text
416 + 4 * 248 + 4 * T * 64
= 1,408 + 256T
= 15,255,168 - 256F bytes
```

Its digest is:

```text
SHA256(
  "glmaxx.rank-set-load-plan.v1.hybrid-serve-v1\0"
  || plan preimage
)
```

Capacity EXL3 and NVFP4 laboratory retain their own plan domains and catalogs.
A profile-byte, tensor-count, catalog, policy, budget, or domain mutation
cannot convert one plan into another.

The dedicated hybrid builder accepts only:

- four validated hybrid manifests;
- one exact WeightPolicy.v2 and selection receipt;
- one completed hybrid budget;
- accepted codec/layout/target-program capabilities; and
- four exact SM120 device identities.

It rejects `WeightPolicy.v1`, production v0.2.2 manifests, laboratory
manifests, capacity/laboratory budgets, and incomplete measurement or quality
state.

## Production type state and health

The hybrid load transaction retains quarantine through four-rank full
verification and adoption. It yields:

```text
ProductionWeightHandle::Hybrid(HybridWeightHandle)
```

only after four exact adoption acknowledgments under the hybrid plan domain.
It never yields the capacity or laboratory handle variant.

Unlike M4, a hybrid handle may proceed through the remaining production
startup gates: target/draft program binding, cache arenas, page-table
receipts, graphs, collectives, known-answer tests, and final all-rank health
consensus. Adoption alone does not publish `HEALTHY` or bind HTTP.

Every graph, step input, prefix namespace, cache namespace, result, and
serving configuration binds the exact WeightPolicy.v2 and hybrid
target-program/catalog/plan identities. A request cannot change membership.

Any validation, load, adoption, graph, collective, cache, or startup
disagreement aborts all ranks. There is no rank-local codec fallback,
capacity/laboratory downgrade, partial model, or retry under a different
policy inside the process.

## Required CPU proof after review

Before hybrid conversion or CUDA load, one coordinated Rust CPU proof must:

1. encode and mutation-test every byte of both WeightPolicy.v2 records;
2. derive all 19,456 expert records and reject every missing/duplicate/mixed
   gate/up or unsupported codec/layout combination;
3. validate source-set equivalence, protected authority, per-tensor source,
   conversion, and native-output identities;
4. independently regenerate deterministic policy selection from accepted
   synthetic evidence inputs;
5. derive the complete policy-dependent tensor inventory and all four
   rank-invariant 224-byte catalogs;
6. reproduce the exact `T`, tensor-plane, file payload-region, file metadata,
   device weight, device metadata, and plan-preimage formulas for boundary
   policies;
7. compare the closed forms with descriptor-by-descriptor arena planning;
8. reject one-byte-short, overflow, final-metadata, count, and alignment
   mutations;
9. prove capacity/laboratory/hybrid schema, policy, catalog, budget, plan, and
   handle substitution fails;
10. validate a completed synthetic four-rank 1M/MTP6 budget and reject every
    omitted or asymmetric term;
11. run four-rank prepare/adopt/failure matrices with CPU device fakes;
12. prove no adopted handle reaches health before every later startup receipt;
13. prove request/graph/cache/prefix identities cannot change policy
    membership; and
14. regenerate only small synthetic fixtures in Git.

CPU acceptance opens the v2 policy, hybrid manifest/catalog/budget, and
builder implementation. It does not authorize source conversion, a full
checkpoint, cn4, CUDA, quality, capacity, or performance claims.

## Exit criteria and nonclaims

This design passes only if review confirms:

- gate/up physical selection is expert-atomic;
- 1D/2D NVFP4 and EXL3 identities cannot alias;
- source, conversion, selection, and native output are distinct;
- all variable tensor/file/device/plan arithmetic is exact;
- hybrid cannot collide with capacity or laboratory identities;
- the completed budget truthfully retains 1M/MTP6 serving capacity;
- production health remains after all load/runtime gates; and
- no implementation, conversion, checkpoint, device, quality, capacity, or
  speed evidence is implied.
