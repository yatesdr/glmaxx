# NF3/ModelOpt-NVFP4 hybrid source and kernel contract v1 r2

Date: 2026-08-03

Status: corrective design candidate; adversarial acceptance required before
CPU implementation

GPU evidence: none

## Purpose and supersession

This contract supersedes
`nf3-nvfp4-hybrid-source-and-kernel-v1.md`. V1 had two implementation-blocking
ambiguities:

1. it described each 12-byte NF3 unit as a two-column by sixteen-K rectangle,
   but did not define the actual 32-code tensor-core fragment mapping or its
   three-word bit placement; and
2. it routed the real ModelOpt source through the existing 128-byte GLMAXX
   NVFP4 codec even though that codec carries one re-quantizer global scale,
   while the checkpoint carries separate gate and up `weight_scale_2`
   scalars.

V1 also called the intended path W4A16 while referring to the existing GLMAXX
SM120 control, whose tensor-core operands are both NVFP4 after dynamic
activation quantization. R2 makes W4A16 and W4A4 different numerical profiles
and prevents either from borrowing the other's evidence.

R2 targets the real read-only checkpoint identified by:

```text
revision                 68babde27a97a4c980c2494e830dd424975cd5a3
config SHA-256           254974797e9f455716a30ab5505ba68272181b20b58a3693e54f94fb8056f3ef
index SHA-256            6eb773222d932418dd0530c63aca498f86ef424da2a4526ccba76b59726da234
tier artifact SHA-256    ebcd6087180033d4512fafa5f154f4fecfbc1ee5e5051448f34859cccc4430f0
```

No checkpoint name is type authority.

## Closed source admission

`config.quantization_config.hybrid_bit_map` is the expert-tier authority. It
must contain exactly decimal-string keys `3` through `77`. Each value is an
array of exactly 256 JSON integers with 192 entries equal to 3 and 64 equal to
4. Layer 78 is absent and is admitted only when all 256 routed experts expose
the ModelOpt-NVFP4 component family. Additional keys, numeric aliases, floats,
strings, booleans, missing entries, or any other value fail the checkpoint.

The config must also bind `quant_method=modelopt`, `quant_algo=NVFP4`, producer
name `modelopt`, the exact producer version, and group-16 float weights. The
complete config, index, every shard, and the separately typed protection
artifact are authenticated before any rank allocates device memory. The
protection artifact is not expert-tier authority.

For every projection of an expert, the selected component family is exact:

```text
NF3:
  weight_packed  U8
  weight_scale   F8_E4M3

ModelOpt NVFP4:
  weight          U8
  weight_scale    F8_E4M3
  weight_scale_2  scalar F32
  input_scale     scalar F32
```

Gate, up, and down must agree with the expert tier. Aliases, biases, extra
components, mixed projection families, overlapping or out-of-bounds ranges,
non-canonical shapes, and rank disagreement fail admission.

| Projection | Logical `[N,K]` | NF3 values | NF3 scales | NVFP4 values | NVFP4 scales |
|---|---|---|---|---|---|
| gate/up | `[2048,6144]` | `[2048,2304]` | `[2048,192]` | `[2048,3072]` | `[2048,384]` |
| down | `[6144,2048]` | `[6144,768]` | `[6144,64]` | `[6144,1024]` | `[6144,128]` |

TP4 gives gate and up contiguous 512-row N shards. Down gives contiguous
512-column K shards, including the corresponding packed bytes and whole scale
groups. Every rank derives and compares one canonical global membership and
source-route stream before constructing its local byte ranges.

## NF3 source arithmetic

For source bytes `b0,b1,b2`, let
`word = b0 | (b1 << 8) | (b2 << 16)`. Source code `j` is
`(word >> (3*j)) & 7` for `j in 0..8`. The exact BF16 codebook is:

```text
code       0     1     2     3     4     5     6     7
bf16    bf80  bf1b  beb6  be03  3e03  3eb6  3f1b  3f80
```

Only E4M3FN bytes `0x00..0x7e` are canonical source scales. `0x00` is exact
positive zero. `0x7f`, every sign-bit-set byte including negative zero, and
both signed NaN encodings fail. One scale applies to each consecutive K/32
group.

