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
- [Phase A CPU preparation proof](docs/phase-a-proof.md)
- [cn0 SM86 non-acceptance compile/link bring-up](docs/cn0-sm86-bringup.md)
- [Offline StepPlan, graph admission, and memory-contract candidate](docs/offline-engine-contract.md)
- [Offline serving foundation: scheduler, sampling, tiers, startup, and FC2](docs/offline-serving-foundation.md)
- [Executable offline serving spine: durable restore through TP4 consensus](docs/offline-serving-spine.md)
- [Pinned tokenizer, chat-template, and streaming text contract](docs/tokenizer-serving-contract.md)
- [Pinned EXL3/Trellis CPU reconstruction candidate](docs/exl3-trellis-cpu-contract.md)
- [Strict safetensors checkpoint ingest contract](docs/checkpoint-ingest.md)
- [Fable offline-foundation review handoff](docs/fable-offline-foundation-handoff.md)
- [Fable Phase-A and engine-contract review handoff](docs/fable-phase-a-engine-handoff.md)
- [Quantization and checkpoint workflow](docs/quantization-workflow.md)
- [Benchmark and quality contract](docs/benchmark-contract.md)
- [Serving observability contract](docs/serving-observability-v1.md)
- [Current production punchlist](docs/production-punchlist.md)
- [Provenance-aware results index](docs/results-index.md)
- [Draft roadmap](docs/roadmap.md)
- [Sources and starting evidence](docs/references.md)

## Relationship to glm52-opt

`../glm52-opt` contains production-serving work, CUDA/collective patches, and
the measured GLM-5.2 evidence base. This repository does not replace it.
Stable results may eventually be handed back there, but exploratory format,
kernel, and engine work stays here.

## Current status

The current local gate passes 208 Rust tests plus formatting, Clippy with
warnings denied, CUDA FFI type checks, deterministic proof regeneration, and
the pinned external-tokenizer proof. The CPU/reference workspace includes
direct NVFP4 packing, EXL3/Trellis reconstruction, strict checkpoint ingest,
hybrid policy machinery, TP4 startup and step consensus, bounded continuous
batching, distributed-sampling oracles, transactional prefix storage,
HBM/DRAM/NVMe residency simulation, active page tables, persistent worker
interfaces, request streaming, and bounded host observability.

This is still a CPU/control-plane and kernel-preparation implementation, not a
production inference engine. The latest checked-in cn4 preparation record
compiled real `sm_120f` cubins with pinned CUDA 13.3 and CUTLASS 4.6.1,
validated layouts and ABI linkage, and passed 162 Rust tests in a container
without GPU access. It created no CUDA context and launched no device kernel.
It therefore proves compileability and artifact presence only.

The prior cn4 authorization has ended and the machine was released to another
development workload. Do not reconnect or launch GPU work without renewed
operator authorization. Corrective Fable implementation reviews are also
pending; an accepted design review is not device-correctness evidence.

The exact proved results and exclusions are in the
[results index](docs/results-index.md). The
[production punchlist](docs/production-punchlist.md) tracks the still-open
SM120 kernels, complete target and draft execution, tier transfers, checkpoint
startup, model-quality gates, concurrency qualification, 1M-context run, and
matched end-to-end benchmarks. Full-checkpoint conversion remains
fail-closed.
