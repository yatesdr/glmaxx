# NVFP4 fused routed-MoE kernel v1 r2

Date: 2026-07-30

Status: corrective design candidate; adversarial review required before CPU
or CUDA implementation

GPU evidence: none

## Purpose

This amendment supersedes the implementation authority of
`docs/nvfp4-fused-routed-moe-v1.md`. The v1 direction remains sound:
offline-interleave FC1 gate/up rows, compact routes deterministically,
quantize activations in expert order, use grouped SM120 block-scaled MMA,
fuse the paired FC1 epilogue, and reduce FC2 slots without floating atomics.

An independent source/ABI re-derivation found seven contract gaps that must
be corrected before the CPU proof:

1. v1 says activation values and SFA both occupy independently padded expert
   slabs, while the current grouped CUTLASS interface and workspace arithmetic
   use compact contiguous value rows and only expert-padded SFA.
2. v1 requires a frozen top-8 route but validates only slot uniqueness and a
   finite weight. That admits missing slots, duplicate experts, and negative
   weights that the pinned GLM router cannot produce.
3. v1 names exactly three compaction kernels but does not assign ownership for
   the required per-token validation.
4. v1 applies route weight in the FC2 epilogue and then adds rounded slot
   partials. The retained reference/control instead performs slot-ordered
   `fmaf(route_weight, projection, accumulator)`, so v1 changes rounding.
5. v1 does not say which metadata layout fields change to `0x1202`, permitting
   values and scales to claim inconsistent logical row mappings.
6. v1 leaves the prefill maximum and corresponding dominant FP32 slot plane
   open-ended even though the reviewed graph contract fixes 3,072 rows.
7. v1 lists preallocated buffers but does not forbid the development
   descriptors' scratch-pointer aliasing from becoming the production ABI.

R2 corrects only those gaps and makes the numerical/workspace boundary exact.
All v1 requirements not explicitly replaced here remain in force. In
particular, r2 replaces v1's production per-layer compaction-digest agreement
with the upstream dense-route receipt and immutable buffer-generation binding
below. A compacted-output digest remains a qualification artifact, not a
production collective or kernel ABI field.

## Nonclaims and authority

This amendment does not:

- implement layout `0x1202`, a converter, CPU proof, descriptor, or kernel;
- change any current `0x1201` checkpoint or accept it as production FC1;
- accept the current development CUDA-core or CUTLASS controls as production;
- authorize cn4, a container, a checkpoint read, or a GPU launch;
- prove quality, capacity, latency, throughput, or full-model fit; or
- pass C06.

The old v1 handoff is superseded. Only an unqualified review of this amendment
may open the separately reviewed CPU proof.

## Fixed production geometry

The fused production path accepts only:

```text
rows R                         1..3,072
top_k                         8
assignments A                 exactly 8 * R
experts                       256
FC1 K / physical N            6,144 / 1,024
FC1 logical output            512
FC2 K / N                     512 / 6,144
```

The graph profile further restricts rows:

| Mode | Legal row buckets |
|---|---|
| decode | `1,2,4,8,16,32,64,128` |
| verify | reviewed fixed buckets with `R <= 448` |
| prefill | reviewed fixed buckets with `R <= 3,072` |

There is no generic 65,536-row fused path. The maximum production assignment
count is therefore 24,576 and fits `u32`; production descriptors reject any
other `A`. Laboratory control entry points may retain partial-route fixtures,
but they use a different ABI/profile ID and cannot be selected by a serving
graph.

## Corrected `0x1202` format identity

Only FC1 combined gate/up tensors use `0x1202`. FC2 remains `0x1201`.

For FC1 logical row `l` and physical row `p`:

```text
l = c         (gate)  -> p = 2*c
l = 512 + c   (up)    -> p = 2*c + 1
c in 0..512
```

The inverse is:

```text
p even -> l = p/2
p odd  -> l = 512 + p/2
```

Both metadata layout fields are `0x1202`:

```text
value_layout_id = 0x1202
scale_layout_id = 0x1202
```

