# GLM-5.2 native rank and cache format v0

Status: **DRAFT — FABLE V2 CONDITIONS PLUS PHASE-A IMPLEMENTATION AMENDMENT**

Specification revision: 0.2.2

Date: 2026-07-28

Implementation order: NVFP4 first, EXL3 extension second

## 1. Purpose

This specification defines:

1. the rank-local checkpoint container loaded by the native engine;
2. the first NVFP4 weight codec;
3. the compatibility boundary for the later EXL3/Trellis codec;
4. the dynamic 368-byte MLA KV record;
5. target and MTP-draft KV/indexer HBM page geometry;
6. the DRAM/NVMe target record and combined draft sidecar.

The container is fixed to GLM-5.2, TP4, and SM120. It is not a general model
format.

The normative keyword meanings are inherited from
[engine-v0.md](engine-v0.md).

## 2. Design rules

- There SHALL be one file per TP rank.
- A rank file SHALL contain already-sharded weights.
- A payload SHALL be in the exact layout consumed by its kernel.
- Runtime whole-tensor transpose, swizzle, or repack is forbidden.
- Runtime persistent EXL3-to-BF16/FP16 reconstruction is forbidden.
- All integer fields are little-endian.
- All offsets are absolute file offsets.
- All unused/reserved bytes SHALL be zero.
- Every payload and metadata region SHALL be cryptographically hashed.
- Unknown required flags, codecs, layouts, tensors, or metadata SHALL fail
  closed.

## 3. File identity

Rank files SHALL be named:

```text
glm52-native-v0-rank0.g5n
glm52-native-v0-rank1.g5n
glm52-native-v0-rank2.g5n
glm52-native-v0-rank3.g5n
```

The name is advisory. Header rank identity is authoritative.

The magic bytes are ASCII:

```text
GLM5NAT0
```

## 4. File organization

```text
4096-byte fixed header
canonical JSON manifest
256-byte tensor descriptor array
UTF-8 string table
codec metadata records
alignment padding
payload region
```

Every top-level region SHALL begin at a 4,096-byte boundary. Every weight
payload SHALL satisfy its descriptor alignment, which MUST be at least 256
bytes.

Regions SHALL NOT overlap. Offsets and lengths SHALL be checked with
overflow-safe arithmetic before any device allocation.

## 5. Fixed file header

The header is exactly 4,096 bytes.

| Offset | Bytes | Field | Required value/meaning |
|---:|---:|---|---|
| 0 | 8 | `magic` | `GLM5NAT0` |
| 8 | 2 | `format_major` | `0` |
| 10 | 2 | `format_minor` | `2` for this draft |
| 12 | 4 | `header_bytes` | `4096` |
| 16 | 4 | `endian_marker` | `0x01020304` |
| 20 | 4 | `flags` | defined below |
| 24 | 4 | `tp_rank` | `0..3` |
| 28 | 4 | `tp_degree` | `4` |
| 32 | 4 | `tensor_count` | descriptor count |
| 36 | 4 | reserved | zero |
| 40 | 8 | `manifest_offset` | aligned absolute offset |
| 48 | 8 | `manifest_bytes` | exact JSON bytes |
| 56 | 8 | `descriptor_offset` | aligned absolute offset |
| 64 | 8 | `descriptor_bytes` | `tensor_count × 256` |
| 72 | 8 | `string_offset` | aligned absolute offset |
| 80 | 8 | `string_bytes` | UTF-8 table length |
| 88 | 8 | `codec_meta_offset` | aligned absolute offset |
| 96 | 8 | `codec_meta_bytes` | metadata region length |
| 104 | 8 | `payload_offset` | aligned absolute offset |
| 112 | 8 | `payload_bytes` | payload region length |
| 120 | 32 | `model_config_sha256` | exact config bytes |
| 152 | 32 | `tokenizer_bundle_sha256` | canonical tokenizer bundle |
| 184 | 32 | `chat_template_sha256` | exact template bytes |
| 216 | 32 | `weight_policy_sha256` | canonical precision map |
| 248 | 32 | `kernel_abi_sha256` | required kernel ABI |
| 280 | 32 | `manifest_sha256` | exact manifest region |
| 312 | 32 | `descriptor_sha256` | exact descriptors |
| 344 | 32 | `payload_sha256` | exact payload region incl. padding |
| 376 | 16 | `file_uuid` | deterministic rank content identity |
| 392 | 16 | `conversion_uuid` | deterministic identity common across ranks |
| 408 | 8 | `created_unix_seconds` | `0`; time belongs in a sidecar |
| 416 | 4 | `header_crc32c` | header with this field zeroed |
| 420 | 32 | `string_sha256` | exact string-table region |
| 452 | 32 | `codec_meta_sha256` | exact codec-metadata region |
| 484 | 3,612 | reserved | zero |

