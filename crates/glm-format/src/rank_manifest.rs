use std::{
    collections::BTreeMap,
    fmt,
    path::{Component, Path},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    CUTLASS_COMMIT, EXL3_MODEL_REVISION, EXL3_SOURCE_REVISION, KERNEL_ABI,
    PINNED_EXL3_INDEX_SHA256, PINNED_EXL3_PAYLOAD_BYTES, PINNED_EXL3_REPOSITORY,
    PINNED_RANK_SOURCE_PAYLOAD_BYTES, PINNED_RANK_TENSOR_COUNT, PINNED_SOURCE_FILE_COUNT,
    PINNED_SOURCE_FILE_MAP_SHA256, PINNED_SOURCE_MANIFEST_SHA256, TensorDescriptor,
    container::{
        CODEC_BF16_ROW_MAJOR, CODEC_EXL3_SOURCE, CODEC_FP16_ROW_MAJOR, CODEC_FP32_ROW_MAJOR,
        CODEC_NVFP4_1D, CODEC_NVFP4_2D, DESCRIPTOR_FLAG_AUX_REQUIRED,
    },
};

pub const PRODUCTION_RANK_MANIFEST_SCHEMA: &str = "glmaxx.rank-manifest.v0.2.2";
const CONVERSION_REPOSITORY: &str = "https://github.com/yatesdr/glmaxx.git";
const REVIEW_ACCEPTANCE_TOKEN: &str = "manifest-abi-v0.2.2-accepted";
const PINNED_RANK_TENSOR_CONTRACT_SHA256: [&str; 4] = [
    "460883a09b5c247bba5458078e4036c090a78cb33824399b7a1e94be5330b12f",
    "a86485e73ebc4ece3066518543c590b3adb75f8749d78d7adbdf048874311386",
    "47ce5c931ccf44008345ddaaf8d72ca82a56508c92f0b2a47ed1cc4ec25a9a79",
    "6fc13c23ac6e9c0aeb9365afb5a05ff284478220a2425e285a64434885a046d4",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RankWeightProfile {
    CapacityExl3,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ValidatedRankManifest {
    pub rank: u8,
    pub profile: RankWeightProfile,
    pub conversion_commit: [u8; 20],
    pub operation_manifest_sha256: [u8; 32],
    pub tensor_contract_sha256: [u8; 32],
    pub profile_budget_sha256: [u8; 32],
    pub review_artifact_sha256: [u8; 32],
    pub format_spec_sha256: [u8; 32],
    pub engine_spec_sha256: [u8; 32],
    pub tensor_source_payload_bytes: u64,
    pub source_verified_file_bytes: u64,
    pub tensor_semantics: Vec<ValidatedTensorSemantic>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ValidatedTensorSemantic {
    pub tensor_id: u32,
    pub role_id: u16,
    pub codec_id: u16,
    pub layer_id: i16,
    pub expert_id: i16,
    pub tp_shard_axis: i8,
    pub ndim: u8,
    pub flags: u8,
    pub source_binding_kind: u8,
    pub logical_dtype: u16,
    pub stored_dtype: u16,
    pub quant_group_elements: u32,
    pub rank_logical_shape: [u32; 4],
    pub global_logical_shape: [u64; 4],
    pub name_sha256: [u8; 32],
    pub reconstruction_id: u16,
    pub collective_after_id: u16,
    pub source_dtype_id: u16,
    pub source_axis: i8,
}

impl ValidatedTensorSemantic {
    #[must_use]
    pub fn encode(self) -> [u8; 128] {
        let mut output = [0_u8; 128];
        put_u32(&mut output, 0, self.tensor_id);
        put_u16(&mut output, 4, self.role_id);
        put_u16(&mut output, 6, self.codec_id);
        put_i16(&mut output, 8, self.layer_id);
        put_i16(&mut output, 10, self.expert_id);
        output[12] = self.tp_shard_axis.to_le_bytes()[0];
        output[13] = self.ndim;
        output[14] = self.flags;
        output[15] = self.source_binding_kind;
        put_u16(&mut output, 16, self.logical_dtype);
        put_u16(&mut output, 18, self.stored_dtype);
        put_u32(&mut output, 20, self.quant_group_elements);
        for (index, dimension) in self.rank_logical_shape.into_iter().enumerate() {
            put_u32(&mut output, 24 + index * 4, dimension);
        }
        for (index, dimension) in self.global_logical_shape.into_iter().enumerate() {
            put_u64(&mut output, 40 + index * 8, dimension);
        }
        output[72..104].copy_from_slice(&self.name_sha256);
        put_u16(&mut output, 104, self.reconstruction_id);
        put_u16(&mut output, 106, self.collective_after_id);
        put_u16(&mut output, 108, self.source_dtype_id);
        output[110] = self.source_axis.to_le_bytes()[0];
        output
    }
}

pub fn rank_invariant_tensor_catalog_sha256(
    tensors: &[ValidatedTensorSemantic],
) -> Result<[u8; 32], RankManifestError> {
    if tensors.is_empty() {
        return Err(RankManifestError::TensorContract);
    }
    let count = u32::try_from(tensors.len()).map_err(|_| RankManifestError::Overflow)?;
    let mut hasher = Sha256::new();
    hasher.update(b"glmaxx.rank-invariant-tensor-catalog.v1\0");
    hasher.update(count.to_le_bytes());
    for (expected_id, tensor) in tensors.iter().copied().enumerate() {
        if tensor.tensor_id
            != u32::try_from(expected_id).map_err(|_| RankManifestError::Overflow)?
        {
            return Err(RankManifestError::Tensor(expected_id));
        }
        hasher.update(tensor.encode());
    }
    Ok(hasher.finalize().into())
}

pub(crate) struct RankManifestContext<'a> {
    pub rank: u32,
    pub descriptors: &'a [TensorDescriptor],
    pub names: &'a [String],
    pub model_config_sha256: [u8; 32],
    pub tokenizer_bundle_sha256: [u8; 32],
    pub chat_template_sha256: [u8; 32],
    pub weight_policy_sha256: [u8; 32],
    pub kernel_abi_sha256: [u8; 32],
    pub payload_sha256: [u8; 32],
}

pub(crate) fn validate_rank_manifest(
    bytes: &[u8],
    context: RankManifestContext<'_>,
) -> Result<Option<ValidatedRankManifest>, RankManifestError> {
    validate_rank_manifest_inner(bytes, context, true)
}

fn validate_rank_manifest_inner(
    bytes: &[u8],
    context: RankManifestContext<'_>,
    require_pinned_inventory: bool,
) -> Result<Option<ValidatedRankManifest>, RankManifestError> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| RankManifestError::Json)?;
    let canonical = serde_json::to_vec(&value).map_err(|_| RankManifestError::Json)?;
    if canonical != bytes {
        return Err(RankManifestError::NonCanonical);
    }
    let Some(schema) = value.get("schema").and_then(serde_json::Value::as_str) else {
        return Ok(None);
    };
    if schema != PRODUCTION_RANK_MANIFEST_SCHEMA {
        if schema.starts_with("glmaxx.rank-manifest.") {
            return Err(RankManifestError::UnsupportedSchema);
        }
        return Ok(None);
    }

    let manifest: RawRankManifest =
        serde_json::from_value(value).map_err(|_| RankManifestError::Structure)?;
    validate_top_level(&manifest, &context)?;
    validate_tensors(&manifest, &context)?;

    let tensor_value =
        serde_json::to_value(&manifest.tensors).map_err(|_| RankManifestError::Json)?;
    let tensor_bytes = serde_json::to_vec(&tensor_value).map_err(|_| RankManifestError::Json)?;
    let tensor_contract_sha256 = parse_sha256(&manifest.tensor_contract_sha256)?;
    if sha256(&tensor_bytes) != tensor_contract_sha256 {
        return Err(RankManifestError::TensorContract);
    }
    if require_pinned_inventory {
        validate_pinned_inventory(&manifest, tensor_contract_sha256)?;
    }

    let tensor_semantics = validated_tensor_semantics(&manifest, &context)?;
    Ok(Some(ValidatedRankManifest {
        rank: manifest.rank,
        profile: parse_profile(&manifest)?,
        conversion_commit: parse_hex::<20>(&manifest.conversion.commit)?,
        operation_manifest_sha256: parse_sha256(&manifest.model.operation_manifest_sha256)?,
        tensor_contract_sha256,
        profile_budget_sha256: nonzero_sha256(&manifest.profile.profile_budget_sha256)?,
        review_artifact_sha256: nonzero_sha256(&manifest.review.artifact_sha256)?,
        format_spec_sha256: nonzero_sha256(&manifest.review.format_spec_sha256)?,
        engine_spec_sha256: nonzero_sha256(&manifest.review.engine_spec_sha256)?,
        tensor_source_payload_bytes: manifest.tensor_source_payload_bytes,
        source_verified_file_bytes: manifest.integrity.source_verified_file_bytes,
        tensor_semantics,
    }))
}

