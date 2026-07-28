# AGENTS.md

This repository is the isolated research home for an SM120-native GLM-5.2
weight format and inference engine. It is intentionally separate from
`../glm52-opt`, which remains the production serving and evidence repository.

## Project posture

- Target hardware is four NVIDIA RTX PRO 6000 Blackwell GPUs (SM120), connected
  over PCIe without NVLink.
- Optimize the real target. Results on SM100/B200/B300 are useful references,
  not acceptance evidence for SM120.
- Co-design the packed weight representation and the consuming kernel. A new
  serialization format without a measured execution advantage is not a win.
- Keep correctness and quality gates hard. Capacity or speed never excuses
  silent numerical corruption.

## Safety and coordination

- Do not start, stop, or overlap GPU work on cn4 without explicit operator
  authorization. Read-only inventory is allowed when it will not disturb a
  running test.
- Do not modify `../glm52-opt` from this repository. Copy only stable,
  provenance-recorded fixtures when a test genuinely needs them.
- Never put model weights, datasets, generated checkpoints, caches, or raw
  benchmark output in Git.
- Pin source revisions, container digests, model revisions, commands, prompt
  hashes, and artifact hashes in every result record.
- A collective route must be selected identically by all ranks. Rank-local
  fallback decisions are forbidden.

## Development contracts

1. Start with a CPU/reference definition of every packed format and operation.
2. Prove pack/dequant equivalence before benchmarking a CUDA kernel.
3. Test actual GLM-5.2 tensor shapes and the small-M decode regime.
4. Separate kernel time, framework overhead, collectives, and end-to-end time.
5. Report cold prefix-cache misses and warm reuse separately.
6. Compare target-only MTP0 quality and speed before enabling MTP.
7. Do not call a result faster if it changes precision membership, context,
   batching, or cache posture without a matched control.
8. Preserve per-position quality values, not only aggregate KLD means.

## Gate sequence

Use the sequence:

design note → adversarial review → CPU proof → GPU microbenchmark → one-layer
replay → checkpoint smoke → quality gates → matched end-to-end benchmark.

Do not skip directly from a plausible format to a full 753B conversion.