Flag bits:

| Bit | Meaning |
|---:|---|
| 0 | all payloads are direct kernel layouts |
| 1 | file contains NVFP4 weights |
| 2 | file contains EXL3 weights |
| 3 | file contains source-precision protected tensors |
| 4 | file belongs to a hybrid policy |
| 5–31 | reserved; MUST be zero |

The loader SHALL verify CRC32C before parsing offsets. Cryptographic payload
verification follows section 26.1.

### 5.1 Deterministic identities

The canonical rank manifests MUST NOT contain `file_uuid`,
`conversion_uuid`, a build timestamp, a random nonce, or another
nondeterministic value.

After all four rank payloads are complete, compute:

```text
conversion_uuid = first_16_bytes(SHA256(
    "g5n-conversion-v0\0"
    || rank0.manifest_sha256
    || rank0.descriptor_sha256
    || rank0.payload_sha256
    || rank1.manifest_sha256
    || rank1.descriptor_sha256
    || rank1.payload_sha256
    || rank2.manifest_sha256
    || rank2.descriptor_sha256
    || rank2.payload_sha256
    || rank3.manifest_sha256
    || rank3.descriptor_sha256
    || rank3.payload_sha256
))
```

For rank `r`, compute:

```text
file_uuid = first_16_bytes(SHA256(
    "g5n-file-v0\0"
    || conversion_uuid
    || little_endian_u32(r)
    || rank_r.manifest_sha256
    || rank_r.descriptor_sha256
    || rank_r.payload_sha256
))
```

The fixed header timestamp SHALL be zero. Human build time, operator, host,
and job identity MAY be recorded in a sidecar that is not part of the rank
file. Two conversions with byte-identical canonical inputs and toolchain
pins SHALL produce byte-identical rank files.

## 6. Canonical manifest

The manifest SHALL be UTF-8 JSON canonicalized using RFC 8785 JSON
Canonicalization Scheme.

It SHALL include:

- source repository and immutable revision;
- tokenizer/config/chat-template source and hashes;
- conversion repository and commit;
- calibration corpus IDs, revisions, sample hashes, seed, template, and
  truncation;
- quantizer and codec source revisions;
- CUDA, compiler, CUTLASS, Rust, and kernel ABI pins;
- TP degree and rank;
- complete tensor inventory;
- tensor-to-codec/protection map;
- logical parameters and physical bytes by role/codec;
- reviewed `profile-budget-v0.json` SHA-256 for serving profiles;
- source and output payload hashes;
- rank sharding rules;
- model operation manifest hash;
- weight profile name;
- license/provenance notices.

Human-readable floating-point values in the manifest are provenance only.
Kernel-consumed scales SHALL be stored in binary codec metadata.

## 7. Tensor descriptors

