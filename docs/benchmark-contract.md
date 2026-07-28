# Benchmark and quality contract

## Measurement layers

### 1. Format correctness

- pack/depack byte tests;
- CPU dequantization oracle;
- scale and swizzle indexing;
- odd dimensions and padding;
- every precision tier;
- deterministic serialization;
- corrupt-header and corrupt-payload rejection.

### 2. Kernel correctness

Compare with BF16/FP32 references across:

- real GLM shapes;
- `M = 1, 2, 4, 8, 16, 32, 64, 128`;
- realistic activation distributions;
- adversarial large/small values;
- all expert counts and tail groups;
- repeated launches and CUDA graph capture.

Report max absolute/relative error and output-distribution metrics. Never use
only a few printed values.

### 3. Kernel performance

Measure:

- median and distribution, not only best time;
- bytes read/written;
- achieved GDDR bandwidth;
- Tensor Core utilization;
- SM occupancy;
- launch count;
- activation quantization time;
- routing/compaction time;
- FC1, activation, FC2, and reduction time;
- temporary memory;
- graph-captured and eager paths.

Benchmark against the actual current NF3, NVFP4, W4A16, MXFP8, and
EXL3/Trellis kernels available on the same software image.

### 4. Model quality

The first KLD cell is a smoke test:

- pinned BF16 logits;
- pinned 2,048-token sample;
- all 2,047 next-token positions;
- full vocabulary;
- FP64 KLD calculation;
- MTP disabled;
- per-position values retained.

Final qualification expands to many non-overlapping windows and multiple
content axes. Preserve raw per-position values so bootstrap and tail analysis
remain possible.

Add:

- GPQA-Diamond or another reasoning set with a pinned evaluator;
- coding and agent/tool-use prompts;
- JSON/schema exactness;
- long-generation repetition checks;
- frozen and randomized long-context retrieval;
- termination and reasoning-parser behavior;
- MTP acceptance after target-only quality passes.

`lm-evaluation-harness` can drive standardized tasks through a local
OpenAI-compatible endpoint. Pin its revision, task YAML, few-shot policy,
chat template, answer extraction, and retained samples.

### 5. End-to-end performance

Report separately:

- target-only MTP0 and MTP3;
- C1, C2, C4, and C16 decode;
- zero, medium, and long starting contexts;
- cold standalone prefill at multiple lengths;
- warm prefix-cache reuse;
- GPU KV capacity and maximum admitted request;
- power, clocks, temperature, and throttling;
- fatal signatures and restarts.

Cold rows require a randomized first full cache block and server-side
`cached_tokens=0`. A throughput number without cache deltas is not accepted.

## Profiler-first rule

Before claiming that a weight kernel limits whole-model performance, produce
an exclusive layer ledger:

```text
layer
├── attention/indexer
├── attention TP/DCP communication
├── MoE compute
├── MoE TP communication
└── norms/residual/other
```

The sum must reproduce layer wall time within a declared tolerance. Optimize
the largest relevant exclusive phase, and rerun the ledger after the change.

## Minimum result bundle

- hypothesis;
- immutable inputs;
- code and artifact hashes;
- exact command/environment;
- raw timings;
- summary statistics;
- quality raw values and aggregates;
- profiler trace or counters;
- comparison table;
- known limitations;
- explicit PASS, FAIL, or INCONCLUSIVE verdict.
