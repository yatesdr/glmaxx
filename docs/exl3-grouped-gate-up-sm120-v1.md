# SM120 EXL3 grouped paired gate/up decode v1

Date: 2026-08-04

Status: design candidate; adversarial acceptance required before CPU or CUDA
implementation

Predecessor: `glmaxx.sm120.exl3.warp_staged_projection.v2`

GPU evidence motivating this design:
`docs/cn4-exl3-staged-k3-ncu-20260804.md`

## Purpose and boundary

The staged single-projection kernel preserves the scalar EXL3 result, but an
M1 gate or up projection launches only 32 CTAs on a 188-SM GPU. GLM-5.2
selects eight distinct routed experts per token. This successor makes that
model invariant visible to the GPU: it launches all active EXL3 experts
together and computes each expert's gate and up projection in the two
half-warps of one warp.

The kernel consumes the same source trellis and FP16 rotations as the retained
control. It does not reconstruct persistent weights, split K, change the
ascending-K recurrence, fuse SwiGLU, execute down projection, scatter route
weights, select experts, or perform a TP collective. Those remain separate
operations. The immediate goal is a bitwise-identical grouped FC1 projection
plane with enough independent CTAs for SM120 decode.

This first route accepts decode/verify row buckets `1..8`. Larger continuous
batches require a later bucketed successor and cannot silently use this ABI.

## Rank-invariant route input

The protected router produces eight distinct expert IDs per real token. The
existing stable compaction order is:

```text
(expert_id ascending, token_row ascending, route_slot ascending)
```

Before launch, Rust filters that common compacted table to assignments whose
immutable target-program backend is three-bit EXL3. Filtering preserves
relative order. K=4 assignments use their separately reviewed route and never
enter this descriptor.
All ranks hash and agree on the full router table, immutable backend policy,
filtered assignment table, expert offsets, active-expert list, and launch
route before any rank enqueues work. A rank-local backend choice, empty-route
choice, retry, or fallback is forbidden.

The filtered device arrays are:

```text
route_experts_u16[assignments]
route_tokens_u32[assignments]
route_slots_u8[assignments]
expert_offsets_u32[257]
active_experts_u16[active_expert_count]
```

`expert_offsets[0]` is zero, offsets are monotonic, and
`expert_offsets[256] == assignments`. `active_experts` contains exactly the
strictly ascending experts whose adjacent offsets differ. Every route token is
less than `rows`, every route slot is below eight, and each token has no
duplicate expert or slot. The M1 all-K3 path therefore has exactly eight
assignments and eight active experts. A zero-K3 assignment set is a
collectively selected no-launch route, not a valid nonempty descriptor.

## Immutable resident pointer tables

Each layer and rank owns six 256-entry device pointer tables, uploaded and
hash-verified with the resident weight generation:

```text
gate_trellis_u16[256]    up_trellis_u16[256]
gate_suh_f16[256]        up_suh_f16[256]
gate_svh_f16[256]        up_svh_f16[256]
```

An entry is nonzero only when that expert's gate/up backend is the split K=3
EXL3 source representation. Gate and up membership is expert-atomic. Every active
expert must resolve six nonzero, correctly aligned pointers within an adopted
resident tensor span. The pointer-table content digest is part of the common
target-program/resident-generation identity. Host pointers, per-launch weight
repacking, and device-side backend dispatch are forbidden.

## Descriptor ABI

The new C/Rust descriptor is `repr(C, align(16))`, exactly 256 bytes and
16-byte aligned. All addresses are raw device `u64` values. Fields are in this
order:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 4 | `abi_version = 1` |
| 4 | 4 | `struct_bytes = 256` |
| 8 | 4 | `flags = 0` |
| 12 | 4 | `rows`, `1..8` |
| 16 | 4 | `assignments`, `1..rows*8` |
| 20 | 4 | `active_expert_count`, `1..min(assignments,256)` |
| 24 | 4 | `hidden = 6144` |
| 28 | 4 | `local_intermediate = 512` |
| 32 | 4 | `experts = 256` |
| 36 | 4 | `top_k = 8` |
| 40 | 4 | `bits = 3` |
| 44 | 4 | `reserved0 = 0` |
| 48 | 8 | `input_f16` `[rows,6144]` |
| 56 | 8 | `route_experts_u16` |
| 64 | 8 | `route_tokens_u32` |
| 72 | 8 | `route_slots_u8` |
| 80 | 8 | `expert_offsets_u32` |
| 88 | 8 | `active_experts_u16` |
| 96 | 8 | `gate_trellis_table_u64` |
| 104 | 8 | `gate_suh_table_u64` |
| 112 | 8 | `gate_svh_table_u64` |
| 120 | 8 | `up_trellis_table_u64` |
| 128 | 8 | `up_suh_table_u64` |
| 136 | 8 | `up_svh_table_u64` |
| 144 | 8 | `rotated_input_f16` |
| 152 | 8 | `projected_f16` |
| 160 | 8 | `gate_output_f16` `[assignments,512]` |
| 168 | 8 | `up_output_f16` `[assignments,512]` |
| 176 | 8 | `validation_error_u32` |
| 184 | 8 | `workspace_bytes` |
| 192 | 8 | `sequence` |
| 200 | 32 | inline `route_digest_u64[4]` |
| 232 | 24 | `reserved[3] = 0` |

The inline digest is the common filtered-route digest already bound into the
rank command and graph key. Rust validates it against the adopted argument
upload receipt before entering the FFI. The launcher validates descriptor
scalar fields, alignment, checked workspace arithmetic, current device 12.0,
and pointer presence before enqueue. Each CUDA kernel bounds-checks the route
element, expert offset, active expert, and resident pointer it consumes before
indexing and sets a launch-local error word on failure. The rank executor
treats any nonzero validation word as a fatal step error before consuming
output.