Each descriptor is exactly 256 bytes.

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 4 | `tensor_id` |
| 4 | 4 | `name_offset` in string table |
| 8 | 2 | `name_bytes` |
| 10 | 2 | `role_id` |
| 12 | 2 | signed `layer_id`; `-1` when global |
| 14 | 2 | signed `expert_id`; `-1` when not expert-specific |
| 16 | 2 | `codec_id` |
| 18 | 2 | `logical_dtype` |
| 20 | 2 | `stored_dtype` |
| 22 | 1 | signed `tp_shard_axis`; `-1` replicated |
| 23 | 1 | `ndim`; `1..4` |
| 24 | 1 | descriptor flags |
| 25 | 3 | reserved |
| 28 | 16 | `logical_shape[4]` as u32 |
| 44 | 16 | `padded_shape[4]` as u32 |
| 60 | 4 | reserved/alignment pad |
| 64 | 8 | `logical_elements` |
| 72 | 8 | `payload_offset` |
| 80 | 8 | `payload_bytes` |
| 88 | 8 | `aux_offset` |
| 96 | 8 | `aux_bytes` |
| 104 | 8 | `codec_metadata_offset` |
| 112 | 8 | `codec_metadata_bytes` |
| 120 | 4 | `payload_alignment` |
| 124 | 4 | `quant_group_elements` |
| 128 | 32 | `payload_sha256` |
| 160 | 32 | `aux_sha256` |
| 192 | 32 | `codec_metadata_sha256` |
| 224 | 32 | reserved |

Shapes are logical row-major dimensions. Unused shape entries SHALL be one.

Descriptor flags:

| Bit | Meaning |
|---:|---|
| 0 | replicated tensor |
| 1 | column-parallel shard |
| 2 | row-parallel shard |
| 3 | routed expert |
| 4 | shared expert |
| 5 | MTP tensor |
| 6 | protected/source precision |
| 7 | aux region required |

Only one of bits 0–2 may be set. Role, name, layer, expert, and shape MUST
match the fixed model operation manifest.

## 8. Dtype identifiers

| ID | Dtype |
|---:|---|
| 0 | invalid |
| 1 | BF16 |
| 2 | FP16 |
| 3 | FP32 |
| 4 | FP8 E4M3 |
| 5 | unsigned E4M3/UE4M3 scale byte |
| 6 | packed E2M1x2 |
| 7 | u8 |
| 8 | u16 |
| 9 | u32 |
| 10 | i16 |

Logical dtype describes the reconstructed tensor. Stored dtype describes the
primary payload plane.

## 9. Codec identifiers

| ID | Codec |
|---:|---|
| `0x0001` | BF16 row-major |
| `0x0002` | FP16 row-major |
| `0x0003` | FP32 row-major |
| `0x0100` | SM120 NVFP4 1D block-16 |
| `0x0101` | SM120 NVFP4 2D block-16×16 |
| `0x0200` | EXL3/Trellis pinned source layout |

Unknown codecs SHALL be rejected.

Codec `0x0200` remains blocking-OPEN and SHALL be rejected by an
implementation until its codec revision is nonzero and this specification
contains the reviewed reconstruction rules.

## 10. Plain protected tensors

BF16, FP16, and FP32 payloads SHALL be little-endian row-major arrays over
the padded shape. Padding values SHALL be zero.

The payload SHALL already represent the rank-local TP shard. The loader
SHALL NOT slice a plain tensor.

## 11. NVFP4 numerical definition

For logical value `x`:

```text
x_reconstructed = e2m1(code) × e4m3(block_scale) × global_scale
```

where:

- `code` is a signed E2M1 value with maximum magnitude 6;
- `block_scale` is E4M3;
- `global_scale` is FP32;
- one block spans 16 K-consecutive values.

For a tensor or row-scale domain with absolute maximum `global_amax`:

```text
global_scale = global_amax / (448 × 6)
```

For each quantization block:

```text
block_scale_real = (block_amax / 6) / global_scale
block_scale = round_satfinite_e4m3(block_scale_real)
code = round_satfinite_e2m1(x / (decode(block_scale) × global_scale))
```

The encoder MUST use decoded stored E4M3 scale when producing E2M1 codes,
not the pre-rounding real scale.