fn validate_pinned_inventory(
    manifest: &RawRankManifest,
    tensor_contract_sha256: [u8; 32],
) -> Result<(), RankManifestError> {
    let expected_contract = PINNED_RANK_TENSOR_CONTRACT_SHA256
        .get(usize::from(manifest.rank))
        .ok_or(RankManifestError::Inventory)?;
    if manifest.tensor_count != PINNED_RANK_TENSOR_COUNT
        || manifest.tensor_source_payload_bytes != PINNED_RANK_SOURCE_PAYLOAD_BYTES
        || parse_sha256(expected_contract)? != tensor_contract_sha256
        || sha256(
            &serde_json::to_vec(&manifest.integrity.source_file_sha256)
                .map_err(|_| RankManifestError::Json)?,
        ) != PINNED_SOURCE_FILE_MAP_SHA256
    {
        return Err(RankManifestError::Inventory);
    }
    Ok(())
}

fn validate_top_level(
    manifest: &RawRankManifest,
    context: &RankManifestContext<'_>,
) -> Result<(), RankManifestError> {
    let rank = u8::try_from(context.rank).map_err(|_| RankManifestError::Identity)?;
    if manifest.schema != PRODUCTION_RANK_MANIFEST_SCHEMA
        || manifest.rank != rank
        || manifest.tp_degree != 4
        || manifest.tensor_count != context.descriptors.len()
        || manifest.tensors.len() != context.descriptors.len()
        || manifest.tensor_source_payload_bytes == 0
        || manifest.calibration.manifest_file != "calibration_manifest.json"
        || manifest.calibration.source_revision != EXL3_MODEL_REVISION
        || manifest.codec.format != "g5n-v0.2.2"
        || manifest.codec.exl3_source_revision != EXL3_SOURCE_REVISION
        || manifest.conversion.repository != CONVERSION_REPOSITORY
        || manifest.integrity.output_hash_location
            != "rank-header.payload_sha256-and-descriptor-plane-sha256"
        || manifest.integrity.source_verification != "FULL_SHA256"
        || manifest.integrity.source_verified_file_bytes < PINNED_EXL3_PAYLOAD_BYTES
        || manifest.license_provenance.source_repository != PINNED_EXL3_REPOSITORY
        || manifest.model.repository != PINNED_EXL3_REPOSITORY
        || manifest.model.revision != EXL3_MODEL_REVISION
        || manifest.review.acceptance_token != REVIEW_ACCEPTANCE_TOKEN
        || manifest.toolchain.cuda != "13.3"
        || manifest.toolchain.cutlass_commit != CUTLASS_COMMIT
        || manifest.toolchain.kernel_abi != KERNEL_ABI
        || manifest.toolchain.rust != "1.92.0"
    {
        return Err(RankManifestError::Identity);
    }
    parse_hex::<20>(&manifest.conversion.commit)?;
    validate_container_digest(&manifest.toolchain.container_digest)?;
    parse_profile(manifest)?;
    let tensor_source_payload_bytes = manifest
        .tensors
        .iter()
        .try_fold(0_u64, |total, tensor| {
            total
                .checked_add(tensor.primary_bytes)
                .and_then(|value| value.checked_add(tensor.aux_bytes))
        })
        .ok_or(RankManifestError::Overflow)?;
    if manifest.tensor_source_payload_bytes != tensor_source_payload_bytes {
        return Err(RankManifestError::TensorContract);
    }
    if context
        .descriptors
        .iter()
        .any(|descriptor| matches!(descriptor.codec_id, CODEC_NVFP4_1D | CODEC_NVFP4_2D))
    {
        return Err(RankManifestError::Profile);
    }

    if parse_sha256(&manifest.integrity.output_payload_sha256)? != context.payload_sha256
        || parse_sha256(&manifest.model.config_sha256)? != context.model_config_sha256
        || parse_sha256(&manifest.profile.weight_policy_sha256)? != context.weight_policy_sha256
        || sha256(manifest.toolchain.kernel_abi.as_bytes()) != context.kernel_abi_sha256
        || parse_sha256(&manifest.tokenizer.chat_template_sha256)? != context.chat_template_sha256
        || parse_sha256(&manifest.model.source_index_sha256)? != PINNED_EXL3_INDEX_SHA256
        || parse_sha256(&manifest.model.source_manifest_sha256)? != PINNED_SOURCE_MANIFEST_SHA256
    {
        return Err(RankManifestError::Identity);
    }
    for digest in [
        &manifest.calibration.manifest_sha256,
        &manifest.license_provenance.license_sha256,
        &manifest.license_provenance.readme_sha256,
        &manifest.model.operation_manifest_sha256,
        &manifest.profile.profile_budget_sha256,
        &manifest.review.artifact_sha256,
        &manifest.review.engine_spec_sha256,
        &manifest.review.format_spec_sha256,
    ] {
        nonzero_sha256(digest)?;
    }

    validate_source_files(manifest)?;
    let tokenizer_sha256 = parse_sha256(&manifest.tokenizer.tokenizer_sha256)?;
    let tokenizer_config_sha256 = parse_sha256(&manifest.tokenizer.tokenizer_config_sha256)?;
    let generation_config_sha256 = parse_sha256(&manifest.tokenizer.generation_config_sha256)?;
    let mut bundle = Sha256::new();
    bundle.update(b"glmaxx-tokenizer-bundle-v0\0");
    for (name, digest) in [
        ("tokenizer.json", tokenizer_sha256),
        ("tokenizer_config.json", tokenizer_config_sha256),
        ("generation_config.json", generation_config_sha256),
    ] {
        bundle.update(name.as_bytes());
        bundle.update([0]);
        bundle.update(digest);
    }
    if <[u8; 32]>::from(bundle.finalize()) != context.tokenizer_bundle_sha256 {
        return Err(RankManifestError::Identity);
    }
    Ok(())
}