The value ID means contiguous physical rows with the logical mapping above.
The scale ID means the existing SM120 block-16 swizzle is evaluated using the
same physical row `p`. Setting only one field to `0x1202`, using
`0x1202` on FC2, or pairing either ID with a split gate/up projection
discriminator is invalid.

The layout-source digest changes and binds:

- both forward and inverse row maps;
- the value-row copy rule;
- the logical-SFB-row copy rule;
- the unchanged nibble order and block-16 grouping; and
- the fixed FC1 dimensions.

The quantization-policy digest does not change. `global_amax`,
`global_scale`, codec, value/scale byte counts, codes, and padding semantics
remain identical.

For one expert:

```text
value bytes = 1,024 * 6,144 / 2  = 3,145,728
SFB bytes   = 1,024 * 6,144 / 16 =   393,216
```

Value row `l` is copied as one contiguous 3,072-byte row to row `p`. An SFB
"row" is logical, not a contiguous physical range. For every
`g in 0..384`, conversion copies:

```text
old_scale[sfb_offset(l, g)] -> new_scale[sfb_offset(p, g)]
```

using the frozen swizzle. The inverse uses the inverse row map. Plane
permutation is byte-exact; canonical metadata is re-encoded with the new
layout IDs and layout-source digest, so the complete serialized tensor is not
falsely described as a pure byte permutation.

The converter must independently prove the 1,024-row bijection and all
393,216 SFB destinations before publication. A transform of values without
scales, scales without values, or metadata without both planes fails.

## Exact production route contract

The upstream protected router supplies dense token-major arrays:

```text
expert[R][8] : u16
weight[R][8] : f32
```

The slot is its array index. For every token:

- all eight slots `0..7` are present exactly once;
- every expert is in `0..255`;
- the eight experts are distinct;
- every weight is finite with a clear sign bit; and
- the route receipt matches the target program's canonical route digest.

The fused path does not accept a sparse route list. A route weight of positive
or negative infinity, NaN, any finite negative value, or negative zero is
invalid. Positive zero is the only accepted zero encoding; the pinned router
normally produces strictly positive weights.

The canonical receipt is:

```text
SHA256(
  "glmaxx.glm52-dense-route.v1\0" ||
  step_id:u64_le ||
  layer:u16_le ||
  route_buffer_generation:u64_le ||
  rows:u32_le ||
  top_k:u8 (=8) ||
  for token in 0..rows:
    for slot in 0..8:
      token:u32_le || slot:u8 || expert:u16_le || weight_bits:u32_le
)
```

There is no structure padding, float text conversion, map iteration, or
alternate negative-zero spelling in those bytes.

The upstream target phase already compares the canonical route receipt across
all four ranks before expert execution. This kernel consumes that receipt and
binds the immutable route-buffer generation that it authenticates. It does
not add a second per-layer hash collective. The CPU and SM120 qualification
proofs independently digest the compacted output and show all four ranks
produce identical bytes; production correctness follows from the already
compared input receipt, immutable buffer generation, fixed algorithm, and
status checks. There is no rank-local fallback.

## Four deterministic route kernels

R2 uses four, not three, fixed kernels:

1. **validate** — one CTA per token reads its fixed eight entries and validates
   expert range/distinctness and weight finite/clear-sign-bit rules;
2. **count** — one CTA per expert scans `(token,slot)` in increasing order and
   writes the exact count;
3. **prefix** — one CTA produces the 257 ascending exclusive offsets, the
   ascending active-expert list, active count, and padded-SFA row offsets; and
4. **scatter** — one CTA per expert repeats the same increasing scan and
   writes compacted token, slot, expert, and weight arrays into its disjoint
   range.

The status word is cleared before validate. Every later kernel reads it after
the prior stream-ordered kernel and returns without writing data-dependent
destinations if it is nonzero. Validate reads only the fixed dense
`R * 8` input, so malformed experts cannot cause an out-of-bounds access
before the status becomes visible.

Counts and offsets are `u32`. The prefix kernel verifies:

```text
offset[0]   = 0
offset[256] = A
sum(count)  = A
active_count in 1..min(256,A)
```