For an all-zero global domain, `global_scale` SHALL be FP32 `1.0`, every
E4M3 block-scale byte SHALL be positive zero `0x00`, and every E2M1 nibble
SHALL be positive zero `0x0`.

Within a nonzero global domain, an all-zero block SHALL also use E4M3 scale
byte `0x00` and positive-zero E2M1 codes.

Quantization for inference weights SHALL use deterministic round-to-nearest
and saturated-finite conversion. Stochastic rounding is forbidden.

## 12. NVFP4 weight scaling modes

### 12.1 Codec `0x0100`: 1D

Each logical row `W[n, :]` uses a scale for each K-consecutive group of 16.
The FP32 `global_scale` applies to the complete tensor shard unless metadata
declares reviewed row scaling.

### 12.2 Codec `0x0101`: 2D

Each logical 16×16 tile over `(N,K)` shares one computed E4M3 scale. For the
hardware scale plane, the scale is repeated for all 16 N rows in that tile.

The converter SHALL retain only the direct hardware scale plane unless an
auxiliary natural-order scale plane is explicitly present for testing.
Production loading SHALL not require that auxiliary plane.

1D and 2D policies are numerically different codecs and MUST have different
IDs, policy hashes, and quality results.

## 13. Packed value plane

The logical weight operand is:

```text
W[N, K]
```

and GEMM computes:

```text
Y[M, N] = A[M, K] × transpose(W[N, K])
```

N SHALL be padded to the reviewed kernel tile requirement. K SHALL be padded
to a multiple of 64 so four block-16 scales form a complete scale tile.

The first direct layout is `value_layout_id = 0x1201`. It stores logical
`W[N_padded,K_padded]` contiguously in row-major order, which is the same
address sequence as column-major GEMM operand `B[K,N]`. Two E2M1 values are
packed per byte. Logical linear element `2i` occupies bits `[3:0]` and
element `2i+1` occupies bits `[7:4]`; `nibble_order = 1`.

The kernel SHALL consume this plane directly without a runtime permutation.
Layout ID zero is invalid.

## 14. Scale plane

The natural scale tensor is:

```text
S[N_padded, K_padded / 16]
```

with one byte per entry. Its second dimension SHALL be padded to a multiple
of four and its first dimension to a multiple of 128.

Production scale payloads SHALL use the Blackwell `32x4x4`/128×4 tiled
layout selected by the kernel. No runtime swizzle is allowed.

The scale layout SHALL be generated by a Rust reference implementation and
proven byte-for-byte against the pinned CUTLASS
`Sm1xxBlockScaledConfig<16>` SFB layout.

The first direct layout is `scale_layout_id = 0x1201`, pinned to CUTLASS
commit `e05f953a5b3d38adc240df2ff928e0421c2abba3` and
`Sm1xxBlockScaledConfig<16>` with K-major SFB. For natural `(n,g)`, where
`g = k / 16`, define:

```text
n_block = n / 128
n0      = n mod 32
n1      = (n mod 128) / 32
k_block = g / 4
g_in    = g mod 4
k_blocks = K_padded / 64

offset = 512 × (n_block × k_blocks + k_block)
       + 16 × n0
       + 4 × n1
       + g_in
```

Every offset in `[0, N_padded × K_padded / 16)` SHALL be produced exactly
once. This formula and the nibble order above MUST be checked against the
pinned CUTLASS types again during the authorized SM120 build.

## 15. NVFP4 codec metadata

