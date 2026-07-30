# NVFP4 fused routed-MoE kernel v1

Date: 2026-07-30

Status: design candidate; adversarial review required before format or kernel
implementation

GPU evidence: none

## Scope

This contract defines the production successor to the existing SM120 NVFP4
CUDA-core and CUTLASS development controls for GLM-5.2 routed FC1/SwiGLU/FC2.
It supports only the fixed TP4 rank shapes:

```text
FC1 A             [assignments, 6,144]
FC1 gate/up W     [1,024, 6,144]
FC1 output        [assignments, 512]
FC2 A             [assignments, 512]
FC2 W             [6,144, 512]
FC2 slot partial  [rows, 8, 6,144]
```

It freezes:

- one offline-only row-interleaved FC1 NVFP4 layout;
- deterministic GPU route compaction;
- expert-local activation quantization;
- grouped SM120 block-scaled MMA;
- fused paired gate/up scaling and SwiGLU;
- FC2 scale/route epilogue and slot-ordered reduction;
- decode/prefill variant-selection constraints; and
- correctness/performance evidence required before replacing controls.

It does not implement the layout or kernel, authorize cn4, or claim speed.

## Why the current control cannot be the final FC1

Physical layout `0x1201` stores:

```text
[gate rows 0..511][up rows 0..511]
```

A normal N-tiled GEMM epilogue sees one local output tile. Matching gate/up
columns are 512 columns apart, so an epilogue tile cannot form
`SiLU(gate[c]) * up[c]` without:

- materializing a global 1,024-column boundary;
- synchronizing unrelated CTAs; or
- building a custom mainloop that computes distant N tiles together.

The existing CUTLASS control correctly materializes a BF16 development
boundary. Calling that final would add avoidable traffic and is one likely
reason a nominal 4-bit route can lose to a 3-bit dynamic kernel.

Version one chooses an offline physical row permutation so each gate/up pair
is adjacent. This changes neither quantized values nor scale codes and adds no
bytes.

## FC1 physical layout `0x1202`

Logical output row `n` maps to physical row:

```text
gate c -> 2*c
up   c -> 2*c + 1
for c in 0..512
```

Within each physical row, K remains contiguous and even/odd values remain
low/high nibbles. SFB uses the existing SM120 block-16 swizzle with the
physical row as `n`:

```text
offset = 512 * ((n / 128) * (K / 64) + g / 4)
       + 16 * (n % 32)
       + 4 * ((n % 128) / 32)
       + g % 4
```

Value, SFB, global-scale, metadata, and padding byte counts are identical to
`0x1201`. Global scales remain one per expert shard. The permutation is
applied during offline conversion and covered by tensor/container hashes.
Runtime transpose, repack, dequantization, or layout conversion is forbidden.

The converter can transform already-quantized `0x1201` bytes without
requantization by permuting complete value rows and their SFB scale rows.
The inverse must reconstruct exactly the original bytes. The production
profile accepts only `0x1202` for FC1; `0x1201` remains a laboratory matched
control. FC2 remains layout `0x1201`.

The tensor semantic discriminator and weight-policy digest include the new
layout ID. Four ranks must agree before loading. Unknown, mixed, or resigned
layout claims fail before device upload.

## Deterministic route compaction

Input routing is token-major, slot-major:

```text
expert[rows][8]
weight[rows][8]
```

Routes are validated for expert range, finite weight, slot uniqueness, and
the frozen top-8 contract before compaction.

Compaction uses three fixed kernels:

1. one CTA per expert scans `(token,slot)` in increasing order and writes its
   exact count;
2. one CTA computes the 257-entry exclusive expert offset array and the
   ascending active-expert list; and
3. one CTA per expert repeats the same scan and writes token, slot, expert,
   and route weight into its assigned range.

Integer counts and disjoint writes require no floating atomic. Assignment
order is expert, then token, then slot. Empty experts have equal begin/end.
All ranks receive identical route input and must produce identical offsets,
active experts, and compaction digest. A mismatch aborts the step; there is no
rank-local CPU fallback.

This `O(256 * rows * 8)` scan is intentional for the bounded GLM row buckets.
Its inclusive cost is measured. A future stable radix route may replace it
only after matched evidence.

## Expert-local activation quantization

Each compacted assignment reads the original BF16 token row. Per row:

1. compute finite absolute maximum in a fixed reduction tree;
2. choose global scale `1.0` for all-zero, otherwise
   `amax / (448 * 6)`;
3. for each K-consecutive block of 16, encode the saturated-finite E4M3 block
   scale with round-to-nearest-even;
4. encode sixteen E2M1 values with round-to-nearest-even; and
5. write values row-major and SFA into that expert's independently
   128-row-padded SM120 slab.

Every padding value/scale byte is zero before MMA. Expert SFA offsets are
derived from the stable expert counts with checked arithmetic. FC1 quantizes
K=6,144 once and reuses the row for paired gate/up. FC2 independently
quantizes K=512 after BF16 SwiGLU.

Hardware conversion must match the Rust oracle at every boundary. Fast-math,
nonfinite inputs, negative-zero scale codes, and noncanonical zero blocks are
forbidden.

## FC1 grouped block-scaled MMA

One grouped problem exists per active expert:

```text
M = expert assignment count
N = 1,024 physical interleaved rows
K = 6,144
```

Operand A is expert-local packed activation/SFA. Operand B points directly at
the expert's immutable `0x1202` value/SFB bytes. Both use E2M1 with UE4M3
block-16 scales and FP32 accumulation through native SM120
`OpClassBlockScaledTensorOp`.

The custom epilogue consumes adjacent accumulator columns `(2*c,2*c+1)`,
multiplies both by:

```text
activation_global[assignment] * weight_global[expert]
```

