# Draft roadmap

This is a discussion draft, not a commitment to implementation order.

## Phase 0 — establish truth

- Import no active code from `glm52-opt`.
- Link or copy only pinned, stable benchmark fixtures.
- Complete the exclusive compute-profiler acceptance on the current serving
  stack.
- Capture current NF3 and EXL3 controls on the post-upgrade motherboard.
- Re-run PCIe peer, collective, and topology matrices after the hardware move.

Exit: a trusted phase ledger and matched baseline table.

## Phase 1 — SM120 expert laboratory

- Record exact GLM expert matrix shapes and routing geometry.
- Build a standalone benchmark driver.
- Benchmark existing BF16, MXFP8, NVFP4, W4A16, NF3, and EXL3 paths.
- Sweep small-M and prefill-M shapes.
- Establish roofline-style byte and operation bounds.

Exit: identify whether the leading gap is format, kernel, dispatch, or
framework overhead.

## Phase 2 — reference format and packer

- Write a versioned format specification.
- Implement a deterministic CPU packer/depacker.
- Define tier tables, alignment, padding, scale swizzle, and checksums.
- Build exhaustive layout tests.
- Pack one real expert and compare reconstructed output.

Exit: byte-stable format ABI and CPU oracle.

## Phase 3 — first native kernel

- Implement the smallest useful SM120 path.
- Prefer one regime initially: C1/small-M decode or prefill, based on Phase 1.
- Keep routing and quantization timings separately visible.
- Compare correctness and time with all controls.

Exit: material microbenchmark win on cn4 with no unexplained numerical error.

## Phase 4 — one-layer replay

- Capture real layer inputs and routes from a pinned BF16/reference run.
- Replay one complete MoE layer.
- Compare residual output, downstream logits, memory, and time.
- Test eager and graph-captured execution.

Exit: layer-level win that survives realistic distributions and routing.

## Phase 5 — existing-runtime integration

- Expose the kernel through a narrow PyTorch/custom-op boundary.
- Add an explicit quantization method and fail-closed loader.
- Preserve current attention, KV, collectives, scheduler, and API.
- Run matched whole-model KLD and performance gates.

Exit: proof of end-to-end value without a new inference engine.

## Phase 6 — native GLM target runner

Only proceed if the integrated result shows remaining framework overhead.

- fixed GLM-5.2 architecture;
- explicit TP4 and PCIe topology;
- target/prefill/MTP execution plans;
- static workspace planning;
- graph capture by known shape families;
- minimal correctness-first tokenizer/request driver.

Exit: target-only runner that matches reference logits and beats the integrated
runtime in a named workload.

## Phase 7 — serving engine

- continuous batching;
- prefix/KV management;
- cancellation and backpressure;
- OpenAI-compatible API;
- observability and failure handling;
- long-duration reliability qualification.

Exit: service-quality engine. Kernel speed alone does not satisfy this phase.

## Suggested first deliverables

1. `docs/baseline-ledger.md`
2. `spec/format-v0.md`
3. `tests/test_format_v0.py`
4. `bench/bench_expert.py`
5. `results/sm120-existing-kernels-<date>/`