For a source code and scale, the W4A16 reference fragment is
`bf16_rne(f32(codebook_bf16) * f32(e4m3fn(scale)))`. The multiplication is one
round-to-nearest-even FP32 operation followed by one BF16 round; FTZ, decimal
codebooks, or a persistent dense BF16 weight plane are not authority.

The independent source proof covers all 2^24 packed words or an algebraically
equivalent exhaustive partition, each of the eight positions and eight code
values, all 64 code pairs at each of seven adjacent boundaries, byte and group
boundaries, all 256 scale bytes, malformed lengths, and actual gate/up/down
ranges on TP ranks 0 and 3. It may not call the native packer's inverse.

## ModelOpt-NVFP4 source arithmetic

In each source value byte the low nibble is lower K and the high nibble is the
following K. Nibbles decode by exact E2M1 bits:

```text
code   0    1    2    3    4    5    6    7
value  0   .5    1   1.5   2    3    4    6
code   8    9    a    b    c    d    e    f
value -0  -.5   -1  -1.5  -2   -3   -4   -6
```

Scale-byte canonicality is the same closed `0x00..0x7e` set as NF3, now one
scale per K/16 group. `weight_scale_2` and `input_scale` must each be finite,
strictly positive F32 values. Their exact little-endian bits remain source
identity.

The first production profile is **ModelOpt-NVFP4 W4A16**. It keeps the input
activation BF16 and therefore authenticates but does not use `input_scale`.
For each K/16 block the kernel forms BF16 weight fragments as
`bf16_rne(f32(E2M1(code)) * f32(E4M3FN(block_scale)))`, accumulates BF16 by
BF16 products into FP32, and applies `weight_scale_2` to the FP32 projection.
Gate and up use their own outer scalars before SwiGLU. Down uses its own outer
scalar before route weighting and the canonical ordered reduction.

The existing GLMAXX block-scaled control dynamically quantizes BF16
activations to NVFP4 and is therefore W4A4. It cannot qualify this W4A16
profile. A future W4A4 profile may use authenticated static `input_scale` or a
reviewed dynamic rule, but it requires a different numerical-policy digest,
KLD evidence, graph identity, and benchmark label.

The ModelOpt source proof covers both nibble positions and all sixteen codes,
all 256 scale bytes, signed zero, separate gate/up outer scalars, scalar
non-finites, TP boundaries, and actual gate/up/down ranges on ranks 0 and 3.

## Exact native NF3 value layout

NF3 codec `0x0300` uses value layout `0x1230`, scale layout `0x1231`, and
`tile_n=256`. A value unit is exactly three little-endian `u32` words and
contains 32 codes. It is a tensor-core fragment, not a rectangular logical
two-column tile.

For logical shape `[N,K]`, require `N % 256 == 0` and `K % 32 == 0`. Let:

```text
npairs       = 256 / 2 = 128
k16          = K / 16
unit_count   = (N / 256) * k16 * npairs
I            = ((nt * k16) + R) * npairs + p
nt           in 0 .. N/256
R            in 0 .. k16
p            in 0 .. 128
wru          = p + 128*nt
n_tile_64    = wru / 32
th_id        = wru % 32
tc_col       = th_id / 4
tc_row       = 2 * (th_id % 4)
```

For `jj in 0..4`, define eight codes in order:

```text
C1 = 64*n_tile_64 + 16*jj + tc_col
C2 = C1 + 8
K0 = 16*R + tc_row

g[jj] = [
  code(C1,K0),   code(C1,K0+1), code(C1,K0+8), code(C1,K0+9),
  code(C2,K0),   code(C2,K0+1), code(C2,K0+8), code(C2,K0+9)
]
```

For each `jj`, pack:

```text
la[jj]   = sum((g[jj][t]   & 3) << (2*t), t=0..3)
lb[jj]   = sum((g[jj][t+4] & 3) << (2*t), t=0..3)
lo16[jj] = la[jj] | (lb[jj] << 8)
ha[jj]   = sum(((g[jj][t]   >> 2) & 1) << t, t=0..3)
hb[jj]   = sum(((g[jj][t+4] >> 2) & 1) << t, t=0..3)
hi8[jj]  = ha[jj] | (hb[jj] << 4)

word0 = lo16[0] | (lo16[1] << 16)
word1 = lo16[2] | (lo16[3] << 16)
word2 = hi8[0] | (hi8[1] << 8) | (hi8[2] << 16) | (hi8[3] << 24)
```