then writes:

```text
BF16_RNE(SiLU_RN(gate) * up)
```

to `[assignment, c]`. It writes no gate/up global intermediate. SiLU and final
BF16 rounding must match the accepted reference definition; approximate
activation instructions require separate quality evidence and are initially
disabled.

The epilogue must prove that every pair is owned by one thread/warp fragment.
No CTA reads another CTA's output and no cross-CTA barrier is assumed.

## FC2 grouped block-scaled MMA

One grouped problem exists per active expert:

```text
M = expert assignment count
N = 6,144
K = 512
```

The epilogue multiplies each FP32 accumulator by the activation and expert
global scales and by that assignment's finite route weight. It writes FP32 to
the unique canonical slot:

```text
slot_partial[token][slot][hidden]
```

The compacted assignment metadata provides token and slot. Validation proved
that no two assignments share a destination, so no floating atomic is used.

A separate reducer visits slots `0..7` in that exact order for every
`(token,hidden)` and writes the rank partial in the target program's frozen
precision. Empty slots contribute positive zero. The TP4 reduction remains a
single later collective boundary and is not fused into this rank-local
kernel.

The FP32 slot plane is workspace, not retained KV or model state. Its exact
maximum is `rows * 8 * 6,144 * 4`; graph/profile admission must budget it.
Reducing its precision is a quality-policy change, not an optimization.

## Variant selection

The immutable graph profile selects variants from rank-common data only:

```text
decode:  rows 1,2,4,8,16,32,64,128
verify:  admitted fixed buckets through 448 rows
prefill: accepted chunk buckets through the reviewed graph maximum
```

Route counts may determine grouped problem shapes but cannot choose a
different algorithm per rank. Decode and prefill may use different measured
tile, stage, cluster, and persistent-CTA settings. Every setting is a
compile-time/profile ID included in graph and result hashes.

Candidate tuning may sweep only SM120:

- threadblock and epilogue tile;
- pipeline stages;
- warp-specialized schedule;
- cluster shape;
- register cap;
- persistent CTA count; and
- grouped scheduler mode.

The winner must pass identical numerical/quality gates before timing. An
unmeasured fallback is forbidden.

## Workspace and allocation

All storage is preallocated before health:

- route count/offset/active-expert arrays;
- compacted token/slot/expert/weight arrays;
- FC1 and FC2 packed activations/SFA/global scales;
- CUTLASS grouped descriptors and workspace;
- BF16 FC1 output;
- FP32 FC2 slot partials;
- rank partial output; and
- validation/status words.

No `cudaMalloc`, host allocation, descriptor growth, or CUTLASS workspace
allocation occurs per step. Workspace aliases are allowed only across
nonoverlapping graph lifetimes proven in the system memory plan. Metadata
initialization cannot overwrite an input still read by another CTA.

## Correctness controls

The following remain built and runnable:

1. CUDA-core direct-layout FC1/FC2;
2. dense CUTLASS `0x1201` control;
3. grouped CUTLASS `0x1201` control with named BF16 boundary;
4. Rust packed arithmetic/reference; and
5. optional protected BF16/FP8 controls.

The production candidate is compared against controls with identical source
BF16 activations, routes, quantized codes, global scales, output membership,
and batch shape. Layout permutation is reversed before byte-level control
comparison where necessary.

## Required CPU proof after design acceptance

Before CUDA implementation:

1. exhaustively prove the 1,024-row `0x1201`↔`0x1202` permutation and inverse;
2. prove value/SFB/global-scale bytes are only permuted, never requantized;
3. mutation-test gate/up swaps, pair stride, scale-row mapping, nibble order,
   padding, and layout discriminator;
4. reproduce paired FC1 output from both layouts for actual K/N;
5. prove route counts, prefix offsets, active experts, and assignments for all
   row/routing fixtures and adversarial empty/skewed cases;
6. prove expert-local SFA capacities and zero padding at every 128-row
   boundary;
7. model adjacent-fragment FC1 epilogue ownership with no cross-tile read;
8. prove FC2 unique `(token,slot)` destinations and fixed-order reducer;
9. rederive workspace at every row bucket with overflow/one-byte-short
   failures; and
10. retain per-element outputs and fail before a PASS record on any mismatch.

## Required SM120 gates

After CPU proof and renewed authorization:

1. compile only `sm_120f` at the pinned CUDA/CUTLASS revisions;
2. prove SFA/SFB offsets against CUTLASS for both layouts and all real shapes;
3. disassemble and count expected block-scaled MMA instructions;
4. run every decode/verify/prefill row and route fixture, including zero,
   single, all, and maximally skewed active experts;
5. compare FC1 pre/post-SwiGLU, FC2 pre/post-route weight, slot partials,
   reducer output, and rank partials;
6. retain every failing element and per-position downstream values;
7. prove graph eager equivalence, repeated bit stability, async error
   handling, and zero allocation drift;
8. separate compaction, quantization, MMA, epilogue/reducer, launch, graph,
   and inclusive time;
9. profile bytes, occupancy, registers, shared memory, tensor-core issue,
   achieved bandwidth, and PCIe/collective exclusion; and
10. beat the matched `0x1201` control without changing precision, routes,
    batch, context, graph, or quality posture.

Only then may the immutable serving policy prefer this NVFP4 kernel. A
standalone all-NVFP4 checkpoint fit claim remains separately prohibited by
the weight budget.

## Claim boundary

This design does not pass C06. It introduces a format revision that requires
adversarial review, CPU pack/inverse proof, container/manifest amendment,
quality revalidation, native implementation, and SM120 evidence. Current
`0x1201` checkpoints remain laboratory inputs until explicitly converted
offline; they are not silently accepted as `0x1202`.
