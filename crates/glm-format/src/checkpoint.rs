use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use crate::{
    EXL3_MODEL_REVISION, Exl3Projection, SafeDtype, SafeTensorDescriptor, ShardedSafetensors,
};

pub const PINNED_EXL3_REPOSITORY: &str = "brandonmusic/GLM-5.2-EXL3-TR3-3.0bpw";
pub const PINNED_EXL3_INDEX_SHA256: [u8; 32] = [
    0x34, 0x62, 0x27, 0xa4, 0xea, 0x44, 0xb6, 0x06, 0x30, 0x17, 0x73, 0x9e, 0xe3, 0x8a, 0x83, 0x03,
    0x19, 0xdc, 0x10, 0x30, 0x5c, 0xcf, 0x71, 0x47, 0x34, 0x09, 0x5e, 0x27, 0xb2, 0x80, 0x64, 0xc2,
];
pub const PINNED_EXL3_TENSOR_COUNT: usize = 935_105;
pub const PINNED_EXL3_SHARD_COUNT: usize = 81;
pub const PINNED_EXL3_PAYLOAD_BYTES: u64 = 316_304_795_648;
pub const PINNED_EXL3_COMPONENT_COUNT: usize = 933_888;
pub const PINNED_PROTECTED_TENSOR_COUNT: usize = 1_217;
pub const TP_DEGREE: u8 = 4;

pub const ROLE_EMBEDDING: u16 = 0x0001;
pub const ROLE_LM_HEAD: u16 = 0x0002;
pub const ROLE_FINAL_NORM: u16 = 0x0003;
pub const ROLE_Q_A_PROJ: u16 = 0x0101;
pub const ROLE_Q_A_NORM: u16 = 0x0102;
pub const ROLE_Q_B_PROJ: u16 = 0x0103;
pub const ROLE_KV_A_PROJ: u16 = 0x0104;
pub const ROLE_KV_A_NORM: u16 = 0x0105;
pub const ROLE_KV_B_PROJ: u16 = 0x0106;
pub const ROLE_O_PROJ: u16 = 0x0107;
pub const ROLE_INDEXER_WQ_B: u16 = 0x0201;
pub const ROLE_INDEXER_WK: u16 = 0x0202;
pub const ROLE_INDEXER_WEIGHTS: u16 = 0x0203;
pub const ROLE_INDEXER_K_NORM_WEIGHT: u16 = 0x0204;
pub const ROLE_INDEXER_K_NORM_BIAS: u16 = 0x0205;
pub const ROLE_ROUTER_WEIGHT: u16 = 0x0301;
pub const ROLE_ROUTER_CORRECTION: u16 = 0x0302;
pub const ROLE_DENSE_GATE: u16 = 0x0401;
pub const ROLE_DENSE_UP: u16 = 0x0402;
pub const ROLE_DENSE_DOWN: u16 = 0x0403;
pub const ROLE_ROUTED_GATE_UP: u16 = 0x0501;
pub const ROLE_ROUTED_DOWN: u16 = 0x0502;
pub const ROLE_SHARED_GATE: u16 = 0x0601;
pub const ROLE_SHARED_UP: u16 = 0x0602;
pub const ROLE_SHARED_DOWN: u16 = 0x0603;
pub const ROLE_INPUT_NORM: u16 = 0x0701;
pub const ROLE_POST_ATTENTION_NORM: u16 = 0x0702;
pub const ROLE_MTP_ENORM: u16 = 0x0801;
pub const ROLE_MTP_HNORM: u16 = 0x0802;
pub const ROLE_MTP_EH_PROJ: u16 = 0x0803;
pub const ROLE_MTP_SHARED_HEAD_NORM: u16 = 0x0804;