fn validate_source_files(manifest: &RawRankManifest) -> Result<(), RankManifestError> {
    let files = &manifest.integrity.source_file_sha256;
    if files.len() != PINNED_SOURCE_FILE_COUNT {
        return Err(RankManifestError::SourceFiles);
    }
    for (name, digest) in files {
        if !safe_source_name(name) {
            return Err(RankManifestError::SourceFiles);
        }
        parse_sha256(digest)?;
    }
    for (name, expected) in [
        (
            "calibration_manifest.json",
            parse_sha256(&manifest.calibration.manifest_sha256)?,
        ),
        ("config.json", parse_sha256(&manifest.model.config_sha256)?),
        ("model.safetensors.index.json", PINNED_EXL3_INDEX_SHA256),
        (
            "tokenizer.json",
            parse_sha256(&manifest.tokenizer.tokenizer_sha256)?,
        ),
        (
            "tokenizer_config.json",
            parse_sha256(&manifest.tokenizer.tokenizer_config_sha256)?,
        ),
        (
            "generation_config.json",
            parse_sha256(&manifest.tokenizer.generation_config_sha256)?,
        ),
        (
            "chat_template.jinja",
            parse_sha256(&manifest.tokenizer.chat_template_sha256)?,
        ),
        (
            "LICENSE",
            parse_sha256(&manifest.license_provenance.license_sha256)?,
        ),
        (
            "README.md",
            parse_sha256(&manifest.license_provenance.readme_sha256)?,
        ),
    ] {
        if files
            .get(name)
            .map(String::as_str)
            .map(parse_sha256)
            .transpose()?
            != Some(expected)
        {
            return Err(RankManifestError::SourceFiles);
        }
    }
    Ok(())
}

fn validate_tensors(
    manifest: &RawRankManifest,
    context: &RankManifestContext<'_>,
) -> Result<(), RankManifestError> {
    for (index, ((tensor, descriptor), name)) in manifest
        .tensors
        .iter()
        .zip(context.descriptors)
        .zip(context.names)
        .enumerate()
    {
        let tensor_id = u32::try_from(index).map_err(|_| RankManifestError::Overflow)?;
        if tensor.tensor_id != tensor_id
            || descriptor.tensor_id != tensor_id
            || tensor.name != *name
            || tensor.role_id != descriptor.role_id
            || tensor.layer_id != descriptor.layer_id
            || tensor.expert_id != descriptor.expert_id
            || tensor.codec_id != descriptor.codec_id
            || tensor.logical_dtype != descriptor.logical_dtype
            || tensor.stored_dtype != descriptor.stored_dtype
            || tensor.tp_shard_axis != descriptor.tp_shard_axis
            || tensor.ndim != descriptor.ndim
            || tensor.flags != descriptor.flags & !DESCRIPTOR_FLAG_AUX_REQUIRED
            || tensor.primary_bytes != descriptor.payload_bytes
            || tensor.aux_bytes != descriptor.aux_bytes
            || tensor.quant_group_elements != descriptor.quant_group_elements
            || parse_sha256(&tensor.codec_metadata_sha256)? != descriptor.codec_metadata_sha256
            || tensor.rank_shape != descriptor.logical_shape[..usize::from(descriptor.ndim)]
            || tensor.padded_shape != descriptor.padded_shape[..usize::from(descriptor.ndim)]
            || tensor.collective_after != expected_collective(descriptor.role_id)
        {
            return Err(RankManifestError::Tensor(index));
        }
        validate_source_binding(tensor, manifest.rank, index)?;
        validate_reconstruction(tensor, index)?;
    }
    Ok(())
}

