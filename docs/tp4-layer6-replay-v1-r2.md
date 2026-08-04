# TP4 layer-6 replay gate v1 corrective amendment r2

Date: 2026-08-04

Status: corrective design candidate; adversarial acceptance required before
CPU fixture or CUDA implementation

GPU evidence: none

Base contract: `docs/tp4-layer6-replay-v1.md`

## Scope and precedence

The base M3 contract remains normative except where this amendment replaces
it. The base freezes one NVFP4-oriented, 320-byte replay record with only a
logical `GraphProfile.v2` identity. It predates:

- the profile-specific TR3 and NVFP4/NF3 target programs;
- `GraphProfile.v3` and `GraphMemoryPlan.v1`;
- the ten graph-visible arenas and owner-derived weight, metadata, and page
  table spans;
- the complete executor target-program set and module-set capability; and
- the acyclic resource-budget-to-physical-plan construction order.

Consequently the old record can admit the correct fixture while executing a
different physical layout or module generation. It also cannot distinguish a
split-EXL3 TR3 replay from a combined NVFP4/NF3 replay. `ReplayProgram.v1`, its
320-byte encoding, and its universal target-program-v2 wording are
superseded. Target math, real-fixture requirements, the independent reference
ladder, layer-7 winner reuse, cache transaction, fault matrix, matched
controls, timing, evidence, and nonclaims remain unchanged.

## Profile-scoped M3 results

M3 is no longer one representation-agnostic token. It is run and accepted
separately for exactly these replay profile IDs:

```text
1  CAPACITY_EXL3_TR3
2  HYBRID_W4A16_NF3
3  NVFP4_LABORATORY_REPLAY
```

Profile 1 uses the exact capacity target-program-v2 family, including three
distinct gate, up, and down EXL3 bindings for every routed expert. Profile 2
uses the exact hybrid target-program-v3 family, including the authenticated
combined gate/up and down codec/layout assignment for every expert. Profile 3
is a separate two-layer all-NVFP4 control using the exact M2
source/conversion lineage required by M4. It has 531 layer-6 bindings and 526
layer-7 bindings, but no final norm/head, production manifest, or serving
type. The profile-3 domains are:

```text
glmaxx.m3-target-program.layer.v1.nvfp4-laboratory-layout-bound\0
glmaxx.m3-target-program.v1.nvfp4-laboratory-layout-bound\0
```

Its two layer-entry preimages retain the target-layer r2 scalar/hash ordering,
use the matching layer domain above, and serialize the exact all-NVFP4 r3
16-byte bindings. Its top-level digest is the second domain followed by the
laboratory source/conversion-policy digest, `u16_le(2)`, and the layer-6 then
layer-7 entry digests. It is not the 533-tensor M4 target program because it
has no final head and includes layer 7. An old ten-byte binding, a rank-local
codec choice, or substitution among any of the three profiles fails.

Each result carries only its own profile token:

```text
tp4-layer6-replay-v1-r2-capacity-tr3-accepted
tp4-layer6-replay-v1-r2-hybrid-w4a16-nf3-accepted
tp4-layer6-replay-v1-r2-nvfp4-laboratory-accepted
```

No token proves another profile. The capacity token is the required
one-layer predecessor for a TR3 checkpoint smoke. The hybrid token is the
required one-layer predecessor for a full hybrid checkpoint smoke. The
laboratory token is the required M3 predecessor for M4. M4 reuses its
source/control lineage and accepted boundary fixture, but its 533-tensor
handle, target program, graph, and result remain separately typed and cannot
inherit the M3 laboratory token.

## Immutable replay weights

For each profile, `ReplayWeightSet.v2` is the closed layer-6/layer-7
projection of the accepted, profile-specific semantic catalog and complete
target program. The profile and complete target-program digest are inputs to
the replay catalog digest. Every routed projection has the exact r3 16-byte
binding plus its authenticated rank-local primary, auxiliary, and metadata
plane spans.

The capacity projection contains three routed descriptors per expert and
retains the tier-selected K=3/K=4 source metadata. The hybrid projection
contains two routed descriptors per expert and retains each expert-atomic
ModelOpt/NVFP4 or NF3 selection. The laboratory replay projection contains
two all-NVFP4 descriptors per expert under its separately accepted
source/conversion policy. A conversion, repack, representation change, or
missing plane between the accepted operator result and replay set fails.

The replay load plan fixes common relative offsets, lengths, and alignments
for every plane in arenas 8 and 9. Four owner threads resolve those common
spans to four rank-local adopted addresses and generations. It is a bounded
M3-only load plan and yields only `ReplayWeightHandle.v2`; it does not satisfy
a laboratory or production rank-set load plan and cannot enter service
startup.

