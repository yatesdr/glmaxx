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
- [Bounded native-rank reader CPU proof](docs/native-rank-reader-proof-v1.md)
- [Strict production rank-manifest CPU proof](docs/production-rank-manifest-validation-v2.md)
- [Production rank-manifest adversarial handoff](docs/fable-production-rank-manifest-validation-v2-handoff.md)
- [Complete target-layer execution candidate](docs/target-layer-execution-v1.md)
- [Target-layer execution adversarial handoff](docs/fable-target-layer-execution-v1-handoff.md)
- [Recurrent MTP0–6 execution candidate](docs/mtp-layer-execution-v1.md)
- [Recurrent MTP adversarial handoff](docs/fable-mtp-layer-execution-v1-handoff.md)
- [Rust-owned SM120 rank executor candidate](docs/sm120-rank-executor-v1.md)
- [SM120 rank executor adversarial handoff](docs/fable-sm120-rank-executor-v1-handoff.md)
- [Fail-closed review provenance verifier v2](docs/review-provenance-verifier-v2.md)
- [Integrated cache lifecycle CPU proof](docs/cache-lifecycle-proof-v1.md)
- [Final cn4 release record](docs/cn4-release-20260729.md)
- [Checkpoint load transaction candidate](docs/checkpoint-load-transaction-v1.md)
- [Corrected checkpoint load transaction adversarial handoff](docs/fable-checkpoint-load-transaction-v1-r2-handoff.md)
- [Fable offline-foundation review handoff](docs/fable-offline-foundation-handoff.md)
- [Fable Phase-A and engine-contract review handoff](docs/fable-phase-a-engine-handoff.md)
- [Quantization and checkpoint workflow](docs/quantization-workflow.md)
- [Benchmark and quality contract](docs/benchmark-contract.md)
- [Serving observability contract](docs/serving-observability-v1.md)
- [Current coordinator/observability adversarial handoff](docs/fable-coordinator-api-backend-v2-handoff.md)
- [Online target/draft prefix publication candidate](docs/online-prefix-publication-v1.md)
- [Online publication adversarial handoff](docs/fable-online-prefix-publication-v1-handoff.md)
- [Distributed sampling and MTP RNG candidate](docs/distributed-sampling-abi-v1.md)
- [Distributed sampling adversarial handoff](docs/fable-distributed-sampling-abi-v1-handoff.md)
- [Tenant/global resource quota candidate](docs/tenant-resource-quotas-v1.md)
- [Resource quota adversarial handoff](docs/fable-tenant-resource-quotas-v1-handoff.md)
- [Nonblocking Linux HTTP transport candidate](docs/nonblocking-http-transport-v1.md)
- [Nonblocking transport adversarial handoff](docs/fable-nonblocking-http-transport-v1-handoff.md)
- [Direct DRAM/NVMe tier I/O candidate](docs/direct-tier-io-v1.md)
- [Direct tier I/O adversarial handoff](docs/fable-direct-tier-io-v1-handoff.md)
- [Quality and MTP numerical acceptance candidate](docs/quality-acceptance-v1.md)
- [Quality acceptance adversarial handoff](docs/fable-quality-acceptance-v1-handoff.md)
- [Quality corpus source and materialization candidate](docs/quality-corpus-manifest-v1.md)
- [Pinned quality corpus source recipe](manifests/quality-corpus-sources-v1.json)
- [Fail-closed review provenance verifier](docs/review-provenance-verifier-v1.md)
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

The current local gate passes 227 Rust tests plus formatting, Clippy with
warnings denied, CUDA FFI type checks, and deterministic proof regeneration.
It also verifies all 27 candidate-based Fable handoffs against their exact
committed inputs and can classify an explicit review artifact by exact
acceptance-token presence only after the review attests the exact candidate,
every pinned input hash, and any required result path. Declared Fable results
are ingested automatically as soon as their files exist. The external-tokenizer
fixture remains pinned from the prior proof; the latest run skipped that
external check because
`GLMAXX_TOKENIZER_DIR` was not set. The CPU/reference workspace includes
direct NVFP4 packing, EXL3/Trellis reconstruction, strict checkpoint ingest,
bounded file-backed four-rank verification, typed production-manifest
validation, source-pinned complete target and recurrent-MTP execution
designs, a Rust-owned persistent SM120 rank-executor design, hybrid policy
machinery, TP4
startup and step consensus, bounded continuous batching, distributed-sampling
oracles, transactional prefix storage, HBM/DRAM/NVMe residency simulation,
active page tables, persistent worker interfaces, request streaming, and
bounded host observability.

This is still a CPU/control-plane and kernel-preparation implementation, not a
production inference engine. The latest checked-in cn4 preparation record
compiled real `sm_120f` cubins with pinned CUDA 13.3 and CUTLASS 4.6.1,
validated layouts and ABI linkage, and passed 162 Rust tests in a container
without GPU access. It created no CUDA context and launched no device kernel.
It therefore proves compileability and artifact presence only.

The final cn4 inventory found an existing four-rank vLLM allocation and no
GLMAXX process. No CUDA work was launched, the session disconnected, and cn4
was released to that workload. Do not reconnect or launch GPU work without
renewed operator authorization. Corrective Fable implementation reviews are
also pending; an accepted design review is not device-correctness evidence.

The exact proved results and exclusions are in the
[results index](docs/results-index.md). The
[production punchlist](docs/production-punchlist.md) tracks the still-open
SM120 kernels, complete target and draft execution, tier transfers, checkpoint
startup, model-quality gates, concurrency qualification, 1M-context run, and
matched end-to-end benchmarks. Full-checkpoint conversion remains
fail-closed.
