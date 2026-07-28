# Draft roadmap

This roadmap follows the repository gate order. No cn4 GPU work begins
without explicit operator authorization.

## Phase 0 — design and adversarial review

- keep `../glm52-opt` read-only;
- pin the model, tokenizer, reference runtime, quantizer, CUDA, and CUTLASS
  identities;
- complete the engine and format specifications;
- have Fable try to falsify the operation graph, memory budget, format,
  collective, KV, MTP, and failure contracts;
- resolve every blocker required for the CPU phase.

Exit: reviewed design disposition with no unresolved contradiction in model
geometry, capacity accounting, or gate order.

## Phase 1 — CPU reference and format proof

- generate the exact GLM-5.2 tensor inventory and operation manifest;
- implement the Rust NVFP4 numerical oracle, deterministic packer, rank-file
  reader, and corruption tests;
- freeze and prove the SM120 value permutation and scale swizzle against the
  selected pinned implementation;
- implement the 368-byte KV oracle, page ownership, prefix keys, and
  HBM/DRAM/NVMe state-machine tests;
- implement the one-layer MTP draft-KV sidecar and atomic target/draft
  publication tests;
- implement MTP0–MTP6 reference transitions after recurrent semantics are
  frozen;
- calculate physical weights, maximum workspaces, graph residency, escrow,
  and one-million-token KV from real bytes.

Exit: byte-stable NVFP4 ABI, pack/dequant equivalence on synthetic and actual
GLM tensor shapes, and no silent unsupported codec path.

EXL3 repeats this CPU gate later from pinned source. It does not block the
NVFP4-first runner, but it blocks the capacity profile.

## Phase 2 — authorized SM120 microbenchmark

Only after explicit operator authorization:

- benchmark inclusive NVFP4 expert operators at actual 6,144×2,048 shapes;
- sweep decode/MTP small-M and routed prefill-M distributions;
- include routing, compaction, activation quantization, epilogues, launches,
  and reconstruction in the timed boundary;
- report hardware counters, physical bytes, and cold/warm effects separately;
- compare against pinned BF16, FP8, W4A16, and existing-runtime controls;
- qualify TP/DCP collective routes on the actual PCIe topology.

Exit: a material explained operator win or a documented redesign decision.

After the EXL3 CPU gate passes, run the same phase for EXL3 before enabling
that backend.

## Phase 3 — one-layer TP4 replay

- capture provenance-recorded real layer inputs and routes;
- execute attention, routing, experts, collectives, residual, and downstream
  logit comparison on all four ranks;
- prove collective order and route identity;
- compare eager and graph-captured execution;
- separate kernel, launch, TP, DCP, and host time.

Exit: layer-level correctness and a matched target-hardware speed result.

## Phase 4 — minimal Rust target runner

- load an NVFP4 small-checkpoint slice into deterministic rank-local arenas;
- execute target-only prefill and greedy MTP0 decode through the real
  descriptors, graphs, TP4 plans, and error paths;
- use the fixed TP4/DCP4 execution plan and graph families;
- exercise maximum scratch, error propagation, and repeated clean startup.

An existing-runtime adapter is optional when it makes a matched A/B easier.
It is not the final architecture or a prerequisite for the Rust runner.

Exit: the NVFP4 small-checkpoint runner matches reference logits and
survives repeated execution. It makes no full-model serving claim.

## Phase 5 — quality, capacity, and long context

- extract and prove the pinned EXL3 codec, then qualify its SM120 kernel;
- freeze the per-rank physical budget;
- load and smoke a fit-capable EXL3 or `hybrid-serve` full checkpoint;
- retain NVFP4 as a laboratory and hybrid operator control, never as a
  standalone serving profile;
- retain per-position full-vocabulary KLD and task results;
- run frozen and randomized retrieval through the model limit;
- measure cold prefill and deep-context decode;
- admit one 1,048,576-token total sequence with active KV in HBM;
- fail closed when a speed profile cannot provide that active capacity.

Exit: quality PASS, honest 1M admission, and a material named performance or
capacity win under matched precision and cache posture.

## Phase 6 — concurrent cache and speculation engine

- iteration-level continuous batching and qualified mixed prefill/decode;
- graph buckets through the measured concurrency ceiling;
- MTP depths zero through six with acceptance and useful-token accounting;
- MTP draft-KV sidecars and stable-position/tie-adjacent quality accounting;
- distributed sharded-vocabulary sampling without full-logit gathers;
- deterministic DCP4 decode query/candidate/partial-softmax exchange;
- page-aligned prefix sharing and copy-on-write tails;
- bounded HBM, DRAM, and NVMe tiers with recovery;
- admission, fairness, cancellation, and backpressure;
- multi-user load and cache-thrash fault tests.

Exit: concurrent correctness, bounded resources, clean tier transitions, and
useful-token throughput that beats matched general-purpose controls.

## Phase 7 — serving and end-to-end qualification

- minimal OpenAI-compatible text API, streaming, and pinned chat templates;
- common sampling controls and deterministic seed behavior;
- observability for queueing, graphs, MTP, collectives, cache tiers, and
  physical memory;
- crash recovery and long-duration reliability;
- matched vLLM/SGLang-class workload matrices across concurrency, topology,
  prefix reuse, MTP depth, and context bands.

Exit: service-quality engine. Kernel speed alone does not satisfy this phase.

## Immediate deliverables

1. Fable review findings and disposition against
   [the review handoff](fable-review-handoff.md).
2. Generated pinned model tensor and operation manifest.
3. Rust workspace containing `glm-format` and `glm-reference`.
4. Synthetic and actual-shape NVFP4 CPU fixtures.
5. Physical memory and one-million-token admission calculator.
