# NVFP4 fused routed-MoE kernel v1 r3

Date: 2026-07-30

Status: corrective integration design candidate; adversarial review required
before CPU or CUDA implementation

GPU evidence: none

## Purpose

This amendment supersedes the implementation authority of
`docs/nvfp4-fused-routed-moe-v1-r2.md` and
`docs/target-program-projection-discriminator-v1.md`. It retains r2's seven
corrections but closes four integration defects found by re-deriving r2
against the canonical NVFP4 codec, immutable weight policy, strict rank
manifest, and target-program binding:

1. r2's byte-permutation argument did not distinguish 1D row-local scales
   from 2D block-16x16 scales. A 2D `0x1201` tensor requires equal scale codes
   across each 16 contiguous logical N rows. Gate/up interleaving does not
   preserve those physical 16-row replica groups.
2. `WeightPolicy.v1` represents one undifferentiated NVFP4 codec, permits
   gate and up to select different backends, and charges two 128-byte metadata
   records where the combined tensor has one.
3. the target-program discriminator binds logical "combined gate/up" but not
   the value and scale layout IDs. It could therefore select the fused graph
   for an incompatible `0x1201` tensor.
4. the only implemented production manifest profile is `capacity-exl3`;
   validation explicitly rejects every NVFP4 descriptor. The proposed
   combined-NVFP4 production branch is presently unreachable.

R3 fixes the first three contracts and makes the fourth an explicit
fail-closed prerequisite rather than implying that the current manifest can
open a hybrid path.

All r2 requirements not explicitly replaced here remain in force. The
original v1 and r2 review handoffs are superseded and must not issue their
tokens.

## Nonclaims and authority

R3 does not:

- implement or accept layouts `0x1202`, a converter, a policy v2, a target
  compiler, a laboratory/hybrid manifest, a CPU proof, an ABI, or a kernel;
- establish the codec or layout of either operator-provided NVFP4 checkpoint;
- authorize reading a checkpoint, connecting to cn4, or launching a GPU;
- make a 2D FC1 fused-kernel claim;
- accept an all-NVFP4 full-model serving profile;
- prove quality, physical fit, capacity, latency, or throughput; or
- pass C06 or C08.

## Fixed FC1 codec and layout boundary

The fused production FC1 accepts exactly:

```text
codec_id         CODEC_NVFP4_1D
value_layout_id  0x1202
scale_layout_id  0x1202
logical shape    [1024,6144]
projection       combined gate/up
```

The r2 forward and inverse row maps remain exact:

```text
logical gate c -> physical 2*c
logical up c   -> physical 2*c+1
physical even p -> logical p/2
physical odd p  -> logical 512+p/2
```

For 1D scaling, each `(logical row, K-group)` owns an independent scale.
Copying that row's value bytes and scale codes to its mapped physical row is
therefore byte-preserving and leaves the quantization policy unchanged.

`CODEC_NVFP4_2D` is forbidden for combined `0x1202` FC1 in r3. Under the
canonical current codec, rows `16t..16t+15` share one scale for each K group.
The interleaved physical rows `16t..16t+15` contain eight gate rows and eight
up rows that came from two distinct logical 16-row groups. A plain r2 row
permutation can produce unequal scale replicas inside the physical group and
cannot be called a canonical 2D tensor.

The first tile is already a counterexample. Let logical gate rows `0..15`
share scale code `a` and logical up rows `512..527` share a different scale
code `b` for one K group. After r2's map, physical rows `0..15` alternate
`a,b,a,b,...`; the current 2D validator requires all sixteen codes to equal
the code at physical row `0`. The permutation passes only accidentally when
`a == b`, which is not a codec invariant.

R3 does not claim that an alternative SM120 scale layout could never support
interleaved 2D weights. Such a layout requires a separate codec/layout ID,
CPU quantization definition, generated-instruction address proof, quality
gate, and benchmark. It is not `0x1202`.

FC2 down tensors remain noninterleaved:

```text
projection       down
logical shape    [6144,512]
value_layout_id  0x1201
scale_layout_id  0x1201
codec_id         CODEC_NVFP4_1D or CODEC_NVFP4_2D
```

The selected FC2 grouped kernel must independently support the exact codec
ID. No 1D/2D fallback or reinterpretation is allowed.

## Source conversion cases

An offline converter has three closed cases:

| Source | Permitted output | Meaning |
|---|---|---|
| 1D `0x1201` combined FC1 | 1D `0x1202` | r2 byte-only row/SFB permutation; no requantization |
| 1D `0x1202` combined FC1 | identical 1D `0x1202` | validate and retain exact bytes |
| 2D `0x1201` combined FC1 | no implicit `0x1202` output | retain as a laboratory control or enter a separately reviewed dequantize/requantize workflow |

A 2D-to-1D conversion changes block-scale membership and normally changes
value codes. If selected, it requires:

- an explicit new conversion-policy identity;
- decoded-source and requantized-output hashes;
- per-position quality evidence against the protected target baseline;
- no claim of byte permutation or unchanged quantization policy; and
- the same subsequent 1D `0x1202` CPU and SM120 gates.

The converter reads the authenticated codec ID and both layout IDs. It never
infers the case from a filename, checkpoint label, tensor shape, or desired
kernel.

## Routed weight policy v2 realization

The logical quality policy may continue to score gate, up, and down
separately, but its physical realization is expert-atomic for gate/up.

The backend enumeration is closed and distinguishes:

```text
EXL3_SOURCE
NVFP4_1D
NVFP4_2D
```

For every `(layer,expert)`:

- gate and up are both EXL3; or
- gate and up are both NVFP4_1D and realize as one combined `0x1202`
  descriptor.

Mixed gate/up backends, gate/up `NVFP4_2D`, split NVFP4, and combined EXL3
are invalid. Down independently selects EXL3, NVFP4_1D, or NVFP4_2D.

The policy hash binds every logical role's exact backend, quality-evidence
digest, and the following physical realization record:

```text
layer:u16_le
expert:u16_le
physical_projection:u8
codec_id:u16_le
value_layout_id:u16_le
scale_layout_id:u16_le
rank_payload_bytes:u64_le
codec_metadata_bytes:u32_le
```

The physical projection enumeration is:

```text
1 split gate
2 split up
3 down
4 combined gate/up
```

The exact current rank-local charges are:

| Physical tensor | Payload planes | Metadata | Total |
|---|---:|---:|---:|
| EXL3 gate, up, or down | 1,192,964 | 96 | 1,193,060 |
| NVFP4 combined gate/up | 3,538,944 | 128 | 3,539,072 |
| NVFP4 down | 1,769,472 | 128 | 1,769,600 |

Consequently:

```text
two split NVFP4-sized records = 2 * 1,769,600 = 3,539,200
one combined NVFP4 record     =                     3,539,072
difference                    =                           128

one all-NVFP4 expert physical total
  = 3,539,072 + 1,769,600
  = 5,308,672 bytes
```

The existing `WeightPolicy.v1` value `3 * 1,769,600 = 5,308,800` is a
conservative estimate but is not exact physical realization. R3 forbids
using it as an exact manifest, arena, or fit charge.

`rank_weight_bytes` is the checked sum of physical realization records plus
protected allocations, counted once per physical descriptor. A hybrid
serving policy contains at least one EXL3 and one NVFP4 physical projection.
An all-NVFP4 full-model policy remains invalid.

## Layout-bound target-program record

The corrected target-program binding remains exactly 16 bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 4 | `tensor_id:u32_le` |
| 4 | 2 | `role_id:u16_le` |
| 6 | 2 | `expert_id:i16_le` |
| 8 | 2 | `codec_id:u16_le` |
| 10 | 1 | `projection_id:u8` |
| 11 | 1 | reserved; zero |
| 12 | 2 | `value_layout_id:u16_le` |
| 14 | 2 | `scale_layout_id:u16_le` |

The projection enumeration from v1 is retained. Layout fields are:

| Representation | value layout | scale layout |
|---|---:|---:|
| non-NVFP4 or EXL3 projection | `0` | `0` |
| NVFP4 combined gate/up fused FC1 | `0x1202` | `0x1202` |
| NVFP4 down | `0x1201` | `0x1201` |

Any other combination fails. In particular:

- combined NVFP4 `0x1201` cannot enter the fused serving graph;
- combined NVFP4 `0x1202/0x1201` or `0x1201/0x1202` is invalid;
- EXL3 cannot borrow an NVFP4 layout ID; and
- tensor shape and projection ID cannot substitute for authenticated layout
  metadata.

The target-program domain becomes:

```text
glmaxx.target-program.v2.routed-layout-bound\0
```

Records are still ordered by
`(role_id,expert_id,projection_id,tensor_id)`. The exact 16-byte record,
including layout fields and the zero reserved byte, is hashed. Graph profile,
step input, resident binding, and target-program receipt all bind the v2
digest. V1 and v2 graphs cannot coexist under one profile identity.

## Authenticated derivation

The compiler receives one typed input only after:

1. descriptor, name, manifest, codec metadata, and every plane hash pass;
2. NVFP4 metadata is decoded canonically;
3. codec ID, shape, byte counts, value layout, scale layout, layout-source
   digest, quantization-policy digest, projection, and role agree;
4. the immutable weight policy maps the logical role set to exactly that
   physical descriptor; and
5. all four ranks produce the same ordered semantic/layout record stream.

The layout IDs come from authenticated decoded metadata, not a caller
parameter. The decoder recognizes each accepted layout ID only together with
its exact layout-source digest. A known ID with the wrong source digest is
unsupported metadata.

The compiled program retains numeric tensor IDs, codec IDs, projection IDs,
and layout IDs. Graph launch performs no string parsing, metadata parsing,
policy choice, repack, or fallback.

## Manifest reachability and profile separation