The ABI string is:

```text
glmaxx.sm120.exl3.grouped_paired_gate_up.v1
```

## Workspace and layout

The only scratch planes are:

```text
rotated_input_f16[2, assignments, 6144]
projected_f16[2, assignments, 512]
```

Projection index zero is gate and one is up. Assignment is the next-major
dimension in both planes. Gate and up output buffers are caller-owned target
layer slots and are not counted as scratch.

Checked workspace bytes are exactly:

```text
2 * assignments * (6144 + 512) * 2
= 26,624 * assignments
```

That is 212,992 bytes for the all-K3 M1 route and 1,703,936 bytes at the
M8/top-8 ceiling. The allocation is part of the rank HBM ledger and graph
profile; it cannot be borrowed from KV or created after health publication.

## Three-kernel launch

One grouped entry point enqueues exactly three kernels on the caller stream:

1. grouped gate/up input rotations;
2. paired staged gate/up projections; and
3. grouped gate/up output rotations.

There is one host launch of each kernel, independent of active expert count.
There is no host loop over experts or projections.

### Input rotation

The rotation grid is:

```text
grid = (assignments, 2, 48)
block = 128
```

`blockIdx.x` selects a compacted assignment, `blockIdx.y` selects gate/up,
and `blockIdx.z` selects one 128-value H128 block. The route token selects the
input row; the route expert selects the corresponding immutable SUH pointer.
The retained H128 sign order, FP16 product rounding, `0x3db504f3`
normalization constant, explicit RN operations, validation, and FP16 store are
unchanged.

### Paired staged projection

The projection grid is:

```text
grid = (32, active_expert_count, 1)
block = 256
static shared = 1,536 bytes
```

`blockIdx.x` owns one 16-column tile. `blockIdx.y` selects an entry in the
strict active-expert list. The expert offsets give its contiguous assignment
range. Since each token selects an expert at most once and `rows <= 8`, an
expert owns at most eight assignments.

Warp `w` owns assignment `expert_offsets[expert] + w`. Lanes `0..15` own its
16 gate columns and lanes `16..31` own the corresponding 16 up columns. All
warps reach both barriers even when their assignment is inactive.

For each eight-K-tile stage, the CTA loads two independent 192-word stages:

```text
linear = threadIdx.x; linear < 384; linear += 256
projection = linear / 192              # 0 gate, 1 up
within = linear % 192
stage_tile = within / 24
word = within % 24
```

Threads `0..127` load two U32 words and threads `128..255` load one. Every
gate/up source word is loaded exactly once per active expert and N tile. The
two 768-byte stages never alias.

Each active lane applies the predecessor's exact cyclic-window decoder and
the exact recurrence:

```text
for k_tile in 0..384:
  for k_local in 0..16:
    accumulator = __fadd_rn(
      accumulator,
      __fmul_rn(rotated[projection,assignment,k],
                decoded_weight(projection,k,n)))
```

Gate and up therefore retain independent ascending-K FP32 accumulators. Each
is rounded once to FP16 in its own projected plane. No shuffle, cross-lane
reduction, FMA contraction, split-K partial, or gate/up arithmetic fusion is
allowed.

For the all-K3 M1 route, this is 256 CTAs instead of 32 CTAs per isolated
projection. It computes all sixteen gate/up expert projections in one grid.

### Output rotation

The output grid is:

```text
grid = (assignments, 2, 4)
block = 128
static shared = 512 bytes
```

It applies the retained H128 order and the expert/projection-specific SVH
pointer, then writes the separate gate or up FP16 assignment plane. SwiGLU is
not part of this entry point.

## Exactness and traffic gates

For each assignment and projection, scalar, isolated staged, and grouped
paired paths must match bitwise at all three planes: rotated input, projected
FP16, and final FP16 output. The grouped path retains the compacted assignment
order exactly. A later FP32 SwiGLU control must receive the same gate/up FP16
bits from both paths.

Each active expert reads exactly two trellis planes:

```text
2 * 1,179,648 = 2,359,296 logical source bytes
```

The all-K3 M1 route reads 18,874,368 logical trellis bytes for eight experts.
This is a kernel address-count claim, not observed DRAM traffic. Runtime weight
repack and persistent reconstructed-weight bytes remain zero.

## Gate sequence

1. adversarial review of this route, ABI, stage mapping, arithmetic, and
   workspace contract;
2. an independent Rust CPU proof that exhausts rows `1..8`, all legal K=3
   expert occupancy patterns, route mutations, and the actual gate and up
   source planes;
3. C/C++/Rust ABI size and alignment checks plus fail-closed descriptor tests;
4. clean `sm_120f` compile, SASS/resource capture, and no-launch ABI probe;
5. synthetic M1/M2/M4/M8 bitwise comparison against both scalar and isolated
   staged controls;
6. hash-gated real checkpoint gate/up comparison for all four ranks;
7. isolated versus grouped matched timing and NCU replay, including M1
   all-distinct and maximally shared expert patterns; and
8. target-layer integration only after the target-program gate is accepted.

The timing gate must retain individual launch samples, output hashes, active
expert counts, route tables, exact source bytes, clocks, power, and profiler
reports. The grouped candidate survives only if it is faster than executing
the same gate/up assignments through the isolated staged control.

## Nonclaims

This design is not a CPU proof, implementation, SM120 result, real-checkpoint
result, mixed-K route, down projection, fused SwiGLU, routed MoE result, TP4
layer replay, checkpoint smoke, quality result, or serving benchmark. It does
not relax any target-layer precision boundary or authorize a full-checkpoint
conversion.
