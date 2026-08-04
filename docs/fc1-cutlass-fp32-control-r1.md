# FC1 CUTLASS FP32 materialization control r1

Date: 2026-08-04

Status: corrective design candidate; adversarial acceptance required before
CPU or CUDA implementation

## Problem

The GLMAXX-owned CUTLASS FC1 control proves that native SM120 NVFP4 MMA can
consume the frozen A/B/SFA/SFB bytes, but it materializes all 1,024 gate/up
columns as BF16 before scaling and SwiGLU. That precision boundary is
deliberately absent from the production design, which retains paired gate/up
FP32 fragments through SwiGLU.

The pinned cn4 probe at `86fe811` found one representative cancellation
boundary:

```text
ascending-K semantic       -172  0xc32c
CUDA-core 256-lane tree    -177  0xc331
CUTLASS BF16-D M1/M256     -176  0xc330
```

CUTLASS was batch-invariant and bitwise repeatable, but its absolute error of
4 exceeds the frozen `0.5 + 0.02 * abs(reference)` bound of 3.94. The result
does not establish whether tensor-core accumulation order or the avoidable
BF16 D boundary accounts for the final one-ULP gate failure.

## Decision

Add a separate development control whose only numerical change is CUTLASS
`ElementD = float`. It materializes `[assignments,1024]` FP32 values, applies
the existing activation-global and expert-global scales in FP32, computes the
ordinary non-fast-math FP32 SiLU/product, and performs one final BF16
round-to-nearest-even store.

The existing `gate_up_accum_f32` allocation and workspace contract already
charge exactly four bytes per assignment and gate/up column. The new control
must use that complete allocation; it may not alias the output, compacted
input, SFA, CUTLASS workspace, or route records. It receives the same
descriptor and consumes the same immutable value and scale bytes. No weight
transpose, repack, SFB rewrite, dequantized plane, rank-local route, or
fallback is permitted.

Use a new exported launcher and explicit report/backend identity. Do not
change the existing BF16-D launcher or relabel its retained evidence.

## Acceptance boundary

The semantic oracle and frozen element rule remain unchanged:

```text
finite(device) && abs(device - semantic) <= 0.5 + 0.02 * abs(semantic)
```

There is no schedule-exact rescue for CUTLASS and no ULP exception. The FP32-D
control must pass every element at M1 and M256 and be bitwise identical across
20 repeats. Its report preserves maximum absolute/relative error, every
failure, output hashes, the row-239/column-20 value, input/weight payload
hashes, runtime repack bytes, persistent dequant bytes, binary/library hashes,
and device/toolchain provenance.

The CPU proof must independently establish:

1. the existing workspace has exactly `assignments * 1024 * 4` bytes for D;
2. the BF16-D and FP32-D controls differ only at the materialized D type and
   downstream load conversion;
3. every pointer range is aligned, bounded, and disjoint at M1 and M256;
4. the semantic oracle and tolerance bytes are unchanged; and
5. mutations to D width, scale placement, gate/up pairing, SiLU, final BF16
   rounding, or workspace size are detected.

After CPU proof, a fresh cn4 run executes in this order:

1. retained BF16-D M1/M256 diagnostic;
2. FP32-D M1/M256, 20 repeats;
3. only if dense FP32-D passes, an identically typed grouped control over the
   frozen routing matrix; and
4. only after the separate fused-MoE design is accepted, the paired-fragment
   production candidate against the unchanged semantic gate and FP32-D
   control.

If FP32-D fails any semantic element, work stops at the control. The failure
must be retained and a separately reviewed numerical contract is required;
the threshold may not be tuned to the observed value. If it passes, it proves
only the native packed-byte/precision control. It does not accept the fused
kernel, a layer, checkpoint, quality, capacity, latency, or throughput.

## Nonclaims

This design does not accept the current BF16-D result, change the model
semantic oracle, reproduce opaque tensor-core accumulation order on CPU,
authorize a production materialized intermediate, or bypass the pending
NVFP4 fused-MoE, manifest, target-program, TP4 replay, and quality gates.
