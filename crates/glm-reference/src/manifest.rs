use serde::Serialize;

pub const FULL_INDEXER_LAYERS: [u16; 21] = [
    0, 1, 2, 6, 10, 14, 18, 22, 26, 30, 34, 38, 42, 46, 50, 54, 58, 62, 66, 70, 74,
];

#[derive(Clone, Copy, Debug, Serialize)]
pub struct ModelConstants {
    pub target_layers: u16,
    pub sparse_layers: u16,
    pub hidden: u32,
    pub expert_intermediate: u32,
    pub routed_experts: u16,
    pub top_k: u8,
    pub tp: u8,
    pub local_intermediate: u32,
    pub local_gate_up_rows: u32,
}

impl Default for ModelConstants {
    fn default() -> Self {
        Self {
            target_layers: 78,
            sparse_layers: 75,
            hidden: 6144,
            expert_intermediate: 2048,
            routed_experts: 256,
            top_k: 8,
            tp: 4,
            local_intermediate: 512,
            local_gate_up_rows: 1024,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SourcePin {
    pub name: &'static str,
    pub identity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct PackedTensorContract {
    pub role_id: u16,
    pub role: &'static str,
    pub source_names: Vec<&'static str>,
    pub source_shape: Vec<u32>,
    pub source_tp_axis: i8,
    pub rank_shape: Vec<u32>,
    pub packed_order: &'static str,
    pub codec: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct OperationStep {
    pub ordinal: u8,
    pub operation: &'static str,
    pub arithmetic: &'static str,
    pub output: &'static str,
    pub collective_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct IndexerGroup {
    pub group: u8,
    pub full_layer: u16,
    pub consumers: Vec<u16>,
    pub key_record: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct MtpContract {
    pub checkpoint_layer_id: u16,
    pub independent_layer_count: u8,
    pub maximum_recurrent_depth: u8,
    pub recurrence: Vec<&'static str>,
    pub index_selection: &'static str,
    pub committed_state: Vec<&'static str>,
    pub draft_kv_record: &'static str,
    pub draft_indexer_record: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct OperationManifest {
    pub schema: &'static str,
    pub model: &'static str,
    pub model_revision: &'static str,
    pub constants: ModelConstants,
    pub source_pins: Vec<SourcePin>,
    pub sparse_layer_ids: Vec<u16>,
    pub tensor_contracts: Vec<PackedTensorContract>,
    pub routed_fc1_steps: Vec<OperationStep>,
    pub indexer_groups: Vec<IndexerGroup>,
    pub mtp_contract: MtpContract,
    pub invariants: Vec<&'static str>,
}

#[must_use]
pub fn operation_manifest() -> OperationManifest {
    let mut indexer_groups = Vec::with_capacity(FULL_INDEXER_LAYERS.len());
    for (group, &full_layer) in FULL_INDEXER_LAYERS.iter().enumerate() {
        let next = FULL_INDEXER_LAYERS.get(group + 1).copied().unwrap_or(78);
        indexer_groups.push(IndexerGroup {
            group: u8::try_from(group).unwrap(),
            full_layer,
            consumers: (full_layer..next).collect(),
            key_record: "glm52_dsa_index_k:e4m3-128:fp32-ue8m0-scale:v1",
        });
    }
    OperationManifest {
        schema: "glmaxx.glm52.operation.v1",
        model: "zai-org/GLM-5.2",
        model_revision: "b4734de4facf877f85769a911abafc5283eab3d9",
        constants: ModelConstants::default(),
        source_pins: vec![
            SourcePin {
                name: "transformers-modeling-sha256",
                identity: "adb8317a21716b01273046e46c807f14f0dbaf035af59b60d52bd6bc3007cf72",
            },
            SourcePin {
                name: "transformers-configuration-sha256",
                identity: "5a81164be746307431ad998f789b6b0bca20eb4c14a726552eb3730268413997",
            },
            SourcePin {
                name: "glm52-opt-read-only-head",
                identity: "d213925ee6701072f117aec59ca94f1bf00d5e7f",
            },
            SourcePin {
                name: "deepseek-mtp-pinned-source-sha256",
                identity: "3a8a0b30e5dc5eb8c1f0ddb2ce317c375dc094de5b5ba8ba78f71d5481deae6d",
            },
            SourcePin {
                name: "deepseek-v32-nvidia-mtp-pinned-source-sha256",
                identity: "8e09e33823d4a6feb5071eb4ef3a5822bf79c1fab7ab59b9e5220be67b5571ca",
            },
            SourcePin {
                name: "cutlass",
                identity: "e05f953a5b3d38adc240df2ff928e0421c2abba3",
            },
        ],
        sparse_layer_ids: (3..78).collect(),
        tensor_contracts: vec![
            PackedTensorContract {
                role_id: 0x0501,
                role: "routed_expert_gate_up",
                source_names: vec![
                    "model.layers.{3..77}.mlp.experts.{0..255}.gate_proj.weight",
                    "model.layers.{3..77}.mlp.experts.{0..255}.up_proj.weight",
                ],
                source_shape: vec![2048, 6144],
                source_tp_axis: 0,
                rank_shape: vec![1024, 6144],
                packed_order: "rank-local gate[512,K], then rank-local up[512,K]",
                codec: "sm120-nvfp4-1d-block16-direct-v1",
            },
            PackedTensorContract {
                role_id: 0x0502,
                role: "routed_expert_down",
                source_names: vec!["model.layers.{3..77}.mlp.experts.{0..255}.down_proj.weight"],
                source_shape: vec![6144, 2048],
                source_tp_axis: 1,
                rank_shape: vec![6144, 512],
                packed_order: "rank-local K slice, output rows unsharded",
                codec: "sm120-nvfp4-1d-block16-direct-v1",
            },
            PackedTensorContract {
                role_id: 0x0301,
                role: "router_weight",
                source_names: vec!["model.layers.{3..77}.mlp.gate.weight"],
                source_shape: vec![256, 6144],
                source_tp_axis: -1,
                rank_shape: vec![256, 6144],
                packed_order: "replicated protected FP32 arithmetic",
                codec: "protected-source-precision",
            },
            PackedTensorContract {
                role_id: 0x0302,
                role: "router_correction_bias",
                source_names: vec!["model.layers.{3..77}.mlp.gate.e_score_correction_bias"],
                source_shape: vec![256],
                source_tp_axis: -1,
                rank_shape: vec![256],
                packed_order: "replicated protected FP32 arithmetic",
                codec: "protected-source-precision",
            },
        ],
        routed_fc1_steps: vec![
            OperationStep {
                ordinal: 0,
                operation: "router",
                arithmetic: "FP32 linear, sigmoid, group-limited top-8, normalized weights ×2.5",
                output: "identical expert IDs and weights on all TP ranks",
                collective_boundary: "none; replicated protected router",
            },
            OperationStep {
                ordinal: 1,
                operation: "stable route compaction",
                arithmetic: "expert ascending, then token ascending, then route slot ascending",
                output: "group offsets and compacted BF16 rows",
                collective_boundary: "none; descriptors must hash-identically",
            },
            OperationStep {
                ordinal: 2,
                operation: "dynamic activation quantization",
                arithmetic: "per compacted row FP32 amax, NVFP4 block-16 E4M3 scales",
                output: "one packed A row reused for gate and up",
                collective_boundary: "none",
            },
            OperationStep {
                ordinal: 3,
                operation: "rank-local routed FC1",
                arithmetic: "NVFP4×NVFP4 MMA, FP32 accumulate",
                output: "gate[assignments,512] and up[assignments,512]",
                collective_boundary: "none",
            },
            OperationStep {
                ordinal: 4,
                operation: "SwiGLU",
                arithmetic: "FP32 SiLU(gate) × up, then BF16 store",
                output: "rank-local FC2 input[assignments,512]",
                collective_boundary: "none",
            },
            OperationStep {
                ordinal: 5,
                operation: "rank-local routed FC2 and weighted scatter",
                arithmetic: "FP32 accumulate; route weight after down projection",
                output: "partial hidden[M,6144]",
                collective_boundary: "one TP4 all-reduce after routed and shared partials are combined",
            },
        ],
        indexer_groups,
        mtp_contract: MtpContract {
            checkpoint_layer_id: 78,
            independent_layer_count: 1,
            maximum_recurrent_depth: 6,
            recurrence: vec![
                "embed the current input token; replace its embedding with zero at logical position 0",
                "RMSNorm the embedding with enorm and the prior hidden state with hnorm",
                "concatenate [normalized embedding, normalized prior hidden] and apply eh_proj[6144,12288]",
                "execute checkpoint layer 78 MLA attention, routed/shared MoE, residual, and TP reduction",
                "form pre_final = residual + block_output and recycled_hidden = shared_head.RMSNorm(pre_final)",
                "compute draft logits as shared vocabulary head(recycled_hidden)",
                "for the next recurrence use the sampled token embedding, logical position + 1, and recycled_hidden",
            ],
            index_selection: "recurrence 0 computes an exact layer-78 top-2048 list; recurrences 1..5 reuse that transient list, but every committed position retains its own layer-78 indexer key",
            committed_state: vec![
                "one 368-byte layer-78 KV record per committed position",
                "one 132-byte layer-78 indexer-key record per committed position",
                "target, target-indexer, draft-KV, and draft-indexer generations publish atomically for MTP-capable prefixes",
                "rejected speculative KV and indexer-key writes become unreachable before slot reuse",
            ],
            draft_kv_record: "nvfp4_ds_mla:fp8-rope-368:dynamic-token-v1",
            draft_indexer_record: "glm52_dsa_index_k:e4m3-128:fp32-ue8m0-scale:v1",
        },
        invariants: vec![
            "all TP ranks use identical routes and group offsets",
            "no rank-local collective fallback",
            "gate and up shards split each source projection independently before concatenation",
            "route weight is applied after down projection",
            "shared expert output is added before the sparse-layer residual",
            "indexer key pages share owner and generation with target KV pages",
            "MTP depth reuses one checkpoint layer rather than loading independent layers",
            "an MTP-capable committed draft position has both KV and indexer-key records",
        ],
    }
}

pub fn operation_manifest_json() -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec_pretty(&operation_manifest())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexer_pattern_covers_every_target_layer_once() {
        let manifest = operation_manifest();
        let flattened: Vec<u16> = manifest
            .indexer_groups
            .iter()
            .flat_map(|group| group.consumers.iter().copied())
            .collect();
        assert_eq!(flattened, (0..78).collect::<Vec<_>>());
        assert_eq!(manifest.sparse_layer_ids, (3..78).collect::<Vec<_>>());
    }

    #[test]
    fn first_kernel_tp4_geometry_is_exact() {
        let constants = ModelConstants::default();
        assert_eq!(
            constants.expert_intermediate / u32::from(constants.tp),
            constants.local_intermediate
        );
        assert_eq!(
            constants.local_gate_up_rows,
            2 * constants.local_intermediate
        );
    }

    #[test]
    fn mtp_recurrence_and_residency_are_frozen() {
        let contract = operation_manifest().mtp_contract;
        assert_eq!(contract.checkpoint_layer_id, 78);
        assert_eq!(contract.independent_layer_count, 1);
        assert_eq!(contract.maximum_recurrent_depth, 6);
        assert_eq!(contract.recurrence.len(), 7);
        assert_eq!(contract.committed_state.len(), 4);
    }
}
