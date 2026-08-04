use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::File,
    io::{self, Read},
    os::unix::fs::{FileExt, MetadataExt},
    path::{Path, PathBuf},
};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    EXL3_MODEL_REVISION, Exl3Metadata, Exl3Projection, PlainDtype, SafeDtype, SafeTensorDescriptor,
    SafeTensorError, ShardedSafetensors, StreamRankError, StreamingRankWriter,
    StreamingTensorIdentity, StreamingTensorSpec,
};

pub const PINNED_EXL3_REPOSITORY: &str = "brandonmusic/GLM-5.2-EXL3-TR3-3.0bpw";
pub const PINNED_EXL3_INDEX_SHA256: [u8; 32] = [
    0x34, 0x62, 0x27, 0xa4, 0xea, 0x44, 0xb6, 0x06, 0x30, 0x17, 0x73, 0x9e, 0xe3, 0x8a, 0x83, 0x03,
    0x19, 0xdc, 0x10, 0x30, 0x5c, 0xcf, 0x71, 0x47, 0x34, 0x09, 0x5e, 0x27, 0xb2, 0x80, 0x64, 0xc2,
];
pub const PINNED_SOURCE_MANIFEST_SHA256: [u8; 32] = [
    0xbf, 0xb6, 0xdc, 0x39, 0xf2, 0x8d, 0xa0, 0x8c, 0x1c, 0xfc, 0x5b, 0x89, 0x60, 0x34, 0x14, 0x04,
    0x6a, 0xdf, 0x70, 0x03, 0x15, 0x2d, 0x69, 0xe9, 0xee, 0x35, 0x0e, 0x11, 0xf7, 0xa1, 0xfa, 0x63,
];
pub const PINNED_SOURCE_FILE_MAP_SHA256: [u8; 32] = [
    0xad, 0x1e, 0x4f, 0xb2, 0x86, 0xad, 0xbc, 0x26, 0x1a, 0x28, 0x00, 0xab, 0x17, 0xe4, 0xab, 0xde,
    0x5b, 0xcd, 0x13, 0xef, 0xb2, 0x2b, 0x15, 0x0d, 0x65, 0xec, 0x42, 0xb4, 0x7e, 0x2a, 0xf5, 0xfe,
];
const PINNED_GITATTRIBUTES_MANIFEST_SHA256: [u8; 32] = [
    0x34, 0x44, 0x8b, 0x82, 0xc1, 0x7d, 0x60, 0xfe, 0xc9, 0xb6, 0x5b, 0x1f, 0x09, 0x3c, 0x11, 0x5d,
    0xdb, 0xaa, 0xdc, 0x04, 0xbe, 0xb1, 0xb0, 0x14, 0x0b, 0x6b, 0xfe, 0xd2, 0xe0, 0x12, 0xa9, 0x30,
];
const PINNED_GITATTRIBUTES_REVISION_SHA256: [u8; 32] = [
    0x5b, 0xb3, 0x6c, 0x32, 0x04, 0x17, 0xdb, 0x43, 0xaf, 0x1d, 0xc6, 0xaf, 0x8b, 0xd0, 0xfc, 0xc1,
    0x54, 0xbb, 0x72, 0x76, 0xed, 0xda, 0xf9, 0x6b, 0x12, 0xc3, 0x95, 0xbd, 0xaf, 0xed, 0x63, 0x4d,
];
const PINNED_README_MANIFEST_SHA256: [u8; 32] = [
    0xed, 0x5a, 0xca, 0x8c, 0xe3, 0xdc, 0x5f, 0x8d, 0xe6, 0x26, 0xc8, 0x7e, 0x48, 0x84, 0x44, 0x34,
    0x3e, 0x43, 0xb1, 0xdc, 0xbd, 0xeb, 0x0e, 0x64, 0x3d, 0xc7, 0x2f, 0xea, 0x63, 0xab, 0x06, 0xe8,
];
const PINNED_README_REVISION_SHA256: [u8; 32] = [
    0xe6, 0x0e, 0x02, 0x30, 0x82, 0xee, 0x17, 0x5a, 0x11, 0xf5, 0x1e, 0x79, 0xe8, 0xdd, 0x88, 0xf5,
    0xe4, 0xed, 0x99, 0x75, 0xfc, 0x90, 0x4e, 0x64, 0xcd, 0xea, 0xbb, 0xbc, 0xf8, 0xab, 0xe2, 0x25,
];
pub const PINNED_SOURCE_FILE_COUNT: usize = 92;
pub const PINNED_EXL3_TENSOR_COUNT: usize = 935_105;
pub const PINNED_EXL3_SHARD_COUNT: usize = 81;
pub const PINNED_EXL3_PAYLOAD_BYTES: u64 = 316_304_795_648;
pub const PINNED_EXL3_COMPONENT_COUNT: usize = 933_888;
pub const PINNED_PROTECTED_TENSOR_COUNT: usize = 1_217;
pub const PINNED_RANK_TENSOR_COUNT: usize = 59_585;
pub const PINNED_RANK_SOURCE_PAYLOAD_BYTES: u64 = 81_590_319_104;
pub const TP_DEGREE: u8 = 4;
const CONVERSION_SYNC_BATCH_TENSORS: usize = 64;
const SOURCE_MANIFEST_MAX_BYTES: u64 = 64 * 1024;
const SOURCE_HASH_BUFFER_BYTES: usize = 8 * 1024 * 1024;

const FLAG_REPLICATED: u8 = 1 << 0;
const FLAG_COLUMN_PARALLEL: u8 = 1 << 1;
const FLAG_ROW_PARALLEL: u8 = 1 << 2;
const FLAG_ROUTED_EXPERT: u8 = 1 << 3;
const FLAG_SHARED_EXPERT: u8 = 1 << 4;
const FLAG_MTP: u8 = 1 << 5;
const FLAG_PROTECTED: u8 = 1 << 6;

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PinnedSourceVerification {
    pub manifest_sha256: [u8; 32],
    pub verified_file_bytes: u64,
    source_markers_verified: bool,
    file_sha256: BTreeMap<String, [u8; 32]>,
    manifest_exceptions: BTreeMap<String, PinnedManifestException>,
}

