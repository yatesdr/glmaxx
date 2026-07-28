# SM120-native design sketch

## Why SM120 needs its own path

SM120 exposes Blackwell narrow-precision `mma.sync` operations, including
block-scaled FP4/FP6/FP8 families. Unlike SM100, its cluster shape is fixed to
1×1×1 and it has no multicast. A kernel whose scheduling model assumes SM100
features can be correct yet inefficient—or unsupported—on SM120.

The target card has 188 SMs and 1,792 GB/s of GDDR7 bandwidth. The relevant
question is not peak TOPS in isolation. It is whether small-M routed-expert
work can feed the Tensor Cores without wasting bandwidth on transformations
or waiting on launches and dispatch.

## Two workload regimes

### Decode

At low concurrency, an MoE expert often receives only one or a few tokens.
This regime is dominated by some combination of:

- reading the active experts' weights;
- routing and compaction;
- activation quantization;
- many small launches;
- poor grouped-GEMM occupancy;
- intermediate writes and reads;
- tensor-parallel synchronization.

A successful decode kernel may look more like a persistent expert matvec/MMA
engine than a conventional large grouped GEMM.

### Prefill

With thousands of input rows, each expert receives a larger token group.
Grouped GEMM becomes more credible, but routing balance, sorting, scale
layout, tile selection, and TP/DCP communication remain important.

Decode and prefill should have separate dispatch and packing decisions.

## Candidate format: native heterogeneous blocks

Version zero should use only formats the SM120 MMA can consume directly:

| Tier | Candidate use |
|---|---|
| NVFP4 | ordinary routed-expert blocks |
| MXFP6/FP6 | blocks needing more dynamic range or precision |
| FP8/MXFP8 | highly sensitive projections or experts |
| BF16 | small critical tensors and reference fallback |

The exact supported operand combinations must be proven with the selected
CUTLASS/CUDA revision before freezing the ABI.

### Allocation inputs

Bit/format assignment should use measured evidence:

- Hessian-weighted reconstruction error;
- activation-weighted output error;
- expert routing frequency;
- expert co-occurrence;
- per-layer downstream logit KLD;
- long-context sensitivity;
- MTP acceptance impact;
- bytes and kernel time, not checkpoint size alone.

### Physical layout

The packer should write:

- tensor-core-ready value ordering;
- tensor-core-ready scale swizzle;
- explicit alignment and padding;
- per-expert offsets;
- per-format expert/tensor tables;
- versioned metadata;
- checksums covering every packed payload.

No runtime repacking is permitted in the final design. If the runtime must
transpose or swizzle every load, the checkpoint format is incomplete.

## Kernel hypotheses

Test, do not assume:

1. Persistent CTAs reduce launch and routing overhead at small M.
2. Fusing activation quantization with expert input staging hides its cost.
3. Fusing FC1 epilogue, activation, and FC2 input construction avoids a
   bandwidth-heavy intermediate.
4. Grouping active experts by precision tier is cheaper than software
   dequantizing a single arbitrary-bit format.
5. Replicating a small set of hot experts or shared experts may save more
   communication than the extra VRAM costs.
6. Expert ordering based on routing co-occurrence may improve locality.
7. A different kernel is required for MTP verifier batches than for C1 target
   decode or prefill.

## Engine boundary

Do not begin with a complete server. Build these layers:

1. CPU format library and oracle.
2. CUDA kernel library with a simple benchmark driver.
3. One-layer GLM replay.
4. PyTorch custom operation.
5. Existing vLLM/SparkInfer integration for matched A/B.
6. Minimal target-only engine if framework overhead remains material.
7. API/scheduler work only after the target-only engine is correct.

The existing runtime is initially an oracle and experiment host, not an enemy
that must be replaced wholesale.
