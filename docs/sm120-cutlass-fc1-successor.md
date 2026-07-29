# SM120 CUTLASS FC1 successor plan

Date: 2026-07-29

Status: GLMAXX-owned packed-byte dense and expert-grouped controls compile and
link without a device launch. Expert-local SFA allocation, grouped metadata,
the native launcher, and the Rust correctness runner are implemented. The
retained CUDA-core control and both materialized gate/up controls must pass
the reviewed SM120 correctness gates before the fused candidate can replace
them.

## Pinned implementation anchors

CUTLASS commit `e05f953a5b3d38adc240df2ff928e0421c2abba3` contains two relevant
SM120 examples:

```text
examples/79_blackwell_geforce_gemm/79a_blackwell_geforce_nvfp4_bf16_gemm.cu
examples/79_blackwell_geforce_gemm/79d_blackwell_geforce_nvfp4_grouped_gemm.cu
```

The dense example establishes the exact operand types and collective builder:

- `nv_float4_t<float_e2m1_t>` A and B;
- row-major A and column-major B;
- 32-element operand alignment;
- FP32 accumulator;
- `arch::Sm120`;
- `OpClassBlockScaledTensorOp`;
- a 128×128×128 thread-block tile and fixed 1×1×1 cluster;
- SFA/SFB layouts supplied by the collective mainloop.

The grouped example establishes pointer-array problem shapes and both
cooperative and ping-pong schedule families. It is a useful construction
reference, not a drop-in GLM operator.

## Direct-byte mapping

For one active expert with `m_e` assignments, the rank-local GEMM is:

```text
A: [m_e, 6144] row-major NVFP4
B: [6144, 1024] column-major NVFP4
D: [m_e, 1024] gate/up accumulators
```

The persistent weight payload is stored as logical row-major
`W[1024,6144]`. Its address sequence is already the required column-major
`B[6144,1024]` sequence. The SFB probe proves every one of its 393,216 scale
offsets against the pinned CUTLASS layout.

The dynamic activation values are row-major and also require no transpose.
The separate SFA probe proves the current offset formula and padded storage
against CUTLASS over 17 assignment shapes from 1 through 65,535, totaling
42,564,864 comparisons.

## Required grouped-SFA correction

The proof also makes a grouped-kernel boundary explicit. The current direct
control stores one global activation SFA plane indexed by compacted assignment
row. A pointer-array grouped GEMM interprets each expert problem's first row
as local row zero. An expert slice that begins at a non-128-row boundary
therefore cannot point into the global swizzle and remain CUTLASS-compatible.

The MMA successor must quantize scales into expert-local SFA slabs:

```text
sfa_bytes(expert e) = round_up(m_e, 128) * 6144 / 16
sfa_offset(e)       = exclusive_prefix_sum(sfa_bytes(0..e))
```

Only active experts receive slabs. The activation value rows remain in stable
expert/token/route-slot compacted order. Problem sizes, A pointers, SFA
pointers, B/SFB pointers, and output pointers are all built from the same
rank-identical expert offsets. These arrays are preallocated per graph bucket;
there is no launch-time allocation or rank-local fallback.

This is a workspace-layout change, not a checkpoint-format change. Persistent
weight bytes and the frozen SFB ABI remain unchanged.

## Why the stock grouped example is only a control

A conventional grouped GEMM materializes all 1,024 gate/up accumulator
columns. The selected GLM operator must instead pair gate column `j` with up
column `512+j`, apply `SiLU(gate) * up`, and store only 512 BF16 columns.
Materializing the 1,024-column accumulator is permitted only as a named
development control; it cannot become the production path.

The production candidate therefore needs a paired-N mainloop/epilogue:

1. load one A tile once;
2. issue block-scaled MMA for the gate B tile;
3. issue block-scaled MMA for the corresponding up B tile;
4. retain both FP32 fragments until their K loop completes;
5. apply FP32 SwiGLU;
6. store one BF16 `[m_e,512]` tile.

For decode, persistent CTAs pull active-expert tile records from a bounded
device queue. For prefill, grouped cooperative and ping-pong schedules are
separate measured candidates. Both consume the same immutable weight bytes
and expert-local SFA workspace.

## Implementation sequence

1. Retain and pass the direct CUDA-core eager and CUDA-graph correctness
   gates.
2. Add a compile-only CUTLASS 79a-derived dense control for
   `[M,1024,6144]`, consuming the existing A/B/SFA/SFB bytes. Complete:
   the owned shared library contains 64 native SM120 NVFP4 `OMMA.SF`
   instructions and the Rust runner is linked.
3. Prove the dense control against the CPU oracle at M1 and M256; retain its
   materialized accumulator only as evidence.
4. Add expert-local SFA byte arithmetic, prefix sums, bounds tests, and a
   quantization writer; prove every written offset with CUTLASS. Complete
   compile-only: Rust/native workspace formulas agree, SFA slabs are bounded,
   and the native writer consumes the exact per-expert offsets.
5. Add a grouped materialized control and test empty, tail, one-hot,
   all-expert, and Zipf routing. Complete through compile/link: the grouped
   pointer-array kernel and a 14-positive/2-negative Rust runner are built;
   device correctness remains review-gated.
6. Replace the materialized epilogue with paired gate/up FP32 fragments and
   BF16 SwiGLU store.
7. Add the decode persistent queue, then tune prefill independently.
8. Run the frozen timing ledger, hardware counters, eager/graph determinism,
   and matched BF16/FP8/direct-CUDA controls.

## Stop conditions

- Any runtime weight transpose, SFB rewrite, or persistent dequantization.
- Any expert-local decision that changes the collective route on one rank.
- Any selected path that writes the full gate/up intermediate.
- Any speed comparison that changes precision membership or routing.
- Any MMA result accepted without the retained per-element and output-hash
  correctness evidence.