The NVFP4 metadata record is exactly 128 bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 2 | metadata major; `0` |
| 2 | 2 | metadata minor |
| 4 | 2 | codec ID |
| 6 | 1 | scaling mode: 1 or 2 |
| 7 | 1 | operand role: weight-B |
| 8 | 4 | logical N |
| 12 | 4 | logical K |
| 16 | 4 | padded N |
| 20 | 4 | padded K |
| 24 | 2 | K group; `16` |
| 26 | 2 | N group; `1` or `16` |
| 28 | 2 | value layout ID |
| 30 | 2 | scale layout ID |
| 32 | 1 | nibble order |
| 33 | 1 | rounding mode |
| 34 | 1 | scale dtype; E4M3 |
| 35 | 1 | global scale mode |
| 36 | 4 | FP32 global scale |
| 40 | 4 | FP32 recorded global amax |
| 44 | 4 | value plane bytes |
| 48 | 4 | scale plane bytes |
| 52 | 4 | reserved |
| 56 | 32 | layout source SHA-256 |
| 88 | 32 | quant policy fragment SHA-256 |
| 120 | 4 | metadata CRC32C with field zero |
| 124 | 4 | reserved |

The tensor descriptor payload points to the packed value plane. Its aux
region points to the swizzled scale plane. `global_scale` is kernel-consumed
metadata and SHALL be copied into immutable device control memory.

## 16. Activation NVFP4

Activations are not stored in the rank file.

The kernel ABI SHALL quantize activations with 1D block-16 scales and one
dynamic FP32 global scale per logical activation row:

```text
row_global_scale = row_amax / (448 × 6)
```

An all-zero row SHALL use FP32 `1.0`, E4M3 scale byte `0x00`, and
positive-zero E2M1 codes. The per-row amax reduction, scale swizzle, and
quantization MUST be fused or explicitly timed as part of the routed-expert
operator. A static calibrated or batch-shared activation global scale is a
different codec and is outside v0.

The weight format does not imply that activation quantization is free or
that FP4×FP4 is always selected. An alternative W4A16 control MAY consume
the same logical weights through a separate reviewed kernel, but persistent
whole-weight dequantization remains forbidden.

## 17. EXL3/Trellis compatibility record

EXL3 is a required destination codec but is not sufficiently defined by
public prose alone. The upstream project describes it as a QTIP variant with
procedural codebooks and tail-biting trellises and explicitly refers
implementers to its quantizer and kernels for the current definition.

Therefore codec `0x0200` SHALL be frozen only after:

1. pinning the exact quantizer and inference kernel revisions;
2. inventorying every component tensor in the pinned TR3 checkpoint;
3. defining bit order, trellis state, codebook generation, scale/normalizer,
   padding, permutation, and accumulation arithmetic;
4. implementing a Rust CPU decoder;
5. reproducing control payload reconstruction byte-for-byte or
   value-for-value under an exact tolerance;
6. defining a direct SM120 payload layout that avoids runtime repack;
7. recording the MIT license attribution for reused source.

Until then, the container MAY carry opaque EXL3 source components for
inspection, but an engine MUST NOT report codec `0x0200` load support.

This OPEN item does not block NVFP4-first engine bring-up.

## 18. Tensor role IDs

The generated model manifest SHALL use stable role IDs from these families:

| Range | Family |
|---:|---|
| `0x0000–0x00ff` | embedding, LM head, final norm |
| `0x0100–0x01ff` | attention and low-rank projections |
| `0x0200–0x02ff` | sparse indexer |
| `0x0300–0x03ff` | router and correction tensors |
| `0x0400–0x04ff` | dense MLP |
| `0x0500–0x05ff` | routed expert MLP |
| `0x0600–0x06ff` | shared expert MLP |
| `0x0700–0x07ff` | norms and residual controls |
| `0x0800–0x08ff` | MTP draft tensors |

Exact IDs are a blocking OPEN item generated from the pinned tensor
inventory. Runtime code SHALL dispatch by role ID, not arbitrary tensor
name, after validation.

The first routed-expert manifest freezes:

| ID | Role |
|---:|---|
| `0x0301` | protected replicated router weight |
| `0x0302` | protected replicated router correction bias |
| `0x0501` | rank-local fused routed gate/up projection |
| `0x0502` | rank-local routed down projection |

## 19. Rank sharding

The manifest SHALL specify for every tensor:

- global logical shape;
- TP shard axis or replication;
- rank-local logical shape;
- rank-local padded shape;
- source slice bounds;
- reconstruction formula when a projection is fused;
- collective boundary after consumption.

All four rank headers SHALL contain the same derived conversion UUID. All
four manifests SHALL share the same weight policy hash. Their tensor IDs,
names, roles, and codec policies SHALL be identical; only rank identity,
rank-local slices, payload hashes, and allowed padding may differ.

The four-file validator SHALL run before any GPU load.

## 20. Dynamic MLA KV record

The runtime KV record is exactly:

| Byte range | Bytes | Meaning |
|---|---:|---|
| `[0,256)` | 256 | 512 E2M1 NoPE values |
| `[256,288)` | 32 | E4M3 scale per 16 NoPE values |
| `[288,292)` | 4 | FP32 RoPE scale |
| `[292,296)` | 4 | FP32 per-token NoPE global scale |
| `[296,304)` | 8 | zero |
| `[304,368)` | 64 | 64 E4M3 RoPE values |

The numerical rules are in engine spec section 16. The complete ABI identity
is:

```text
nvfp4_ds_mla:fp8-rope-368:dynamic-token-v1
```

Any static-calibrated record with zero padding at `[292,296)` has different
semantics and SHALL use a different namespace.

## 21. HBM cache geometry

Target kernel-facing HBM SHALL be:

```text
u8 target_kv[target_layer=78][local_page][token=64][record=368]
```

Each layer page fragment is:

```text
64 × 368 = 23,552 bytes
```

Each complete owner-local logical page over all target layers is:

```text
78 × 23,552 = 1,837,056 bytes
```

At the exact model limit there are 16,384 logical pages. Under reviewed
round-robin DCP4 ownership there are 4,096 pages and 262,144 committed token
slots per rank before sequence-tail slack.

An MTP-capable sequence SHALL additionally allocate:

```text
u8 draft_kv[draft_layer=1][local_page][token=64][record=368]
u8 draft_indexer_k[draft_layer=1][local_page][token=64][record=132]
```

The draft KV fragment is 23,552 bytes and the draft-indexer fragment is 8,448
bytes per logical page, for a 32,000-byte combined sidecar payload. Across the
model limit it consumes 524,288,000 bytes = 0.48828125 GiB aggregate, or
0.1220703125 GiB per DCP4 rank before tail slack.

Target and draft page tables are distinct but SHALL use the same logical page
ordinal and owner. The mandatory indexer table below shares that ordinal and
owner. An MTP-capable page commit SHALL publish matching target, indexer, and
draft generations atomically.

The mandatory sparse-indexer key cache SHALL be:

```text
u8 indexer_k[full_group=21][local_page][token=64][record=132]
```

The group order is the full-indexer layer order fixed by engine section 16.1.
One logical page payload is `21 × 64 × 132 = 177,408` bytes. The indexer page
table uses the same logical page ordinal, owner, and generation as target KV.

## 22. Sealed target tier record

DRAM and NVMe SHALL store a contiguous sealed page record. It does not change
KV numerical bytes.

Each target record SHALL occupy 450 4-KiB blocks:

```text
4,096-byte header
1,837,056-byte payload
2,048-byte zero padding
= 1,843,200 bytes
```

The payload order SHALL be:

```text
[layer=0..77][token=0..63][record_byte=0..367]
```

GPU pack/unpack kernels MAY gather/scatter the HBM layer fragments. They
SHALL preserve every payload byte.

### 22.1 Sealed draft sidecar record

An MTP-capable sealed prefix SHALL also store one combined draft record
occupying nine 4-KiB blocks:

```text
4,096-byte header
32,000-byte payload
768-byte zero padding
= 36,864 bytes
```

Its payload is token-major so a committed position's two required records
cannot be split:

```text
[token=0..63][draft_kv_byte=0..367][draft_indexer_key_byte=0..131]
```

