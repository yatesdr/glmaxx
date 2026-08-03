# NF3/NVFP4 hybrid source and kernel contract v1

Date: 2026-08-03

Status: superseded by `nf3-nvfp4-hybrid-source-and-kernel-v1-r2.md`; do not
implement or issue the v1 acceptance token

## Purpose

This contract adds the real GLM-5.2 NVFP4/NF3 hybrid checkpoint as a distinct
source and execution profile. It does not reinterpret the existing EXL3/NVFP4
hybrid policy and never describes the checkpoint as pure NVFP4. The goals are
exact source admission, bounded offline conversion into an SM120-ready packed
layout, fast cold load without runtime repacking, and specialized small-M and
prefill kernels without persistent dense weight expansion.

## Source membership and admission

Admission binds the complete config, safetensors index, tier map, and every
payload file by authenticated content identity before publication. The
quantization config must select the expected ModelOpt NVFP4 producer family and
contain exactly layers 3 through 77. Every listed layer has exactly 256 integer
memberships, exactly 192 values equal to 3, and exactly 64 values equal to 4.
Layer 78 is absent from the map and is uniformly NVFP4. No other absent,
additional, fractional, string-valued, or unsupported membership is legal.

For each expert, gate, up, and down must all expose the component family chosen
by the map. NF3 is exactly `weight_packed:U8` plus
`weight_scale:F8_E4M3`. NVFP4 is exactly `weight:U8`,
`weight_scale:F8_E4M3`, scalar `weight_scale_2:F32`, and scalar
`input_scale:F32`. Mixed projection families, aliases, extras, overlap,
out-of-bounds offsets, non-canonical shapes, and rank disagreement fail the
whole checkpoint before device allocation.

The only admitted full expert shapes are:

| Projection | Logical `[N,K]` | NF3 packed | NF3 scale | NVFP4 packed | NVFP4 scale |
|---|---|---|---|---|---|
| gate/up | `[2048,6144]` | `[2048,2304]` | `[2048,192]` | `[2048,3072]` | `[2048,384]` |
| down | `[6144,2048]` | `[6144,768]` | `[6144,64]` | `[6144,1024]` | `[6144,128]` |

TP4 gate/up slice output rows into four contiguous 512-row shards. Down slices
the K dimension into four contiguous 512-column shards. Packed and scale
boundaries divide exactly by TP4; a generic byte chunk or caller-supplied axis
is forbidden. All four ranks derive and hash the same global membership before
constructing rank-local byte ranges.

## NF3 numerical definition

Eight source codes occupy three bytes. For source bytes `b0,b1,b2`, code `j`
for `j in 0..8` is:

```text
word = b0 | (b1 << 8) | (b2 << 16)
code[j] = (word >> (3*j)) & 7
```

The codebook is fixed by exact BF16 bits, not decimal parsing:

```text
code       0     1     2     3     4     5     6     7
bf16    bf80  bf1b  beb6  be03  3e03  3eb6  3f1b  3f80
```

One finite non-negative E4M3FN scale byte applies to each consecutive K/32
group. Zero is legal and stays exact zero. Negative values, NaN encoding, an
incomplete group, or a non-canonical dtype fails validation. The CPU oracle
converts the BF16 codebook entry and E4M3FN scale to FP32, multiplies once, and
rounds round-to-nearest-even to BF16. Kernel comparisons use that BF16 weight
as their authority; a decimal codebook or FP32-only reference is insufficient.

The source proof independently parses the safetensors byte range and may not
call the native packer's unpack helper. It covers all eight code positions,
all 256 adjacent two-code combinations, byte and group boundaries, signedness,
zero scales, every finite scale code, the NaN code, malformed lengths, and
actual gate/up/down tensors from at least one NF3 expert on TP ranks 0 and 3.
It retains per-position values and full reconstruction digests.

## ModelOpt NVFP4 numerical definition

Within each source byte the low nibble is the lower-K value and the high
nibble is the following value. Nibbles decode through the exact E2M1 table:

```text
code   0    1    2    3    4    5    6    7
value  0   .5    1   1.5   2    3    4    6
code   8    9    a    b    c    d    e    f
value -0  -.5   -1  -1.5  -2   -3   -4   -6
```

One E4M3FN block scale applies to each consecutive K/16 group. The finite,
positive scalar F32 `weight_scale_2` is the projection's outer multiplier, so
the source's real-valued weight is
`E2M1(code) * E4M3FN(block_scale) * weight_scale_2`. Gate and up retain their
separate outer multipliers after fusion. Scalar `input_scale` is required and
authenticated but is explicitly unused by this BF16-activation W4A16 profile;
its presence cannot silently select W4A4 arithmetic.