Compacted order is expert, then token, then slot. The scatter kernel proves
its final cursor equals `offset[e+1]` for every expert. No integer or floating
atomic determines order. The only atomics allowed in validation set status
bits; their order cannot affect any accepted output.

## Correct activation storage

For either projection, packed E2M1 activation values are compact
assignment-major rows:

```text
FC1 values[A][6,144/2]
FC2 values[A][512/2]
```

They are not padded independently per expert. Since compaction is
expert-major, active expert `e` owns the contiguous value interval beginning
at `offset[e]`. Grouped operand-A pointers are:

```text
FC1 value_base + offset[e] * 6,144/2
FC2 value_base + offset[e] *   512/2
```

The grouped GEMM uses exact `M = count[e]` and cannot address the next expert
or graph-bucket tail. Bytes beyond actual `A` are dead capacity, are not
hashed as logical activation data, and need not be cleared per step. The CPU
proof and SM120 qualification both verify that grouped descriptors and
generated memory accesses are bounded by exact `M`, not padded value capacity.

Only SFA uses independent 128-row expert padding. Define:

```text
padded(M) = ceil(M / 128) * 128
P         = sum_e padded(count[e])
```

Then:

```text
FC1 SFA bytes = P * 6,144/16 = P * 384
FC2 SFA bytes = P *   512/16 = P *  32
```

Each expert's complete SFA padding is zero before grouped MMA. Prefix stores
one shared 257-entry padded-row offset table; FC1/FC2 multiply that row offset
by 384/32 respectively. For a graph bucket with maximum assignments
`A_bucket`:

```text
P_max = A_bucket + 127 * min(A_bucket, 256)
```

and both SFA allocations use that fail-closed maximum. Values use
`A_bucket * K/2`, not `P_max * K/2`.

## Exact activation quantization arithmetic

Each input is BF16 widened exactly to binary32. A nonfinite BF16 input sets
status before `abs`, maximum reduction, scale encoding, or division.

For one assignment, one fixed 256-thread CTA computes:

```text
amax = fixed-tree max(abs(x[k]))
global = amax == +0 ? 1.0f : div_rn(amax, 2688.0f)
group_amax = fixed-order max(abs(x[g*16 + lane]))
unencoded = div_rn(div_rn(group_amax, 6.0f), global)
sfa = E4M3_RNE_SATFINITE(unencoded), or +0 for an all-zero group
decoded = mul_rn(E4M3_DECODE(sfa), global)
value = E2M1_RNE_SATFINITE(div_rn(x, decoded)), or +0 when sfa is zero
```

The two divisions in `unencoded` cannot be reassociated. Fast math, FTZ that
changes a finite BF16 case, FMA contraction across the stated boundaries,
negative-zero scale codes, and noncanonical codes behind a zero SFA are
forbidden. FC1 quantizes K=6,144 once and shares the result across the paired
gate/up columns. FC2 re-quantizes the BF16 FC1 output at K=512.

The CPU proof compares every code, SFA byte, global-scale bit pattern, and
expert padding byte for actual row buckets and adversarial finite BF16
classes. Numerical kernel comparison is separate from byte-exact
quantization comparison.

## Corrected FC1 epilogue

The block-scaled MMA accumulates physical columns `2*c` and `2*c+1` in FP32.
The epilogue proves that one local fragment owner sees both columns; every
selected N tile begins on an even column and has an even width.

For assignment `a` owned by expert `e`:

```text
combined = mul_rn(activation_global[a], weight_global[e])
gate     = mul_rn(accum[a,2*c],   combined)
up       = mul_rn(accum[a,2*c+1], combined)
output   = BF16_RNE(mul_rn(silu_f32(gate), up))
```

`silu_f32` is the ordinary CUDA `expf` path compiled without
`--use_fast_math`; `__expf` and approximate activation instructions are
forbidden in the initial candidate. Because host and device transcendental
implementations are not promised bit-identical, the CPU proof preserves FP32
gate/up values and classifies BF16 equality separately from a pinned
FP32/ULP tolerance at rounding-adjacent cases. Downstream per-position
quality remains mandatory.

