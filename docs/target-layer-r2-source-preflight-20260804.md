# Target-layer r2 immutable-source preflight

Date: 2026-08-04

Status: independent source/arithmetic preflight; adversarial acceptance and
implementation remain pending

Integration source at execution:
`1f4fbf8b2b26390def5d45c9608de977607ae70e`

Target contract candidate:
`d4817ff9ff7eec09c74e98a99db5c27690286013`

## Scope

This CPU-only check independently revalidated the immutable external source
facts and closed-form byte arithmetic required by
`docs/fable-target-layer-execution-v1-r2-handoff.md`. It fetched no model
weights, accessed no checkpoint, connected to no GPU host, and made no CUDA,
implementation, model-correctness, quality, capacity, or performance claim.

The repository's `review-proof` command verified all 21 candidate inputs and
the exact candidate commit. The required Fable result remains absent; this
record is not a substitute for that review.

## Immutable upstream bytes

The following exact-revision files were downloaded with `curl -fL` and hashed
with `shasum -a 256`:

| Source | SHA-256 |
|---|---|
| GLM-5.2 `config.json` at `b4734de4facf877f85769a911abafc5283eab3d9` | `185f93ee6d12548e16a847e279dc0c3c90b1524c970b0866b42fb545747d859a` |
| Transformers `modeling_glm_moe_dsa.py` at `5204b4fe36956e9214b9279f1e1be2fd5dd1d9f3` | `adb8317a21716b01273046e46c807f14f0dbaf035af59b60d52bd6bc3007cf72` |
| Transformers `configuration_glm_moe_dsa.py` at `5204b4fe36956e9214b9279f1e1be2fd5dd1d9f3` | `5a81164be746307431ad998f789b6b0bca20eb4c14a726552eb3730268413997` |

The config extraction was:

```text
vocab_size          154880
rms_norm_eps        1e-5
rope_type           default
rope_theta          8000000
moe_router_dtype    float32
transformers_version 5.12.0
```

The pinned model source constructs Q-A and KV-A `GlmMoeDsaRMSNorm` without an
epsilon argument, selecting its `1e-6` default. It constructs indexer K
`LayerNorm` with explicit `eps=1e-6`. Input, post-attention, and final norms
receive the config's `1e-5`. The RoPE helper rotates interleaved even/odd input
pairs and concatenates all first rotated components before all second rotated
components, corroborating the contract's de-interleaved stored order.

## Independent arithmetic

The operation manifest contains 75 sparse target layers (`3..77`). Among
them, 18 are full-indexer layers (`6,10,...,74`) and 57 reuse shared winners;
layers `0..2` are dense. The target-program binding count is therefore:

```text
embedding                         1
final head                        2
3 dense layers * 17              51
18 full sparse layers * 531      9558
57 shared sparse layers * 526    29982
total                             39594
```

The fixed table ceilings independently reduce to:

```text
prefill: 3072 * (48 + 40) + 64 * 48 = 273408 bytes
C1 decode:       1 * (48 + 40) +  1 * 48 =    136 bytes
verifier:      448 * (48 + 40) + 64 * 48 =  42496 bytes
```

The `StepInput.v3` prefix is also exactly 430 bytes:

```text
32 + 8 + 32 + 4*32 + 4*32 + 32 + 32 + 32 + 2 + 4 = 430
```

No discrepancy was found in this bounded preflight. CPU implementation remains
closed until the exact target-layer r2 review token and its independently
required sampling/step dependencies are accepted.