fn validated_tensor_semantics(
    manifest: &RawRankManifest,
    context: &RankManifestContext<'_>,
) -> Result<Vec<ValidatedTensorSemantic>, RankManifestError> {
    manifest
        .tensors
        .iter()
        .zip(context.descriptors)
        .enumerate()
        .map(|(index, (tensor, descriptor))| {
            let mut global_logical_shape = [1_u64; 4];
            for (destination, source) in global_logical_shape
                .iter_mut()
                .zip(tensor.global_shape.iter().copied())
            {
                *destination = source;
            }
            Ok(ValidatedTensorSemantic {
                tensor_id: u32::try_from(index).map_err(|_| RankManifestError::Overflow)?,
                role_id: descriptor.role_id,
                codec_id: descriptor.codec_id,
                layer_id: descriptor.layer_id,
                expert_id: descriptor.expert_id,
                tp_shard_axis: descriptor.tp_shard_axis,
                ndim: descriptor.ndim,
                flags: tensor.flags,
                source_binding_kind: source_binding_kind_id(&tensor.source.kind)
                    .ok_or(RankManifestError::Tensor(index))?,
                logical_dtype: descriptor.logical_dtype,
                stored_dtype: descriptor.stored_dtype,
                quant_group_elements: descriptor.quant_group_elements,
                rank_logical_shape: descriptor.logical_shape,
                global_logical_shape,
                name_sha256: sha256(tensor.name.as_bytes()),
                reconstruction_id: reconstruction_id(&tensor.reconstruction)
                    .ok_or(RankManifestError::Tensor(index))?,
                collective_after_id: collective_after_id(&tensor.collective_after)
                    .ok_or(RankManifestError::Tensor(index))?,
                source_dtype_id: source_dtype_id(&tensor.source_dtype)
                    .ok_or(RankManifestError::Tensor(index))?,
                source_axis: tensor.source.axis,
            })
        })
        .collect()
}

fn source_binding_kind_id(kind: &str) -> Option<u8> {
    match kind {
        "replicated" => Some(1),
        "contiguous_tp_slice" => Some(2),
        "explicit_rank_components" => Some(3),
        _ => None,
    }
}

fn reconstruction_id(reconstruction: &str) -> Option<u16> {
    match reconstruction {
        "byte_exact_source_precision" => Some(1),
        "exl3_tr3_trellis_v0" => Some(2),
        _ => None,
    }
}

fn collective_after_id(collective: &str) -> Option<u16> {
    match collective {
        "none" => Some(0),
        "tp_embedding_reduce" => Some(1),
        "distributed_sampling" => Some(2),
        "tp_all_reduce" => Some(3),
        _ => None,
    }
}

fn source_dtype_id(dtype: &str) -> Option<u16> {
    [
        "BOOL",
        "F4",
        "F6_E2M3",
        "F6_E3M2",
        "U8",
        "I8",
        "U16",
        "I16",
        "U32",
        "I32",
        "U64",
        "I64",
        "F16",
        "BF16",
        "F32",
        "F64",
        "F8_E4M3",
        "F8_E5M2",
        "F8_E8M0",
        "F8_E4M3FNUZ",
        "F8_E5M2FNUZ",
        "C64",
    ]
    .iter()
    .position(|candidate| *candidate == dtype)
    .and_then(|index| u16::try_from(index + 1).ok())
    .or_else(|| (dtype == "EXL3_TR3_COMPONENTS").then_some(0x8000))
}

fn validate_source_binding(
    tensor: &RawManifestTensor,
    rank: u8,
    index: usize,
) -> Result<(), RankManifestError> {
    let ndim = usize::from(tensor.ndim);
    if tensor.global_shape.len() != ndim || tensor.source_shape.len() > 4 {
        return Err(RankManifestError::Tensor(index));
    }
    let rank_shape: Vec<u64> = tensor.rank_shape.iter().copied().map(u64::from).collect();
    let source = &tensor.source;
    match source.kind.as_str() {
        "replicated" => {
            if source.axis != -1
                || source.axis != tensor.tp_shard_axis
                || source.start != 0
                || source.end != 0
                || source.components.as_slice() != [tensor.name.as_str()]
                || tensor.global_shape != rank_shape
                || tensor.source_shape != tensor.global_shape
            {
                return Err(RankManifestError::Tensor(index));
            }
        }
        "contiguous_tp_slice" => {
            let axis =
                usize::try_from(source.axis).map_err(|_| RankManifestError::Tensor(index))?;
            let extent = *tensor
                .global_shape
                .get(axis)
                .ok_or(RankManifestError::Tensor(index))?;
            let shard = extent
                .checked_div(4)
                .filter(|_| extent.is_multiple_of(4))
                .ok_or(RankManifestError::Tensor(index))?;
            let start = shard
                .checked_mul(u64::from(rank))
                .ok_or(RankManifestError::Overflow)?;
            let mut expected_rank_shape = tensor.global_shape.clone();
            expected_rank_shape[axis] = shard;
            if source.axis != tensor.tp_shard_axis
                || source.start != start
                || source.end != start + shard
                || source.components.as_slice() != [tensor.name.as_str()]
                || rank_shape != expected_rank_shape
                || tensor.source_shape != tensor.global_shape
            {
                return Err(RankManifestError::Tensor(index));
            }
        }
        "explicit_rank_components" => {
            let stem = tensor
                .name
                .strip_suffix(".weight")
                .ok_or(RankManifestError::Tensor(index))?;
            let expected: Vec<String> = ["mcg", "suh", "svh", "trellis"]
                .into_iter()
                .map(|component| format!("{stem}.rank{rank}.{component}"))
                .collect();
            let axis =
                usize::try_from(source.axis).map_err(|_| RankManifestError::Tensor(index))?;
            let mut expected_global_shape = rank_shape;
            let extent = expected_global_shape
                .get_mut(axis)
                .ok_or(RankManifestError::Tensor(index))?;
            *extent = extent.checked_mul(4).ok_or(RankManifestError::Overflow)?;
            if source.axis != tensor.tp_shard_axis
                || source.start != u64::from(rank)
                || source.end != u64::from(rank) + 1
                || source.components != expected
                || tensor.global_shape != expected_global_shape
                || !tensor.source_shape.is_empty()
                || tensor.source_dtype != "EXL3_TR3_COMPONENTS"
            {
                return Err(RankManifestError::Tensor(index));
            }
        }
        _ => return Err(RankManifestError::Tensor(index)),
    }
    Ok(())
}