A target record MAY exist without a draft record and is then MTP0-only. A
draft record MUST NOT exist or attach without its paired target and
target-indexer records.

### 22.2 Sealed indexer-key sidecar record

Every sealed target prefix SHALL store one mandatory indexer-key sidecar
occupying 45 4-KiB blocks:

```text
4,096-byte header
177,408-byte payload
2,816-byte zero padding
= 184,320 bytes
```

Its payload order SHALL be:

```text
[full_group=0..20][token=0..63][record_byte=0..131]
```

The indexer record MUST NOT exist or attach without its paired target record.
A target record without its indexer record is incomplete and cannot attach to
an attention-capable sequence.

## 23. Sealed tier headers

The target, draft, and indexer headers are 4,096 bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | target magic `G5KVPAG0`; draft magic `G5MTPG00`; indexer magic `G5IDXPG0` |
| 8 | 2 | major `0` |
| 10 | 2 | minor `3` |
| 12 | 4 | header bytes `4096` |
| 16 | 32 | content namespace SHA-256 |
| 48 | 32 | logical page/content SHA-256 |
| 80 | 32 | parent page SHA-256 |
| 112 | 16 | record UUID |
| 128 | 8 | publication generation |
| 136 | 8 | logical page ordinal |
| 144 | 4 | writer owner rank; advisory after tier publication |
| 148 | 4 | valid tokens; MUST be 64 for shared records |
| 152 | 4 | target layers `78`; draft components `2`; indexer groups `21` |
| 156 | 4 | token record bytes; target `368`, draft composite `500`, indexer `132` |
| 160 | 8 | target payload `1,837,056`; draft `32,000`; indexer `177,408` |
| 168 | 32 | payload SHA-256 |
| 200 | 32 | token ID page SHA-256 |
| 232 | 8 | created Unix seconds |
| 240 | 4 | header CRC32C with field zero |
| 244 | 32 | paired target page key; zero for a target record |
| 276 | 3,820 | reserved zero |

A draft or indexer header SHALL contain the exact target page key at
`[244,276)`. Target, draft, and indexer headers SHALL use different content
namespace hashes and therefore different page keys.

Mutable or partial pages SHALL NOT be serialized as shared tier records.
Private paused tails, if later supported, require a different record type
and are OPEN for v0.

## 24. Prefix content key

For a sealed 64-token page, compute:

```text
page_key = SHA256(
    content_namespace_hash
    || parent_page_key
    || little_endian_u32(64)
    || little_endian_u32(token_id[0])
    || ...
    || little_endian_u32(token_id[63])
)
```

The result SHALL equal the tier header logical page/content SHA-256.

The content namespace SHALL include:

- model config and weight policy hashes;
- tokenizer and chat-template hashes;
- target or draft KV ABI string and record role;
- draft-indexer ABI string for the combined draft record;
- the sparse-indexer key ABI string for indexer records;
- page size.

It SHALL exclude DCP ownership, writer rank, HBM layout, and kernel revisions
that leave the record arithmetic and bytes unchanged. A semantic kernel
change SHALL use a new KV ABI instead of invalidating content through an
attachment detail.

The HBM attachment ABI SHALL separately include:

- DCP4 owner mapping;
- target/draft KV/indexer page-table and physical HBM layout;
- kernel ABI and alignment requirements.

Restore validates the content namespace and payload hash before applying the
current attachment ABI. The header `owner_rank` is not part of the page key.

## 25. NVMe publication

A target-only record write SHALL:

1. allocate a new generation and aligned location;
2. write payload and padding;
3. write the completed header;
4. flush according to the configured durability mode;
5. append a checksummed index entry;
6. publish the index generation atomically.

A target write SHALL complete and flush both target and mandatory indexer
records, then publish both index entries in one generation. An MTP-capable
write SHALL also complete and flush the combined draft-KV/draft-indexer record
and publish all three entries in that generation. An incomplete draft write
MAY still publish the paired target plus indexer records as MTP0-only; it MUST
NOT publish an orphan draft or indexer entry.

