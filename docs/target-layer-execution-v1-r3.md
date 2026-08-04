# Target-layer execution v1 r3 profile-program amendment

Date: 2026-08-04

Status: corrective design candidate; adversarial acceptance required before
CPU or CUDA implementation

GPU evidence: none

## Scope and precedence

The target-layer r2 program compiler fixes every sparse target layer at 512
routed bindings and a total of 39,594 target bindings. That is correct only
when every expert has one combined gate/up descriptor plus one down
descriptor. The real TR3 3.25-bpw checkpoint instead has separate gate, up,
and down EXL3 descriptors for every expert, so it requires 768 routed
bindings per sparse layer.

R2 also serializes ten-byte bindings with no projection or physical-layout
field, while the later NVFP4 and W4A16/NF3 contracts require an exact
16-byte layout-bound record. A tensor ID is sufficient for execution only
after compilation; it is not a semantic replacement for gate/up/down and
codec-layout validation.

This amendment is normative over conflicting target-layer v1/r2 program
serialization and count text. Target math, phases, rows, page writes,
pending-logit slots, collective order, logical buffer lifetimes, precision,
and numerical controls remain unchanged. The physical GraphProfile v3 and
ten-arena plan remain separate required successors.

The engine supports two production target-program families required by the
goal and one separately typed M4 laboratory family:

```text
CAPACITY_EXL3_TR3       split EXL3 gate, up, and down
HYBRID_W4A16_NF3       combined ModelOpt/NVFP4 or NF3 gate/up plus down
NVFP4_LABORATORY_M4    separately bounded all-NVFP4 layer-6 control
```

They have different hash domains and type states. No request, rank, graph,
kernel, or hot-reload generation may reinterpret one as another.

## Common 16-byte binding

Every r3 target binding is exactly:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 4 | `tensor_id:u32_le` |
| 4 | 2 | `role_id:u16_le` |
| 6 | 2 | `expert_id:i16_le`; `-1` means nonexpert |
| 8 | 2 | `codec_id:u16_le` |
| 10 | 1 | `projection_id:u8` |
| 11 | 1 | reserved zero |
| 12 | 2 | `value_layout_id:u16_le` |
| 14 | 2 | `scale_layout_id:u16_le` |

Projection IDs are closed:

```text
0 nonrouted
1 split gate
2 split up
3 down
4 combined gate/up
```

Within every entry records are strictly ordered by
`(role_id,expert_id,projection_id,tensor_id)`. The first three fields are a
unique semantic key; tensor ID is the final identity tie-breaker. Duplicate
semantic keys, duplicate tensor IDs, noncanonical order, unknown values, or a
nonzero reserved byte fail before hashing.

Every field is derived from an already authenticated semantic catalog,
decoded codec metadata, immutable weight policy, and rank-set load plan. All
four ranks produce the same record bytes. Rank-local file ranges, payload
hashes, device offsets, addresses, and generations remain separately checked
resident-binding identities and never enter this common record.

## Capacity EXL3 TR3 target program

The capacity family permits only:

| Representation | Projection | Codec | Value layout | Scale layout |
|---|---:|---|---:|---:|
| protected/nonrouted | 0 | authenticated plain codec | 0 | 0 |
| split EXL3 gate | 1 | `CODEC_EXL3_SOURCE` | 0 | 0 |
| split EXL3 up | 2 | `CODEC_EXL3_SOURCE` | 0 | 0 |
| EXL3 down | 3 | `CODEC_EXL3_SOURCE` | 0 | 0 |

Every sparse target expert has exactly one projection 1, one projection 2,
and one projection 3. Gate and up are distinct descriptors and tensor IDs;
they are never combined or inferred from adjacency. Their K=3/K=4 width is
an authenticated per-expert source property bound by the capacity policy and
resident tensor metadata, not another target-program projection value.

The four domains are:

```text
glmaxx.target-program.embedding.v2.capacity-exl3-layout-bound\0
glmaxx.target-program.layer.v2.capacity-exl3-layout-bound\0
glmaxx.target-program.final-head.v2.capacity-exl3-layout-bound\0
glmaxx.target-program.v2.capacity-exl3-layout-bound\0
```

Embedding, layer, and final-head preimages retain every scalar/hash field and
their order from target-layer r2. They replace the domain with the matching
line above, replace each ten-byte binding with the exact 16-byte record, and
order bindings by the r3 key. The final head consumes the exact accepted
distributed-sampling v1+r2 composite
`95fa7aa3b4b0b78a3f8313705d25e4c11682632fce6d8b8c2355b8130745f58c`.

The top-level digest is:

```text
SHA256(
  "glmaxx.target-program.v2.capacity-exl3-layout-bound\0" ||
  embedding_entry_sha256 || u16_le(78) ||
  layer_entry_sha256[0] || ... || layer_entry_sha256[77] ||
  final_head_entry_sha256
)
```

The exact binding counts are:

```text
embedding                                      1
three dense layers                 3 * 17 =   51
18 FULL sparse layers             18 * 787 = 14,166
57 SHARED sparse layers           57 * 782 = 44,574
final head                                     2
total                                     58,794
```