fn validate_reconstruction(
    tensor: &RawManifestTensor,
    index: usize,
) -> Result<(), RankManifestError> {
    let (expected_reconstruction, expected_source_dtype, expected_source_kind) =
        match tensor.codec_id {
            CODEC_BF16_ROW_MAJOR => (
                "byte_exact_source_precision",
                "BF16",
                PlainSourceKind::Protected,
            ),
            CODEC_FP16_ROW_MAJOR => (
                "byte_exact_source_precision",
                "F16",
                PlainSourceKind::Protected,
            ),
            CODEC_FP32_ROW_MAJOR => (
                "byte_exact_source_precision",
                "F32",
                PlainSourceKind::Protected,
            ),
            CODEC_EXL3_SOURCE => (
                "exl3_tr3_trellis_v0",
                "EXL3_TR3_COMPONENTS",
                PlainSourceKind::Exl3,
            ),
            CODEC_NVFP4_1D | CODEC_NVFP4_2D => {
                return Err(RankManifestError::Tensor(index));
            }
            _ => return Err(RankManifestError::Tensor(index)),
        };
    let source_kind_matches = match expected_source_kind {
        PlainSourceKind::Protected => {
            matches!(
                tensor.source.kind.as_str(),
                "replicated" | "contiguous_tp_slice"
            )
        }
        PlainSourceKind::Exl3 => tensor.source.kind == "explicit_rank_components",
    };
    if tensor.reconstruction != expected_reconstruction
        || tensor.source_dtype != expected_source_dtype
        || !source_kind_matches
    {
        return Err(RankManifestError::Tensor(index));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum PlainSourceKind {
    Protected,
    Exl3,
}

fn parse_profile(manifest: &RawRankManifest) -> Result<RankWeightProfile, RankManifestError> {
    match (
        manifest.profile.name.as_str(),
        manifest.codec.profile.as_str(),
    ) {
        ("capacity-exl3", "capacity-exl3-v0") => Ok(RankWeightProfile::CapacityExl3),
        _ => Err(RankManifestError::Profile),
    }
}

fn expected_collective(role_id: u16) -> &'static str {
    match role_id {
        0x0001 => "tp_embedding_reduce",
        0x0002 => "distributed_sampling",
        0x0107 | 0x0403 | 0x0502 | 0x0603 => "tp_all_reduce",
        _ => "none",
    }
}

fn safe_source_name(name: &str) -> bool {
    let mut components = Path::new(name).components();
    !name.is_empty()
        && !name
            .bytes()
            .any(|byte| byte == 0 || byte.is_ascii_control())
        && matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none()
}

fn validate_container_digest(value: &str) -> Result<(), RankManifestError> {
    let Some(digest) = value.strip_prefix("sha256:") else {
        return Err(RankManifestError::Identity);
    };
    parse_hex::<32>(digest).map(|_| ())
}

fn nonzero_sha256(value: &str) -> Result<[u8; 32], RankManifestError> {
    let digest = parse_sha256(value)?;
    if digest == [0; 32] {
        return Err(RankManifestError::Identity);
    }
    Ok(digest)
}

fn parse_sha256(value: &str) -> Result<[u8; 32], RankManifestError> {
    parse_hex(value)
}

fn parse_hex<const N: usize>(value: &str) -> Result<[u8; N], RankManifestError> {
    if value.len() != N * 2
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(RankManifestError::Digest);
    }
    let mut output = [0_u8; N];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        output[index] = (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]);
    }
    Ok(output)
}