## ReplayProgram v2

Every concrete fixture, execution path, and graph uses one fixed 480-byte
`ReplayProgram.v2`. All integers are little-endian, all hashes are raw 32-byte
values, and all reserved values are zero:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `G5M3RP2\0` |
| 8 | 2 | version, exactly `2` |
| 10 | 2 | record bytes, exactly `480` |
| 12 | 1 | mode: `1=DECODE`, `2=PREFILL` |
| 13 | 1 | primary layer, exactly `6` |
| 14 | 1 | continuation layer, exactly `7` |
| 15 | 1 | rank count, exactly `4` |
| 16 | 4 | real query rows |
| 20 | 4 | graph row bucket |
| 24 | 4 | sequence bucket |
| 28 | 1 | attention transport |
| 29 | 1 | execution path: `1=EAGER`, `2=CAPTURED` |
| 30 | 1 | replay profile ID, `1..3` |
| 31 | 1 | flags, zero |
| 32 | 32 | operation-manifest SHA-256 |
| 64 | 32 | replay-weight-catalog SHA-256 |
| 96 | 32 | layer-6/7 target-record-stream SHA-256 |
| 128 | 32 | exact profile target-program SHA-256 |
| 160 | 32 | `GraphProfile.v3` SHA-256 |
| 192 | 32 | exact `GraphMemoryPlan.v1` SHA-256 |
| 224 | 32 | executor target-only program-set SHA-256 |
| 256 | 32 | adopted module-set capability SHA-256 |
| 288 | 32 | accepted rank-set resource-budget SHA-256 |
| 320 | 32 | exact `CollectiveSchedule.v2` SHA-256 |
| 352 | 32 | fixture SHA-256 |
| 384 | 32 | numerical-policy SHA-256 |
| 416 | 32 | route-table/topology SHA-256 |
| 448 | 32 | codec-capability SHA-256 |

The record digest is:

```text
SHA256("glmaxx.m3-replay-program.v2\0" || exact 480-byte record)
```

The record contains 32 header bytes plus fourteen 32-byte identities. No
field is a text-encoded digest. Unknown modes, profiles, transports, paths,
or nonzero flags fail before hashing.

The layer-record stream retains the base ordering and 16-byte record encoding
but uses a profile-separated domain:

```text
SHA256(
  "glmaxx.m3-layer-records.v2\0" ||
  u8(profile_id) || three_zero_bytes ||
  profile_target_program_sha256 ||
  u32_le(record_count) || ordered 16-byte records
)
```

Every record is a byte-identical member of the selected target program,
and the stream is exactly the closed layer-6/layer-7 projection. A full
target-program digest from one profile cannot authenticate records from the
other. The replay executor receives the immutable record object and
reconstructs the digest; hash equality without membership, count, ordering,
and closed-set validation is insufficient.

## Physical graph and executor binding

For a concrete replay record, these values must agree exactly:

```text
mode / rows / buckets / transport
profile target program
layer-record stream
executor graph kind
target-only program-set digest
module-set capability and graph-memory ABI
logical GraphProfile.v2 entry
physical GraphMemoryPlan.v1
GraphProfile.v3 entry binding
collective schedule and topology route
rank-set resource budget
fixture and numerical policy
```

The program-set digest uses the accepted executor formula with
`mtp_program_present=0` and a zero MTP digest. Neither layer contains an MTP
node. The module set contains the exact target and device-validation families
for the chosen profile; a capacity, hybrid, or laboratory target module
cannot validate another profile's bindings.

M3 uses the exact ten logical arenas from the physical-memory contract.
Arenas 8 and 9 contain only the adopted replay weight generation, but retain
the executor's immutable weight and codec-metadata roles. Arena 10 contains
the exact fixture page-table generation. Target KV and indexer writes use
arenas 3 and 4. M3 has no final head, pending-logit output, proposal state, or
draft sidecar; therefore class 30 and every recurrent-state use are absent.
Arena 5 nevertheless has one fixed 256-byte aligned, zeroed guard allocation
in the replay resource budget so the mandatory ten-record native arena table
has no null/zero-length special case. No graph node reads or writes it.

The remaining arena lengths are exact accepted ceilings derived from the
two concrete layer programs, fixtures, operator workspaces, collective
schedule, and bounded result/status storage. The resource budget binds ten
byte/alignment pairs plus the context/module, collective-library,
graph-runtime, allocator-padding, and emergency-escrow ceilings. The
construction order is:

```text
accepted profile/catalog/operator plans
-> replay rank-set resource budget
-> GraphMemoryPlan.v1 per graph
-> GraphProfile.v3
-> final replay memory plan
-> ten owner-created DeviceArenaBinding.v1 records per rank
-> four RankGraphMemoryReceipt.v1 records
-> executable replay generation
```

The resource budget cannot contain the physical-plan or GraphProfile-v3
digest. The final replay memory plan contains both and repeats the same exact
charges. This preserves the accepted acyclic construction. Allocation,
pointer discovery, repack, workspace growth, route selection, or module load
inside capture or execution is forbidden.

Eager and captured controls use the same physical plan and native span table.
Only `execution_path` and its replay-program digest differ. Eager execution is
still M3-only and cannot become a production fallback.

## Fixture and result identity corrections

Both real fixtures add the replay profile ID, exact profile target-program
digest, layer-record-stream digest, replay catalog/load-plan digest,
GraphProfile-v3 digest, concrete physical-plan digest, program-set digest,
module-set digest, resource-budget digest, and all ten arena generations.

Layer-6 and layer-7 phase evidence identifies every primary, auxiliary, and
codec-metadata use. Capacity evidence retains gate/up/down values separately
and the K=3/K=4 class for every selected expert. Hybrid evidence retains each
expert's combined gate/up backend, down backend, layouts, and policy record.
Backend timing is reported separately; an aggregate routed-MoE time cannot
hide a missing or unused codec family.

Laboratory evidence retains all-NVFP4 layer-6/layer-7 membership and the
exact source/output conversion identity. It cannot be reported as hybrid
evidence even if some hybrid experts select NVFP4.

The two reference branches consume the same profile-specific native planes
as the device. A TR3 source-control delta cannot set a hybrid tolerance, and
neither production profile may borrow the laboratory all-NVFP4 reference.
The laboratory profile uses only its own accepted source/output lineage.

Every result additionally retains the common physical-plan identities and
four rank-local arena-binding/resolved-span receipts. A result is accepted
only when its profile-specific token, complete record bytes, and all
predecessor result hashes are present. Design acceptance or a result for the
other profile cannot open a checkpoint smoke.

## Corrected CPU/mock gate

After this design and all named dependencies are accepted, one coordinated
Rust implementation must extend the base CPU/mock gate to:

1. encode/decode and mutation-test all 480 bytes and both v2 hash domains;
2. reject every v1 record, 320-byte record, text digest, unknown profile, and
   nonzero reserved byte;
3. derive closed layer-6/layer-7 replay sets for all three profiles;
4. prove every TR3 expert has distinct gate/up/down bindings, every hybrid
   expert has the exact combined gate/up and down policy, and every
   laboratory expert has the exact all-NVFP4 policy;
5. reject cross-profile target programs, records, catalogs, modules,
   capabilities, graph plans, schedules, fixtures, and numerical policies;
6. reconstruct exact physical plans and all buffer uses for decode and the
   accepted prefill bucket, including every immutable plane and page-table
   span;
7. materialize four different rank-local ten-arena address/generation tables
   while preserving one common plan/profile/program/schedule identity;
8. prove class 30 and recurrent uses are absent, arena 5 is exactly the
   unused 256-byte guard, and no MTP program can enter the program set;
9. subtract one byte from every nonzero class, use, arena, and budget term in
   turn and prove fail-closed rejection;
10. execute the complete base native-CPU layer-6/layer-7 reference and cache,
    winner-reuse, continuation, fault, cleanup, and stale-generation matrix
    for all three profiles; and
11. prove no replay handle/token can convert to another profile, the
    M4 laboratory path, production `HEALTHY`, HTTP, scheduler, or prefix
    publication.

The implementation review must pin exact source and synthetic fixture hashes,
the three known record encodings, every table digest, all test output, and the
absence of CUDA/model claims. Only an accepted CPU/mock result may open
profile-matched SM120 M3 execution after the accepted M2 and collective
results and a fresh occupancy check.

## Gate effect and nonclaims

Adversarial acceptance of this amendment opens only the coordinated
profile-specific CPU/mock implementation. It does not accept current Rust,
CUDA, a checkpoint, M3 device execution, M4, full-model logits, quality,
capacity, concurrency, serving, hot reload, cold boot, or performance.

The base nonclaims remain. A device M3 result is profile-local and covers only
layers 6 and 7 plus offline continuation sensitivity. It never substitutes
for a real checkpoint smoke or the later full decode path.