An index entry SHALL contain at minimum:

- content namespace hash;
- page key;
- file/segment identity;
- byte offset;
- record generation;
- payload hash;
- last-access epoch;
- reference/pin state only if crash-recoverable.

On recovery, a header or index checksum failure SHALL invalidate that record.
The engine MAY recompute a missing prefix; it MUST NOT use a partially
validated payload.

The exact bounded log compaction protocol is OPEN and requires review before
the NVMe serving gate.

## 26. Validation requirements

### 26.1 Rank-file integrity policy

Every rank file SHALL have one strong verification path:

1. `FULL_SHA256`: recompute and compare every required manifest,
   descriptor, metadata, auxiliary, and payload SHA-256 while staging the
   file; or
2. `FS_VERITY`: verify a pinned fs-verity root digest for the immutable file,
   then rely on kernel-verified Merkle blocks as they are read.

First load after conversion or transfer SHALL use `FULL_SHA256` unless the
transfer produced and independently verified the pinned fs-verity root.
Routine restarts MAY use `FS_VERITY`. File size/mtime/inode receipts and
noncryptographic checksums alone are insufficient. CRC32C remains an early
corruption/error check, not an authenticity substitute.

Strong verification SHOULD pipeline across ranks and with host-to-device
staging. The selected mode, root/file digest, bytes verified, and elapsed
time SHALL be recorded.

### Container

- corrupt magic, CRC, offset, length, and hash rejection;
- overflow and overlap rejection;
- unknown required flag/codec rejection;
- four-rank conversion UUID and policy agreement;
- missing/extra tensor rejection;
- deterministic conversion.

### NVFP4 weights

- every E2M1 code and E4M3 scale class;
- zero, subnormal-scale, extrema, outlier, and tail blocks;
- 1D and 2D scale policies;
- padding;
- natural-to-swizzled scale equivalence;
- CPU reconstruction versus independent FP32 reference;
- selected GLM real tensors and TP slices.

### KV

- exact 368-byte map;
- canonical all-zero NoPE and RoPE bytes;
- dynamic NoPE scale at `[292,296)`;
- static-record namespace rejection;
- record writer/reader error floor;
- 64-token and layer boundary behavior;
- target/indexer mandatory pairing plus optional combined
  draft-KV/draft-indexer pairing, atomic commit, and orphan rejection;
- DCP ownership/remap without changing the content page key;
- bit-exact HBM/DRAM/NVMe round trip;
- prefix key and copy-on-write tail behavior.

### EXL3

The codec remains unsupported until its reviewed CPU oracle and component
inventory pass.

## 27. ABI freeze rules

Changing any of these requires a format minor or major change:

- tensor descriptor size or meaning;
- codec arithmetic;
- value/scale permutation;
- nibble order;
- scale granularity or outer scale scope;
- rank sharding;
- KV byte meaning;
- page size;
- prefix content-namespace inputs;
- tier payload order.

Changing only the DCP owner mapping or HBM layout changes the attachment ABI,
not the ownership-neutral tier record format or its content page key.

A writer SHALL emit only an ABI version accepted by the exact kernel ABI in
the header. A loader SHALL never guess a compatible interpretation.

## 28. Blocking OPEN items for independent review

1. Exact model tensor role table.
2. NVFP4 value layout ID, nibble order, and pinned kernel source hash.
3. Independent pinned-CUTLASS proof of the frozen scale swizzle formula.
4. Selection between 1D and 2D NVFP4 for each GLM tensor class.
5. Complete EXL3/Trellis internal codec.
6. Private paused-tail record representation.
7. Crash-consistent bounded target/draft NVMe index and compaction protocol.

Items 1–4 block the NVFP4 format freeze. Item 5 blocks EXL3 support but does
not block M1–M4 NVFP4 work. Items 6–7 block tiered-cache serving.
