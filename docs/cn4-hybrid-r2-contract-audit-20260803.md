# cn4 hybrid r2 contract audit

Date: 2026-08-03

Status: read-only source/header evidence; no checkpoint admission or GPU claim

## Scope

The audit used password-only SSH to cn4 and read only `config.json`, the
safetensors index, selected safetensors headers, and previously hash-pinned
source files. It did not create a CUDA context, run `nvidia-smi`, read tensor
value planes, write a remote file, or start/stop a process.

Checkpoint root:
`/home/claude/LLM/GLM-5.2-hybrid`.

## Revalidated source identity

```text
config SHA-256       254974797e9f455716a30ab5505ba68272181b20b58a3693e54f94fb8056f3ef
index SHA-256        6eb773222d932418dd0530c63aca498f86ef424da2a4526ccba76b59726da234
index total_size     365,968,736,768
index tensor names   148,289
```

The config has `quant_method=modelopt`, `quant_algo=NVFP4`, producer version
`0.39.0.dev290+gf9d9a71de.d20260214`, and exactly 75 hybrid-map layers 3
through 77. The map contains 14,400 values equal to 3 and 4,800 equal to 4;
layer 78 is absent. Entry `hybrid_bit_map["9"][99]` is 3.

Representative metadata-only headers independently reproduced:

- layer 3 expert 0 NF3 gate/up U8 `[2048,2304]`, E4M3 `[2048,192]`, and down
  U8 `[6144,768]`, E4M3 `[6144,64]`;
- layer 3 expert 6 ModelOpt-NVFP4 gate/up U8 `[2048,3072]`, E4M3
  `[2048,384]`, and down U8 `[6144,1024]`, E4M3 `[6144,128]`, with scalar F32
  `weight_scale_2` and `input_scale` for each projection; and
- layer 78 expert 0 exposes that same four-component ModelOpt family.

No scalar payload value was needed or inspected: independent tensor identities
already require the format to preserve gate and up scalars separately.

## W4A16 versus the existing GLMAXX control

The previously pinned adapter source has SHA-256
`a8b4e19c5e776ece1d6c7ff2c48da236d1bd4032a3399f7b6a9563955c99f61b`.
It explicitly describes the hybrid route as W4A16, discards checkpoint
`input_scale` for that path, allocates `w13_weight_scale_2` as
`[num_kept,2]`, and loads gate and up into separate columns. Its current
preparation call selects only column 0 as `g13`, so that staging code cannot
serve as proof that a one-global-scale fused representation preserves both
source projections.

The pinned SparkInfer preparation source SHA-256 is
`b54175a861730662350a2ef5ee63989c8afafc907b6a3c13a3331928cfb9285f`.
It defines a W4A16 representation with one `w13_global_scale` per expert and
the exact NF3 32-code fragment mapping later made normative in the r2 GLMAXX
contract.

By contrast, the current GLMAXX CUTLASS control declares both operands as
E2M1/NVFP4 and dynamically quantizes BF16 activations before MMA. Its
`expert_global_scales` path supplies one scale per expert to both FC1 halves.
That is useful W4A4 diagnostic evidence but is not a W4A16 checkpoint-quality
control and cannot validate two source outer scalars.

## Consequence

The real checkpoint requires a distinct ModelOpt W4A16 codec/metadata/ABI or
an explicitly quality-gated re-quantization. The r2 design chooses the direct
codec: standard source E2M1/E4M3 planes, two FC1 outer scalars, one FC2 outer
scalar, BF16 activations, and no use of `input_scale`. A later W4A4 candidate
must have a different numerical-policy and graph identity.

This evidence accepts no converter, native image, kernel, checkpoint, quality,
capacity, or speed result.