A FULL sparse layer has 19 protected/indexer bindings plus 768 routed
bindings. A SHARED sparse layer has 14 protected bindings plus 768 routed
bindings. The target count excludes layer 78. The complete capacity rank
inventory remains 59,585 because the separate MTP program owns layer 78 and
its protected/routed records.

Any 39,594-record capacity program, combined EXL3 record, missing split
projection, nonzero EXL3 layout ID, or r1/r2 target-program domain fails
before GraphProfile or executor program-set construction.

## Hybrid W4A16/NF3 target program

The production NVFP4/NF3 checkpoint uses the already specified hybrid v3
record membership and domains from
`sm120-w4a16-nf3-fused-moe-v1-r2.md`. R3 incorporates them without changing
their bytes:

```text
glmaxx.target-program.embedding.v3.hybrid-layout-bound\0
glmaxx.target-program.layer.v3.hybrid-layout-bound\0
glmaxx.target-program.final-head.v3.hybrid-layout-bound\0
glmaxx.target-program.v3.hybrid-w4a16-nf3-layout-bound\0
```

Legal routed records are combined gate/up projection 4 and down projection 3
under the exact ModelOpt/NVFP4 or NF3 codec/layout pairs in that contract.
Split records are invalid for this family. Every sparse expert therefore has
two routed descriptors. Exact counts are:

```text
embedding                                      1
three dense layers                 3 * 17 =   51
18 FULL sparse layers             18 * 531 =  9,558
57 SHARED sparse layers           57 * 526 = 29,982
final head                                     2
total                                     39,594
```

The target program consumes the same corrected sampling composite as the
capacity program. Its target digest, hybrid MTP digest, resident identity,
common binding semantics, weight policy, manifest, load plan, GraphProfile,
and executor program set must all select the hybrid family. An NVFP4-only
laboratory target program cannot substitute.

## NVFP4 laboratory M4 separation

The M4 533-tensor layer-6 subset uses the separately reviewed NVFP4
laboratory manifest and target-program-v2 layout rules. It is neither the
58,794-record capacity program nor the 39,594-record full hybrid program.

M4 compiles only the exact truncated target program named by its accepted
runner and cannot convert its laboratory weight handle or digest into a
production target program. Passing M4 does not establish a full hybrid or
capacity target-program result.

## Step, graph, resident, and hot-reload binding

For a production generation, exactly one target-program family and digest is
process-common. That digest occupies the target-program field in StepPlan v4,
StepInput v3, GraphProfile v2/v3, GraphMemoryPlan v1, executor target-only or
target-plus-MTP program-set digest, module node records, completion receipts,
and resident-weight identity.

The physical plan reconstructs all primary, auxiliary, and codec-metadata
uses from these exact bindings and the authenticated rank-set load plan. A
capacity split expert produces three independent resident tensor-plane use
sets; a hybrid combined expert produces two. The arena-8/9 relative spans
must be common in shape/offset/length/alignment across ranks and resolve to
four rank-local owner addresses only after adoption.

A compatible hot reload may change kernels/configuration only if the target
program, weight policy, resident binding semantics, and ten-arena weight/
metadata/page-table generations remain identical. Changing family, binding
count, projection, codec, layout, tensor membership, or resident identity is
a cold generation and cannot claim zero weight read/H2D.

## Coordinated CPU gate

After every named design is accepted and before CUDA work, one CPU proof must:

1. encode/decode/mutate every byte and enum of the 16-byte binding;
2. compile all 78 target layers for the pinned capacity plan, proving 17,
   787, 782, and 58,794 exact counts;
3. prove every capacity expert has separate gate/up/down IDs and the tier map
   selects exact K=3/K=4 source metadata without changing projection identity;
4. compile a complete synthetic hybrid program, proving 17, 531, 526, and
   39,594 exact counts under the retained v3 domains;
5. reject missing, duplicate, swapped, mixed split/combined, wrong codec,
   wrong layout, wrong role, wrong expert, wrong shape, and rank-divergent
   records at experts 0 and 255;
6. independently serialize every entry and top-level preimage and mutate each
   scalar/hash field, including the sampling composite;
7. resolve every binding through four adopted tensor-device CPU fakes and
   prove no name, metadata parser, policy choice, repack, or raw address
   remains in graph launch;
8. build profile-matched StepPlan, GraphProfile v3, physical plan, resident,
   module/program-set, and completion identities and reject every cross-family
   substitution;
9. prove 533-tensor M4 type state cannot reach either full production program;
   and
10. retain exact current-Rust nonclaims and bounded synthetic fixtures.

The implementation/proof requires its own adversarial acceptance before any
M3/M4 or production SM120 launch.

## Gate effect and nonclaims

Acceptance opens only the coordinated CPU compiler/proof above after the
selected source, manifest, codec, target, sampling, resident, physical-memory,
executor, and profile designs are each accepted. It does not accept current
Rust, a native target module, CUDA, a graph, layer replay, checkpoint smoke,
text, quality, KV capacity, concurrency, hot reload, or performance. It
authorizes no cn4 work.