const FULL_INDEXER_LAYERS: [u16; 22] = [
    0, 1, 2, 6, 10, 14, 18, 22, 26, 30, 34, 38, 42, 46, 50, 54, 58, 62, 66, 70, 74, 78,
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Exl3Component {
    Mcg,
    Suh,
    Svh,
    Trellis,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Exl3ComponentContract {
    pub layer: u16,
    pub expert: u16,
    pub rank: u8,
    pub projection: Exl3Projection,
    pub component: Exl3Component,
}

impl Exl3ComponentContract {
    #[must_use]
    pub const fn role_id(&self) -> u16 {
        match self.projection {
            Exl3Projection::Gate | Exl3Projection::Up => ROLE_ROUTED_GATE_UP,
            Exl3Projection::Down => ROLE_ROUTED_DOWN,
        }
    }

    #[must_use]
    pub const fn is_mtp(&self) -> bool {
        self.layer == 78
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedTensorContract {
    pub name: String,
    pub role_id: u16,
    pub layer_id: i16,
    pub dtype: SafeDtype,
    pub source_shape: Vec<u64>,
    /// `-1` is replicated; otherwise this is the source row-major TP axis.
    pub tp_axis: i8,
    pub rank_shape: Vec<u64>,
    pub is_mtp: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckpointTensorContract {
    Exl3(Exl3ComponentContract),
    Protected(ProtectedTensorContract),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckpointInventoryReport {
    pub structure_sha256: [u8; 32],
    pub tensor_count: usize,
    pub shard_count: usize,
    pub payload_bytes: u64,
    pub exl3_component_count: usize,
    pub protected_tensor_count: usize,
}

pub fn validate_pinned_exl3_checkpoint(
    checkpoint: &ShardedSafetensors,
    claimed_revision: &str,
) -> Result<CheckpointInventoryReport, CheckpointError> {
    if claimed_revision != EXL3_MODEL_REVISION {
        return Err(CheckpointError::Revision(claimed_revision.to_owned()));
    }
    let is_index = checkpoint
        .source_path()
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".safetensors.index.json"));
    if is_index && checkpoint.structure_sha256() != PINNED_EXL3_INDEX_SHA256 {
        return Err(CheckpointError::IndexIdentity);
    }
    if checkpoint.tensor_names().len() != PINNED_EXL3_TENSOR_COUNT {
        return Err(CheckpointError::TensorCount(
            checkpoint.tensor_names().len(),
        ));
    }
    validate_shard_names(checkpoint.shards())?;

    let mut protected = protected_tensor_contracts();
    if protected.len() != PINNED_PROTECTED_TENSOR_COUNT {
        return Err(CheckpointError::Internal);
    }
    let mut exl3_seen = vec![false; PINNED_EXL3_COMPONENT_COUNT];
    let mut exl3_count = 0_usize;
    let mut protected_count = 0_usize;
    let mut payload_bytes = 0_u64;
    let mut dtype_bytes = BTreeMap::new();

    for name in checkpoint.tensor_names() {
        let descriptor = checkpoint.tensor(name).ok_or(CheckpointError::Internal)?;
        payload_bytes = payload_bytes
            .checked_add(descriptor.bytes)
            .ok_or(CheckpointError::Overflow)?;
        *dtype_bytes.entry(descriptor.dtype).or_insert(0_u64) = dtype_bytes
            .get(&descriptor.dtype)
            .copied()
            .unwrap_or(0)
            .checked_add(descriptor.bytes)
            .ok_or(CheckpointError::Overflow)?;

        if let Some(contract) = protected.remove(name) {
            validate_protected_descriptor(&contract, descriptor)?;
            protected_count += 1;
            continue;
        }
        let contract =
            parse_exl3_component(name).ok_or_else(|| CheckpointError::Unknown(name.to_owned()))?;
        validate_exl3_descriptor(name, &contract, descriptor)?;
        let index = exl3_component_index(&contract);
        if exl3_seen[index] {
            return Err(CheckpointError::Duplicate(name.to_owned()));
        }
        exl3_seen[index] = true;
        exl3_count += 1;
    }

    if let Some(name) = protected.into_keys().next() {
        return Err(CheckpointError::Missing(name));
    }
    if let Some(index) = exl3_seen.iter().position(|seen| !seen) {
        return Err(CheckpointError::Missing(exl3_component_name(index)));
    }
    if exl3_count != PINNED_EXL3_COMPONENT_COUNT
        || protected_count != PINNED_PROTECTED_TENSOR_COUNT
        || payload_bytes != PINNED_EXL3_PAYLOAD_BYTES
        || dtype_bytes.get(&SafeDtype::Bf16) != Some(&37_781_026_816)
        || dtype_bytes.get(&SafeDtype::F32) != Some(&77_824)
        || dtype_bytes.get(&SafeDtype::F16) != Some(&3_107_979_264)
        || dtype_bytes.get(&SafeDtype::I16) != Some(&275_414_777_856)
        || dtype_bytes.get(&SafeDtype::I32) != Some(&933_888)
        || dtype_bytes.len() != 5
    {
        return Err(CheckpointError::ByteInventory);
    }

    Ok(CheckpointInventoryReport {
        structure_sha256: checkpoint.structure_sha256(),
        tensor_count: PINNED_EXL3_TENSOR_COUNT,
        shard_count: PINNED_EXL3_SHARD_COUNT,
        payload_bytes,
        exl3_component_count: exl3_count,
        protected_tensor_count: protected_count,
    })
}

#[must_use]
pub fn protected_tensor_contracts() -> BTreeMap<String, ProtectedTensorContract> {
    let mut contracts = BTreeMap::new();
    add_protected(
        &mut contracts,
        "model.embed_tokens.weight",
        ROLE_EMBEDDING,
        -1,
        SafeDtype::Bf16,
        &[154_880, 6_144],
        0,
        false,
    );
    add_protected(
        &mut contracts,
        "lm_head.weight",
        ROLE_LM_HEAD,
        -1,
        SafeDtype::Bf16,
        &[154_880, 6_144],
        0,
        false,
    );
    add_protected(
        &mut contracts,
        "model.norm.weight",
        ROLE_FINAL_NORM,
        -1,
        SafeDtype::Bf16,
        &[6_144],
        -1,
        false,
    );

    for layer in 0_u16..=78 {
        let mtp = layer == 78;
        add_layer(
            &mut contracts,
            layer,
            "input_layernorm.weight",
            ROLE_INPUT_NORM,
            SafeDtype::Bf16,
            &[6_144],
            -1,
            mtp,
        );
        add_layer(
            &mut contracts,
            layer,
            "post_attention_layernorm.weight",
            ROLE_POST_ATTENTION_NORM,
            SafeDtype::Bf16,
            &[6_144],
            -1,
            mtp,
        );
        add_layer(
            &mut contracts,
            layer,
            "self_attn.q_a_proj.weight",
            ROLE_Q_A_PROJ,
            SafeDtype::Bf16,
            &[2_048, 6_144],
            -1,
            mtp,
        );
        add_layer(
            &mut contracts,
            layer,
            "self_attn.q_a_layernorm.weight",
            ROLE_Q_A_NORM,
            SafeDtype::Bf16,
            &[2_048],
            -1,
            mtp,
        );
        add_layer(
            &mut contracts,
            layer,
            "self_attn.q_b_proj.weight",
            ROLE_Q_B_PROJ,
            SafeDtype::Bf16,
            &[16_384, 2_048],
            0,
            mtp,
        );
        add_layer(
            &mut contracts,
            layer,
            "self_attn.kv_a_proj_with_mqa.weight",
            ROLE_KV_A_PROJ,
            SafeDtype::Bf16,
            &[576, 6_144],
            -1,
            mtp,
        );
        add_layer(
            &mut contracts,
            layer,
            "self_attn.kv_a_layernorm.weight",
            ROLE_KV_A_NORM,
            SafeDtype::Bf16,
            &[512],
            -1,
            mtp,
        );
        add_layer(
            &mut contracts,
            layer,
            "self_attn.kv_b_proj.weight",
            ROLE_KV_B_PROJ,
            SafeDtype::Bf16,
            &[28_672, 512],
            0,
            mtp,
        );
        add_layer(
            &mut contracts,
            layer,
            "self_attn.o_proj.weight",
            ROLE_O_PROJ,
            SafeDtype::Bf16,
            &[6_144, 16_384],
            1,
            mtp,
        );

        if FULL_INDEXER_LAYERS.contains(&layer) {
            add_layer(
                &mut contracts,
                layer,
                "self_attn.indexer.wq_b.weight",
                ROLE_INDEXER_WQ_B,
                SafeDtype::Bf16,
                &[4_096, 2_048],
                -1,
                mtp,
            );
            add_layer(
                &mut contracts,
                layer,
                "self_attn.indexer.wk.weight",
                ROLE_INDEXER_WK,
                SafeDtype::Bf16,
                &[128, 6_144],
                -1,
                mtp,
            );
            add_layer(
                &mut contracts,
                layer,
                "self_attn.indexer.weights_proj.weight",
                ROLE_INDEXER_WEIGHTS,
                SafeDtype::Bf16,
                &[32, 6_144],
                -1,
                mtp,
            );
            add_layer(
                &mut contracts,
                layer,
                "self_attn.indexer.k_norm.weight",
                ROLE_INDEXER_K_NORM_WEIGHT,
                SafeDtype::Bf16,
                &[128],
                -1,
                mtp,
            );
            add_layer(
                &mut contracts,
                layer,
                "self_attn.indexer.k_norm.bias",
                ROLE_INDEXER_K_NORM_BIAS,
                SafeDtype::Bf16,
                &[128],
                -1,
                mtp,
            );
        }

        if layer < 3 {
            add_layer(
                &mut contracts,
                layer,
                "mlp.gate_proj.weight",
                ROLE_DENSE_GATE,
                SafeDtype::Bf16,
                &[12_288, 6_144],
                0,
                false,
            );
            add_layer(
                &mut contracts,
                layer,
                "mlp.up_proj.weight",
                ROLE_DENSE_UP,
                SafeDtype::Bf16,
                &[12_288, 6_144],
                0,
                false,
            );
            add_layer(
                &mut contracts,
                layer,
                "mlp.down_proj.weight",
                ROLE_DENSE_DOWN,
                SafeDtype::Bf16,
                &[6_144, 12_288],
                1,
                false,
            );
        } else {
            add_layer(
                &mut contracts,
                layer,
                "mlp.gate.weight",
                ROLE_ROUTER_WEIGHT,
                SafeDtype::Bf16,
                &[256, 6_144],
                -1,
                mtp,
            );
            add_layer(
                &mut contracts,
                layer,
                "mlp.gate.e_score_correction_bias",
                ROLE_ROUTER_CORRECTION,
                SafeDtype::F32,
                &[256],
                -1,
                mtp,
            );
            add_layer(
                &mut contracts,
                layer,
                "mlp.shared_experts.gate_proj.weight",
                ROLE_SHARED_GATE,
                SafeDtype::Bf16,
                &[2_048, 6_144],
                0,
                mtp,
            );
            add_layer(
                &mut contracts,
                layer,
                "mlp.shared_experts.up_proj.weight",
                ROLE_SHARED_UP,
                SafeDtype::Bf16,
                &[2_048, 6_144],
                0,
                mtp,
            );
            add_layer(
                &mut contracts,
                layer,
                "mlp.shared_experts.down_proj.weight",
                ROLE_SHARED_DOWN,
                SafeDtype::Bf16,
                &[6_144, 2_048],
                1,
                mtp,
            );
        }
    }

    add_layer(
        &mut contracts,
        78,
        "enorm.weight",
        ROLE_MTP_ENORM,
        SafeDtype::Bf16,
        &[6_144],
        -1,
        true,
    );
    add_layer(
        &mut contracts,
        78,
        "hnorm.weight",
        ROLE_MTP_HNORM,
        SafeDtype::Bf16,
        &[6_144],
        -1,
        true,
    );
    add_layer(
        &mut contracts,
        78,
        "eh_proj.weight",
        ROLE_MTP_EH_PROJ,
        SafeDtype::Bf16,
        &[6_144, 12_288],
        -1,
        true,
    );
    add_layer(
        &mut contracts,
        78,
        "shared_head.norm.weight",
        ROLE_MTP_SHARED_HEAD_NORM,
        SafeDtype::Bf16,
        &[6_144],
        -1,
        true,
    );
    contracts
}

#[must_use]
pub fn parse_exl3_component(name: &str) -> Option<Exl3ComponentContract> {
    let rest = name.strip_prefix("model.layers.")?;
    let (layer, rest) = rest.split_once(".mlp.experts.")?;
    let layer = parse_canonical_u16(layer)?;
    if !(3..=78).contains(&layer) {
        return None;
    }
    let mut fields = rest.split('.');
    let expert = parse_canonical_u16(fields.next()?)?;
    if expert >= 256 {
        return None;
    }
    let projection = match fields.next()? {
        "gate_proj" => Exl3Projection::Gate,
        "up_proj" => Exl3Projection::Up,
        "down_proj" => Exl3Projection::Down,
        _ => return None,
    };
    let rank = fields.next()?.strip_prefix("rank")?;
    let rank = parse_canonical_u8(rank)?;
    if rank >= TP_DEGREE {
        return None;
    }
    let component = match fields.next()? {
        "mcg" => Exl3Component::Mcg,
        "suh" => Exl3Component::Suh,
        "svh" => Exl3Component::Svh,
        "trellis" => Exl3Component::Trellis,
        _ => return None,
    };
    if fields.next().is_some() {
        return None;
    }
    Some(Exl3ComponentContract {
        layer,
        expert,
        rank,
        projection,
        component,
    })
}

#[allow(clippy::too_many_arguments)]
fn add_layer(
    contracts: &mut BTreeMap<String, ProtectedTensorContract>,
    layer: u16,
    suffix: &str,
    role_id: u16,
    dtype: SafeDtype,
    shape: &[u64],
    tp_axis: i8,
    is_mtp: bool,
) {
    add_protected(
        contracts,
        &format!("model.layers.{layer}.{suffix}"),
        role_id,
        layer as i16,
        dtype,
        shape,
        tp_axis,
        is_mtp,
    );
}

#[allow(clippy::too_many_arguments)]
fn add_protected(
    contracts: &mut BTreeMap<String, ProtectedTensorContract>,
    name: &str,
    role_id: u16,
    layer_id: i16,
    dtype: SafeDtype,
    shape: &[u64],
    tp_axis: i8,
    is_mtp: bool,
) {
    let mut rank_shape = shape.to_vec();
    if tp_axis >= 0 {
        let extent = &mut rank_shape[tp_axis as usize];
        assert!(extent.is_multiple_of(u64::from(TP_DEGREE)));
        *extent /= u64::from(TP_DEGREE);
    }
    let contract = ProtectedTensorContract {
        name: name.to_owned(),
        role_id,
        layer_id,
        dtype,
        source_shape: shape.to_vec(),
        tp_axis,
        rank_shape,
        is_mtp,
    };
    assert!(contracts.insert(name.to_owned(), contract).is_none());
}

fn validate_protected_descriptor(
    contract: &ProtectedTensorContract,
    descriptor: &SafeTensorDescriptor,
) -> Result<(), CheckpointError> {
    if descriptor.dtype != contract.dtype || descriptor.shape != contract.source_shape {
        return Err(CheckpointError::Descriptor(contract.name.clone()));
    }
    Ok(())
}

fn validate_exl3_descriptor(
    name: &str,
    contract: &Exl3ComponentContract,
    descriptor: &SafeTensorDescriptor,
) -> Result<(), CheckpointError> {
    let (logical_k, logical_n) = match contract.projection {
        Exl3Projection::Gate | Exl3Projection::Up => (6_144, 512),
        Exl3Projection::Down => (512, 6_144),
    };
    let (dtype, shape) = match contract.component {
        Exl3Component::Mcg => (SafeDtype::I32, vec![]),
        Exl3Component::Suh => (SafeDtype::F16, vec![logical_k]),
        Exl3Component::Svh => (SafeDtype::F16, vec![logical_n]),
        Exl3Component::Trellis => (SafeDtype::I16, vec![logical_k / 16, logical_n / 16, 48]),
    };
    if descriptor.dtype != dtype || descriptor.shape != shape {
        return Err(CheckpointError::Descriptor(name.to_owned()));
    }
    Ok(())
}

fn validate_shard_names(shards: &BTreeSet<std::path::PathBuf>) -> Result<(), CheckpointError> {
    let mut expected = BTreeSet::from([
        "model-embed.safetensors".into(),
        "model-head.safetensors".into(),
    ]);
    for layer in 0_u16..=78 {
        expected.insert(format!("model-layer-{layer:03}.safetensors").into());
    }
    if shards != &expected {
        return Err(CheckpointError::Shards);
    }
    Ok(())
}

fn exl3_component_index(contract: &Exl3ComponentContract) -> usize {
    let layer = usize::from(contract.layer - 3);
    let projection = match contract.projection {
        Exl3Projection::Gate => 0,
        Exl3Projection::Up => 1,
        Exl3Projection::Down => 2,
    };
    let component = match contract.component {
        Exl3Component::Mcg => 0,
        Exl3Component::Suh => 1,
        Exl3Component::Svh => 2,
        Exl3Component::Trellis => 3,
    };
    ((((layer * 256 + usize::from(contract.expert)) * 3 + projection) * 4
        + usize::from(contract.rank))
        * 4)
        + component
}

fn exl3_component_name(index: usize) -> String {
    let component = ["mcg", "suh", "svh", "trellis"][index % 4];
    let index = index / 4;
    let rank = index % 4;
    let index = index / 4;
    let projection = ["gate", "up", "down"][index % 3];
    let index = index / 3;
    let expert = index % 256;
    let layer = index / 256 + 3;
    format!("model.layers.{layer}.mlp.experts.{expert}.{projection}_proj.rank{rank}.{component}")
}

fn parse_canonical_u16(value: &str) -> Option<u16> {
    let parsed: u16 = value.parse().ok()?;
    (parsed.to_string() == value).then_some(parsed)
}

fn parse_canonical_u8(value: &str) -> Option<u8> {
    let parsed: u8 = value.parse().ok()?;
    (parsed.to_string() == value).then_some(parsed)
}

#[derive(Debug)]
pub enum CheckpointError {
    Revision(String),
    IndexIdentity,
    TensorCount(usize),
    Shards,
    Unknown(String),
    Missing(String),
    Duplicate(String),
    Descriptor(String),
    ByteInventory,
    Overflow,
    Internal,
}

impl fmt::Display for CheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CheckpointError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_inventory_count_shapes_and_tp_rules_are_exact() {
        let contracts = protected_tensor_contracts();
        assert_eq!(contracts.len(), PINNED_PROTECTED_TENSOR_COUNT);
        let mut bf16_count = 0_usize;
        let mut bf16_bytes = 0_u64;
        let mut fp32_count = 0_usize;
        let mut fp32_bytes = 0_u64;
        for contract in contracts.values() {
            let elements = contract.source_shape.iter().product::<u64>();
            match contract.dtype {
                SafeDtype::Bf16 => {
                    bf16_count += 1;
                    bf16_bytes += elements * 2;
                }
                SafeDtype::F32 => {
                    fp32_count += 1;
                    fp32_bytes += elements * 4;
                }
                dtype => panic!("unexpected protected dtype {dtype:?}"),
            }
        }
        assert_eq!(bf16_count, 1_141);
        assert_eq!(bf16_bytes, 37_781_026_816);
        assert_eq!(fp32_count, 76);
        assert_eq!(fp32_bytes, 77_824);
        assert_eq!(
            contracts["model.embed_tokens.weight"].rank_shape,
            [38_720, 6_144]
        );
        assert_eq!(
            contracts["model.layers.0.self_attn.q_a_proj.weight"].tp_axis,
            -1
        );
        assert_eq!(
            contracts["model.layers.0.self_attn.q_b_proj.weight"].rank_shape,
            [4_096, 2_048]
        );
        assert_eq!(
            contracts["model.layers.0.self_attn.o_proj.weight"].rank_shape,
            [6_144, 4_096]
        );
        assert_eq!(
            contracts["model.layers.3.mlp.shared_experts.down_proj.weight"].rank_shape,
            [6_144, 512]
        );
        assert_eq!(
            contracts["model.layers.78.eh_proj.weight"].rank_shape,
            [6_144, 12_288]
        );
        assert!(contracts["model.layers.78.eh_proj.weight"].is_mtp);
        assert!(!contracts.contains_key("model.layers.3.self_attn.indexer.wk.weight"));
        assert!(contracts.contains_key("model.layers.6.self_attn.indexer.wk.weight"));
    }

    #[test]
    fn exl3_component_parser_is_canonical_and_bijective() {
        let first = "model.layers.3.mlp.experts.0.gate_proj.rank0.mcg";
        let last = "model.layers.78.mlp.experts.255.down_proj.rank3.trellis";
        let first_contract = parse_exl3_component(first).unwrap();
        let last_contract = parse_exl3_component(last).unwrap();
        assert_eq!(exl3_component_index(&first_contract), 0);
        assert_eq!(
            exl3_component_index(&last_contract),
            PINNED_EXL3_COMPONENT_COUNT - 1
        );
        assert_eq!(exl3_component_name(0), first);
        assert_eq!(exl3_component_name(PINNED_EXL3_COMPONENT_COUNT - 1), last);
        assert_eq!(first_contract.role_id(), ROLE_ROUTED_GATE_UP);
        assert_eq!(last_contract.role_id(), ROLE_ROUTED_DOWN);
        assert!(!first_contract.is_mtp());
        assert!(last_contract.is_mtp());
        for invalid in [
            "model.layers.03.mlp.experts.0.gate_proj.rank0.mcg",
            "model.layers.3.mlp.experts.00.gate_proj.rank0.mcg",
            "model.layers.3.mlp.experts.256.gate_proj.rank0.mcg",
            "model.layers.3.mlp.experts.0.gate_proj.rank4.mcg",
            "model.layers.2.mlp.experts.0.gate_proj.rank0.mcg",
            "model.layers.3.mlp.experts.0.gate_proj.rank0.mcg.extra",
        ] {
            assert!(parse_exl3_component(invalid).is_none(), "{invalid}");
        }
    }

    #[test]
    fn pinned_byte_inventory_rederives_exact_index_total() {
        let bf16 = 37_781_026_816_u64;
        let fp32 = 77_824_u64;
        let fp16 = 3_107_979_264_u64;
        let i16 = 275_414_777_856_u64;
        let i32 = 933_888_u64;
        assert_eq!(bf16 + fp32 + fp16 + i16 + i32, PINNED_EXL3_PAYLOAD_BYTES);
        assert_eq!(76_usize * 256 * 3 * 4 * 4, PINNED_EXL3_COMPONENT_COUNT);
    }
}