The current production manifest schema
`glmaxx.rank-manifest.v0.2.2` remains capacity-EXL3-only. Its validator must
continue rejecting:

- any NVFP4 descriptor;
- `nvfp4-laboratory` or `hybrid-serve`;
- the 533-tensor M4 subset; and
- a future-schema prefix it does not implement.

R3 does not silently extend that accepted schema.

Before M4, a separately reviewed laboratory manifest must:

- use a distinct schema and `nvfp4-laboratory` profile;
- bind its exact included and omitted tensor IDs;
- expose canonical codec/layout fields to the typed compiler input;
- bind the M4 `0x1202` 1D conversion policy and source/output hashes;
- enforce the exact 533 descriptors, 1,982,245,376 payload bytes, 65,536
  codec-metadata bytes, and zero payload-alignment slack per rank; and
- remain incapable of reaching production `HEALTHY` or the serving API.

The routed metadata total is:

```text
512 routed NVFP4 records * 128 = 65,536
```

The present plain protected codec uses zero-byte codec metadata, so the
expected complete codec-metadata region is 65,536 bytes. The laboratory
manifest proof must nevertheless derive that value from all 533 descriptors
and reject any protected-codec change that would make the copied constant
stale.

Before full hybrid serving, a separately reviewed hybrid manifest and load
plan must:

- accept one exact `WeightPolicy.v2` and reject v1;
- derive descriptor membership and tensor count from physical realization
  records rather than a fixed capacity-EXL3 count;
- bind all protected and routed tensors, payload/metadata bytes, layout
  identities, quality receipts, policy hash, and four-rank invariant catalog;
- use a completed hybrid profile budget and reject all-NVFP4 membership;
- construct identical target-program v2 records on all ranks; and
- keep every unsupported laboratory/capacity/hybrid schema transition
  fail-closed.

No combined-NVFP4 branch is production-reachable before those gates.

## Corrected M4 arithmetic

The current small-checkpoint document's routed payload arithmetic is exact:

```text
combined planes = 3,145,728 + 393,216 = 3,538,944
down planes     = 1,572,864 + 196,608 = 1,769,472
per expert      =                         5,308,416 payload bytes
256 experts     =                     1,358,954,496 payload bytes
```

Adding routed codec metadata gives:

```text
512 * 128 = 65,536 bytes
routed payload plus metadata = 1,359,020,032 bytes
```

The 533 tensor count is valid only for a combined-NVFP4 laboratory
realization. It does not describe capacity EXL3 or an arbitrary hybrid
policy. The M4 source/checkpoint cannot be called ready for `0x1202` until
its FC1 codec and layout IDs are authenticated and the applicable conversion
case above passes.

## Revised CPU gate

After r3 review, the coordinated CPU proof must additionally:

1. construct adversarial 2D `0x1201` scale replicas whose r2 row permutation
   becomes noncanonical, and prove r3 rejects them;
2. exhaust the 1D `0x1201` to `0x1202` forward/inverse value and scale map;
3. reject every codec/layout/projection combination outside the closed
   tables;
4. distinguish NVFP4 1D and 2D in the policy hash;
5. reject mixed gate/up backends and count combined metadata exactly once;
6. independently rederive every physical and M4 byte value above;
7. encode/hash the 16-byte target binding and mutation-test each byte,
   layout ID, source digest, projection, codec, role, shape, and tensor ID;
8. prove v1/v2 target programs and graph identities cannot collide;
9. show current capacity manifest validation still rejects NVFP4 and M4;
10. use synthetic typed laboratory/hybrid inputs only, clearly separated
    from unimplemented manifest acceptance; and
11. retain all r2 layout, route, quantization, epilogue, workspace, overlap,
    and one-byte-short tests.

CPU acceptance opens format/policy/target-program implementation only within
the reviewed boundary. Laboratory and hybrid manifests remain behind their
own design reviews. CUDA remains behind the existing authorization and
hardware gates.

## Revised SM120 gate

The r2 SM120 matrix remains required and additionally proves:

- FC1 rejects 2D and every non-`0x1202/0x1202` layout;
- FC2 selects the exact 1D or 2D kernel variant bound by its record;
- generated code consumes the layout-bound v2 target program;
- a mutated layout ID cannot reuse a captured graph;
- no rank can select a local codec/layout fallback; and
- inclusive results name exact codec, layout, conversion policy, source
  digest, and graph-profile identity.

Only matched 1D `0x1202` fused evidence can qualify r3 FC1.

## Exit criteria

R3 is ready for its CPU proof only if adversarial review confirms that:

- the 2D permutation defect is real and the 1D restriction closes it;
- policy roles map to exact physical descriptors and byte charges;
- target-program layout identity cannot drift from authenticated metadata;
- current manifests remain closed while laboratory/hybrid prerequisites are
  explicit;
- M4 arithmetic is correctly scoped; and
- no format, implementation, manifest, device, checkpoint, quality, fit,
  capacity, or speed result is implied.
