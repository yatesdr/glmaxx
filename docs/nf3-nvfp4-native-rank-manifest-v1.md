# NF3/NVFP4 native rank manifest v1

Date: 2026-08-03

Status: design candidate; adversarial acceptance required before implementation

GPU evidence: none

## Purpose

This contract defines the native rank-file, tensor-catalog, and resident-arena
identity for the real GLM-5.2 NVFP4/NF3 checkpoint under TP4 and MTP3. It
closes the alignment gap left by the minimum record charge in
`hybrid-mtp3-capacity-ledger-v1.md`.

This is not the EXL3/NVFP4 hybrid profile. It uses distinct schema, profile,
policy, catalog, load-plan, header-flag, and codec identities. No existing
profile byte or acceptance token can admit it.

## Pinned source identity

The source is the read-only cn4 inventory recorded in
`cn4-hybrid-source-inventory-20260803.md`:

```text
checkpoint revision                 68babde27a97a4c980c2494e830dd424975cd5a3
config.json SHA-256                 254974797e9f455716a30ab5505ba68272181b20b58a3693e54f94fb8056f3ef
model index SHA-256                 6eb773222d932418dd0530c63aca498f86ef424da2a4526ccba76b59726da234
mxfp8_tier_nokvb.json SHA-256       ebcd6087180033d4512fafa5f154f4fecfbc1ee5e5051448f34859cccc4430f0
```

The expert assignment authority is
`config.quantization_config.hybrid_bit_map`. The separately authenticated
`mxfp8_tier_nokvb.json` is a module-prefix/protection artifact and is not a
substitute expert tier map. The index must independently corroborate each
assignment by exposing exactly the component family required by the source
contract.

The exact inventory is:

```text
target NF3 experts                         14,400
target NVFP4 experts                        4,800
draft NVFP4 experts                           256
all experts                                19,456
routed physical descriptors                38,912
protected descriptors                       1,217
total native tensor descriptors            40,129
```

Every expert has exactly two physical descriptors: fused gate/up followed by
down semantically. Descriptor tensor IDs are nevertheless assigned by the
canonical name order below, not by this explanatory order.

## Distinct format and type identities

The native container uses format minor 3 and adds:

```text
profile enum                 Nvfp4Nf3HybridServe = 4
manifest schema              glmaxx.nf3-nvfp4-rank-manifest.v1
weight-policy schema         glmaxx.nf3-nvfp4-weight-policy.v1
catalog schema               glmaxx.nf3-nvfp4-tensor-catalog.v1
budget schema                glmaxx.hybrid-mtp3-budget.v1
load-plan domain             glmaxx.rank-set-load-plan.v1.nf3-nvfp4-mtp3-v1\0
NF3 codec ID                 0x0300
NF3 value-layout ID          0x1230
NF3 scale-layout ID          0x1231
```

Header flag bit 5 is `NF3`. This direct native file has exactly
`DIRECT | NVFP4 | PROTECTED | HYBRID | NF3 = 59`; `EXL3` is clear. A minor-2
reader, profile byte 3, missing/extra flag, EXL3 codec, or policy from another
profile fails before allocation.

The fixed 256-byte tensor descriptor remains structurally sufficient. The
new format revision teaches the decoder the NF3 codec and 192-byte metadata;
it does not change a descriptor field or reinterpret a reserved byte.

## Canonical physical inventory

The policy-derived physical names are closed:

```text
model.layers.{layer}.mlp.experts.{expert}.gate_up_proj.weight
model.layers.{layer}.mlp.experts.{expert}.down_proj.weight
```

Target `layer` is 3 through 77, draft `layer` is 78, and `expert` is 0 through
255. Unsigned decimal has no leading zero. Protected names are exactly the
compiled 1,217-entry protected inventory.

The complete 40,129-name set is sorted by unsigned UTF-8 bytes and tensor IDs
are its zero-based indices. Duplicate Unicode spellings, locale collation,
numeric sorting, aliases, source component names, or caller order fail.
All four ranks derive the same names, IDs, roles, codecs, layouts, logical
shapes, and source routes before rank-local offsets are planned.

The common catalog retains the 224-byte hybrid semantic shape specified in
`hybrid-serving-manifest-v1.md` under
the new schema and hash domain. NF3 records use projection 4 for fused gate/up
or 3 for down, codec `0x0300`, layouts `0x1230/0x1231`, and the authenticated
NF3 native-layout source digest. NVFP4 uses the existing r3 projection and
layout identities. Protected extension bytes are zero.

```text
catalog bytes = 40,129 * 224 = 8,988,896
descriptor-array bytes = 40,129 * 256 = 10,273,024
```

File descriptor, string, manifest, and top-level 4,096-byte region padding
are storage/host metadata. Any resident copies remain explicit runtime-budget
terms; they cannot be hidden in the weight arena.

## Payload order and exact weight arena

