# Goal: working GLMAXX GLM-5.2 engine

Build and optimize a Rust-owned GLM-5.2 engine for four RTX PRO 6000
Blackwell GPUs (SM120, TP4 over PCIe). Run the real TR3 3.25-bpw and
ModelOpt NVFP4/NF3 checkpoints with continuous batching, prefix caching,
MTP0, and MTP3.

Targets:

- C1/context-zero median decode: >=50 useful tok/s MTP0 and >=100 useful
  tok/s MTP3.
- >=524,288 physically backed NVFP4-KV tokens under MTP3, with 1M-compatible
  addressing and no design ceiling below 1M.
- Cold boot <=50% of matched vLLM; compatible kernel/config hot reloads must
  reuse resident weights or retain a safe ABI for doing so later.
- Exact KLD procedure and Local Inference Lab decode matrix for both real
  checkpoint profiles.

Work in reviewed order: strict source admission and CPU proofs; optimized
SM120 kernels; real TP4 layer replay; four-prompt MTP0 checkpoint smoke;
quality; MTP3; physical KV allocation; serving and matched benchmarks.
Rust owns loading, runtime, scheduling, serving, validation, and evidence;
SM120-only CUDA/CUTLASS kernels are allowed.

cn4 access is fully authorized at `192.168.13.34` (`derek`, `claude`). Keep
all GLMAXX work under `~/glmaxx/` in unique worktree/build/cache/evidence
paths. Never touch or reuse vLLM worktrees, containers, images, volumes,
caches, ports, shared memory, checkpoints, or results. Do not stop unrelated
processes or clear shared state without a fresh operator check.

Every experiment must use an immutable commit and a unique UTC evidence
directory recording commands, hashes, model inputs, toolchains, topology,
logs, metrics, and a machine-readable summary. Keep weights, caches, datasets,
and raw evidence out of Git. Rank routes and fallbacks must be identical.

Complete only when both real checkpoints generate quality-gated output,
MTP0/MTP3 targets and physical KV capacity are measured and pass, cold/hot
load behavior is evidenced, and the remaining optimization list is ordered
by measured impact.