The independent NVFP4 source proof covers all sixteen nibbles in both byte
positions, every finite E4M3FN scale, negative/NaN scales, zero groups, scalar
non-finites, TP slice boundaries, gate/up outer-scale separation, and actual
gate/up/down tensors on ranks 0 and 3. It proves the exact producer-to-native
value and scale permutation against the mathematical source oracle before a
real tensor is exposed to an existing GLMAXX NVFP4 kernel.

## Native packed representation

Conversion is offline and streaming. No complete int32 code tensor or dense
BF16 weight matrix may exist. A bounded converter reads source rows, verifies
them, slices TP ownership, and emits the native code and scale planes directly.
Its peak scratch is explicit in the conversion receipt and initially capped at
64 MiB per rank worker.

The native NF3 value plane remains exactly three bits per logical value. It is
permuted into 12-byte units containing two output columns by sixteen K values:
two words carry the low two-bit planes and one word carries the high-bit plane.
Units are ordered N-tile-major, K16-row-major, then output-column-pair. The
layout binds `tile_n=256`; changing that tile requires a new layout ID and
offline conversion, never runtime reinterpretation. Scale bytes are permuted
into the matching K/32/N order without numerical re-encoding.

Gate and up are fused in `gate || up` row order after TP slicing. Per rank:

| Native record | Logical shape | Code bytes | Scale bytes | Payload bytes |
|---|---|---:|---:|---:|
| fused gate/up | `[1024,6144]` | 2,359,296 | 196,608 | 2,555,904 |
| down | `[6144,512]` | 1,179,648 | 98,304 | 1,277,952 |

An NF3 expert therefore consumes exactly 3,833,856 rank-local payload bytes
before fixed metadata. The 14,400 target NF3 experts consume
55,207,526,400 payload bytes per rank. These exact sums, not average bpw,
are allocation authority.

NF3 uses codec ID `0x0300`, value-layout ID `0x1230`, and scale-layout ID
`0x1231`. Its 192-byte little-endian metadata is exact:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `G5NF3V1\0` |
| 8 | 2 | version, exactly 1 |
| 10 | 2 | metadata bytes, exactly 192 |
| 12 | 2 | codec ID, exactly `0x0300` |
| 14 | 2 | flags, zero |
| 16 | 4 | logical N |
| 20 | 4 | logical K |
| 24 | 4 | padded N, equal to logical N in v1 |
| 28 | 4 | padded K, equal to logical K in v1 |
| 32 | 1 | TP rank, `0..3` |
| 33 | 1 | TP size, exactly 4 |
| 34 | 1 | TP axis, `0=N`, `1=K` |
| 35 | 1 | role, `1=fused gate/up`, `2=down` |
| 36 | 2 | tile N, exactly 256 |
| 38 | 2 | scale group K, exactly 32 |
| 40 | 2 | value-layout ID, exactly `0x1230` |
| 42 | 2 | scale-layout ID, exactly `0x1231` |
| 44 | 1 | bits per value, exactly 3 |
| 45 | 1 | scale format, `1=E4M3FN` |
| 46 | 2 | reserved, zero |
| 48 | 8 | code-plane bytes |
| 56 | 8 | scale-plane bytes |
| 64 | 16 | eight BF16 codebook entries in code order |
| 80 | 32 | source-route SHA-256 |
| 112 | 32 | complete tier-map SHA-256 |
| 144 | 32 | native-layout-source SHA-256 |
| 176 | 4 | CRC32C with this field zeroed |
| 180 | 12 | reserved, zero |

V1 requires N divisible by 256 and K divisible by 32. Code bytes are exactly
`N*K*3/8`; scale bytes are exactly `N*K/32`; every product and offset uses
checked 64-bit arithmetic. With two metadata records, an NF3 expert consumes
3,834,240 physical bytes and all target NF3 experts consume 55,213,056,000
physical bytes per rank before container descriptors and alignment.

The source-route digest is
`SHA256("glmaxx.nf3.source-route.v1\0" || count:u32 || ordered component
descriptor digests)`. Fused gate/up order is gate codes, gate scales, up codes,
up scales; down order is down codes, down scales. A component descriptor digest
is over the domain `glmaxx.source-component.v1\0`, length-prefixed UTF-8 tensor
and shard names, dtype ID, rank and LE u64 dimensions, complete shard SHA-256,
absolute data offset and byte length, and payload SHA-256. The authenticated
rank manifest retains those descriptors; metadata binds their route digest.