/// Canonical filename-to-SHA-256 source manifest parsed from exact bytes.
///
/// This type deliberately proves only syntax, uniqueness, and the digest of
/// the manifest bytes. A checkpoint profile must separately bind that digest,
/// the required file set, repository/revision identity, tensor inventory, and
/// any narrowly reviewed publisher exceptions before these rows become an
/// admission authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSourceManifest {
    sha256: [u8; 32],
    files: BTreeMap<String, [u8; 32]>,
}

impl CanonicalSourceManifest {
    pub fn parse(bytes: &[u8]) -> Result<Self, PinnedSourceError> {
        if bytes.last() != Some(&b'\n') {
            return Err(PinnedSourceError::ManifestSyntax);
        }
        let mut files = BTreeMap::new();
        for line in bytes[..bytes.len() - 1].split(|&byte| byte == b'\n') {
            if line.len() < 67 || &line[64..66] != b"  " {
                return Err(PinnedSourceError::ManifestSyntax);
            }
            let name =
                std::str::from_utf8(&line[66..]).map_err(|_| PinnedSourceError::ManifestSyntax)?;
            if name.is_empty()
                || name == "."
                || name == ".."
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
            {
                return Err(PinnedSourceError::ManifestSyntax);
            }
            let digest = decode_sha256(&line[..64])?;
            if files.insert(name.to_owned(), digest).is_some() {
                return Err(PinnedSourceError::ManifestSyntax);
            }
        }
        Ok(Self {
            sha256: Sha256::digest(bytes).into(),
            files,
        })
    }

