# Quantization and checkpoint workflow

## Learn the standard pipeline first

The first full workflow should reproduce the official LLM Compressor
GLM-5.2 example:

- distributed initialization;
- disk/CPU offload;
- 512 calibration samples;
- 2,048-token sequences;
- FP8 block-scaled attention;
- NVFP4 MoE;
- dense front layers, gates, indexer projection, and LM head protected;
- compressed checkpoint export.

This is a training exercise and a baseline. It establishes that downloads,
offload, calibration, serialization, and vLLM loading work before changing
the quantization method.

## Tool roles

### LLM Compressor

Best first end-to-end tool because it has an exact GLM-5.2 MoE example,
distributed calibration, sequential onloading, disk offload, and vLLM-facing
compressed-tensors export.

### NVIDIA Model Optimizer

Use as the hardware-native NVFP4 reference and for advanced calibration,
layerwise workflows, QAT, or custom quantizer experiments. Treat exported
layout compatibility with the chosen SM120 kernel as a separate gate.

### EXL3/Trellis

Use as the arbitrary-bit/Hessian-informed control. It is especially valuable
for quality-per-byte comparisons, but software reconstruction cost and the
custom runtime must be included in performance comparisons.

## Progressive scale

1. Synthetic matrices with exact GLM dimensions.
2. One BF16 expert tensor.
3. All experts in one layer.
4. Several sensitivity-selected layers.
5. A checkpoint with most source tensors untouched.
6. Full checkpoint.

Every stage must be resumable. A 753B conversion should never lose all work
because one late tensor or output shard fails.

## Required checkpoint manifest

Record at minimum:

- base model repository and immutable revision;
- tokenizer/config revisions;
- converter repository and commit;
- CUDA, driver, PyTorch, CUTLASS, and compiler versions;
- calibration dataset identifiers, revisions, licenses, sample IDs, hashes,
  shuffle seed, chat template, truncation, and token counts;
- format policy and every ignore/protection rule;
- tensor-to-tier map;
- source and output hashes;
- total logical parameters and physical bytes by tier;
- packer ABI version;
- kernel ABI version;
- exact command and environment;
- host/GPU inventory;
- start/end times and peak CPU/GPU/storage use.

## Candidate ladder

Begin with controls that answer one question each:

1. Published/pinned NVFP4+FP8 recipe reproduction.
2. Same format with quality-first protected tensors.
3. Native heterogeneous 4/6/8-bit expert tiers.
4. Routing-aware expert allocation.
5. Hessian/activation-aware block allocation.
6. Only after these, a non-native sub-4-bit/codebook experiment.

Do not mix a new format, a new kernel, new protected tensors, and a different
calibration corpus in one first result.