Unit `I` occupies bytes `12*I..12*I+12` as `word0,word1,word2`. Four adjacent
units form 48 bytes consumed as three 16-byte-aligned vector loads; every such
group begins at a 16-byte boundary. This grouping adds no padding and does not
alter unit order.

For source scale `(n,g)` with `g in 0..K/32`, let `u=n%64`. Its scale-plane
offset is:

```text
((g * (N/64) + n/64) * 64) + 8*(u%8) + u/8
```

This is a bijective 8x8 transpose inside each 64-row scale block. Native
scale bytes retain their exact admitted E4M3FN encodings; there is no silent
float16 conversion, clamping, or global-scale substitution in layout v1.

After TP slicing, fused gate/up uses logical rows `0..511` for gate and
`512..1023` for up. Down uses `[6144,512]`. The exact per-rank planes remain:

| Record | Code bytes | Scale bytes | Payload bytes |
|---|---:|---:|---:|
| fused gate/up | 2,359,296 | 196,608 | 2,555,904 |
| down | 1,179,648 | 98,304 | 1,277,952 |

## Direct ModelOpt-NVFP4 native representation

The real checkpoint uses distinct codec `0x0102`, named
`CODEC_MODELOPT_NVFP4_W4A16`. It is not codec `0x0100` or `0x0101` and never
uses their single-global-scale 128-byte metadata.

Values stay row-major with the admitted low-K nibble order. Scales use the
existing address-only `0x1201` SM120 K-major scale permutation. Both fused
gate/up and down use value and scale layout IDs `0x1201`; codec and projection
jointly provide the numerical and role discriminator. Fused rows are
`gate[0..512] || up[0..512]`. In particular, codec `0x0102` combined FC1 does
not enter the codec-`0x0100` interleaved-`0x1202` graph.

For scale `(n,g)`, with padded N and K equal to the admitted logical rank
shape, the native offset is the checked existing formula:

```text
n_block  = n / 128
n0       = n % 32
n1       = (n % 128) / 32
k_block  = g / 4
group_in = g % 4
offset   = 512 * (n_block * (K/64) + k_block)
         + 16*n0 + 4*n1 + group_in
```

The fused descriptor retains two weight outer scalars and two authenticated
input scalars. Down retains one of each; its second scalar slots are exact
positive-zero bits. Source component bytes are permuted, not re-quantized.

The rank-local planes are unchanged from the source accounting:

| Record | Value bytes | Scale bytes | Payload bytes |
|---|---:|---:|---:|
| fused gate/up `[1024,6144]` | 3,145,728 | 393,216 | 3,538,944 |
| down `[6144,512]` | 1,572,864 | 196,608 | 1,769,472 |

## Codec metadata

Both routed codecs use exactly 192 metadata bytes. NF3 retains the v1 field
table, with every integer explicitly little-endian and every digest count
encoded `u32_le`. ModelOpt-NVFP4 metadata is:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `G5M4W16\0` |
| 8 | 2 | version, exactly 1 |
| 10 | 2 | metadata bytes, exactly 192 |
| 12 | 2 | codec, exactly `0x0102` |
| 14 | 2 | flags: bit 0 set only for fused two-scalar FC1 |
| 16 | 4 | logical N |
| 20 | 4 | logical K |
| 24 | 4 | padded N, equal to logical N |
| 28 | 4 | padded K, equal to logical K |
| 32 | 1 | TP rank, `0..3` |
| 33 | 1 | TP size, exactly 4 |
| 34 | 1 | TP axis, `0=N`, `1=K` |
| 35 | 1 | role, `1=fused gate/up`, `2=down` |
| 36 | 2 | tile N, exactly 128 |
| 38 | 2 | scale group K, exactly 16 |
| 40 | 2 | value-layout ID, exactly `0x1201` |
| 42 | 2 | scale-layout ID, exactly `0x1201` |
| 44 | 1 | bits/value, exactly 4 |
| 45 | 1 | scale format, exactly `1=E4M3FN` |
| 46 | 2 | reserved, zero |
| 48 | 8 | value-plane bytes |
| 56 | 8 | scale-plane bytes |
| 64 | 4 | gate or down `weight_scale_2` bits |
| 68 | 4 | up `weight_scale_2`, or +0.0 for down |
| 72 | 4 | gate or down `input_scale` bits |
| 76 | 4 | up `input_scale`, or +0.0 for down |
| 80 | 32 | ordered source-route SHA-256 |
| 112 | 32 | complete hybrid-bit-map SHA-256 |
| 144 | 32 | native-layout-source SHA-256 |
| 176 | 4 | CRC32C with this field zeroed |
| 180 | 12 | reserved, zero |