Payload descriptors are traversed in tensor-ID order. Within an NF3 or NVFP4
descriptor, the value/code plane precedes its scale plane. Every nonempty
plane starts on a 256-byte boundary. Protected payloads retain the existing
compiled ordering and TP slicing. The final payload is not rounded.

The rank-local routed planes are:

| Codec and role | Value/code bytes | Scale bytes | Payload bytes | Metadata bytes |
|---|---:|---:|---:|---:|
| NF3 fused gate/up | 2,359,296 | 196,608 | 2,555,904 | 192 |
| NF3 down | 1,179,648 | 98,304 | 1,277,952 | 192 |
| NVFP4 fused gate/up | 3,145,728 | 393,216 | 3,538,944 | 128 |
| NVFP4 down | 1,572,864 | 196,608 | 1,769,472 | 128 |

Every routed plane length is a multiple of 256. The routed stream therefore
adds no payload gap. Because it enters every protected payload at the same
256-byte phase as the existing protected planner, the unchanged protected
inventory contributes exactly 11,959,396,352 bytes and no new successor gap.
The implementation must still reproduce this descriptor by descriptor; the
closed form is only a cross-check.

```text
protected payload                                      11,959,396,352
NF3 payload       14,400 * (2,555,904 + 1,277,952)      55,207,526,400
NVFP4 payload      5,056 * (3,538,944 + 1,769,472)      26,839,351,296
rank tensor-plane bytes                                94,006,274,048
file payload-region bytes                              94,006,274,048
device weight-arena bytes                              94,006,274,048
```

The file payload region and device arena are separately authenticated fields
even though their values are equal for this exact layout. A future plane or
alignment change must change the manifest identity rather than preserve this
constant.

## File and device metadata arenas

Protected tensors use zero codec-metadata bytes. File codec metadata is dense
in tensor-ID order:

```text
NF3 metadata          28,800 * 192 = 5,529,600
NVFP4 metadata        10,112 * 128 = 1,294,336
file codec metadata bytes             6,823,936
```

Device metadata preserves tensor-ID order while skipping zero-length records.
Every nonempty record starts at a 256-byte offset and the final record is not
tail-rounded.

Under unsigned UTF-8 ordering the final routed physical name is:

```text
model.layers.9.mlp.experts.99.gate_up_proj.weight
```

This follows because `9` is the lexicographically greatest admitted decimal
layer spelling, `99` is the greatest expert spelling, and `gate_up` sorts
after `down`. The pinned config has
`hybrid_bit_map["9"][99] = 3`; the pinned index corroborates NF3
`weight_packed` and `weight_scale` components for gate and up. The final
metadata length is therefore 192 bytes, not 128.

For `R = 38,912`:

```text
device metadata-arena bytes = (R - 1) * 256 + 192
                            = 9,961,408
raw metadata bytes           = 6,823,936
device alignment padding     = 3,137,472
```

The exact immutable device-arena charge is consequently:

```text
device weight arena          94,006,274,048
device metadata arena             9,961,408
total immutable arenas       94,016,235,456
```

This is 3,137,472 bytes above the 94,013,097,984-byte minimum codec-record
sum in the capacity ledger.

## Capacity cross-check, not a fit claim

Combining these exact arenas with the reviewed MTP3 cache candidate gives:

```text
immutable arenas             94,016,235,456
MTP3 cache arena              4,330,317,824
weight plus cache            98,346,553,280
measured minimum rank floor 101,367,742,464
remaining for all else        3,021,189,184
```

The older non-context, non-loader sensitivity terms total 2,550,136,832
bytes, leaving only 471,052,352 provisional bytes. This is not an admission
or fit result. Modules, graphs, libraries, collectives, exact workspaces,
runtime metadata, allocator fragmentation, and physical cache writes remain
measured gates, including the independent 1-GiB escrow requirement.

## Required CPU proof

After adversarial acceptance, one checked planner must:

1. parse the pinned config and index independently and reproduce every tier;
2. generate the complete physical name set and prove 40,129 unique IDs;
3. reproduce the exact final metadata-bearing name and its NF3 source family;
4. emit every descriptor plane and metadata offset with checked `u64` math;
5. compare descriptor iteration with every closed form above on all ranks;
6. prove every payload and metadata range is aligned, bounded, disjoint, and
   gap-accounted;
7. serialize, decode, and mutation-test the new profile, flags, codec,
   metadata, catalog, policy, and load-plan domains;
8. reject one-byte-short arenas, a 128-byte final-tail lie, reordered names,
   changed tier entries, missing draft experts, profile aliasing, overflow,
   and rank disagreement; and
9. emit a machine-readable exact HBM ledger consumed unchanged by startup.

No GPU allocation, checkpoint conversion, serving health, or HTTP publication
is authorized by this design. The subsequent gate is the CPU proof, followed
by implementation review and only then a four-rank physical allocation test.
