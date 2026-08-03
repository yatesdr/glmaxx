# GLM-5.2 manifest source audit

Date: 2026-07-29

Status: supporting evidence for independent manifest review; this record does
not close the review gate or authorize a CUDA launch.

## Immutable inputs

| Input | Identity |
|---|---|
| model | `zai-org/GLM-5.2` |
| model revision | `b4734de4facf877f85769a911abafc5283eab3d9` |
| exact-revision `config.json` SHA-256 | `185f93ee6d12548e16a847e279dc0c3c90b1524c970b0866b42fb545747d859a` |
| production image | `voipmonitor/vllm@sha256:10261c7d65101c8aba2ce1fb59eabe73aff9d35eca5043b330cc0ce76d3c98d0` |
| installed Transformers modeling source SHA-256 | `adb8317a21716b01273046e46c807f14f0dbaf035af59b60d52bd6bc3007cf72` |
| installed Transformers configuration source SHA-256 | `5a81164be746307431ad998f789b6b0bca20eb4c14a726552eb3730268413997` |
| exact official Transformers commit | `5204b4fe36956e9214b9279f1e1be2fd5dd1d9f3` |
| read-only `../glm52-opt` HEAD | `d213925ee6701072f117aec59ca94f1bf00d5e7f` |
| pinned `deepseek_mtp.py` SHA-256 | `3a8a0b30e5dc5eb8c1f0ddb2ce317c375dc094de5b5ba8ba78f71d5481deae6d` |
| pinned NVIDIA V32 `mtp.py` SHA-256 | `8e09e33823d4a6feb5071eb4ef3a5822bf79c1fab7ab59b9e5220be67b5571ca` |

The exact model configuration was fetched without weights from:

```text
https://huggingface.co/zai-org/GLM-5.2/resolve/b4734de4facf877f85769a911abafc5283eab3d9/config.json
```

The two installed Transformers hashes were reproduced inside the pinned
production image, not inferred from a public release tag. The MTP hashes were
reproduced from these read-only files:

```text
../glm52-opt/workspace/vllm-v20-indexer-wht-prototype/vllm/model_executor/models/deepseek_mtp.py
../glm52-opt/workspace/vllm-v20-indexer-wht-prototype/vllm/models/deepseek_v32/nvidia/mtp.py
```

No file in `../glm52-opt` was modified.

The official upstream source identity was independently resolved after the
cn4 release. Walking the official `huggingface/transformers` history for
`modeling_glm_moe_dsa.py` found commit
`5204b4fe36956e9214b9279f1e1be2fd5dd1d9f3`; at that commit the modeling and
configuration files hash exactly to the two installed-source hashes above.
The public `v5.12.0` tag's modeling file instead hashes to
`e62d5eec32e96fd3441db67ef7f595f3d37af6feace5eccc4e26e13fa6ef17dc`
and uses a different indexer RoPE path, so that tag is not an acceptable
substitute for the pinned bytes.

## Configuration facts

Independent parsing of the exact-revision configuration produced:

| Field | Value |
|---|---:|
| `hidden_size` | 6,144 |
| `moe_intermediate_size` | 2,048 |
| `n_routed_experts` | 256 |
| `num_experts_per_tok` | 8 |
| `num_hidden_layers` | 78 |
| `num_nextn_predict_layers` | 1 |
| `index_topk` | 2,048 |
| `vocab_size` | 154,880 |
| `rms_norm_eps` | `1e-5` |
| `rope_parameters.rope_type` | `default` |
| `rope_parameters.rope_theta` | 8,000,000 |

The exact config has no yarn factor, `mscale`, or `mscale_all_dim`; the
default RoPE path therefore does not modify the `256^-0.5` attention scale.
On 2026-08-03 the official raw config was fetched with redirects from the
exact model revision URL and independently hashed to the pinned
`185f93ee6d12548e16a847e279dc0c3c90b1524c970b0866b42fb545747d859a`.

`indexer_types` has 78 entries: 21 `full` and 57 `shared`. The exact `full`
layer IDs are:

```text
0, 1, 2, 6, 10, 14, 18, 22, 26, 30, 34, 38, 42, 46, 50, 54, 58, 62, 66, 70, 74
```

This exactly matches `FULL_INDEXER_LAYERS` in the generated operation
manifest. Forming each group from a `full` layer through the layer immediately
before the next `full` layer covers every target layer `0..77` exactly once.

## Operation facts reproduced from pinned source