No gate/up global plane exists. No CTA reads another CTA's accumulator or
output, and no device-wide barrier is assumed.

## Corrected FC2 epilogue and reducer

The grouped FC2 MMA produces one FP32 down projection per compacted
assignment and hidden column. Its epilogue applies only the activation and
weight global scales:

```text
combined = mul_rn(fc2_activation_global[a], weight_global[e])
projection[token[a], slot[a], h] =
    mul_rn(accum[a,h], combined)
```

The FP32 plane is named `slot_projection`, not weighted slot partial. Unique
validated `(token,slot)` ownership makes all writes disjoint.

The separate reducer alone applies route weight:

```text
acc = +0.0f
for slot in 0..8:
    acc = fma_rn(weight[token,slot],
                 slot_projection[token,slot,h],
                 acc)
routed_partial[token,h] = acc
```

This preserves the retained reference/control's route-weight-after-projection
and slot-ordered FMA boundary. Multiplying route weight in the epilogue and
then adding a rounded product is forbidden because it changes results at
rounding boundaries.

All production tokens have eight validated destinations, so every live slot
is overwritten before reduction. A nonzero status prevents publication of
the routed partial. Laboratory sparse-route controls must clear missing slots
to positive zero and cannot reuse the production graph/profile ID.

`routed_partial` is FP32. It is combined with the separately produced shared
expert FP32 partial in the target program before the one allowed TP4
reduction. Reducing the slot plane or routed partial to BF16/FP16 is a
precision-policy change.

## Exact capacity terms

For a graph row bucket `R`, production has:

```text
A = 8 * R
E = min(A, 256)
P_max = A + 127 * E
```

The independently sized data planes are:

```text
compacted route arrays          A * (4 + 1 + 2 + 4)
FC1 activation values           A * 6,144/2
FC1 SFA                         P_max * 384
FC1 activation globals          A * 4
FC1 BF16 output / FC2 input      A * 512 * 2
FC2 activation values           A * 512/2
FC2 SFA                         P_max * 32
FC2 activation globals          A * 4
FC2 slot_projection FP32        R * 8 * 6,144 * 4
routed_partial FP32             R * 6,144 * 4
```

Route counts, assignment offsets, active experts, active count, padded-row
offsets, and status are separately charged fixed metadata:

```text
align256(256 * sizeof(u32))  route counts
align256(257 * sizeof(u32))  assignment offsets
align256(256 * sizeof(u16))  active experts
align256(257 * sizeof(u32))  padded-row offsets
align256(16)                 active count plus status words
```

Each compacted route plane is independently `align256` charged; it is not
legal to charge only the 11-byte logical sum per assignment. The 32-byte
upstream route receipt is owned by the target program and referenced by
generation rather than hidden in this workspace. CUTLASS grouped descriptors
and opaque workspace are separate build-generated terms; neither may hide
inside an output plane or development pointer.

Required maxima are:

| R | A | `P_max*384` | `P_max*32` | slot projection | routed partial |
|---:|---:|---:|---:|---:|---:|
| 1 | 8 | 393,216 | 32,768 | 196,608 | 24,576 |
| 32 | 256 | 12,582,912 | 1,048,576 | 6,291,456 | 786,432 |
| 128 | 1,024 | 12,877,824 | 1,073,152 | 25,165,824 | 3,145,728 |
| 448 | 3,584 | 13,860,864 | 1,155,072 | 88,080,384 | 11,010,048 |
| 3,072 | 24,576 | 21,921,792 | 1,826,816 | 603,979,776 | 75,497,472 |

The CPU proof covers every configured row bucket, `A=8R`, all feasible active
expert counts, the exact route-derived `P`, worst-case `P_max`, arithmetic
overflow, and one-byte-short allocations.

The static system memory plan uses graph-bucket capacities, not actual
per-step counts. Aliasing is allowed only when a separately reviewed
liveness table proves:

- the producer has completed on the same stream/event dependency;
- no graph node or asynchronous error path can still read the old plane;
- the aliased alignment is at least the maximum of both uses; and
- the maximum byte charge, not the smaller logical length, is reserved.

