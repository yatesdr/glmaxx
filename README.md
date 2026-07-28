# glm5-native

Research toward an SM120-native weight format and inference engine for
GLM-5.2 on four RTX PRO 6000 Blackwell GPUs.

## Thesis

The workstation Blackwell target is not a small datacenter Blackwell system.
SM120 has different MMA, scheduling, and cluster constraints, while the target
machine has PCIe and no NVLink. Generic vLLM/SGLang kernels and SM100-oriented
MoE paths may therefore leave substantial performance unused.

The project will test that thesis rather than assume it. Its first goal is to
measure where time goes. Its second is to build an SM120-native routed-expert
laboratory. A new checkpoint format or standalone engine follows only after a
kernel-level result proves worthwhile.

## Initial direction

The leading candidate is a hardware-native heterogeneous expert format:

- NVFP4 for most routed-expert weights;
- native 6-bit, FP8, or BF16 protection for sensitive experts/tensors;
- router, sparse indexer, shared expert, dense front layers, LM head, and
  selected MTP tensors protected according to measured sensitivity;
- weights and scale factors stored directly in the swizzle/layout consumed by
  SM120 block-scaled MMA;
- separate decode-small-M and prefill-medium-M kernels;
- fused routing, activation quantization, expert GEMMs, activation, and expert
  reduction where measurements justify it.

An arbitrary software-decoded 3-bit format is not the first experiment. It
must beat the hardware-native path after including unpack, dequantization,
register pressure, and occupancy—not merely store fewer bytes.

## Repository map

- [Research charter](docs/charter.md)
- [Hardware and lab plan](docs/hardware-lab.md)
- [SM120-native design sketch](docs/sm120-design.md)
- [Quantization and checkpoint workflow](docs/quantization-workflow.md)
- [Benchmark and quality contract](docs/benchmark-contract.md)
- [Draft roadmap](docs/roadmap.md)
- [Sources and starting evidence](docs/references.md)

## Relationship to glm52-opt

`../glm52-opt` contains production-serving work, CUDA/collective patches, and
the measured GLM-5.2 evidence base. This repository does not replace it.
Stable results may eventually be handed back there, but exploratory format,
kernel, and engine work stays here.

## Current status

Documentation scaffold only. No format ABI, checkpoint converter, or CUDA
kernel has been selected or implemented.