The installed GLM modeling source and pinned vLLM sources establish the
following graph facts used by `manifests/glm52-operation-v1.json`:

1. The router performs its linear operation in FP32, applies sigmoid,
   correction bias and group-limited selection, selects eight experts,
   normalizes the selected weights, and applies routed scaling factor 2.5.
2. Expert gate and up weights are represented together as
   `[experts, 2 * intermediate, hidden]`; their outputs are split into gate and
   up, combined as `SiLU(gate) * up`, and passed through the down projection.
   Route weights multiply the down-projection output before token accumulation.
3. A sparse layer contains both routed and shared expert paths. Their output
   is combined before the layer residual boundary.
4. The draft checkpoint starts at `num_hidden_layers`, hence checkpoint layer
   78, and `num_nextn_predict_layers = 1` means deeper MTP uses that one layer
   recurrently rather than loading independent layers.
5. Each draft recurrence masks the token embedding at logical position zero,
   applies `enorm` to the embedding and `hnorm` to the previous hidden state,
   concatenates them, applies `eh_proj`, then executes an attention-plus-MoE
   decoder block.
6. The general pinned path forms the pre-final-norm hidden as residual plus
   block output, applies the shared final RMSNorm once, uses that normalized
   hidden as the recycled state, and applies the shared vocabulary head for
   draft logits. The pinned NVIDIA V32 path implements the equivalent
   post-final-norm recycle and head-only logits convention.
7. The pinned prototype implements `index_share_for_mtp_iteration`: recurrence
   zero computes the sparse-attention top-k list and later recurrences reuse
   it. Its compaction path preserves top-k rows for the surviving draft slots.
8. A full target indexer projects 32 128-value queries from the Q residual,
   projects and epsilon-`1e-6` LayerNorms one 128-value key, applies
   interleaved RoPE to the first 64 query/key values, computes FP32 dot
   products scaled by `128^-0.5`, applies ReLU, weights the 32 heads with the
   protected projection scaled by `32^-0.5`, then selects the causal top
   2,048 positions.
9. Main MLA splits each local query head into 192 NoPE and 64 RoPE values,
   expands the 512-value normalized latent through the head-specific KV-B
   weight into 192 key and 256 value values, applies interleaved RoPE, uses
   the 256-value total QK dimension's inverse-square-root scale, and applies
   the row-parallel output projection.
10. Each decoder layer adds the attention result to the pre-norm residual,
    applies post-attention RMSNorm, adds the dense or routed-plus-shared MLP
    result to that second residual, and returns the full-indexer's winner list
    for the following shared layers.

The source audit supports, but deliberately does not independently accept,
the manifest's stable route-compaction order, exact TP collective placement,
or durable draft/indexer publication rules. Those are contract choices and
remain in the scope of the adversarial review.

## Reproduction commands

The audit used hash-only or metadata-only commands. On cn4:

```bash
sha256sum /home/derek/glmaxx/deps/glm52-source/config.json
docker run --rm --entrypoint /bin/bash \
  voipmonitor/vllm@sha256:10261c7d65101c8aba2ce1fb59eabe73aff9d35eca5043b330cc0ce76d3c98d0 \
  -lc 'sha256sum /opt/venv/lib/python3.12/site-packages/transformers/models/glm_moe_dsa/modeling_glm_moe_dsa.py /opt/venv/lib/python3.12/site-packages/transformers/models/glm_moe_dsa/configuration_glm_moe_dsa.py'
```

On the development host:

```bash
git -C ../glm52-opt rev-parse HEAD
shasum -a 256 \
  ../glm52-opt/workspace/vllm-v20-indexer-wht-prototype/vllm/model_executor/models/deepseek_mtp.py \
  ../glm52-opt/workspace/vllm-v20-indexer-wht-prototype/vllm/models/deepseek_v32/nvidia/mtp.py
```

The cn4 operations did not request a GPU and did not launch CUDA.

The later upstream provenance check ran on the development host and did not
connect to cn4:

```bash
curl -fsSL \
  https://raw.githubusercontent.com/huggingface/transformers/5204b4fe36956e9214b9279f1e1be2fd5dd1d9f3/src/transformers/models/glm_moe_dsa/modeling_glm_moe_dsa.py \
  | shasum -a 256
curl -fsSL \
  https://raw.githubusercontent.com/huggingface/transformers/5204b4fe36956e9214b9279f1e1be2fd5dd1d9f3/src/transformers/models/glm_moe_dsa/configuration_glm_moe_dsa.py \
  | shasum -a 256
```
