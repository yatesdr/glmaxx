# NF3/ModelOpt-NVFP4 native rank manifest v1 r2

Date: 2026-08-03

Status: corrective design candidate; adversarial acceptance required before
implementation

GPU evidence: none

## Purpose and authority

This contract supersedes `nf3-nvfp4-native-rank-manifest-v1.md` and consumes
the corrected source contract
`nf3-nvfp4-hybrid-source-and-kernel-v1-r2.md`. The payload inventory is
unchanged, but every ModelOpt-NVFP4 physical tensor now uses distinct W4A16
codec `0x0102` and 192-byte metadata carrying the source outer scalars. It
cannot alias the existing single-global-scale codec `0x0100` or its W4A4
control.

The only source authority is the authenticated config, index, every shard,
and typed protection artifact rooted at:

```text
revision                 68babde27a97a4c980c2494e830dd424975cd5a3
config SHA-256           254974797e9f455716a30ab5505ba68272181b20b58a3693e54f94fb8056f3ef
index SHA-256            6eb773222d932418dd0530c63aca498f86ef424da2a4526ccba76b59726da234
protection SHA-256       ebcd6087180033d4512fafa5f154f4fecfbc1ee5e5051448f34859cccc4430f0
```

`config.quantization_config.hybrid_bit_map` is expert assignment authority.
The protection artifact cannot select NF3 versus ModelOpt-NVFP4.

## Closed identities

The container uses format minor 3 and the following new identities:

```text
profile enum       Nvfp4Nf3HybridServe = 4
manifest schema    glmaxx.nf3-modelopt-nvfp4-rank-manifest.v1
policy schema      glmaxx.nf3-modelopt-nvfp4-weight-policy.v1
catalog schema     glmaxx.nf3-modelopt-nvfp4-tensor-catalog.v1
budget schema      glmaxx.hybrid-mtp3-budget.v1
load-plan domain   glmaxx.rank-set-load-plan.v1.nf3-modelopt-w4a16-mtp3-v1\0
NF3 codec          0x0300
ModelOpt W4A16     0x0102
```

NF3 layouts are `0x1230/0x1231`. ModelOpt W4A16 values and scales use
address-layout IDs `0x1201/0x1201`; codec `0x0102`, projection, and the
authenticated layout-source digest distinguish their numerical meaning from
codec `0x0100` and `0x0101`.

Header flag bit 5 is NF3. Exact flags are
`DIRECT | NVFP4 | PROTECTED | HYBRID | NF3 = 59`; EXL3 is clear. A minor-2
reader, old schema/domain, profile byte other than 4, flag drift, codec
`0x0100/0x0101`, EXL3 codec, laboratory policy, or W4A4 graph fails before
allocation.

The fixed 256-byte tensor descriptor is unchanged. Codec metadata length,
hash, source-route identity, and payload planes remain authenticated descriptor
fields. No reserved descriptor byte is reinterpreted.

## Exact physical inventory

The complete policy-derived counts are:

```text
target NF3 experts                         14,400
target ModelOpt-NVFP4 experts               4,800
draft ModelOpt-NVFP4 experts                  256
all experts                                19,456
routed physical descriptors                38,912
protected descriptors                       1,217
total descriptors                          40,129
```

Every routed expert has two physical descriptors: fused gate/up and down.
Target layers are 3 through 77, draft layer is 78, and expert IDs are 0 through
255. Physical names are exactly:

```text
model.layers.{layer}.mlp.experts.{expert}.gate_up_proj.weight
model.layers.{layer}.mlp.experts.{expert}.down_proj.weight
```

Decimal components have no leading zero. These names plus the exact compiled
1,217-name protected inventory are unique and sorted by unsigned UTF-8 bytes;
their zero-based positions are tensor IDs. Locale, numeric sorting, aliases,
source component names, caller order, or duplicate Unicode spellings fail.

The final routed name remains
`model.layers.9.mlp.experts.99.gate_up_proj.weight`. The pinned tier entry is
NF3 and the index must expose its exact NF3 gate/up components. R2 does not use
that fact to guess the final metadata length: both routed codecs are exactly
192 bytes.

The common semantic catalog retains the reviewed 224-byte structural shape
under the new schema/domain. It binds codec, projection, both layout IDs,
representation-source digest, numerical-policy digest, expert-policy digest,
plane hashes/bytes, and metadata hash/bytes. Protected extension bytes are
zero.

```text
catalog bytes          = 40,129 * 224 = 8,988,896
descriptor-array bytes = 40,129 * 256 = 10,273,024
```

All four ranks independently derive the same ordered semantic stream before
planning rank-local offsets.

## Exact rank payload arena

Payload descriptors follow tensor-ID order. Inside each routed descriptor the
value/code plane precedes the scale plane. Every nonempty plane begins on a
256-byte boundary; every routed plane length is already a multiple of 256.
Protected tensors retain the compiled TP slicing and planner. The final payload
is not rounded.