    #[must_use]
    pub const fn sha256(&self) -> [u8; 32] {
        self.sha256
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    #[must_use]
    pub fn file_sha256(&self, name: &str) -> Option<[u8; 32]> {
        self.files.get(name).copied()
    }

    #[must_use]
    pub fn files(&self) -> &BTreeMap<String, [u8; 32]> {
        &self.files
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PinnedManifestException {
    pub manifest_sha256: [u8; 32],
    pub revision_sha256: [u8; 32],
}

impl PinnedSourceVerification {
    #[must_use]
    pub const fn manifest_sha256(&self) -> [u8; 32] {
        self.manifest_sha256
    }

    #[must_use]
    pub fn file_count(&self) -> usize {
        self.file_sha256.len()
    }

    #[must_use]
    pub const fn verified_file_bytes(&self) -> u64 {
        self.verified_file_bytes
    }

    #[must_use]
    pub const fn source_markers_verified(&self) -> bool {
        self.source_markers_verified
    }

    #[must_use]
    pub fn file_sha256(&self, name: &str) -> Option<[u8; 32]> {
        self.file_sha256.get(name).copied()
    }

    #[must_use]
    pub fn files(&self) -> &BTreeMap<String, [u8; 32]> {
        &self.file_sha256
    }

    #[must_use]
    pub fn manifest_exceptions(&self) -> &BTreeMap<String, PinnedManifestException> {
        &self.manifest_exceptions
    }
}

#[derive(Clone, Debug)]
enum PinnedTensorSource {
    Protected { name: String, tp_axis: i8 },
    Exl3 { stem: String },
}

#[derive(Clone, Debug)]
struct PinnedRankTensor {
    spec: StreamingTensorSpec,
    source: PinnedTensorSource,
}

#[derive(Clone, Debug)]
pub struct PinnedRankPlan {
    rank: u8,
    tensors: Vec<PinnedRankTensor>,
    source_payload_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PinnedSourceBinding {
    pub axis: i8,
    pub components: Vec<String>,
    pub end: u64,
    pub kind: &'static str,
    pub start: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PinnedRankManifestTensor {
    pub aux_bytes: u64,
    pub codec_id: u16,
    pub codec_metadata_sha256: String,
    pub collective_after: &'static str,
    pub expert_id: i16,
    pub flags: u8,
    pub global_shape: Vec<u64>,
    pub layer_id: i16,
    pub logical_dtype: u16,
    pub name: String,
    pub ndim: u8,
    pub padded_shape: Vec<u32>,
    pub primary_bytes: u64,
    pub quant_group_elements: u32,
    pub rank_shape: Vec<u32>,
    pub reconstruction: &'static str,
    pub role_id: u16,
    pub source: PinnedSourceBinding,
    pub source_dtype: &'static str,
    pub source_shape: Vec<u64>,
    pub stored_dtype: u16,
    pub tensor_id: u32,
    pub tp_shard_axis: i8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PinnedConversionProgress {
    pub completed_tensors: usize,
    pub total_tensors: usize,
    pub completed_payload_bytes: u64,
    pub total_payload_bytes: u64,
}

impl PinnedRankPlan {
    #[must_use]
    pub const fn rank(&self) -> u8 {
        self.rank
    }

    #[must_use]
    pub fn tensor_specs(&self) -> Vec<StreamingTensorSpec> {
        self.tensors
            .iter()
            .map(|tensor| tensor.spec.clone())
            .collect()
    }

    #[must_use]
    pub fn tensor_count(&self) -> usize {
        self.tensors.len()
    }

    #[must_use]
    pub const fn source_payload_bytes(&self) -> u64 {
        self.source_payload_bytes
    }

    pub fn manifest_tensors(&self) -> Result<Vec<PinnedRankManifestTensor>, CheckpointError> {
        let protected = protected_tensor_contracts();
        self.tensors
            .iter()
            .map(|tensor| {
                let spec = &tensor.spec;
                let rank_shape = spec.logical_shape()[..usize::from(spec.ndim())].to_vec();
                let padded_shape = spec.padded_shape()[..usize::from(spec.ndim())].to_vec();
                let (global_shape, source_shape, source_dtype, source, reconstruction) =
                    match &tensor.source {
                        PinnedTensorSource::Protected { name, tp_axis } => {
                            let contract = protected.get(name).ok_or(CheckpointError::Internal)?;
                            let (kind, start, end) = if *tp_axis < 0 {
                                ("replicated", 0, 0)
                            } else {
                                let axis = usize::try_from(*tp_axis)
                                    .map_err(|_| CheckpointError::Internal)?;
                                let extent = *contract
                                    .source_shape
                                    .get(axis)
                                    .ok_or(CheckpointError::Internal)?;
                                let shard = extent
                                    .checked_div(u64::from(TP_DEGREE))
                                    .ok_or(CheckpointError::Overflow)?;
                                let start = shard
                                    .checked_mul(u64::from(self.rank))
                                    .ok_or(CheckpointError::Overflow)?;
                                (
                                    "contiguous_tp_slice",
                                    start,
                                    start.checked_add(shard).ok_or(CheckpointError::Overflow)?,
                                )
                            };
                            (
                                contract.source_shape.clone(),
                                contract.source_shape.clone(),
                                contract.dtype.name(),
                                PinnedSourceBinding {
                                    axis: *tp_axis,
                                    components: vec![name.clone()],
                                    end,
                                    kind,
                                    start,
                                },
                                "byte_exact_source_precision",
                            )
                        }
                        PinnedTensorSource::Exl3 { stem } => {
                            let mut global_shape = rank_shape
                                .iter()
                                .map(|&extent| u64::from(extent))
                                .collect::<Vec<_>>();
                            let axis = usize::try_from(spec.tp_shard_axis)
                                .map_err(|_| CheckpointError::Internal)?;
                            global_shape[axis] = global_shape[axis]
                                .checked_mul(u64::from(TP_DEGREE))
                                .ok_or(CheckpointError::Overflow)?;
                            (
                                global_shape,
                                Vec::new(),
                                "EXL3_TR3_COMPONENTS",
                                PinnedSourceBinding {
                                    axis: spec.tp_shard_axis,
                                    components: ["mcg", "suh", "svh", "trellis"]
                                        .into_iter()
                                        .map(|component| format!("{stem}.{component}"))
                                        .collect(),
                                    end: u64::from(self.rank) + 1,
                                    kind: "explicit_rank_components",
                                    start: u64::from(self.rank),
                                },
                                "exl3_tr3_trellis_v0",
                            )
                        }
                    };
                Ok(PinnedRankManifestTensor {
                    aux_bytes: spec.aux_bytes(),
                    codec_id: spec.codec_id(),
                    codec_metadata_sha256: encode_hex(&spec.metadata_sha256()),
                    collective_after: collective_after(spec.role_id),
                    expert_id: spec.expert_id,
                    flags: spec.flags,
                    global_shape,
                    layer_id: spec.layer_id,
                    logical_dtype: spec.logical_dtype(),
                    name: spec.name.clone(),
                    ndim: spec.ndim(),
                    padded_shape,
                    primary_bytes: spec.primary_bytes(),
                    quant_group_elements: spec.quant_group_elements(),
                    rank_shape,
                    reconstruction,
                    role_id: spec.role_id,
                    source,
                    source_dtype,
                    source_shape,
                    stored_dtype: spec.stored_dtype(),
                    tensor_id: spec.tensor_id,
                    tp_shard_axis: spec.tp_shard_axis,
                })
            })
            .collect()
    }

    pub fn write_incomplete(
        &self,
        checkpoint: &ShardedSafetensors,
        writer: &mut StreamingRankWriter,
    ) -> Result<(), CheckpointConversionError> {
        self.write_incomplete_with_progress(checkpoint, writer, |_| {})
    }

    pub fn write_incomplete_with_progress(
        &self,
        checkpoint: &ShardedSafetensors,
        writer: &mut StreamingRankWriter,
        mut progress: impl FnMut(PinnedConversionProgress),
    ) -> Result<(), CheckpointConversionError> {
        let mut completed_payload_bytes = 0_u64;
        let mut pending_payload_bytes = 0_u64;
        let mut pending_tensors = 0_usize;
        for (index, tensor) in self.tensors.iter().enumerate() {
            if writer.tensor_spec(index) != Some(&tensor.spec) {
                return Err(CheckpointConversionError::Plan);
            }
            if writer.tensor_complete(index)? {
                completed_payload_bytes = completed_payload_bytes
                    .checked_add(tensor_payload_bytes(&tensor.spec)?)
                    .ok_or(CheckpointConversionError::Plan)?;
                continue;
            }
            match &tensor.source {
                PinnedTensorSource::Protected { name, tp_axis } => {
                    let mut primary: Box<dyn Read> = if *tp_axis < 0 {
                        Box::new(checkpoint.tensor_reader(name)?)
                    } else {
                        Box::new(checkpoint.tensor_shard_reader(
                            name,
                            u8::try_from(*tp_axis).map_err(|_| CheckpointConversionError::Plan)?,
                            self.rank,
                            TP_DEGREE,
                        )?)
                    };
                    let mut aux = io::empty();
                    writer.write_tensor_deferred(index, &mut primary, &mut aux)?;
                }
                PinnedTensorSource::Exl3 { stem } => {
                    let mut primary = checkpoint.tensor_reader(&format!("{stem}.trellis"))?;
                    let marker = checkpoint.tensor_reader(&format!("{stem}.mcg"))?;
                    let suh = checkpoint.tensor_reader(&format!("{stem}.suh"))?;
                    let svh = checkpoint.tensor_reader(&format!("{stem}.svh"))?;
                    let mut aux = marker.chain(suh).chain(svh);
                    writer.write_tensor_deferred(index, &mut primary, &mut aux)?;
                }
            }
            pending_payload_bytes = pending_payload_bytes
                .checked_add(tensor_payload_bytes(&tensor.spec)?)
                .ok_or(CheckpointConversionError::Plan)?;
            pending_tensors += 1;
            if pending_tensors == CONVERSION_SYNC_BATCH_TENSORS {
                writer.commit_pending()?;
                completed_payload_bytes = completed_payload_bytes
                    .checked_add(pending_payload_bytes)
                    .ok_or(CheckpointConversionError::Plan)?;
                pending_payload_bytes = 0;
                pending_tensors = 0;
                progress(PinnedConversionProgress {
                    completed_tensors: writer.completed_tensors(),
                    total_tensors: self.tensor_count(),
                    completed_payload_bytes,
                    total_payload_bytes: self.source_payload_bytes,
                });
            }
        }
        writer.commit_pending()?;
        completed_payload_bytes = completed_payload_bytes
            .checked_add(pending_payload_bytes)
            .ok_or(CheckpointConversionError::Plan)?;
        progress(PinnedConversionProgress {
            completed_tensors: writer.completed_tensors(),
            total_tensors: self.tensor_count(),
            completed_payload_bytes,
            total_payload_bytes: self.source_payload_bytes,
        });
        Ok(())
    }
}

#[must_use]
pub fn pinned_exl3_weight_policy_sha256() -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"glmaxx-weight-policy-v0\0capacity-exl3-v0\0");
    let protected = protected_tensor_contracts();
    for contract in protected.values() {
        hasher.update(contract.name.as_bytes());
        hasher.update([0]);
        hasher.update(contract.role_id.to_le_bytes());
        hasher.update([contract.tp_axis as u8]);
        hasher.update(contract.dtype.name().as_bytes());
        hasher.update([0]);
    }
    for layer in 3_u16..=78 {
        for expert in 0_u16..256 {
            for (projection, role_id, tp_axis) in [
                ("gate", ROLE_ROUTED_GATE_UP, 0_i8),
                ("up", ROLE_ROUTED_GATE_UP, 0_i8),
                ("down", ROLE_ROUTED_DOWN, 1_i8),
            ] {
                hasher.update(
                    format!("model.layers.{layer}.mlp.experts.{expert}.{projection}_proj.weight")
                        .as_bytes(),
                );
                hasher.update([0]);
                hasher.update(role_id.to_le_bytes());
                hasher.update([tp_axis as u8]);
                hasher.update(0x0200_u16.to_le_bytes());
            }
        }
    }
    hasher.finalize().into()
}

const fn collective_after(role_id: u16) -> &'static str {
    match role_id {
        ROLE_EMBEDDING => "tp_embedding_reduce",
        ROLE_LM_HEAD => "distributed_sampling",
        ROLE_O_PROJ | ROLE_DENSE_DOWN | ROLE_ROUTED_DOWN | ROLE_SHARED_DOWN => "tp_all_reduce",
        _ => "none",
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn tensor_payload_bytes(spec: &StreamingTensorSpec) -> Result<u64, CheckpointConversionError> {
    spec.primary_bytes()
        .checked_add(spec.aux_bytes())
        .ok_or(CheckpointConversionError::Plan)
}

/// Recomputes the SHA-256 of every file named by the checkpoint's immutable
/// source manifest. Weight shards are hashed through the descriptors already
/// opened by [`ShardedSafetensors`], closing the verify-then-reopen race.
pub fn verify_pinned_source_files(
    checkpoint: &ShardedSafetensors,
    mut progress: impl FnMut(usize, usize, u64, &str),
) -> Result<PinnedSourceVerification, PinnedSourceError> {
    let source_path = checkpoint.source_path();
    if source_path.file_name().and_then(|name| name.to_str())
        != Some("model.safetensors.index.json")
    {
        return Err(PinnedSourceError::Inventory);
    }
    let root = source_path.parent().ok_or(PinnedSourceError::Inventory)?;
    let source_markers_verified = verify_optional_source_markers(root)?;

    let manifest_path = root.join("MANIFEST.sha256");
    let (manifest_bytes, manifest_fingerprint) =
        read_small_regular_file(&manifest_path, SOURCE_MANIFEST_MAX_BYTES)?;
    let manifest = CanonicalSourceManifest::parse(&manifest_bytes)?;
    if manifest.sha256() != PINNED_SOURCE_MANIFEST_SHA256 {
        return Err(PinnedSourceError::ManifestIdentity);
    }
    verify_regular_fingerprint(&manifest_path, &manifest_fingerprint)?;
    let expected = manifest.files();
    if manifest.len() != PINNED_SOURCE_FILE_COUNT
        || manifest.file_sha256("model.safetensors.index.json") != Some(PINNED_EXL3_INDEX_SHA256)
        || checkpoint
            .shards()
            .iter()
            .any(|shard| !expected.contains_key(&shard.to_string_lossy().into_owned()))
    {
        return Err(PinnedSourceError::Inventory);
    }

    let mut verified_file_bytes = 0_u64;
    let mut verified_files = BTreeMap::new();
    let mut manifest_exceptions = BTreeMap::new();
    for (completed, (name, expected_sha256)) in expected.iter().enumerate() {
        let path = root.join(name);
        let (observed_sha256, bytes) = if checkpoint.shards().contains(Path::new(name)) {
            let digest = checkpoint.hash_shard_file(name)?;
            let metadata = path.symlink_metadata().map_err(PinnedSourceError::Io)?;
            (digest, metadata.len())
        } else {
            hash_regular_file(&path)?
        };
        if &observed_sha256 != expected_sha256 {
            if !is_pinned_publisher_manifest_exception(name, expected_sha256, &observed_sha256) {
                return Err(PinnedSourceError::FileDigest(name.clone()));
            }
            manifest_exceptions.insert(
                name.clone(),
                PinnedManifestException {
                    manifest_sha256: *expected_sha256,
                    revision_sha256: observed_sha256,
                },
            );
        }
        verified_files.insert(name.clone(), observed_sha256);
        verified_file_bytes = verified_file_bytes
            .checked_add(bytes)
            .ok_or(PinnedSourceError::Overflow)?;
        progress(completed + 1, expected.len(), verified_file_bytes, name);
    }
    let verified_file_hex: BTreeMap<_, _> = verified_files
        .iter()
        .map(|(name, digest)| (name, encode_hex(digest)))
        .collect();
    let canonical_file_map =
        serde_json::to_vec(&verified_file_hex).map_err(|_| PinnedSourceError::ManifestIdentity)?;
    if Sha256::digest(&canonical_file_map).as_slice() != PINNED_SOURCE_FILE_MAP_SHA256 {
        return Err(PinnedSourceError::ManifestIdentity);
    }
    Ok(PinnedSourceVerification {
        manifest_sha256: PINNED_SOURCE_MANIFEST_SHA256,
        verified_file_bytes,
        source_markers_verified,
        file_sha256: verified_files,
        manifest_exceptions,
    })
}

fn is_pinned_publisher_manifest_exception(
    name: &str,
    manifest_sha256: &[u8; 32],
    revision_sha256: &[u8; 32],
) -> bool {
    [
        (
            ".gitattributes",
            &PINNED_GITATTRIBUTES_MANIFEST_SHA256,
            &PINNED_GITATTRIBUTES_REVISION_SHA256,
        ),
        (
            "README.md",
            &PINNED_README_MANIFEST_SHA256,
            &PINNED_README_REVISION_SHA256,
        ),
    ]
    .into_iter()
    .any(|(exception_name, exception_manifest, exception_revision)| {
        name == exception_name
            && manifest_sha256 == exception_manifest
            && revision_sha256 == exception_revision
    })
}

fn decode_sha256(bytes: &[u8]) -> Result<[u8; 32], PinnedSourceError> {
    if bytes.len() != 64 {
        return Err(PinnedSourceError::ManifestSyntax);
    }
    let mut digest = [0_u8; 32];
    for (output, pair) in digest.iter_mut().zip(bytes.chunks_exact(2)) {
        *output = decode_hex_nibble(pair[0])
            .and_then(|high| decode_hex_nibble(pair[1]).map(|low| high << 4 | low))
            .ok_or(PinnedSourceError::ManifestSyntax)?;
    }
    Ok(digest)
}

fn decode_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceFingerprint {
    device: u64,
    inode: u64,
    bytes: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
}

impl SourceFingerprint {
    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            bytes: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
        }
    }
}

/// A source file held open across hashing and semantic inspection.
///
/// Construction rejects symlinks, non-regular files, and any file with more
/// than one hard link. [`Self::revalidate`] proves that the path still names
/// the same device/inode/length/mtime identity observed by both the pathname
/// and the opened descriptor. Checkpoint admission must retain this value
/// until it has consumed the authenticated bytes; reopening `path()` is not an
/// equivalent operation.
#[derive(Debug)]
pub struct RetainedSourceFile {
    path: PathBuf,
    file: File,
    fingerprint: SourceFingerprint,
}

impl RetainedSourceFile {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PinnedSourceError> {
        let path = path.as_ref();
        let path_metadata = path.symlink_metadata().map_err(PinnedSourceError::Io)?;
        if !path_metadata.file_type().is_file()
            || path_metadata.file_type().is_symlink()
            || path_metadata.nlink() != 1
        {
            return Err(PinnedSourceError::UnsafePath(path.to_owned()));
        }
        let file = File::open(path).map_err(PinnedSourceError::Io)?;
        let file_metadata = file.metadata().map_err(PinnedSourceError::Io)?;
        let fingerprint = SourceFingerprint::from_metadata(&path_metadata);
        if SourceFingerprint::from_metadata(&file_metadata) != fingerprint {
            return Err(PinnedSourceError::UnsafePath(path.to_owned()));
        }
        Ok(Self {
            path: path.to_owned(),
            file,
            fingerprint,
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub const fn len(&self) -> u64 {
        self.fingerprint.bytes
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn sha256(&self) -> Result<[u8; 32], PinnedSourceError> {
        let mut buffer = vec![
            0_u8;
            usize::try_from(self.len().clamp(1, SOURCE_HASH_BUFFER_BYTES as u64))
                .map_err(|_| PinnedSourceError::Overflow)?
        ];
        let mut hasher = Sha256::new();
        let mut consumed = 0_u64;
        while consumed < self.len() {
            let chunk = usize::try_from((self.len() - consumed).min(buffer.len() as u64))
                .map_err(|_| PinnedSourceError::Overflow)?;
            read_exact_source_at(&self.file, &mut buffer[..chunk], consumed)?;
            hasher.update(&buffer[..chunk]);
            consumed += chunk as u64;
        }
        self.revalidate()?;
        Ok(hasher.finalize().into())
    }

    pub fn revalidate(&self) -> Result<(), PinnedSourceError> {
        let file_metadata = self.file.metadata().map_err(PinnedSourceError::Io)?;
        if SourceFingerprint::from_metadata(&file_metadata) != self.fingerprint {
            return Err(PinnedSourceError::SourceChanged(self.path.clone()));
        }
        verify_regular_fingerprint(&self.path, &self.fingerprint)
    }
}

fn open_regular_file(path: &Path) -> Result<(File, SourceFingerprint), PinnedSourceError> {
    let retained = RetainedSourceFile::open(path)?;
    Ok((retained.file, retained.fingerprint))
}

fn read_small_regular_file(
    path: &Path,
    maximum_bytes: u64,
) -> Result<(Vec<u8>, SourceFingerprint), PinnedSourceError> {
    let (file, fingerprint) = open_regular_file(path)?;
    if fingerprint.bytes == 0 || fingerprint.bytes > maximum_bytes {
        return Err(PinnedSourceError::ManifestSyntax);
    }
    let mut bytes =
        vec![0_u8; usize::try_from(fingerprint.bytes).map_err(|_| PinnedSourceError::Overflow)?];
    read_exact_source_at(&file, &mut bytes, 0)?;
    verify_regular_fingerprint(path, &fingerprint)?;
    Ok((bytes, fingerprint))
}

fn hash_regular_file(path: &Path) -> Result<([u8; 32], u64), PinnedSourceError> {
    let retained = RetainedSourceFile::open(path)?;
    let bytes = retained.len();
    Ok((retained.sha256()?, bytes))
}

fn verify_regular_fingerprint(
    path: &Path,
    expected: &SourceFingerprint,
) -> Result<(), PinnedSourceError> {
    let metadata = path.symlink_metadata().map_err(PinnedSourceError::Io)?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || &SourceFingerprint::from_metadata(&metadata) != expected
    {
        return Err(PinnedSourceError::SourceChanged(path.to_owned()));
    }
    Ok(())
}

fn verify_source_marker(path: &Path, expected: &str) -> Result<(), PinnedSourceError> {
    let (bytes, fingerprint) = read_small_regular_file(path, 1024)?;
    if bytes != format!("{expected}\n").as_bytes() {
        return Err(PinnedSourceError::SourceMarker(path.to_owned()));
    }
    verify_regular_fingerprint(path, &fingerprint)
}

fn verify_optional_source_markers(root: &Path) -> Result<bool, PinnedSourceError> {
    let repository_marker = root.join("glmaxx-source-repository.txt");
    let revision_marker = root.join("glmaxx-source-revision.txt");
    match (
        repository_marker
            .try_exists()
            .map_err(PinnedSourceError::Io)?,
        revision_marker
            .try_exists()
            .map_err(PinnedSourceError::Io)?,
    ) {
        (false, false) => Ok(false),
        (true, true) => {
            verify_source_marker(&repository_marker, PINNED_EXL3_REPOSITORY)?;
            verify_source_marker(&revision_marker, EXL3_MODEL_REVISION)?;
            Ok(true)
        }
        _ => Err(PinnedSourceError::SourceMarkerSet),
    }
}

fn read_exact_source_at(
    file: &File,
    mut output: &mut [u8],
    mut offset: u64,
) -> Result<(), PinnedSourceError> {
    while !output.is_empty() {
        let read = file
            .read_at(output, offset)
            .map_err(PinnedSourceError::Io)?;
        if read == 0 {
            return Err(PinnedSourceError::Io(io::Error::from(
                io::ErrorKind::UnexpectedEof,
            )));
        }
        offset = offset
            .checked_add(read as u64)
            .ok_or(PinnedSourceError::Overflow)?;
        output = &mut output[read..];
    }
    Ok(())
}

pub fn pinned_exl3_rank_plan(rank: u8) -> Result<PinnedRankPlan, CheckpointError> {
    if rank >= TP_DEGREE {
        return Err(CheckpointError::Rank(rank));
    }
    let mut sources = BTreeMap::new();
    for contract in protected_tensor_contracts().into_values() {
        let mut flags = FLAG_PROTECTED;
        flags |= match contract.tp_axis {
            -1 => FLAG_REPLICATED,
            0 => FLAG_COLUMN_PARALLEL,
            1 => FLAG_ROW_PARALLEL,
            _ => return Err(CheckpointError::Internal),
        };
        if contract.role_id & 0xff00 == 0x0600 {
            flags |= FLAG_SHARED_EXPERT;
        }
        if contract.is_mtp {
            flags |= FLAG_MTP;
        }
        let shape = shape4(&contract.rank_shape)?;
        let dtype = match contract.dtype {
            SafeDtype::Bf16 => PlainDtype::Bf16,
            SafeDtype::F16 => PlainDtype::Fp16,
            SafeDtype::F32 => PlainDtype::Fp32,
            _ => return Err(CheckpointError::Internal),
        };
        let identity = StreamingTensorIdentity {
            tensor_id: 0,
            name: contract.name.clone(),
            role_id: contract.role_id,
            layer_id: contract.layer_id,
            expert_id: -1,
            tp_shard_axis: contract.tp_axis,
            flags,
        };
        let spec = StreamingTensorSpec::plain(
            identity,
            dtype,
            u8::try_from(contract.rank_shape.len()).map_err(|_| CheckpointError::Overflow)?,
            shape,
            shape,
        )
        .map_err(CheckpointError::Stream)?;
        if sources
            .insert(
                contract.name.clone(),
                (
                    spec,
                    PinnedTensorSource::Protected {
                        name: contract.name,
                        tp_axis: contract.tp_axis,
                    },
                ),
            )
            .is_some()
        {
            return Err(CheckpointError::Internal);
        }
    }

    for layer in 3_u16..=78 {
        for expert in 0_u16..256 {
            for projection in [
                Exl3Projection::Gate,
                Exl3Projection::Up,
                Exl3Projection::Down,
            ] {
                let projection_name = match projection {
                    Exl3Projection::Gate => "gate",
                    Exl3Projection::Up => "up",
                    Exl3Projection::Down => "down",
                };
                let (logical_k, logical_n, role_id, tp_axis, parallel_flag) = match projection {
                    Exl3Projection::Gate | Exl3Projection::Up => {
                        (6_144, 512, ROLE_ROUTED_GATE_UP, 0, FLAG_COLUMN_PARALLEL)
                    }
                    Exl3Projection::Down => (512, 6_144, ROLE_ROUTED_DOWN, 1, FLAG_ROW_PARALLEL),
                };
                let name = format!(
                    "model.layers.{layer}.mlp.experts.{expert}.{projection_name}_proj.weight"
                );
                let stem = format!(
                    "model.layers.{layer}.mlp.experts.{expert}.{projection_name}_proj.rank{rank}"
                );
                let metadata =
                    Exl3Metadata::new(projection, layer, expert, rank, 3, logical_k, logical_n)
                        .map_err(|_| CheckpointError::Internal)?;
                let spec = StreamingTensorSpec::exl3_source(
                    StreamingTensorIdentity {
                        tensor_id: 0,
                        name: name.clone(),
                        role_id,
                        layer_id: layer as i16,
                        expert_id: expert as i16,
                        tp_shard_axis: tp_axis,
                        flags: parallel_flag
                            | FLAG_ROUTED_EXPERT
                            | if layer == 78 { FLAG_MTP } else { 0 },
                    },
                    metadata,
                )
                .map_err(CheckpointError::Stream)?;
                if sources
                    .insert(name, (spec, PinnedTensorSource::Exl3 { stem }))
                    .is_some()
                {
                    return Err(CheckpointError::Internal);
                }
            }
        }
    }

    if sources.len() != PINNED_RANK_TENSOR_COUNT {
        return Err(CheckpointError::Internal);
    }
    let mut source_payload_bytes = 0_u64;
    let mut tensors = Vec::with_capacity(sources.len());
    for (index, (_, (mut spec, source))) in sources.into_iter().enumerate() {
        spec.tensor_id = u32::try_from(index).map_err(|_| CheckpointError::Overflow)?;
        source_payload_bytes = source_payload_bytes
            .checked_add(spec.primary_bytes())
            .and_then(|bytes| bytes.checked_add(spec.aux_bytes()))
            .ok_or(CheckpointError::Overflow)?;
        tensors.push(PinnedRankTensor { spec, source });
    }
    Ok(PinnedRankPlan {
        rank,
        tensors,
        source_payload_bytes,
    })
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

fn shape4(shape: &[u64]) -> Result<[u32; 4], CheckpointError> {
    if shape.is_empty() || shape.len() > 4 {
        return Err(CheckpointError::Internal);
    }
    let mut output = [1_u32; 4];
    for (destination, &source) in output.iter_mut().zip(shape) {
        *destination = u32::try_from(source).map_err(|_| CheckpointError::Overflow)?;
    }
    Ok(output)
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
    Rank(u8),
    Stream(StreamRankError),
    Internal,
}

impl fmt::Display for CheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CheckpointError {}

#[derive(Debug)]
pub enum PinnedSourceError {
    Io(io::Error),
    SafeTensor(SafeTensorError),
    ManifestIdentity,
    ManifestSyntax,
    Inventory,
    FileDigest(String),
    SourceMarkerSet,
    SourceMarker(PathBuf),
    UnsafePath(PathBuf),
    SourceChanged(PathBuf),
    Overflow,
}

impl From<SafeTensorError> for PinnedSourceError {
    fn from(error: SafeTensorError) -> Self {
        Self::SafeTensor(error)
    }
}

impl fmt::Display for PinnedSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for PinnedSourceError {}

#[derive(Debug)]
pub enum CheckpointConversionError {
    SafeTensor(SafeTensorError),
    Stream(StreamRankError),
    Plan,
}

impl From<SafeTensorError> for CheckpointConversionError {
    fn from(error: SafeTensorError) -> Self {
        Self::SafeTensor(error)
    }
}

impl From<StreamRankError> for CheckpointConversionError {
    fn from(error: StreamRankError) -> Self {
        Self::Stream(error)
    }
}

impl fmt::Display for CheckpointConversionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CheckpointConversionError {}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct TempDirectory(PathBuf);

    impl TempDirectory {
        fn new() -> Self {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "glmaxx-checkpoint-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn source_markers_are_optional_as_a_complete_exact_pair_only() {
        let root = TempDirectory::new();
        assert!(!verify_optional_source_markers(&root.0).unwrap());

        let repository = root.0.join("glmaxx-source-repository.txt");
        let revision = root.0.join("glmaxx-source-revision.txt");
        fs::write(&repository, format!("{PINNED_EXL3_REPOSITORY}\n")).unwrap();
        assert!(matches!(
            verify_optional_source_markers(&root.0),
            Err(PinnedSourceError::SourceMarkerSet)
        ));

        fs::write(&revision, format!("{EXL3_MODEL_REVISION}\n")).unwrap();
        assert!(verify_optional_source_markers(&root.0).unwrap());

        fs::write(&revision, b"wrong\n").unwrap();
        assert!(matches!(
            verify_optional_source_markers(&root.0),
            Err(PinnedSourceError::SourceMarker(_))
        ));
    }

    #[test]
    fn source_manifest_parser_is_canonical_and_fail_closed() {
        let valid = concat!(
            "0000000000000000000000000000000000000000000000000000000000000000  a.json\n",
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff  model-0.safetensors\n"
        );
        let parsed = CanonicalSourceManifest::parse(valid.as_bytes()).unwrap();
        let expected_manifest_sha256: [u8; 32] = Sha256::digest(valid.as_bytes()).into();
        assert_eq!(parsed.len(), 2);
        assert!(!parsed.is_empty());
        assert_eq!(parsed.sha256(), expected_manifest_sha256);
        assert_eq!(parsed.file_sha256("a.json"), Some([0; 32]));
        assert_eq!(parsed.file_sha256("model-0.safetensors"), Some([0xff; 32]));
        assert_eq!(parsed.file_sha256("missing"), None);

        for malformed in [
            valid.trim_end(),
            "0000000000000000000000000000000000000000000000000000000000000000  ../a\n",
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF  a\n",
            "0000000000000000000000000000000000000000000000000000000000000000 a\n",
        ] {
            assert!(matches!(
                CanonicalSourceManifest::parse(malformed.as_bytes()),
                Err(PinnedSourceError::ManifestSyntax)
            ));
        }

        let duplicate = concat!(
            "0000000000000000000000000000000000000000000000000000000000000000  a.json\n",
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff  a.json\n"
        );
        assert!(matches!(
            CanonicalSourceManifest::parse(duplicate.as_bytes()),
            Err(PinnedSourceError::ManifestSyntax)
        ));
    }

    #[test]
    fn hybrid_compiled_source_manifest_uses_the_same_canonical_cpu_boundary() {
        let bytes = include_bytes!("../../../manifests/glm52-hybrid-source-v1.sha256");
        let manifest = CanonicalSourceManifest::parse(bytes).unwrap();
        assert_eq!(manifest.len(), 194);
        assert_eq!(
            manifest.sha256(),
            decode_sha256(b"a4d9cb546e8fdae5dd7e494228750ee0a2723904170a7c700e461704d5683ab7")
                .unwrap()
        );
        assert_eq!(
            manifest.file_sha256("model.safetensors.index.json"),
            Some(
                decode_sha256(b"6eb773222d932418dd0530c63aca498f86ef424da2a4526ccba76b59726da234")
                    .unwrap()
            )
        );
        assert!(
            manifest
                .file_sha256("model-00001-of-00184.safetensors")
                .is_some()
        );
        assert!(
            manifest
                .file_sha256("model-00184-of-00184.safetensors")
                .is_some()
        );
        assert_eq!(
            manifest
                .files()
                .keys()
                .filter(|name| name.ends_with(".safetensors"))
                .count(),
            184
        );
    }

    #[test]
    fn retained_source_file_closes_reopen_link_and_mutation_boundaries() {
        let root = TempDirectory::new();
        let source = root.0.join("source.bin");
        fs::write(&source, b"authenticated source bytes").unwrap();
        let retained = RetainedSourceFile::open(&source).unwrap();
        let expected_sha256: [u8; 32] = Sha256::digest(b"authenticated source bytes").into();
        assert_eq!(retained.path(), source);
        assert_eq!(retained.len(), 26);
        assert!(!retained.is_empty());
        assert_eq!(retained.sha256().unwrap(), expected_sha256);

        let replacement_target = root.0.join("original.bin");
        fs::rename(&source, &replacement_target).unwrap();
        fs::write(&source, b"replacement source bytes").unwrap();
        assert!(matches!(
            retained.revalidate(),
            Err(PinnedSourceError::SourceChanged(path)) if path == source
        ));

        let linked_source = root.0.join("linked-source.bin");
        let hard_link = root.0.join("hard-link.bin");
        fs::write(&linked_source, b"linked").unwrap();
        fs::hard_link(&linked_source, &hard_link).unwrap();
        assert!(matches!(
            RetainedSourceFile::open(&linked_source),
            Err(PinnedSourceError::UnsafePath(path)) if path == linked_source
        ));

        let symlink = root.0.join("symbolic-link.bin");
        std::os::unix::fs::symlink(&replacement_target, &symlink).unwrap();
        assert!(matches!(
            RetainedSourceFile::open(&symlink),
            Err(PinnedSourceError::UnsafePath(path)) if path == symlink
        ));

        let empty = root.0.join("empty.bin");
        fs::write(&empty, b"").unwrap();
        let retained_empty = RetainedSourceFile::open(&empty).unwrap();
        let empty_sha256: [u8; 32] = Sha256::digest([]).into();
        assert!(retained_empty.is_empty());
        assert_eq!(retained_empty.sha256().unwrap(), empty_sha256);
    }

    #[test]
    fn publisher_manifest_exception_is_exact_and_nonextensible() {
        let exceptions = [
            (
                ".gitattributes",
                PINNED_GITATTRIBUTES_MANIFEST_SHA256,
                PINNED_GITATTRIBUTES_REVISION_SHA256,
            ),
            (
                "README.md",
                PINNED_README_MANIFEST_SHA256,
                PINNED_README_REVISION_SHA256,
            ),
        ];
        for (name, manifest, revision) in exceptions {
            assert!(is_pinned_publisher_manifest_exception(
                name, &manifest, &revision
            ));
            let mut wrong_revision = revision;
            wrong_revision[0] ^= 1;
            assert!(!is_pinned_publisher_manifest_exception(
                name,
                &manifest,
                &wrong_revision
            ));
        }
        assert!(!is_pinned_publisher_manifest_exception(
            "config.json",
            &PINNED_README_MANIFEST_SHA256,
            &PINNED_README_REVISION_SHA256
        ));
        assert!(!is_pinned_publisher_manifest_exception(
            ".gitattributes",
            &PINNED_README_MANIFEST_SHA256,
            &PINNED_README_REVISION_SHA256
        ));
    }

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

    #[test]
    fn four_rank_native_plans_have_identical_names_and_exact_source_bytes() {
        let plans: Vec<_> = (0_u8..4)
            .map(|rank| pinned_exl3_rank_plan(rank).unwrap())
            .collect();
        for plan in &plans {
            assert_eq!(plan.tensor_count(), PINNED_RANK_TENSOR_COUNT);
            assert_eq!(
                plan.source_payload_bytes(),
                PINNED_RANK_SOURCE_PAYLOAD_BYTES
            );
        }
        let names: Vec<_> = plans[0]
            .tensors
            .iter()
            .map(|tensor| tensor.spec.name.as_str())
            .collect();
        for plan in &plans[1..] {
            assert_eq!(
                names,
                plan.tensors
                    .iter()
                    .map(|tensor| tensor.spec.name.as_str())
                    .collect::<Vec<_>>()
            );
        }
        assert_eq!(plans[0].tensors[0].spec.name, "lm_head.weight");
        assert_eq!(
            plans[0].tensors.last().unwrap().spec.name,
            "model.norm.weight"
        );
        assert!(matches!(
            pinned_exl3_rank_plan(4),
            Err(CheckpointError::Rank(4))
        ));
    }

    #[test]
    fn rank_manifest_inventory_and_weight_policy_are_stable() {
        let rank0 = pinned_exl3_rank_plan(0).unwrap();
        let rank3 = pinned_exl3_rank_plan(3).unwrap();
        let rank0_manifest = rank0.manifest_tensors().unwrap();
        let rank3_manifest = rank3.manifest_tensors().unwrap();
        assert_eq!(rank0_manifest.len(), PINNED_RANK_TENSOR_COUNT);
        assert_eq!(rank3_manifest.len(), PINNED_RANK_TENSOR_COUNT);
        assert_eq!(
            rank0_manifest
                .iter()
                .map(|tensor| (&tensor.name, tensor.codec_id, tensor.role_id))
                .collect::<Vec<_>>(),
            rank3_manifest
                .iter()
                .map(|tensor| (&tensor.name, tensor.codec_id, tensor.role_id))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            rank0_manifest
                .iter()
                .map(|tensor| tensor.primary_bytes + tensor.aux_bytes)
                .sum::<u64>(),
            rank0.source_payload_bytes()
        );
        let policy = pinned_exl3_weight_policy_sha256();
        assert_ne!(policy, [0; 32]);
        assert_eq!(policy, pinned_exl3_weight_policy_sha256());
    }
}
