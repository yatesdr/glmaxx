# Research charter

## Objective

Determine whether a GLM-5.2 execution stack designed specifically for four
SM120 workstation GPUs can materially improve:

1. single-user decode;
2. batched decode;
3. cold prefill;
4. model and KV capacity;

while preserving the established quality and long-context gates.

## Questions in order

1. Which exclusive phases dominate the target workload?
2. Is routed-expert execution bandwidth-, compute-, launch-, dispatch-, or
   collective-limited at each batch shape?
3. How close are current kernels to the card's measured bandwidth and
   block-scaled MMA ceilings?
4. Can a hardware-native mixed-precision layout improve quality per byte
   without introducing software dequantization?
5. Can fused SM120-specific EXL3 and NVFP4 MoE kernels improve inclusive
   operator and layer wall time?
6. How much additional value can a fixed GLM-5.2 Rust executor obtain from
   static memory planning, graph specialization, continuous batching, MTP,
   prefix sharing, and topology-specific collectives?

## Non-goals for the first phase

- Training GLM-5.2 from scratch.
- Full-model QAT.
- Building the complete serving API or scheduler before proving the fixed
  executor and its kernel opportunities.
- Optimizing B200/B300 and treating the result as proof for SM120.
- Publishing a checkpoint based only on a small KLD smoke cell.

## Success criteria

A research milestone must report all three:

- a reproducible quality result against a pinned BF16 reference;
- a measured target-hardware speed or capacity result;
- an explanation of the mechanism, including bytes moved and operations
  introduced or removed.

For a new weight format to proceed beyond the laboratory, it should provide a
material gain over both the current NF3/NVFP4 hybrid and EXL3/Trellis control
in at least one important regime without an unacceptable quality regression.

## Core distinction

There are two related but independent projects:

- **Weight representation:** capacity, memory traffic, quantization error, and
  the cost of reconstructing values.
- **Inference engine:** routing, batching, launch count, graph capture,
  collectives, cache management, and scheduling.

Every experiment must state which side it changes. A result that changes both
needs an intermediate control to identify the cause.
