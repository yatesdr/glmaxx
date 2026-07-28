# Sources and starting evidence

## Primary external sources

- GLM-5.2 model card, architecture summary, and 753B parameter count:
  <https://huggingface.co/zai-org/GLM-5.2>
- LLM Compressor exact GLM-5.2 MoE quantization example:
  <https://github.com/vllm-project/llm-compressor/blob/main/examples/quantizing_moe/glm5_example.py>
- LLM Compressor capabilities and large-model offload:
  <https://github.com/vllm-project/llm-compressor>
- Red Hat GLM-5.2 NVFP4+FP8 checkpoint, recipe, and reported six-A100 run:
  <https://huggingface.co/RedHatAI/GLM-5.2-NVFP4-FP8>
- NVIDIA Model Optimizer:
  <https://github.com/NVIDIA/Model-Optimizer>
- NVIDIA CUTLASS Blackwell and SM120 GEMM documentation:
  <https://docs.nvidia.com/cutlass/latest/media/docs/cpp/blackwell_functionality.html>
- NVIDIA CUTLASS block-scaled GEMM tutorial:
  <https://docs.nvidia.com/cutlass/latest/media/docs/operators/tutorials/006_block_scaled_gemm.html>
- RTX PRO 6000 specifications:
  <https://www.nvidia.com/en-us/products/workstations/professional-desktop-gpus/rtx-pro-6000/>
- Supermicro M12SWA-TF specifications:
  <https://www.supermicro.com/tw/products/motherboard/m12swa-tf>
- ExLlamaV3 and EXL3 format:
  <https://github.com/turboderp-org/exllamav3>
- Language Model Evaluation Harness:
  <https://github.com/EleutherAI/lm-evaluation-harness>

## Public evidence of SM120 specialization gaps

These are issue and release records, not timeless conclusions. Recheck them
against the exact revisions used by an experiment.

- CUTLASS grouped block-scaled MoE behavior on SM120:
  <https://github.com/NVIDIA/cutlass/issues/3096>
- vLLM SM120 NVFP4/MoE support discussion:
  <https://github.com/vllm-project/vllm/issues/31085>
- Recent vLLM release history containing SM12x-specific MoE and FP4 work:
  <https://github.com/vllm-project/vllm/releases>

## Existing local evidence to consult, not modify

From `../glm52-opt`:

- `RESULTS.md`
- `MEASUREMENT-LIBRARY.md`
- `design/breakthrough-analysis.md`
- `design/cn4-tr3-qualification-20260728.md`
- `design/v20-nvfp4-scaling-kld-n3-comparison-20260728.md`
- `v20-latest-head-profiler-fable-handoff.md`
- `harness/run_v20_quality_weight_kld.sh`
- `harness/run_glm52_tr3_dynamic_kld.sh`

Important starting facts from that evidence:

- packed CKV removed most DCP prefill transport and placed the test profile
  near its DCP1 ceiling;
- the remaining per-layer compute split still needs accepted exclusive
  profiling;
- quality is sensitive to precision membership, not only nominal bits;
- dynamic per-token NVFP4 KV scaling recovered deep-context failures;
- EXL3/Trellis greatly increased KV capacity but must earn its decode/prefill
  cost on the exact target;
- cold versus warm cache accounting is mandatory.