fn hex_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => unreachable!(),
    }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_i16(output: &mut [u8], offset: usize, value: i16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    output[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[derive(Debug)]
pub enum RankManifestError {
    Json,
    NonCanonical,
    UnsupportedSchema,
    Structure,
    Identity,
    Profile,
    Digest,
    SourceFiles,
    TensorContract,
    Inventory,
    Tensor(usize),
    Overflow,
}

impl fmt::Display for RankManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RankManifestError {}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawRankManifest {
    calibration: RawCalibration,
    codec: RawCodec,
    conversion: RawConversion,
    integrity: RawIntegrity,
    license_provenance: RawLicense,
    model: RawModel,
    profile: RawProfile,
    rank: u8,
    review: RawReview,
    schema: String,
    tensor_contract_sha256: String,
    tensor_count: usize,
    tensor_source_payload_bytes: u64,
    tensors: Vec<RawManifestTensor>,
    tokenizer: RawTokenizer,
    toolchain: RawToolchain,
    tp_degree: u8,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawCalibration {
    manifest_file: String,
    manifest_sha256: String,
    source_revision: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawCodec {
    exl3_source_revision: String,
    format: String,
    profile: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawConversion {
    commit: String,
    repository: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawIntegrity {
    output_hash_location: String,
    output_payload_sha256: String,
    source_file_sha256: BTreeMap<String, String>,
    source_verification: String,
    source_verified_file_bytes: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawLicense {
    license_sha256: String,
    readme_sha256: String,
    source_repository: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawModel {
    config_sha256: String,
    operation_manifest_sha256: String,
    repository: String,
    revision: String,
    source_index_sha256: String,
    source_manifest_sha256: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawProfile {
    name: String,
    profile_budget_sha256: String,
    weight_policy_sha256: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawReview {
    acceptance_token: String,
    artifact_sha256: String,
    engine_spec_sha256: String,
    format_spec_sha256: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawManifestTensor {
    aux_bytes: u64,
    codec_id: u16,
    codec_metadata_sha256: String,
    collective_after: String,
    expert_id: i16,
    flags: u8,
    global_shape: Vec<u64>,
    layer_id: i16,
    logical_dtype: u16,
    name: String,
    ndim: u8,
    padded_shape: Vec<u32>,
    primary_bytes: u64,
    quant_group_elements: u32,
    rank_shape: Vec<u32>,
    reconstruction: String,
    role_id: u16,
    source: RawSourceBinding,
    source_dtype: String,
    source_shape: Vec<u64>,
    stored_dtype: u16,
    tensor_id: u32,
    tp_shard_axis: i8,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawSourceBinding {
    axis: i8,
    components: Vec<String>,
    end: u64,
    kind: String,
    start: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawTokenizer {
    chat_template_sha256: String,
    generation_config_sha256: String,
    tokenizer_config_sha256: String,
    tokenizer_sha256: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawToolchain {
    container_digest: String,
    cuda: String,
    cutlass_commit: String,
    kernel_abi: String,
    rust: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::container::{CODEC_BF16_ROW_MAJOR, DTYPE_BF16, PAYLOAD_ALIGNMENT};

    struct Fixture {
        manifest: RawRankManifest,
        descriptors: Vec<TensorDescriptor>,
        names: Vec<String>,
        model_config_sha256: [u8; 32],
        tokenizer_bundle_sha256: [u8; 32],
        chat_template_sha256: [u8; 32],
        weight_policy_sha256: [u8; 32],
        kernel_abi_sha256: [u8; 32],
        payload_sha256: [u8; 32],
    }

    impl Fixture {
        fn context(&self) -> RankManifestContext<'_> {
            RankManifestContext {
                rank: 0,
                descriptors: &self.descriptors,
                names: &self.names,
                model_config_sha256: self.model_config_sha256,
                tokenizer_bundle_sha256: self.tokenizer_bundle_sha256,
                chat_template_sha256: self.chat_template_sha256,
                weight_policy_sha256: self.weight_policy_sha256,
                kernel_abi_sha256: self.kernel_abi_sha256,
                payload_sha256: self.payload_sha256,
            }
        }

        fn canonical_bytes(&mut self) -> Vec<u8> {
            let tensors = serde_json::to_value(&self.manifest.tensors).unwrap();
            self.manifest.tensor_contract_sha256 =
                encode_hex(&sha256(&serde_json::to_vec(&tensors).unwrap()));
            let value = serde_json::to_value(&self.manifest).unwrap();
            serde_json::to_vec(&value).unwrap()
        }

        fn validate_core(
            &self,
            bytes: &[u8],
        ) -> Result<Option<ValidatedRankManifest>, RankManifestError> {
            validate_rank_manifest_inner(bytes, self.context(), false)
        }
    }

    #[test]
    fn typed_manifest_core_is_strictly_bound_to_header_and_descriptor() {
        let mut fixture = make_fixture();
        let bytes = fixture.canonical_bytes();
        let validated = fixture.validate_core(&bytes).unwrap().unwrap();
        assert_eq!(validated.rank, 0);
        assert_eq!(validated.profile, RankWeightProfile::CapacityExl3);
        assert_eq!(validated.tensor_source_payload_bytes, 16);
        assert!(matches!(
            validate_rank_manifest(&bytes, fixture.context()),
            Err(RankManifestError::Inventory)
        ));

        fixture.manifest.tensors[0].global_shape[0] = 8;
        let bytes = fixture.canonical_bytes();
        assert!(matches!(
            fixture.validate_core(&bytes),
            Err(RankManifestError::Tensor(0))
        ));

        let mut fixture = make_fixture();
        fixture.manifest.tensors[0].role_id = 0x0701;
        let bytes = fixture.canonical_bytes();
        assert!(matches!(
            fixture.validate_core(&bytes),
            Err(RankManifestError::Tensor(0))
        ));

        let mut fixture = make_fixture();
        fixture.manifest.tensors[0].source_dtype = "F32".to_owned();
        let bytes = fixture.canonical_bytes();
        assert!(matches!(
            fixture.validate_core(&bytes),
            Err(RankManifestError::Tensor(0))
        ));

        let mut fixture = make_fixture();
        fixture.manifest.tensors[0].tp_shard_axis = 0;
        fixture.descriptors[0].tp_shard_axis = 0;
        let bytes = fixture.canonical_bytes();
        assert!(matches!(
            fixture.validate_core(&bytes),
            Err(RankManifestError::Tensor(0))
        ));
    }

    #[test]
    fn tensor_semantic_entry_is_exact_and_catalog_hash_is_domain_separated() {
        let mut fixture = make_fixture();
        let bytes = fixture.canonical_bytes();
        let validated = fixture.validate_core(&bytes).unwrap().unwrap();
        assert_eq!(validated.tensor_semantics.len(), 1);
        let semantic = validated.tensor_semantics[0];
        assert_eq!(
            semantic,
            ValidatedTensorSemantic {
                tensor_id: 0,
                role_id: 0x0003,
                codec_id: CODEC_BF16_ROW_MAJOR,
                layer_id: -1,
                expert_id: -1,
                tp_shard_axis: -1,
                ndim: 2,
                flags: 0,
                source_binding_kind: 1,
                logical_dtype: DTYPE_BF16,
                stored_dtype: DTYPE_BF16,
                quant_group_elements: 0,
                rank_logical_shape: [2, 4, 1, 1],
                global_logical_shape: [2, 4, 1, 1],
                name_sha256: sha256(b"model.norm.weight"),
                reconstruction_id: 1,
                collective_after_id: 0,
                source_dtype_id: 14,
                source_axis: -1,
            }
        );
        let encoded = semantic.encode();
        assert_eq!(encoded.len(), 128);
        assert_eq!(&encoded[0..4], 0_u32.to_le_bytes());
        assert_eq!(&encoded[4..6], 0x0003_u16.to_le_bytes());
        assert_eq!(&encoded[8..10], (-1_i16).to_le_bytes());
        assert_eq!(&encoded[10..12], (-1_i16).to_le_bytes());
        assert_eq!(encoded[12], 0xff);
        assert_eq!(encoded[13], 2);
        assert_eq!(encoded[15], 1);
        assert_eq!(
            &encoded[24..40],
            &[2, 0, 0, 0, 4, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0]
        );
        assert_eq!(&encoded[40..48], 2_u64.to_le_bytes());
        assert_eq!(&encoded[48..56], 4_u64.to_le_bytes());
        assert_eq!(&encoded[72..104], &sha256(b"model.norm.weight"));
        assert_eq!(&encoded[104..106], 1_u16.to_le_bytes());
        assert_eq!(&encoded[106..108], 0_u16.to_le_bytes());
        assert_eq!(&encoded[108..110], 14_u16.to_le_bytes());
        assert_eq!(encoded[110], 0xff);
        assert!(encoded[111..].iter().all(|&byte| byte == 0));

        let digest = rank_invariant_tensor_catalog_sha256(&validated.tensor_semantics).unwrap();
        let mut independent = Sha256::new();
        independent.update(b"glmaxx.rank-invariant-tensor-catalog.v1\0");
        independent.update(1_u32.to_le_bytes());
        independent.update(encoded);
        assert_eq!(digest, <[u8; 32]>::from(independent.finalize()));
        assert_ne!(digest, sha256(&encoded));
    }

    #[test]
    fn source_dtype_ids_cover_the_frozen_enumeration_without_aliases() {
        for (index, name) in [
            "BOOL",
            "F4",
            "F6_E2M3",
            "F6_E3M2",
            "U8",
            "I8",
            "U16",
            "I16",
            "U32",
            "I32",
            "U64",
            "I64",
            "F16",
            "BF16",
            "F32",
            "F64",
            "F8_E4M3",
            "F8_E5M2",
            "F8_E8M0",
            "F8_E4M3FNUZ",
            "F8_E5M2FNUZ",
            "C64",
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(source_dtype_id(name), u16::try_from(index + 1).ok());
        }
        assert_eq!(source_dtype_id("EXL3_TR3_COMPONENTS"), Some(0x8000));
        assert_eq!(source_dtype_id("BFLOAT16"), None);
    }

    #[test]
    fn production_manifest_rejects_unknown_fields_and_unreviewed_profiles() {
        let mut fixture = make_fixture();
        let bytes = fixture.canonical_bytes();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["toolchain"]["unreviewed"] = serde_json::Value::Bool(true);
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(matches!(
            fixture.validate_core(&bytes),
            Err(RankManifestError::Structure)
        ));

        let mut fixture = make_fixture();
        fixture.manifest.profile.name = "hybrid-serve".to_owned();
        fixture.manifest.codec.profile = "hybrid-serve-v0".to_owned();
        let bytes = fixture.canonical_bytes();
        assert!(matches!(
            fixture.validate_core(&bytes),
            Err(RankManifestError::Profile)
        ));
    }

    #[test]
    fn manifest_schema_dispatch_is_fail_closed_for_future_production_versions() {
        let fixture = make_fixture();
        assert!(matches!(
            validate_rank_manifest(
                br#"{"schema":"glmaxx.rank-manifest.v99"}"#,
                fixture.context()
            ),
            Err(RankManifestError::UnsupportedSchema)
        ));
        assert!(
            validate_rank_manifest(br#"{"schema":"reader-test"}"#, fixture.context())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn pinned_capacity_inventory_source_bindings_match_validator() {
        let mut expected_catalog = None;
        for rank in 0_u8..4 {
            let plan = crate::pinned_exl3_rank_plan(rank).unwrap();
            let tensor_value = serde_json::to_value(plan.manifest_tensors().unwrap()).unwrap();
            let tensor_contract_sha256 = sha256(&serde_json::to_vec(&tensor_value).unwrap());
            assert_eq!(
                encode_hex(&tensor_contract_sha256),
                PINNED_RANK_TENSOR_CONTRACT_SHA256[usize::from(rank)]
            );
            let tensors: Vec<RawManifestTensor> = serde_json::from_value(tensor_value).unwrap();
            assert_eq!(tensors.len(), plan.tensor_count());
            let source_payload_bytes = tensors
                .iter()
                .enumerate()
                .try_fold(0_u64, |total, (index, tensor)| {
                    validate_source_binding(tensor, rank, index)?;
                    validate_reconstruction(tensor, index)?;
                    total
                        .checked_add(tensor.primary_bytes)
                        .and_then(|value| value.checked_add(tensor.aux_bytes))
                        .ok_or(RankManifestError::Overflow)
                })
                .unwrap();
            assert_eq!(source_payload_bytes, PINNED_RANK_SOURCE_PAYLOAD_BYTES);
            assert_eq!(source_payload_bytes, plan.source_payload_bytes());
            let semantics: Vec<_> = tensors
                .iter()
                .enumerate()
                .map(|(index, tensor)| {
                    let mut rank_logical_shape = [1_u32; 4];
                    rank_logical_shape[..tensor.rank_shape.len()]
                        .copy_from_slice(&tensor.rank_shape);
                    let mut global_logical_shape = [1_u64; 4];
                    global_logical_shape[..tensor.global_shape.len()]
                        .copy_from_slice(&tensor.global_shape);
                    ValidatedTensorSemantic {
                        tensor_id: u32::try_from(index).unwrap(),
                        role_id: tensor.role_id,
                        codec_id: tensor.codec_id,
                        layer_id: tensor.layer_id,
                        expert_id: tensor.expert_id,
                        tp_shard_axis: tensor.tp_shard_axis,
                        ndim: tensor.ndim,
                        flags: tensor.flags,
                        source_binding_kind: source_binding_kind_id(&tensor.source.kind).unwrap(),
                        logical_dtype: tensor.logical_dtype,
                        stored_dtype: tensor.stored_dtype,
                        quant_group_elements: tensor.quant_group_elements,
                        rank_logical_shape,
                        global_logical_shape,
                        name_sha256: sha256(tensor.name.as_bytes()),
                        reconstruction_id: reconstruction_id(&tensor.reconstruction).unwrap(),
                        collective_after_id: collective_after_id(&tensor.collective_after).unwrap(),
                        source_dtype_id: source_dtype_id(&tensor.source_dtype).unwrap(),
                        source_axis: tensor.source.axis,
                    }
                })
                .collect();
            let catalog = rank_invariant_tensor_catalog_sha256(&semantics).unwrap();
            if let Some(expected) = expected_catalog {
                assert_eq!(catalog, expected);
            } else {
                expected_catalog = Some(catalog);
            }
        }
        assert_eq!(
            encode_hex(&expected_catalog.unwrap()),
            "bb08eef4631a430a4fe3660087c349927f25eb69281bde76d85a5f6eef222deb"
        );
    }

    fn make_fixture() -> Fixture {
        let name = "model.norm.weight".to_owned();
        let model_config_sha256 = repeated_digest(2);
        let tokenizer_sha256 = repeated_digest(3);
        let tokenizer_config_sha256 = repeated_digest(4);
        let generation_config_sha256 = repeated_digest(5);
        let chat_template_sha256 = repeated_digest(6);
        let weight_policy_sha256 = repeated_digest(11);
        let payload_sha256 = repeated_digest(13);
        let codec_metadata_sha256 = sha256(&[]);

        let mut tokenizer_bundle = Sha256::new();
        tokenizer_bundle.update(b"glmaxx-tokenizer-bundle-v0\0");
        for (name, digest) in [
            ("tokenizer.json", tokenizer_sha256),
            ("tokenizer_config.json", tokenizer_config_sha256),
            ("generation_config.json", generation_config_sha256),
        ] {
            tokenizer_bundle.update(name.as_bytes());
            tokenizer_bundle.update([0]);
            tokenizer_bundle.update(digest);
        }
        let tokenizer_bundle_sha256 = tokenizer_bundle.finalize().into();

        let mut source_file_sha256 = BTreeMap::from([
            ("calibration_manifest.json".to_owned(), digest_string(1)),
            ("config.json".to_owned(), encode_hex(&model_config_sha256)),
            (
                "model.safetensors.index.json".to_owned(),
                encode_hex(&PINNED_EXL3_INDEX_SHA256),
            ),
            ("tokenizer.json".to_owned(), encode_hex(&tokenizer_sha256)),
            (
                "tokenizer_config.json".to_owned(),
                encode_hex(&tokenizer_config_sha256),
            ),
            (
                "generation_config.json".to_owned(),
                encode_hex(&generation_config_sha256),
            ),
            (
                "chat_template.jinja".to_owned(),
                encode_hex(&chat_template_sha256),
            ),
            ("LICENSE".to_owned(), digest_string(7)),
            ("README.md".to_owned(), digest_string(8)),
        ]);
        let mut index = 0;
        while source_file_sha256.len() < PINNED_SOURCE_FILE_COUNT {
            source_file_sha256.insert(format!("extra-{index:03}.safetensors"), digest_string(9));
            index += 1;
        }

        let tensor = RawManifestTensor {
            aux_bytes: 0,
            codec_id: CODEC_BF16_ROW_MAJOR,
            codec_metadata_sha256: encode_hex(&codec_metadata_sha256),
            collective_after: "none".to_owned(),
            expert_id: -1,
            flags: 0,
            global_shape: vec![2, 4],
            layer_id: -1,
            logical_dtype: DTYPE_BF16,
            name: name.clone(),
            ndim: 2,
            padded_shape: vec![2, 4],
            primary_bytes: 16,
            quant_group_elements: 0,
            rank_shape: vec![2, 4],
            reconstruction: "byte_exact_source_precision".to_owned(),
            role_id: 0x0003,
            source: RawSourceBinding {
                axis: -1,
                components: vec![name.clone()],
                end: 0,
                kind: "replicated".to_owned(),
                start: 0,
            },
            source_dtype: "BF16".to_owned(),
            source_shape: vec![2, 4],
            stored_dtype: DTYPE_BF16,
            tensor_id: 0,
            tp_shard_axis: -1,
        };
        let descriptor = TensorDescriptor {
            tensor_id: 0,
            name_offset: 0,
            name_bytes: u16::try_from(name.len()).unwrap(),
            role_id: 0x0003,
            layer_id: -1,
            expert_id: -1,
            codec_id: CODEC_BF16_ROW_MAJOR,
            logical_dtype: DTYPE_BF16,
            stored_dtype: DTYPE_BF16,
            tp_shard_axis: -1,
            ndim: 2,
            flags: 0,
            logical_shape: [2, 4, 1, 1],
            padded_shape: [2, 4, 1, 1],
            payload_offset: 0,
            payload_bytes: 16,
            aux_offset: 0,
            aux_bytes: 0,
            codec_metadata_offset: 0,
            codec_metadata_bytes: 0,
            payload_alignment: PAYLOAD_ALIGNMENT as u32,
            quant_group_elements: 0,
            payload_sha256: repeated_digest(14),
            aux_sha256: sha256(&[]),
            codec_metadata_sha256,
        };
        let manifest = RawRankManifest {
            calibration: RawCalibration {
                manifest_file: "calibration_manifest.json".to_owned(),
                manifest_sha256: digest_string(1),
                source_revision: EXL3_MODEL_REVISION.to_owned(),
            },
            codec: RawCodec {
                exl3_source_revision: EXL3_SOURCE_REVISION.to_owned(),
                format: "g5n-v0.2.2".to_owned(),
                profile: "capacity-exl3-v0".to_owned(),
            },
            conversion: RawConversion {
                commit: "11".repeat(20),
                repository: CONVERSION_REPOSITORY.to_owned(),
            },
            integrity: RawIntegrity {
                output_hash_location: "rank-header.payload_sha256-and-descriptor-plane-sha256"
                    .to_owned(),
                output_payload_sha256: encode_hex(&payload_sha256),
                source_file_sha256,
                source_verification: "FULL_SHA256".to_owned(),
                source_verified_file_bytes: PINNED_EXL3_PAYLOAD_BYTES,
            },
            license_provenance: RawLicense {
                license_sha256: digest_string(7),
                readme_sha256: digest_string(8),
                source_repository: PINNED_EXL3_REPOSITORY.to_owned(),
            },
            model: RawModel {
                config_sha256: encode_hex(&model_config_sha256),
                operation_manifest_sha256: digest_string(10),
                repository: PINNED_EXL3_REPOSITORY.to_owned(),
                revision: EXL3_MODEL_REVISION.to_owned(),
                source_index_sha256: encode_hex(&PINNED_EXL3_INDEX_SHA256),
                source_manifest_sha256: encode_hex(&PINNED_SOURCE_MANIFEST_SHA256),
            },
            profile: RawProfile {
                name: "capacity-exl3".to_owned(),
                profile_budget_sha256: digest_string(12),
                weight_policy_sha256: encode_hex(&weight_policy_sha256),
            },
            rank: 0,
            review: RawReview {
                acceptance_token: REVIEW_ACCEPTANCE_TOKEN.to_owned(),
                artifact_sha256: digest_string(15),
                engine_spec_sha256: digest_string(16),
                format_spec_sha256: digest_string(17),
            },
            schema: PRODUCTION_RANK_MANIFEST_SCHEMA.to_owned(),
            tensor_contract_sha256: String::new(),
            tensor_count: 1,
            tensor_source_payload_bytes: 16,
            tensors: vec![tensor],
            tokenizer: RawTokenizer {
                chat_template_sha256: encode_hex(&chat_template_sha256),
                generation_config_sha256: encode_hex(&generation_config_sha256),
                tokenizer_config_sha256: encode_hex(&tokenizer_config_sha256),
                tokenizer_sha256: encode_hex(&tokenizer_sha256),
            },
            toolchain: RawToolchain {
                container_digest: format!("sha256:{}", digest_string(18)),
                cuda: "13.3".to_owned(),
                cutlass_commit: CUTLASS_COMMIT.to_owned(),
                kernel_abi: KERNEL_ABI.to_owned(),
                rust: "1.92.0".to_owned(),
            },
            tp_degree: 4,
        };
        Fixture {
            manifest,
            descriptors: vec![descriptor],
            names: vec![name],
            model_config_sha256,
            tokenizer_bundle_sha256,
            chat_template_sha256,
            weight_policy_sha256,
            kernel_abi_sha256: sha256(KERNEL_ABI.as_bytes()),
            payload_sha256,
        }
    }

    fn repeated_digest(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn digest_string(byte: u8) -> String {
        encode_hex(&repeated_digest(byte))
    }

    fn encode_hex(bytes: &[u8]) -> String {
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use std::fmt::Write;
            write!(output, "{byte:02x}").unwrap();
        }
        output
    }
}