Fused source-route order is gate value, gate scale, gate outer, gate input,
up value, up scale, up outer, up input. Down order is its four components.
Each component descriptor uses the domain
`glmaxx.source-component.v1\0` and binds length-prefixed UTF-8 tensor and shard
names, dtype, rank and LE dimensions, complete shard hash, absolute data range,
and payload hash. The route digest uses domain
`glmaxx.modelopt-nvfp4.source-route.v1\0`, `count:u32_le`, then the ordered
component descriptor digests.

NF3 metadata remains codec `0x0300` with its exact codebook bits and domains,
but its layout-source digest now binds the complete r2 value and scale formulas
above. A v1 candidate digest cannot identify r2 bytes.

## Streaming conversion and ABI boundary

Conversion is offline, deterministic, and bounded to 64 MiB scratch per rank
worker. It reads authenticated source rows, applies exact TP slicing, and emits
native planes directly. A complete int32-code tensor, dense BF16 expert, or
runtime checkpoint repack is forbidden.

Rust owns admission, conversion, rank consensus, immutable arena publication,
routing, graph selection, launch status, and evidence. CUDA exposes distinct
SM120-only W4A16 entry points for codec `0x0300` and codec `0x0102`.

The FC1 launch table supplies two F32 outer scales per ModelOpt expert. The
epilogue applies gate and up scales separately before SwiGLU. The FC2 table
supplies one F32 outer scale. Canonical codec `0x0100` may use a one-scale
entry point, but pointer aliasing or a stride guess cannot make it codec
`0x0102`. Every graph binds codec, role, layout IDs, numerical-policy digest,
and exact outer-scale arity.

NF3 reconstructs codebook and E4M3 scale fragments in registers and uses
BF16 MMA with FP32 accumulation. ModelOpt W4A16 reconstructs E2M1 and E4M3
weight fragments while leaving activations BF16. Neither writes a persistent
dense weight plane. The W4A4 control remains a diagnostic and speed candidate
only.

Small-M uses deterministic route compaction, codec-specialized work entries,
disjoint FP32 route scratch, and a canonical reduction. Prefill may stably
partition codec work into separate launches, but accepted input order,
reduction order, collective sequence, and failure posture are rank invariant.
Shared-output atomics and rank-local fallbacks are forbidden correctness paths.

## Required CPU proof

After design acceptance, the CPU gate must:

1. independently parse the real config/index and reproduce 14,400 target NF3,
   4,800 target ModelOpt-NVFP4, and 256 draft ModelOpt-NVFP4 experts;
2. prove the source arithmetic and malformed matrix without calling native
   inverse helpers;
3. exhaustively prove that the r2 NF3 coordinate stream covers each logical
   value exactly once for `[1024,6144]` and `[6144,512]`, then prove every
   three-word pack/unpack bit;
4. prove both scale-offset permutations are bijections at actual shapes;
5. prove separate gate/up scalar preservation and reject a one-scalar fused
   descriptor even when fixture values happen to compare equal;
6. encode, decode, stream-validate, and mutation-test both 192-byte metadata
   records, all reserved bytes, CRCs, domains, routes, tier identity, shapes,
   ranks, axes, roles, plane lengths, and scalar arity;
7. reproduce every per-expert, per-tier, target, draft, and rank byte sum with
   checked `u64` arithmetic; and
8. retain per-position reconstruction values and hashes from actual rank-0 and
   rank-3 gate/up/down source ranges without publishing checkpoint bytes.

## Gate sequence and nonclaims

The sequence is design review, CPU proof, implementation review, SM120 W4A16
controls, actual-shape codec microbenchmarks, mixed one-layer TP4 replay,
authenticated checkpoint smoke, MTP0 KLD, MTP3 correctness/KLD, then matched
end-to-end performance.

This design does not accept a parser, converter, rank manifest, container,
kernel, checkpoint, quality result, fit, capacity, cold start, hot reload, or
speed claim. Acceptance opens only CPU source parsing, native packing,
metadata, and proof implementation. W4A4 remains outside that authorization.