| Codec and role | Value/code bytes | Scale bytes | Payload bytes | Metadata bytes |
|---|---:|---:|---:|---:|
| NF3 fused gate/up | 2,359,296 | 196,608 | 2,555,904 | 192 |
| NF3 down | 1,179,648 | 98,304 | 1,277,952 | 192 |
| ModelOpt W4A16 fused gate/up | 3,145,728 | 393,216 | 3,538,944 | 192 |
| ModelOpt W4A16 down | 1,572,864 | 196,608 | 1,769,472 | 192 |

The descriptor-by-descriptor planner must reproduce:

```text
protected payload                                      11,959,396,352
NF3 payload       14,400 * (2,555,904 + 1,277,952)      55,207,526,400
ModelOpt payload   5,056 * (3,538,944 + 1,769,472)      26,839,351,296
rank tensor-plane bytes                                94,006,274,048
file payload-region bytes                              94,006,274,048
device weight-arena bytes                              94,006,274,048
```

The ModelOpt subtotal separates as 25,480,396,800 target bytes and
1,358,954,496 draft bytes. Closed forms are cross-checks, never a substitute
for complete ordered range simulation.

## File and device metadata arenas

Protected tensors have zero codec metadata. File codec metadata is dense in
tensor-ID order without per-record padding:

```text
NF3 metadata          28,800 * 192 = 5,529,600
ModelOpt metadata     10,112 * 192 = 1,941,504
raw file metadata bytes              7,471,104
```

Device metadata preserves tensor-ID order while skipping protected zero-length
records. Each nonempty record starts at a 256-byte offset; the final record is
not tail-rounded. With `R=38,912` and `L=192`:

```text
device metadata bytes = (R - 1) * 256 + L = 9,961,408
raw metadata bytes                             7,471,104
device alignment padding                       2,490,304
```

File metadata bytes and device metadata bytes are different authenticated
fields. Host catalog, descriptor, manifest, string, load-plan, and validation
copies are not weight-arena bytes; every resident copy is nevertheless an
explicit runtime-budget term.

The immutable device charge is:

```text
device weight arena       94,006,274,048
device metadata arena          9,961,408
total immutable arenas    94,016,235,456
```

## ModelOpt scalar binding

Every ModelOpt descriptor carries the exact 192-byte metadata from the r2
source contract. Fused gate/up has two finite positive weight outer scalars
and two finite positive authenticated input scalars. Down has one of each and
exact positive-zero bits in its second slots.

The resident target program binds codec `0x0102`, projection, layouts,
metadata hash, numerical-policy digest, and scalar arity. FC1 exposes two
outer-scale addresses or one address plus an exact stride of two F32 values;
FC2 exposes one. Codec `0x0100` one-scale descriptors cannot enter this graph,
even if fixture scalar values happen to compare equal. Graph capture may not
infer arity from shape or pointer aliasing.

## Capacity sensitivity, not fit evidence

Combining the exact immutable arenas with the reviewed MTP3 cache candidate
still gives:

```text
immutable arenas             94,016,235,456
MTP3 cache arena              4,330,317,824
weight plus cache            98,346,553,280
measured minimum rank floor 101,367,742,464
remaining for all else        3,021,189,184
older sensitivity terms       2,550,136,832
provisional margin              471,052,352
```

The unchanged device result follows because replacing 128-byte ModelOpt file
records with 192-byte records does not change their 256-byte device stride or
the 192-byte final tail. None of these numbers proves allocation, escrow,
checkpoint admission, quality, or performance. Modules, graphs, collectives,
libraries, exact workspaces, runtime metadata, allocator fragmentation, and a
separate 1-GiB escrow remain physical gates.

## Required CPU proof

After design acceptance, one checked planner must:

1. authenticate and parse the pinned config/index and reproduce every target
   and draft tier plus all five source-component counts;
2. generate, UTF-8-sort, and uniquely number all 40,129 physical names;
3. independently reproduce the final routed name and its NF3 component family;
4. emit every payload and metadata range on all four ranks with checked `u64`
   arithmetic and compare ordered simulation to each closed form;
5. prove all ranges aligned, bounded, disjoint, and gap-accounted, including
   the unrounded payload and metadata tails;
6. encode/decode and mutation-test format minor, profile, flags, both codecs,
   layouts, schemas, domains, catalog, policy, descriptor binding, source
   routes, and ModelOpt scalar arity;
7. reject a one-byte-short plane/arena, 128-byte ModelOpt metadata, a one-scale
   fused descriptor, reordered names, changed tier entry, absent draft expert,
   old-profile alias, overflow, and rank disagreement; and
8. emit one exact machine-readable HBM ledger consumed unchanged by startup.

## Gate and nonclaims

Design acceptance opens only the CPU planner/serializer proof. Implementation
requires a separate adversarial review before any native image publication or
cn4 allocation test. GPU control, actual-shape replay, checkpoint smoke, KLD,
capacity, cold start, hot reload, and performance retain their own gates.

This contract does not accept a file, checkpoint, converter, device arena,
kernel, quality result, capacity result, or speed claim.