The native container and rank manifest gain a distinct NF3 codec/profile; they
do not widen an existing EXL3 codec or use a checkpoint name as type
information. Readers validate metadata, source route, and both planes before
creating a resident tensor.

NVFP4 uses its existing distinct native SM120 codec after the new source proof.
The hybrid contract does not equate producer bytes with the current GLMAXX
re-quantizer: conversion carries the producer's E2M1 values, E4M3 scales, and
outer multipliers through an independently checked permutation without
re-quantization. The fused gate/up descriptor binds both outer multipliers.

## Kernel execution

Rust owns source admission, conversion, arena construction, rank consensus,
routing, launch selection, serving, and evidence. CUDA supplies only SM120
specialized kernels behind a versioned C ABI. NF3 and NVFP4 descriptors are
validated before launch and dispatch never occurs inside a weight loop.

The NF3 kernel stages native 12-byte units, reconstructs three-bit codes in
registers, selects exact BF16 codebook fragments, applies the K/32 scale, and
uses BF16 tensor-core MMA with FP32 accumulation. It never materializes a
dense weight tile in global memory. The control path must match the CPU oracle
at actual GLM-5.2 rank shapes before timing.

Decode and prefill use different routing schedules:

- small M uses one deterministic combined work table. NF3 and NVFP4 entries
  call compile-time-specialized CTA bodies, write per-route FP32 scratch, and
  finish with one canonical ordered reduction; shared-output atomics are not a
  correctness path;
- prefill compacts routes once, partitions them into stable NF3 and NVFP4 bins,
  and runs specialized grouped kernels before the same ordered reduction; and
- every rank derives the same membership, route order, failure posture, and
  collective sequence. Rank-local fallback is forbidden.

The first performance controls may use two launches, provided their zeroing,
scratch, and reduction order are explicit and identical to the combined-grid
candidate. Profiling separates route construction, NF3, NVFP4, reduction,
collective, framework, and end-to-end time. Decode covers M=1,2,4,8 and prefill
covers actual routed row distributions rather than only dense GEMM shapes.

## Cold load and hot tuning

The production native image is converted once. Startup verifies immutable
headers and streams already-native planes directly into fixed HBM offsets; it
does not unpack or repack NF3 and does not re-quantize NVFP4. Per-phase bytes
and time distinguish storage, verification, staging, H2D, module load, graph
creation, KV allocation, and health publication.

Four persistent Rust rank workers own a versioned `ResidentWeightArena`.
Compatible cubin and tuning-table generations bind the same layout IDs and
descriptor hashes, quiesce collectively at a step boundary, validate on all
ranks, and commit or roll back atomically without rereading or retransferring
weights. This ABI reserves capacity and addresses for NVFP4 KV independently
of either weight codec.

## Quality and performance gates

NF3 quality is not inferred from packing equivalence. MTP0 KLD uses the pinned
prior cn4 procedure and retains per-position values, top-two margins, and
p50/p95/p99/max. NVFP4 and the complete hybrid receive separate matched
quality evidence. Precision membership, protected tensors, prompts, context,
batching, cache posture, and arithmetic stay fixed across comparisons.

After quality passes, the pinned Local Inference Lab v0.4.29 script runs the
goal matrix. Kernel and end-to-end results retain every sample, MTP acceptance,
physical steps, clocks, power, memory, KV capacity, and failures. A speed claim
requires the real four-rank checkpoint and an explicit NF3/NVFP4 timing split.

## Gate sequence

1. adversarial acceptance of this source, layout, arithmetic, isolation, and
   execution contract;
2. independent CPU source decoder, streaming native packer, metadata, exact
   accounting, malformed-input matrix, and real-source proof;
3. adversarial implementation and evidence review;
4. SM120 NF3 control microbenchmark and CPU-oracle comparison;
5. optimized NF3 plus existing NVFP4 actual-shape microbenchmarks;
6. mixed 192:64 one-layer TP4 replay;
7. authenticated full-checkpoint smoke and MTP0 quality;
8. MTP3 correctness and quality; and
9. matched decode, prefill, capacity, cold-start, and hot-reload evidence.

This design does not accept the checkpoint, staging exports, an implementation,
a container, a kernel, a quality value, a capacity claim, or a speed result.
Acceptance opens only CPU source parsing, native-plane packing, metadata, and
proof work; the new complete rank-manifest/profile ABI still requires its own
review before publication.
