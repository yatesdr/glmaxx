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
laboratory. Its destination is a lean Rust inference engine fixed to GLM-5.2,
TP4, and four SM120 GPUs. Format and kernel gates still precede full-engine
work so specialization is backed by measured value.

## Initial direction

The leading candidate is a hardware-native heterogeneous expert format:

- EXL3 for the capacity-critical majority of routed-expert weights;
- NVFP4 for a measured hot/quality-tolerant subset;
- native 6-bit, FP8, or BF16 protection for sensitive experts/tensors;
- router, sparse indexer, shared expert, dense front layers, LM head, and
  selected MTP tensors protected according to measured sensitivity;
- weights and scale factors stored directly in the swizzle/layout consumed by
  SM120 block-scaled MMA;
- separate decode-small-M and prefill-medium-M kernels;
- fused routing, activation quantization, expert GEMMs, activation, and expert
  reduction where measurements justify it.
- a Rust control plane with continuous batching, configurable MTP0–MTP6,
  prefix caching, and HBM/DRAM/NVMe KV tiers;
- EXL3/Trellis and NVFP4 as first-class weight backends, with a measured
  per-tensor/per-expert hybrid allowed.

Bring-up is NVFP4-first. EXL3 remains a required capacity backend, but it is
added after the NVFP4 laboratory path and before any full-checkpoint serving
gate. An all-NVFP4 serving profile cannot fit on the four-card target.

An arbitrary software-decoded 3-bit format is not the first experiment. It
must beat the hardware-native path after including unpack, dequantization,
register pressure, and occupancy—not merely store fewer bytes.

## Repository map

- [Research charter](docs/charter.md)
- [Hardware and lab plan](docs/hardware-lab.md)
- [SM120-native design sketch](docs/sm120-design.md)
- [From-scratch Rust engine plan](docs/native-engine-plan.md)
- [Normative engine v0 specification](spec/engine-v0.md)
- [Rank/checkpoint and KV format v0 specification](spec/format-v0.md)
- [Fable adversarial review](fable-adversarial.md)
- [Fable review disposition](docs/fable-review-disposition.md)
- [Fable v0.2 re-review handoff](docs/fable-review-handoff.md)
- [Fable v0.2 adversarial re-review](fable-adversarial-v2.md)
- [Fable v2 disposition](docs/fable-v2-disposition.md)
- [Generated GLM-5.2 operation manifest](manifests/glm52-operation-v1.json)
- [Frozen first NVFP4 physical ABI](docs/nvfp4-physical-abi.md)
- [cn4 kernel readiness and punchlist](docs/cn4-kernel-readiness.md)
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

Phase A now contains a Rust CPU/reference workspace, deterministic direct
NVFP4 packer and rank reader, KV/indexer/cache proofs, the fixed routed-FC1
Rust/C ABI, and an SM120 CUDA correctness baseline. The actual TP4 gate/up
fixture is reproducible from a committed recipe and digest.

No GPU claim has been made. CUDA compilation and SM120 correctness remain
blocked on independent review of the generated manifest/v0.2.2 amendment and
a separately authorized cn4 session. The first performance task is to replace
the retained CUDA-core control with CUTLASS block-scaled MMA.