## Production ABI boundary

The current development descriptors reuse:

- `compacted_input_bf16` as grouped metadata/CUTLASS scratch; and
- `token_output_f32` as FC2 SFA offsets/CUTLASS scratch.

Those aliases are forbidden in the fused production ABI.

The later CPU ABI proof must define dedicated, nonoverlapping pointers and
capacities for:

- dense route input and upstream receipt;
- counts, assignment offsets, active experts/count, padded-row offsets,
  compacted route arrays, and status;
- both weight planes, metadata/layout IDs, and global scales;
- FC1/FC2 activation value, SFA, and global planes;
- FC1 output, FC2 slot projection, and routed partial;
- grouped problem descriptors; and
- FC1/FC2 CUTLASS opaque workspace.

Every pointer has an explicit alignment and byte capacity. Rust derives all
offsets from a checked graph-bucket plan before health. The native library
exports build-generated required metadata/workspace tables keyed by exact
profile ID; Rust compares them with a checked-in manifest and rejects a
linked-library mismatch before allocation or capture.

No field can serve two simultaneous semantic roles. Per-step descriptor
construction changes only values in preallocated pinned/device records and
does not allocate or grow a vector.

## Revised CPU proof gate

After this design is accepted, one CPU proof must:

1. prove both `0x1202` metadata fields and the projection discriminator;
2. exhaust all 1,024 forward/inverse value rows and 393,216 SFB locations;
3. round-trip value/SFB planes and independently re-encode canonical
   `0x1201`/`0x1202` metadata;
4. mutation-test one-plane-only conversion and every discriminator/ID lie;
5. reject missing slots, duplicate experts, out-of-range experts, nonfinite
   or negative weights, and noncanonical zero;
6. prove the four route stages, exact upstream receipt, immutable
   buffer-generation binding, and qualification-only compacted-output digest;
7. prove compact values versus independently padded SFA for all active-expert
   distributions and 128-row boundaries;
8. match every activation quantization bit boundary under the stated
   arithmetic;
9. model local paired-fragment FC1 ownership and forbidden odd/torn tiles;
10. distinguish FC2 epilogue-weighted rounding from the accepted
    slot-ordered FMA and accept only the latter;
11. rederive every table/formula/workspace maximum and one-byte-short
    failure;
12. prove production row/assignment ceilings and reject the development
    65,536-row posture;
13. prove every dedicated ABI plane is nonoverlapping and sufficiently
    aligned; and
14. retain per-element/code/offset evidence and refuse PASS on any mismatch.

CPU acceptance opens only container/manifest and thin-ABI implementation.
CUDA remains behind renewed authorization and the existing SM120 gates.

## Revised SM120 gate

The v1 SM120 matrix remains required and additionally proves:

- both linked layout IDs and the combined gate/up discriminator;
- route validation status gating before any data-dependent write;
- exact `A=8R` and 3,072-row rejection boundary;
- compact-value and padded-SFA address ranges for every active expert;
- no read of dead graph-bucket value capacity;
- local paired-fragment ownership in generated code;
- unweighted FP32 slot projection and slot-ordered route-weight FMA;
- build-generated grouped descriptor/workspace agreement with Rust;
- one-byte-short rejection for every independent plane; and
- zero allocation and pointer-role drift across eager, capture, replay,
  failure, and cancellation paths.

Only after those checks and matched quality/performance evidence may a serving
profile select the fused path.

## Exit criteria

R2 is ready for the CPU proof only if adversarial review confirms that:

- layout values, scales, metadata, and projection identity cannot diverge;
- the production router accepts exactly the pinned dense top-8 semantics;
- validation becomes visible before any unsafe route-derived address;
- packed values and expert-padded SFA have unambiguous, correctly budgeted
  layouts;
- FC1 and FC2 rounding boundaries match their accepted controls;
- the dominant 3,072-row slot workspace is explicit;
- development scratch aliases cannot enter production; and
- no CPU, CUDA, quality, fit, or speed claim is implied.
