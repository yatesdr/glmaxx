use std::fmt;

use glm_format::{
    NATIVE_PAYLOAD_ALIGNMENT, NativeRankReader, RankPayloadProof, RankTensorSink,
    RankWeightProfile, TensorDescriptor, ValidatedRankManifest, pinned_exl3_rank_plan,
    rank_invariant_tensor_catalog_sha256,
};
use sha2::{Digest, Sha256};

use crate::checkpoint_cuda::CudaArenaVerificationEvidence;

pub const LOAD_PLAN_HEADER_BYTES: usize = 416;
pub const RANK_LOAD_ENTRY_BYTES: usize = 248;
pub const TENSOR_ARENA_ENTRY_BYTES: usize = 64;
pub const PREPARED_RANK_RECEIPT_BYTES: usize = 256;
pub const RANK_LOAD_VERIFICATION_EVIDENCE_BYTES: usize = 256;
pub const RANK_SET_SIZE: usize = 4;
pub const READER_CHUNK_BYTES: u32 = 8 * 1024 * 1024;

const LOAD_PLAN_DOMAIN: &[u8] = b"glmaxx.rank-set-load-plan.v1\0";
const ARENA_LAYOUT_DOMAIN: &[u8] = b"glmaxx.rank-arena-layout.v1\0";
const PREPARED_RANK_DOMAIN: &[u8] = b"glmaxx.prepared-rank-receipt.v1\0";
const PREPARED_RANK_SET_DOMAIN: &[u8] = b"glmaxx.prepared-rank-set.v1\0";
const ADOPTED_RANK_SET_DOMAIN: &[u8] = b"glmaxx.adopted-rank-set.v1\0";
const RANK_LOAD_VERIFICATION_EVIDENCE_DOMAIN: &[u8] =
    b"glmaxx.rank-load-verification-evidence.v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum LoadVerificationMode {
    FullSha256 = 1,
    FsVerity = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum LoadProfile {
    Nvfp4Laboratory = 1,
    CapacityExl3 = 2,
    HybridServe = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RankSetLoadPlanHeader {
    pub verification_mode: LoadVerificationMode,
    pub profile: LoadProfile,
    pub tensor_count: u32,
    pub conversion_uuid: [u8; 16],
    pub weight_policy_sha256: [u8; 32],
    pub kernel_abi_sha256: [u8; 32],
    pub memory_plan_sha256: [u8; 32],
    pub codec_capability_sha256: [u8; 32],
    pub model_config_sha256: [u8; 32],
    pub tokenizer_bundle_sha256: [u8; 32],
    pub chat_template_sha256: [u8; 32],
    pub operation_manifest_sha256: [u8; 32],
    pub tensor_catalog_sha256: [u8; 32],
    pub profile_budget_sha256: [u8; 32],
    pub staging_slot_bytes: u32,
    pub staging_slots_per_rank: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RankSetLoadEnvironment {
    pub verification_mode: LoadVerificationMode,
    pub profile: LoadProfile,
    pub device_identity_sha256: [[u8; 32]; RANK_SET_SIZE],
    pub memory_plan_sha256: [u8; 32],
    pub codec_capability_sha256: [u8; 32],
    pub staging_slot_bytes: u32,
    pub staging_slots_per_rank: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RankLoadEntry {
    pub rank: u8,
    pub device_identity_sha256: [u8; 32],
    pub file_uuid: [u8; 16],
    pub manifest_sha256: [u8; 32],
    pub descriptor_sha256: [u8; 32],
    pub payload_sha256: [u8; 32],
    pub tensor_count: u32,
    pub file_payload_bytes: u64,
    pub device_weight_arena_bytes: u64,
    pub device_metadata_arena_bytes: u64,
    pub arena_layout_sha256: [u8; 32],
    pub tensor_contract_sha256: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TensorArenaEntry {
    pub tensor_id: u32,
    pub role_id: u16,
    pub codec_id: u16,
    pub descriptor_flags: u32,
    pub metadata_destination_offset: u64,
    pub metadata_bytes: u64,
    pub primary_destination_offset: u64,
    pub primary_bytes: u64,
    pub auxiliary_destination_offset: u64,
    pub auxiliary_bytes: u64,
    pub required_device_alignment: u32,
}

impl TensorArenaEntry {
    #[must_use]
    pub fn encode(self) -> [u8; TENSOR_ARENA_ENTRY_BYTES] {
        let mut output = [0_u8; TENSOR_ARENA_ENTRY_BYTES];
        put_u32(&mut output, 0, self.tensor_id);
        put_u16(&mut output, 4, self.role_id);
        put_u16(&mut output, 6, self.codec_id);
        put_u32(&mut output, 8, self.descriptor_flags);
        put_u64(&mut output, 12, self.metadata_destination_offset);
        put_u64(&mut output, 20, self.metadata_bytes);
        put_u64(&mut output, 28, self.primary_destination_offset);
        put_u64(&mut output, 36, self.primary_bytes);
        put_u64(&mut output, 44, self.auxiliary_destination_offset);
        put_u64(&mut output, 52, self.auxiliary_bytes);
        put_u32(&mut output, 60, self.required_device_alignment);
        output
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankSetLoadPlan {
    pub header: RankSetLoadPlanHeader,
    pub ranks: [RankLoadEntry; RANK_SET_SIZE],
    pub tensors: [Vec<TensorArenaEntry>; RANK_SET_SIZE],
    plan_sha256: [u8; 32],
}

impl RankSetLoadPlan {
    pub fn new(
        header: RankSetLoadPlanHeader,
        ranks: [RankLoadEntry; RANK_SET_SIZE],
        tensors: [Vec<TensorArenaEntry>; RANK_SET_SIZE],
    ) -> Result<Self, LoadPlanError> {
        validate_header(&header)?;
        validate_ranks(&header, &ranks, &tensors)?;
        let mut plan = Self {
            header,
            ranks,
            tensors,
            plan_sha256: [0; 32],
        };
        plan.plan_sha256 = hash_domain(LOAD_PLAN_DOMAIN, &plan.canonical_preimage()?);
        Ok(plan)
    }

    pub fn canonical_preimage(&self) -> Result<Vec<u8>, LoadPlanError> {
        let tensor_records = usize::try_from(self.header.tensor_count)
            .map_err(|_| LoadPlanError::Overflow)?
            .checked_mul(RANK_SET_SIZE)
            .ok_or(LoadPlanError::Overflow)?;
        let capacity = LOAD_PLAN_HEADER_BYTES
            .checked_add(
                RANK_LOAD_ENTRY_BYTES
                    .checked_mul(RANK_SET_SIZE)
                    .ok_or(LoadPlanError::Overflow)?,
            )
            .and_then(|bytes| {
                bytes.checked_add(
                    TENSOR_ARENA_ENTRY_BYTES
                        .checked_mul(tensor_records)
                        .ok_or(LoadPlanError::Overflow)
                        .ok()?,
                )
            })
            .ok_or(LoadPlanError::Overflow)?;
        let mut output = Vec::with_capacity(capacity);
        output.extend_from_slice(&encode_header(&self.header));
        for rank in self.ranks {
            output.extend_from_slice(&encode_rank(rank));
        }
        for rank_tensors in &self.tensors {
            for tensor in rank_tensors {
                output.extend_from_slice(&tensor.encode());
            }
        }
        if output.len() != capacity {
            return Err(LoadPlanError::Encoding);
        }
        Ok(output)
    }

    #[must_use]
    pub fn rank(&self, rank: u8) -> Option<&RankLoadEntry> {
        self.ranks.get(usize::from(rank))
    }

    #[must_use]
    pub const fn plan_sha256(&self) -> [u8; 32] {
        self.plan_sha256
    }

    pub fn uploaded_bytes(&self, rank: u8) -> Result<u64, LoadPlanError> {
        Ok(self.expected_upload_summary(rank)?.uploaded_bytes)
    }

    pub fn expected_upload_summary(
        &self,
        rank: u8,
    ) -> Result<RankArenaUploadSummary, LoadPlanError> {
        let tensors = self
            .tensors
            .get(usize::from(rank))
            .ok_or(LoadPlanError::Rank)?;
        let mut metadata_bytes = 0_u64;
        let mut primary_bytes = 0_u64;
        let mut auxiliary_bytes = 0_u64;
        for tensor in tensors {
            metadata_bytes = metadata_bytes
                .checked_add(tensor.metadata_bytes)
                .ok_or(LoadPlanError::Overflow)?;
            primary_bytes = primary_bytes
                .checked_add(tensor.primary_bytes)
                .ok_or(LoadPlanError::Overflow)?;
            auxiliary_bytes = auxiliary_bytes
                .checked_add(tensor.auxiliary_bytes)
                .ok_or(LoadPlanError::Overflow)?;
        }
        let uploaded_bytes = metadata_bytes
            .checked_add(primary_bytes)
            .and_then(|bytes| bytes.checked_add(auxiliary_bytes))
            .ok_or(LoadPlanError::Overflow)?;
        Ok(RankArenaUploadSummary {
            rank,
            tensor_count: u32::try_from(tensors.len()).map_err(|_| LoadPlanError::Overflow)?,
            metadata_bytes,
            primary_bytes,
            auxiliary_bytes,
            uploaded_bytes,
        })
    }
}

pub fn build_rank_set_load_plan(
    readers: [&NativeRankReader; RANK_SET_SIZE],
    environment: RankSetLoadEnvironment,
) -> Result<RankSetLoadPlan, LoadPlanError> {
    NativeRankReader::validate_rank_set(readers).map_err(|_| LoadPlanError::Reader)?;
    let sources = [
        authenticated_rank_load_source(readers[0])?,
        authenticated_rank_load_source(readers[1])?,
        authenticated_rank_load_source(readers[2])?,
        authenticated_rank_load_source(readers[3])?,
    ];
    let contract = pinned_capacity_tensor_load_contract()?;
    build_rank_set_load_plan_from_sources(sources, environment, &contract)
}

#[derive(Clone, Copy)]
struct AuthenticatedRankLoadSource<'a> {
    rank: u8,
    conversion_uuid: [u8; 16],
    file_uuid: [u8; 16],
    model_config_sha256: [u8; 32],
    tokenizer_bundle_sha256: [u8; 32],
    chat_template_sha256: [u8; 32],
    weight_policy_sha256: [u8; 32],
    kernel_abi_sha256: [u8; 32],
    manifest_sha256: [u8; 32],
    descriptor_sha256: [u8; 32],
    payload_sha256: [u8; 32],
    file_payload_bytes: u64,
    descriptors: &'a [TensorDescriptor],
    manifest: &'a ValidatedRankManifest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TensorLoadContract {
    tensor_id: u32,
    role_id: u16,
    codec_id: u16,
    descriptor_flags: u32,
    metadata_bytes: u64,
    primary_bytes: u64,
    auxiliary_bytes: u64,
    required_device_alignment: u32,
}

fn authenticated_rank_load_source(
    reader: &NativeRankReader,
) -> Result<AuthenticatedRankLoadSource<'_>, LoadPlanError> {
    Ok(AuthenticatedRankLoadSource {
        rank: u8::try_from(reader.rank).map_err(|_| LoadPlanError::Rank)?,
        conversion_uuid: reader.conversion_uuid,
        file_uuid: reader.file_uuid,
        model_config_sha256: reader.model_config_sha256,
        tokenizer_bundle_sha256: reader.tokenizer_bundle_sha256,
        chat_template_sha256: reader.chat_template_sha256,
        weight_policy_sha256: reader.weight_policy_sha256,
        kernel_abi_sha256: reader.kernel_abi_sha256,
        manifest_sha256: reader.manifest_sha256,
        descriptor_sha256: reader.descriptor_sha256,
        payload_sha256: reader.payload_sha256,
        file_payload_bytes: reader.file_payload_bytes(),
        descriptors: &reader.descriptors,
        manifest: reader.validated_manifest().ok_or(LoadPlanError::Manifest)?,
    })
}

fn pinned_capacity_tensor_load_contract() -> Result<Vec<TensorLoadContract>, LoadPlanError> {
    let plan = pinned_exl3_rank_plan(0).map_err(|_| LoadPlanError::Manifest)?;
    Ok(plan
        .tensor_specs()
        .into_iter()
        .map(|tensor| TensorLoadContract {
            tensor_id: tensor.tensor_id,
            role_id: tensor.role_id,
            codec_id: tensor.codec_id(),
            descriptor_flags: u32::from(tensor.descriptor_flags()),
            metadata_bytes: tensor.codec_metadata_bytes(),
            primary_bytes: tensor.primary_bytes(),
            auxiliary_bytes: tensor.aux_bytes(),
            required_device_alignment: NATIVE_PAYLOAD_ALIGNMENT,
        })
        .collect())
}

fn build_rank_set_load_plan_from_sources(
    sources: [AuthenticatedRankLoadSource<'_>; RANK_SET_SIZE],
    environment: RankSetLoadEnvironment,
    contract: &[TensorLoadContract],
) -> Result<RankSetLoadPlan, LoadPlanError> {
    let first = sources[0];
    let first_manifest = first.manifest;
    let expected_profile = match first_manifest.profile {
        RankWeightProfile::CapacityExl3 => LoadProfile::CapacityExl3,
    };
    if environment.profile != expected_profile {
        return Err(LoadPlanError::Profile);
    }
    let tensor_count = u32::try_from(contract.len()).map_err(|_| LoadPlanError::Overflow)?;
    if tensor_count == 0 || first.descriptors.len() != contract.len() {
        return Err(LoadPlanError::Tensor);
    }
    let tensor_catalog_sha256 =
        rank_invariant_tensor_catalog_sha256(&first_manifest.tensor_semantics)
            .map_err(|_| LoadPlanError::Manifest)?;

    let mut tensor_layouts: [Vec<TensorArenaEntry>; RANK_SET_SIZE] =
        std::array::from_fn(|_| Vec::new());
    let mut rank_entries = [RankLoadEntry {
        rank: 0,
        device_identity_sha256: [0; 32],
        file_uuid: [0; 16],
        manifest_sha256: [0; 32],
        descriptor_sha256: [0; 32],
        payload_sha256: [0; 32],
        tensor_count: 0,
        file_payload_bytes: 0,
        device_weight_arena_bytes: 0,
        device_metadata_arena_bytes: 0,
        arena_layout_sha256: [0; 32],
        tensor_contract_sha256: [0; 32],
    }; RANK_SET_SIZE];

    for (rank, source) in sources.into_iter().enumerate() {
        let manifest = source.manifest;
        if usize::from(source.rank) != rank || usize::from(manifest.rank) != rank {
            return Err(LoadPlanError::Rank);
        }
        if source.conversion_uuid != first.conversion_uuid
            || source.model_config_sha256 != first.model_config_sha256
            || source.tokenizer_bundle_sha256 != first.tokenizer_bundle_sha256
            || source.chat_template_sha256 != first.chat_template_sha256
            || source.weight_policy_sha256 != first.weight_policy_sha256
            || source.kernel_abi_sha256 != first.kernel_abi_sha256
        {
            return Err(LoadPlanError::Identity);
        }
        if source.descriptors.len()
            != usize::try_from(tensor_count).map_err(|_| LoadPlanError::Overflow)?
        {
            return Err(LoadPlanError::Tensor);
        }
        if manifest.profile != first_manifest.profile
            || manifest.operation_manifest_sha256 != first_manifest.operation_manifest_sha256
            || manifest.profile_budget_sha256 != first_manifest.profile_budget_sha256
            || rank_invariant_tensor_catalog_sha256(&manifest.tensor_semantics)
                .map_err(|_| LoadPlanError::Manifest)?
                != tensor_catalog_sha256
        {
            return Err(LoadPlanError::Manifest);
        }
        let (tensors, weight_bytes, metadata_bytes) =
            derive_tensor_arena_entries(source.descriptors, contract)?;
        let rank_u8 = u8::try_from(rank).map_err(|_| LoadPlanError::Overflow)?;
        let arena_layout_sha256 =
            arena_layout_sha256(rank_u8, weight_bytes, metadata_bytes, &tensors);
        tensor_layouts[rank] = tensors;
        rank_entries[rank] = RankLoadEntry {
            rank: rank_u8,
            device_identity_sha256: environment.device_identity_sha256[rank],
            file_uuid: source.file_uuid,
            manifest_sha256: source.manifest_sha256,
            descriptor_sha256: source.descriptor_sha256,
            payload_sha256: source.payload_sha256,
            tensor_count,
            file_payload_bytes: source.file_payload_bytes,
            device_weight_arena_bytes: weight_bytes,
            device_metadata_arena_bytes: metadata_bytes,
            arena_layout_sha256,
            tensor_contract_sha256: manifest.tensor_contract_sha256,
        };
    }

    RankSetLoadPlan::new(
        RankSetLoadPlanHeader {
            verification_mode: environment.verification_mode,
            profile: environment.profile,
            tensor_count,
            conversion_uuid: first.conversion_uuid,
            weight_policy_sha256: first.weight_policy_sha256,
            kernel_abi_sha256: first.kernel_abi_sha256,
            memory_plan_sha256: environment.memory_plan_sha256,
            codec_capability_sha256: environment.codec_capability_sha256,
            model_config_sha256: first.model_config_sha256,
            tokenizer_bundle_sha256: first.tokenizer_bundle_sha256,
            chat_template_sha256: first.chat_template_sha256,
            operation_manifest_sha256: first_manifest.operation_manifest_sha256,
            tensor_catalog_sha256,
            profile_budget_sha256: first_manifest.profile_budget_sha256,
            staging_slot_bytes: environment.staging_slot_bytes,
            staging_slots_per_rank: environment.staging_slots_per_rank,
        },
        rank_entries,
        tensor_layouts,
    )
}

fn derive_tensor_arena_entries(
    descriptors: &[TensorDescriptor],
    contract: &[TensorLoadContract],
) -> Result<(Vec<TensorArenaEntry>, u64, u64), LoadPlanError> {
    if descriptors.is_empty() || descriptors.len() != contract.len() {
        return Err(LoadPlanError::Tensor);
    }
    for (expected_id, (descriptor, expected)) in descriptors.iter().zip(contract).enumerate() {
        if descriptor.tensor_id
            != u32::try_from(expected_id).map_err(|_| LoadPlanError::Overflow)?
            || descriptor.tensor_id != expected.tensor_id
            || descriptor.role_id != expected.role_id
            || descriptor.codec_id != expected.codec_id
            || u32::from(descriptor.flags) != expected.descriptor_flags
            || descriptor.codec_metadata_bytes != expected.metadata_bytes
            || descriptor.payload_bytes != expected.primary_bytes
            || descriptor.aux_bytes != expected.auxiliary_bytes
            || descriptor.payload_alignment != expected.required_device_alignment
            || expected.primary_bytes == 0
            || expected.required_device_alignment == 0
            || !expected.required_device_alignment.is_power_of_two()
        {
            return Err(LoadPlanError::Tensor);
        }
    }
    derive_tensor_arena_entries_from_contract(contract)
}

fn derive_tensor_arena_entries_from_contract(
    contract: &[TensorLoadContract],
) -> Result<(Vec<TensorArenaEntry>, u64, u64), LoadPlanError> {
    if contract.is_empty() {
        return Err(LoadPlanError::Tensor);
    }
    let mut weight_cursor = 0_u64;
    let mut metadata_cursor = 0_u64;
    let mut tensors = Vec::with_capacity(contract.len());
    for (expected_id, expected) in contract.iter().enumerate() {
        if expected.tensor_id != u32::try_from(expected_id).map_err(|_| LoadPlanError::Overflow)?
            || expected.role_id == 0
            || expected.codec_id == 0
            || expected.primary_bytes == 0
            || expected.required_device_alignment == 0
            || !expected.required_device_alignment.is_power_of_two()
        {
            return Err(LoadPlanError::Tensor);
        }
        let alignment = u64::from(expected.required_device_alignment);
        let metadata_destination_offset = if expected.metadata_bytes == 0 {
            0
        } else {
            metadata_cursor = align_up(metadata_cursor, alignment)?;
            let destination = metadata_cursor;
            metadata_cursor = metadata_cursor
                .checked_add(expected.metadata_bytes)
                .ok_or(LoadPlanError::Overflow)?;
            destination
        };
        weight_cursor = align_up(weight_cursor, alignment)?;
        let primary_destination_offset = weight_cursor;
        weight_cursor = weight_cursor
            .checked_add(expected.primary_bytes)
            .ok_or(LoadPlanError::Overflow)?;
        let auxiliary_destination_offset = if expected.auxiliary_bytes == 0 {
            0
        } else {
            weight_cursor = align_up(weight_cursor, alignment)?;
            let destination = weight_cursor;
            weight_cursor = weight_cursor
                .checked_add(expected.auxiliary_bytes)
                .ok_or(LoadPlanError::Overflow)?;
            destination
        };
        tensors.push(TensorArenaEntry {
            tensor_id: expected.tensor_id,
            role_id: expected.role_id,
            codec_id: expected.codec_id,
            descriptor_flags: expected.descriptor_flags,
            metadata_destination_offset,
            metadata_bytes: expected.metadata_bytes,
            primary_destination_offset,
            primary_bytes: expected.primary_bytes,
            auxiliary_destination_offset,
            auxiliary_bytes: expected.auxiliary_bytes,
            required_device_alignment: expected.required_device_alignment,
        });
    }
    Ok((tensors, weight_cursor, metadata_cursor))
}

#[must_use]
pub fn arena_layout_sha256(
    rank: u8,
    device_weight_arena_bytes: u64,
    device_metadata_arena_bytes: u64,
    tensors: &[TensorArenaEntry],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(ARENA_LAYOUT_DOMAIN);
    hasher.update([rank]);
    hasher.update([0; 7]);
    hasher.update(device_weight_arena_bytes.to_le_bytes());
    hasher.update(device_metadata_arena_bytes.to_le_bytes());
    hasher.update(
        u32::try_from(tensors.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    hasher.update([0; 4]);
    for tensor in tensors {
        hasher.update(tensor.encode());
    }
    hasher.finalize().into()
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RankLoadTimingEvidence {
    pub storage_read_nanoseconds: u64,
    pub host_to_pinned_copy_nanoseconds: u64,
    pub h2d_submission_nanoseconds: u64,
    pub h2d_drain_nanoseconds: u64,
    pub full_arena_readback_nanoseconds: u64,
}

/// Canonical evidence for one rank's authenticated file-to-HBM load.
///
/// Construction validates every byte count and identity against the immutable
/// rank-set plan, a completed native-reader proof, the planned upload summary,
/// and an unforgeable successful arena read-back value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RankLoadVerificationEvidence {
    rank: u8,
    verification_mode: LoadVerificationMode,
    plan_sha256: [u8; 32],
    device_identity_sha256: [u8; 32],
    owner_allocation_generation: u64,
    verified_file_payload_bytes: u64,
    tensor_count: u32,
    uploaded_metadata_bytes: u64,
    uploaded_primary_bytes: u64,
    uploaded_auxiliary_bytes: u64,
    uploaded_bytes: u64,
    maximum_reader_scratch_bytes: u64,
    pinned_ring_bytes: u64,
    timings: RankLoadTimingEvidence,
    cuda_arena_verification_sha256: [u8; 32],
    software_provenance_sha256: [u8; 32],
}

impl RankLoadVerificationEvidence {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        plan: &RankSetLoadPlan,
        rank: u8,
        owner_allocation_generation: u64,
        payload: RankPayloadProof,
        upload: RankArenaUploadSummary,
        timings: RankLoadTimingEvidence,
        arena: CudaArenaVerificationEvidence,
        software_provenance_sha256: [u8; 32],
    ) -> Result<Self, LoadPlanError> {
        let expected = plan.rank(rank).ok_or(LoadPlanError::Rank)?;
        let tensor_count =
            u32::try_from(payload.tensor_count).map_err(|_| LoadPlanError::Overflow)?;
        let maximum_reader_scratch_bytes = u64::try_from(payload.maximum_reader_scratch_bytes)
            .map_err(|_| LoadPlanError::Overflow)?;
        let pinned_ring_bytes = u64::from(plan.header.staging_slot_bytes)
            .checked_mul(u64::from(plan.header.staging_slots_per_rank))
            .ok_or(LoadPlanError::Overflow)?;
        let uploaded_bytes = upload
            .metadata_bytes
            .checked_add(upload.primary_bytes)
            .and_then(|bytes| bytes.checked_add(upload.auxiliary_bytes))
            .ok_or(LoadPlanError::Overflow)?;
        if owner_allocation_generation == 0
            || is_zero(&software_provenance_sha256)
            || payload.rank != u32::from(rank)
            || tensor_count != expected.tensor_count
            || payload.payload_bytes != expected.file_payload_bytes
            || payload.payload_sha256 != expected.payload_sha256
            || payload.stream_chunks == 0
            || maximum_reader_scratch_bytes == 0
            || timings.storage_read_nanoseconds != payload.storage_read_nanoseconds
            || upload.rank != rank
            || upload.tensor_count != expected.tensor_count
            || upload.uploaded_bytes != uploaded_bytes
            || upload != plan.expected_upload_summary(rank)?
            || arena.rank() != rank
            || arena.plan_sha256() != plan.plan_sha256
            || arena.owner_allocation_generation() != owner_allocation_generation
            || arena.weight_bytes() != expected.device_weight_arena_bytes
            || arena.metadata_bytes() != expected.device_metadata_arena_bytes
            || arena.readback_chunk_bytes() != READER_CHUNK_BYTES
            || arena.readback_chunks() == 0
            || arena.expected_weight_sha256() != arena.observed_weight_sha256()
            || arena.expected_metadata_sha256() != arena.observed_metadata_sha256()
        {
            return Err(LoadPlanError::Evidence);
        }
        Ok(Self {
            rank,
            verification_mode: plan.header.verification_mode,
            plan_sha256: plan.plan_sha256,
            device_identity_sha256: expected.device_identity_sha256,
            owner_allocation_generation,
            verified_file_payload_bytes: payload.payload_bytes,
            tensor_count,
            uploaded_metadata_bytes: upload.metadata_bytes,
            uploaded_primary_bytes: upload.primary_bytes,
            uploaded_auxiliary_bytes: upload.auxiliary_bytes,
            uploaded_bytes,
            maximum_reader_scratch_bytes,
            pinned_ring_bytes,
            timings,
            cuda_arena_verification_sha256: arena.evidence_sha256(),
            software_provenance_sha256,
        })
    }

    #[must_use]
    pub fn encode(self) -> [u8; RANK_LOAD_VERIFICATION_EVIDENCE_BYTES] {
        let mut output = [0_u8; RANK_LOAD_VERIFICATION_EVIDENCE_BYTES];
        output[0..8].copy_from_slice(b"G5LVE1\0\0");
        put_u16(&mut output, 8, 1);
        put_u16(
            &mut output,
            10,
            u16::try_from(RANK_LOAD_VERIFICATION_EVIDENCE_BYTES).expect("constant fits"),
        );
        output[12] = self.rank;
        output[13] = self.verification_mode as u8;
        output[16..48].copy_from_slice(&self.plan_sha256);
        output[48..80].copy_from_slice(&self.device_identity_sha256);
        put_u64(&mut output, 80, self.owner_allocation_generation);
        put_u64(&mut output, 88, self.verified_file_payload_bytes);
        put_u32(&mut output, 96, self.tensor_count);
        put_u64(&mut output, 104, self.uploaded_metadata_bytes);
        put_u64(&mut output, 112, self.uploaded_primary_bytes);
        put_u64(&mut output, 120, self.uploaded_auxiliary_bytes);
        put_u64(&mut output, 128, self.uploaded_bytes);
        put_u64(&mut output, 136, self.maximum_reader_scratch_bytes);
        put_u64(&mut output, 144, self.pinned_ring_bytes);
        put_u64(&mut output, 152, self.timings.storage_read_nanoseconds);
        put_u64(
            &mut output,
            160,
            self.timings.host_to_pinned_copy_nanoseconds,
        );
        put_u64(&mut output, 168, self.timings.h2d_submission_nanoseconds);
        put_u64(&mut output, 176, self.timings.h2d_drain_nanoseconds);
        put_u64(
            &mut output,
            184,
            self.timings.full_arena_readback_nanoseconds,
        );
        output[192..224].copy_from_slice(&self.cuda_arena_verification_sha256);
        output[224..256].copy_from_slice(&self.software_provenance_sha256);
        output
    }

    #[must_use]
    pub fn evidence_sha256(self) -> [u8; 32] {
        hash_domain(RANK_LOAD_VERIFICATION_EVIDENCE_DOMAIN, &self.encode())
    }

    #[must_use]
    pub const fn rank(self) -> u8 {
        self.rank
    }

    #[must_use]
    pub const fn plan_sha256(self) -> [u8; 32] {
        self.plan_sha256
    }

    #[must_use]
    pub const fn owner_allocation_generation(self) -> u64 {
        self.owner_allocation_generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedRankReceipt {
    pub rank: u8,
    pub verification_mode: LoadVerificationMode,
    pub device_identity_sha256: [u8; 32],
    pub file_uuid: [u8; 16],
    pub plan_sha256: [u8; 32],
    pub payload_sha256: [u8; 32],
    pub arena_layout_sha256: [u8; 32],
    pub device_weight_arena_bytes: u64,
    pub device_metadata_arena_bytes: u64,
    pub verified_file_payload_bytes: u64,
    pub uploaded_plane_metadata_bytes: u64,
    pub owner_allocation_generation: u64,
    pub verification_evidence_sha256: [u8; 32],
}

impl PreparedRankReceipt {
    pub fn new(
        plan: &RankSetLoadPlan,
        rank: u8,
        owner_allocation_generation: u64,
        verification_evidence: RankLoadVerificationEvidence,
    ) -> Result<Self, LoadPlanError> {
        if verification_evidence.rank() != rank
            || verification_evidence.plan_sha256() != plan.plan_sha256
            || verification_evidence.owner_allocation_generation() != owner_allocation_generation
        {
            return Err(LoadPlanError::Evidence);
        }
        Self::from_evidence_sha256(
            plan,
            rank,
            owner_allocation_generation,
            verification_evidence.evidence_sha256(),
        )
    }

    fn from_evidence_sha256(
        plan: &RankSetLoadPlan,
        rank: u8,
        owner_allocation_generation: u64,
        verification_evidence_sha256: [u8; 32],
    ) -> Result<Self, LoadPlanError> {
        let expected = plan.rank(rank).ok_or(LoadPlanError::Rank)?;
        if owner_allocation_generation == 0 || is_zero(&verification_evidence_sha256) {
            return Err(LoadPlanError::Receipt);
        }
        Ok(Self {
            rank,
            verification_mode: plan.header.verification_mode,
            device_identity_sha256: expected.device_identity_sha256,
            file_uuid: expected.file_uuid,
            plan_sha256: plan.plan_sha256,
            payload_sha256: expected.payload_sha256,
            arena_layout_sha256: expected.arena_layout_sha256,
            device_weight_arena_bytes: expected.device_weight_arena_bytes,
            device_metadata_arena_bytes: expected.device_metadata_arena_bytes,
            verified_file_payload_bytes: expected.file_payload_bytes,
            uploaded_plane_metadata_bytes: plan.uploaded_bytes(rank)?,
            owner_allocation_generation,
            verification_evidence_sha256,
        })
    }

    #[cfg(test)]
    pub(crate) fn test_only(
        plan: &RankSetLoadPlan,
        rank: u8,
        owner_allocation_generation: u64,
        verification_evidence_sha256: [u8; 32],
    ) -> Result<Self, LoadPlanError> {
        Self::from_evidence_sha256(
            plan,
            rank,
            owner_allocation_generation,
            verification_evidence_sha256,
        )
    }

    #[must_use]
    pub fn encode(self) -> [u8; PREPARED_RANK_RECEIPT_BYTES] {
        let mut output = [0_u8; PREPARED_RANK_RECEIPT_BYTES];
        output[0..8].copy_from_slice(b"G5PRP1\0\0");
        put_u16(&mut output, 8, 1);
        put_u16(
            &mut output,
            10,
            u16::try_from(PREPARED_RANK_RECEIPT_BYTES).expect("constant fits"),
        );
        output[12] = self.rank;
        output[13] = self.verification_mode as u8;
        output[16..48].copy_from_slice(&self.device_identity_sha256);
        output[48..64].copy_from_slice(&self.file_uuid);
        output[64..96].copy_from_slice(&self.plan_sha256);
        output[96..128].copy_from_slice(&self.payload_sha256);
        output[128..160].copy_from_slice(&self.arena_layout_sha256);
        put_u64(&mut output, 160, self.device_weight_arena_bytes);
        put_u64(&mut output, 168, self.device_metadata_arena_bytes);
        put_u64(&mut output, 176, self.verified_file_payload_bytes);
        put_u64(&mut output, 184, self.uploaded_plane_metadata_bytes);
        put_u64(&mut output, 192, self.owner_allocation_generation);
        output[200..232].copy_from_slice(&self.verification_evidence_sha256);
        output
    }

    #[must_use]
    pub fn receipt_sha256(self) -> [u8; 32] {
        hash_domain(PREPARED_RANK_DOMAIN, &self.encode())
    }

    fn validate(self, plan: &RankSetLoadPlan) -> Result<(), LoadPlanError> {
        let expected = Self::from_evidence_sha256(
            plan,
            self.rank,
            self.owner_allocation_generation,
            self.verification_evidence_sha256,
        )?;
        if self == expected {
            Ok(())
        } else {
            Err(LoadPlanError::Receipt)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedRankSet {
    pub plan_sha256: [u8; 32],
    pub rank_receipt_sha256: [[u8; 32]; RANK_SET_SIZE],
    pub rank_set_receipt_sha256: [u8; 32],
    receipts: [PreparedRankReceipt; RANK_SET_SIZE],
}

impl PreparedRankSet {
    pub fn new(
        plan: &RankSetLoadPlan,
        receipts: [PreparedRankReceipt; RANK_SET_SIZE],
    ) -> Result<Self, LoadPlanError> {
        for (expected_rank, receipt) in receipts.iter().copied().enumerate() {
            if usize::from(receipt.rank) != expected_rank {
                return Err(LoadPlanError::Rank);
            }
            receipt.validate(plan)?;
        }
        let rank_receipt_sha256 = receipts.map(PreparedRankReceipt::receipt_sha256);
        let mut hasher = Sha256::new();
        hasher.update(PREPARED_RANK_SET_DOMAIN);
        for digest in rank_receipt_sha256 {
            hasher.update(digest);
        }
        Ok(Self {
            plan_sha256: plan.plan_sha256,
            rank_receipt_sha256,
            rank_set_receipt_sha256: hasher.finalize().into(),
            receipts,
        })
    }

    #[must_use]
    pub const fn adoption_command(&self) -> AdoptionCommand {
        AdoptionCommand {
            plan_sha256: self.plan_sha256,
            rank_set_receipt_sha256: self.rank_set_receipt_sha256,
        }
    }

    #[must_use]
    pub const fn receipt(&self, rank: u8) -> Option<&PreparedRankReceipt> {
        if rank < RANK_SET_SIZE as u8 {
            Some(&self.receipts[rank as usize])
        } else {
            None
        }
    }

    pub fn complete_adoption(
        &self,
        acknowledgements: [AdoptionAcknowledgement; RANK_SET_SIZE],
    ) -> Result<AdoptedRankSetReceipt, LoadPlanError> {
        let command = self.adoption_command();
        for (expected_rank, acknowledgement) in acknowledgements.iter().enumerate() {
            if acknowledgement.rank as usize != expected_rank
                || acknowledgement.plan_sha256 != command.plan_sha256
                || acknowledgement.rank_set_receipt_sha256 != command.rank_set_receipt_sha256
                || acknowledgement.owner_allocation_generation
                    != self.receipts[expected_rank].owner_allocation_generation
            {
                return Err(LoadPlanError::Adoption);
            }
        }
        let mut hasher = Sha256::new();
        hasher.update(ADOPTED_RANK_SET_DOMAIN);
        hasher.update(command.plan_sha256);
        hasher.update(command.rank_set_receipt_sha256);
        for acknowledgement in acknowledgements {
            hasher.update([acknowledgement.rank]);
            hasher.update([0; 7]);
            hasher.update(acknowledgement.owner_allocation_generation.to_le_bytes());
        }
        Ok(AdoptedRankSetReceipt {
            plan_sha256: command.plan_sha256,
            rank_set_receipt_sha256: command.rank_set_receipt_sha256,
            adopted_rank_set_sha256: hasher.finalize().into(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdoptionCommand {
    plan_sha256: [u8; 32],
    rank_set_receipt_sha256: [u8; 32],
}

impl AdoptionCommand {
    #[must_use]
    pub const fn plan_sha256(self) -> [u8; 32] {
        self.plan_sha256
    }

    #[must_use]
    pub const fn rank_set_receipt_sha256(self) -> [u8; 32] {
        self.rank_set_receipt_sha256
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdoptionAcknowledgement {
    pub rank: u8,
    pub plan_sha256: [u8; 32],
    pub rank_set_receipt_sha256: [u8; 32],
    pub owner_allocation_generation: u64,
}

impl AdoptionAcknowledgement {
    pub fn new(
        command: AdoptionCommand,
        receipt: PreparedRankReceipt,
    ) -> Result<Self, LoadPlanError> {
        if command.plan_sha256 != receipt.plan_sha256 {
            return Err(LoadPlanError::Adoption);
        }
        Ok(Self {
            rank: receipt.rank,
            plan_sha256: command.plan_sha256,
            rank_set_receipt_sha256: command.rank_set_receipt_sha256,
            owner_allocation_generation: receipt.owner_allocation_generation,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdoptedRankSetReceipt {
    plan_sha256: [u8; 32],
    rank_set_receipt_sha256: [u8; 32],
    adopted_rank_set_sha256: [u8; 32],
}

impl AdoptedRankSetReceipt {
    #[must_use]
    pub const fn plan_sha256(self) -> [u8; 32] {
        self.plan_sha256
    }

    #[must_use]
    pub const fn rank_set_receipt_sha256(self) -> [u8; 32] {
        self.rank_set_receipt_sha256
    }

    #[must_use]
    pub const fn adopted_rank_set_sha256(self) -> [u8; 32] {
        self.adopted_rank_set_sha256
    }

    #[cfg(test)]
    pub(crate) const fn test_only(adopted_rank_set_sha256: [u8; 32]) -> Self {
        Self {
            plan_sha256: [0x51; 32],
            rank_set_receipt_sha256: [0x52; 32],
            adopted_rank_set_sha256,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RankSetAbortCommand {
    plan_sha256: [u8; 32],
    load_attempt_generation: u64,
}

impl RankSetAbortCommand {
    pub fn new(
        plan: &RankSetLoadPlan,
        load_attempt_generation: u64,
    ) -> Result<Self, LoadPlanError> {
        if load_attempt_generation == 0 {
            return Err(LoadPlanError::Transition);
        }
        Ok(Self {
            plan_sha256: plan.plan_sha256,
            load_attempt_generation,
        })
    }

    #[must_use]
    pub const fn plan_sha256(self) -> [u8; 32] {
        self.plan_sha256
    }

    #[must_use]
    pub const fn load_attempt_generation(self) -> u64 {
        self.load_attempt_generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RankSetLoadCoordinatorState {
    Preparing,
    Adopting,
    Adopted,
    Aborted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RankSetLoadAction {
    Wait,
    Adopt(AdoptionCommand),
    Complete(AdoptedRankSetReceipt),
    Abort(RankSetAbortCommand),
}

/// Process-wide coordinator for one four-rank checkpoint-load attempt.
///
/// Rank threads retain their allocation lifecycles. This coordinator owns
/// only authenticated receipts and emits one process-common route. Any rank
/// failure, malformed/duplicate message, or phase violation changes the
/// attempt to terminal `Aborted`; it never returns a rank-local fallback.
pub struct RankSetLoadCoordinator<'a> {
    plan: &'a RankSetLoadPlan,
    abort_command: RankSetAbortCommand,
    owner_allocation_generations: [u64; RANK_SET_SIZE],
    state: RankSetLoadCoordinatorState,
    prepared_receipts: [Option<PreparedRankReceipt>; RANK_SET_SIZE],
    prepared_set: Option<PreparedRankSet>,
    adoption_acknowledgements: [Option<AdoptionAcknowledgement>; RANK_SET_SIZE],
    adopted_receipt: Option<AdoptedRankSetReceipt>,
    terminal_error: Option<LoadPlanError>,
}

impl<'a> RankSetLoadCoordinator<'a> {
    pub fn new(
        plan: &'a RankSetLoadPlan,
        load_attempt_generation: u64,
        owner_allocation_generations: [u64; RANK_SET_SIZE],
    ) -> Result<Self, LoadPlanError> {
        if load_attempt_generation == 0 || owner_allocation_generations.contains(&0) {
            return Err(LoadPlanError::Transition);
        }
        Ok(Self {
            plan,
            abort_command: RankSetAbortCommand::new(plan, load_attempt_generation)?,
            owner_allocation_generations,
            state: RankSetLoadCoordinatorState::Preparing,
            prepared_receipts: [None; RANK_SET_SIZE],
            prepared_set: None,
            adoption_acknowledgements: [None; RANK_SET_SIZE],
            adopted_receipt: None,
            terminal_error: None,
        })
    }

    #[must_use]
    pub const fn state(&self) -> RankSetLoadCoordinatorState {
        self.state
    }

    #[must_use]
    pub const fn abort_command(&self) -> RankSetAbortCommand {
        self.abort_command
    }

    #[must_use]
    pub const fn terminal_error(&self) -> Option<LoadPlanError> {
        self.terminal_error
    }

    #[must_use]
    pub const fn adopted_receipt(&self) -> Option<AdoptedRankSetReceipt> {
        self.adopted_receipt
    }

    pub fn report_prepared(&mut self, receipt: PreparedRankReceipt) -> RankSetLoadAction {
        if self.state == RankSetLoadCoordinatorState::Aborted {
            return RankSetLoadAction::Abort(self.abort_command);
        }
        if self.state != RankSetLoadCoordinatorState::Preparing {
            return self.fail(LoadPlanError::Transition);
        }
        let rank = usize::from(receipt.rank);
        if rank >= RANK_SET_SIZE
            || self.prepared_receipts[rank].is_some()
            || receipt.owner_allocation_generation != self.owner_allocation_generations[rank]
            || receipt.validate(self.plan).is_err()
        {
            return self.fail(LoadPlanError::Receipt);
        }
        self.prepared_receipts[rank] = Some(receipt);
        if self.prepared_receipts.iter().any(Option::is_none) {
            return RankSetLoadAction::Wait;
        }
        let receipts = std::array::from_fn(|index| {
            self.prepared_receipts[index].expect("all ranks checked above")
        });
        match PreparedRankSet::new(self.plan, receipts) {
            Ok(prepared) => {
                let command = prepared.adoption_command();
                self.prepared_set = Some(prepared);
                self.state = RankSetLoadCoordinatorState::Adopting;
                RankSetLoadAction::Adopt(command)
            }
            Err(error) => self.fail(error),
        }
    }

    pub fn report_rank_failure(&mut self, rank: u8, error: LoadPlanError) -> RankSetLoadAction {
        if self.state == RankSetLoadCoordinatorState::Aborted {
            return RankSetLoadAction::Abort(self.abort_command);
        }
        if usize::from(rank) >= RANK_SET_SIZE {
            return self.fail(LoadPlanError::Rank);
        }
        self.fail(error)
    }

    pub fn report_adoption_acknowledgement(
        &mut self,
        acknowledgement: AdoptionAcknowledgement,
    ) -> RankSetLoadAction {
        if self.state == RankSetLoadCoordinatorState::Aborted {
            return RankSetLoadAction::Abort(self.abort_command);
        }
        if self.state != RankSetLoadCoordinatorState::Adopting {
            return self.fail(LoadPlanError::Transition);
        }
        let rank = usize::from(acknowledgement.rank);
        let Some(prepared) = self.prepared_set.as_ref() else {
            return self.fail(LoadPlanError::Adoption);
        };
        if rank >= RANK_SET_SIZE
            || self.adoption_acknowledgements[rank].is_some()
            || acknowledgement.plan_sha256 != prepared.plan_sha256
            || acknowledgement.rank_set_receipt_sha256 != prepared.rank_set_receipt_sha256
            || acknowledgement.owner_allocation_generation
                != prepared
                    .receipt(acknowledgement.rank)
                    .map(|receipt| receipt.owner_allocation_generation)
                    .unwrap_or(0)
        {
            return self.fail(LoadPlanError::Adoption);
        }
        self.adoption_acknowledgements[rank] = Some(acknowledgement);
        if self.adoption_acknowledgements.iter().any(Option::is_none) {
            return RankSetLoadAction::Wait;
        }
        let acknowledgements = std::array::from_fn(|index| {
            self.adoption_acknowledgements[index].expect("all ranks checked above")
        });
        match prepared.complete_adoption(acknowledgements) {
            Ok(adopted) => {
                self.adopted_receipt = Some(adopted);
                self.state = RankSetLoadCoordinatorState::Adopted;
                RankSetLoadAction::Complete(adopted)
            }
            Err(error) => self.fail(error),
        }
    }

    fn fail(&mut self, error: LoadPlanError) -> RankSetLoadAction {
        self.state = RankSetLoadCoordinatorState::Aborted;
        self.prepared_receipts.fill(None);
        self.prepared_set = None;
        self.adoption_acknowledgements.fill(None);
        self.adopted_receipt = None;
        self.terminal_error.get_or_insert(error);
        RankSetLoadAction::Abort(self.abort_command)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RankArenaState {
    Allocated,
    Staging,
    Prepared,
    Adopted,
    Aborted,
}

/// Rank-local ownership state for one quarantined allocation generation.
///
/// This object intentionally carries no CUDA handle. The persistent owner
/// thread retains the real allocations and advances this state beside them.
/// An executor-visible permit cannot be produced until the coordinator has
/// validated all four adoption acknowledgements.
#[derive(Debug, Eq, PartialEq)]
pub struct RankArenaLifecycle {
    rank: u8,
    plan_sha256: [u8; 32],
    owner_allocation_generation: u64,
    prepared_rank_sha256: Option<[u8; 32]>,
    rank_set_receipt_sha256: Option<[u8; 32]>,
    state: RankArenaState,
}

impl RankArenaLifecycle {
    pub fn allocated(
        plan: &RankSetLoadPlan,
        rank: u8,
        owner_allocation_generation: u64,
    ) -> Result<Self, LoadPlanError> {
        if plan.rank(rank).is_none() || owner_allocation_generation == 0 {
            return Err(LoadPlanError::Rank);
        }
        Ok(Self {
            rank,
            plan_sha256: plan.plan_sha256,
            owner_allocation_generation,
            prepared_rank_sha256: None,
            rank_set_receipt_sha256: None,
            state: RankArenaState::Allocated,
        })
    }

    #[must_use]
    pub const fn state(&self) -> RankArenaState {
        self.state
    }

    pub fn begin_staging(&mut self) -> Result<(), LoadPlanError> {
        if self.state != RankArenaState::Allocated {
            return Err(LoadPlanError::Transition);
        }
        self.state = RankArenaState::Staging;
        Ok(())
    }

    pub fn prepare(
        &mut self,
        plan: &RankSetLoadPlan,
        receipt: PreparedRankReceipt,
    ) -> Result<(), LoadPlanError> {
        if self.state != RankArenaState::Staging
            || plan.plan_sha256 != self.plan_sha256
            || receipt.rank != self.rank
            || receipt.owner_allocation_generation != self.owner_allocation_generation
        {
            return Err(LoadPlanError::Transition);
        }
        receipt.validate(plan)?;
        self.prepared_rank_sha256 = Some(receipt.receipt_sha256());
        self.state = RankArenaState::Prepared;
        Ok(())
    }

    pub fn acknowledge_adoption(
        &mut self,
        prepared: &PreparedRankSet,
    ) -> Result<AdoptionAcknowledgement, LoadPlanError> {
        if self.state != RankArenaState::Prepared
            || prepared.plan_sha256 != self.plan_sha256
            || prepared.rank_receipt_sha256[usize::from(self.rank)]
                != self.prepared_rank_sha256.ok_or(LoadPlanError::Transition)?
        {
            return Err(LoadPlanError::Adoption);
        }
        let command = prepared.adoption_command();
        self.rank_set_receipt_sha256 = Some(command.rank_set_receipt_sha256);
        self.state = RankArenaState::Adopted;
        Ok(AdoptionAcknowledgement {
            rank: self.rank,
            plan_sha256: command.plan_sha256,
            rank_set_receipt_sha256: command.rank_set_receipt_sha256,
            owner_allocation_generation: self.owner_allocation_generation,
        })
    }

    /// Marks the owner generation terminal. `true` means the caller must
    /// perform the one physical synchronize/free; repeated calls return
    /// `false`, preventing double free in mock and device implementations.
    pub fn abort(&mut self) -> bool {
        if self.state == RankArenaState::Aborted {
            false
        } else {
            self.state = RankArenaState::Aborted;
            self.prepared_rank_sha256 = None;
            self.rank_set_receipt_sha256 = None;
            true
        }
    }

    pub fn execution_permit(
        self,
        adopted: AdoptedRankSetReceipt,
    ) -> Result<WeightArenaExecutionPermit, LoadPlanError> {
        if self.state != RankArenaState::Adopted
            || self.plan_sha256 != adopted.plan_sha256
            || self.rank_set_receipt_sha256 != Some(adopted.rank_set_receipt_sha256)
        {
            return Err(LoadPlanError::Adoption);
        }
        Ok(WeightArenaExecutionPermit {
            rank: self.rank,
            plan_sha256: self.plan_sha256,
            owner_allocation_generation: self.owner_allocation_generation,
            adopted_rank_set_sha256: adopted.adopted_rank_set_sha256,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct WeightArenaExecutionPermit {
    rank: u8,
    plan_sha256: [u8; 32],
    owner_allocation_generation: u64,
    adopted_rank_set_sha256: [u8; 32],
}

impl WeightArenaExecutionPermit {
    #[cfg(test)]
    pub(crate) const fn test_only(
        rank: u8,
        plan_sha256: [u8; 32],
        owner_allocation_generation: u64,
    ) -> Self {
        Self {
            rank,
            plan_sha256,
            owner_allocation_generation,
            adopted_rank_set_sha256: [0x71; 32],
        }
    }

    #[must_use]
    pub const fn rank(&self) -> u8 {
        self.rank
    }

    #[must_use]
    pub const fn plan_sha256(&self) -> [u8; 32] {
        self.plan_sha256
    }

    #[must_use]
    pub const fn owner_allocation_generation(&self) -> u64 {
        self.owner_allocation_generation
    }

    #[must_use]
    pub const fn adopted_rank_set_sha256(&self) -> [u8; 32] {
        self.adopted_rank_set_sha256
    }
}

pub trait QuarantinedArenaWriter {
    type Error: fmt::Display;

    fn weight_capacity(&self) -> u64;
    fn metadata_capacity(&self) -> u64;
    fn write_weight(&mut self, offset: u64, bytes: &[u8]) -> Result<(), Self::Error>;
    fn write_metadata(&mut self, offset: u64, bytes: &[u8]) -> Result<(), Self::Error>;
    fn drain_and_seal(&mut self) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RankArenaUploadSummary {
    pub rank: u8,
    pub tensor_count: u32,
    pub metadata_bytes: u64,
    pub primary_bytes: u64,
    pub auxiliary_bytes: u64,
    pub uploaded_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ActiveTensorUpload {
    entry: TensorArenaEntry,
    primary_cursor: u64,
    auxiliary_cursor: u64,
}

/// Maps a verified native-rank stream into one precomputed quarantined arena.
///
/// The sink allocates nothing on successful chunk callbacks. It never owns an
/// execution permit and cannot seal until every tensor and plane is complete.
pub struct PlannedRankTensorSink<'a, W: QuarantinedArenaWriter> {
    rank: u8,
    tensors: &'a [TensorArenaEntry],
    writer: W,
    next_tensor: usize,
    active: Option<ActiveTensorUpload>,
    metadata_bytes: u64,
    primary_bytes: u64,
    auxiliary_bytes: u64,
    failed: bool,
}

impl<'a, W: QuarantinedArenaWriter> PlannedRankTensorSink<'a, W> {
    pub fn new(plan: &'a RankSetLoadPlan, rank: u8, writer: W) -> Result<Self, LoadPlanError> {
        let rank_entry = plan.rank(rank).ok_or(LoadPlanError::Rank)?;
        if writer.weight_capacity() != rank_entry.device_weight_arena_bytes
            || writer.metadata_capacity() != rank_entry.device_metadata_arena_bytes
        {
            return Err(LoadPlanError::ArenaSize);
        }
        Ok(Self {
            rank,
            tensors: &plan.tensors[usize::from(rank)],
            writer,
            next_tensor: 0,
            active: None,
            metadata_bytes: 0,
            primary_bytes: 0,
            auxiliary_bytes: 0,
            failed: false,
        })
    }

    pub fn drain_and_seal(mut self) -> Result<(W, RankArenaUploadSummary), LoadPlanError> {
        if self.failed || self.active.is_some() || self.next_tensor != self.tensors.len() {
            return Err(LoadPlanError::Incomplete);
        }
        self.writer
            .drain_and_seal()
            .map_err(|_| LoadPlanError::Writer)?;
        let uploaded_bytes = self
            .metadata_bytes
            .checked_add(self.primary_bytes)
            .and_then(|bytes| bytes.checked_add(self.auxiliary_bytes))
            .ok_or(LoadPlanError::Overflow)?;
        Ok((
            self.writer,
            RankArenaUploadSummary {
                rank: self.rank,
                tensor_count: u32::try_from(self.next_tensor)
                    .map_err(|_| LoadPlanError::Overflow)?,
                metadata_bytes: self.metadata_bytes,
                primary_bytes: self.primary_bytes,
                auxiliary_bytes: self.auxiliary_bytes,
                uploaded_bytes,
            },
        ))
    }

    fn fail(&mut self, error: LoadPlanError) -> Result<(), String> {
        self.failed = true;
        Err(error.to_string())
    }
}

impl<W: QuarantinedArenaWriter> RankTensorSink for PlannedRankTensorSink<'_, W> {
    fn begin_tensor(
        &mut self,
        rank: u32,
        index: usize,
        _name: &str,
        descriptor: &TensorDescriptor,
        codec_metadata: &[u8],
    ) -> Result<(), String> {
        if self.failed
            || self.active.is_some()
            || rank != u32::from(self.rank)
            || index != self.next_tensor
        {
            return self.fail(LoadPlanError::Transition);
        }
        let Some(entry) = self.tensors.get(index).copied() else {
            return self.fail(LoadPlanError::Tensor);
        };
        if descriptor.tensor_id != entry.tensor_id
            || descriptor.role_id != entry.role_id
            || descriptor.codec_id != entry.codec_id
            || u32::from(descriptor.flags) != entry.descriptor_flags
            || descriptor.payload_bytes != entry.primary_bytes
            || descriptor.aux_bytes != entry.auxiliary_bytes
            || descriptor.codec_metadata_bytes != entry.metadata_bytes
            || descriptor.payload_alignment != entry.required_device_alignment
            || codec_metadata.len()
                != usize::try_from(entry.metadata_bytes)
                    .map_err(|_| LoadPlanError::Overflow.to_string())?
        {
            return self.fail(LoadPlanError::Tensor);
        }
        if let Err(error) = self
            .writer
            .write_metadata(entry.metadata_destination_offset, codec_metadata)
        {
            self.failed = true;
            return Err(error.to_string());
        }
        self.metadata_bytes = self
            .metadata_bytes
            .checked_add(entry.metadata_bytes)
            .ok_or_else(|| LoadPlanError::Overflow.to_string())?;
        self.active = Some(ActiveTensorUpload {
            entry,
            primary_cursor: 0,
            auxiliary_cursor: 0,
        });
        Ok(())
    }

    fn primary_chunk(&mut self, bytes: &[u8]) -> Result<(), String> {
        let Some(mut active) = self.active else {
            return self.fail(LoadPlanError::Transition);
        };
        let byte_count =
            u64::try_from(bytes.len()).map_err(|_| LoadPlanError::Overflow.to_string())?;
        let end = active
            .primary_cursor
            .checked_add(byte_count)
            .ok_or_else(|| LoadPlanError::Overflow.to_string())?;
        if self.failed || end > active.entry.primary_bytes {
            return self.fail(LoadPlanError::Bounds);
        }
        let destination = active
            .entry
            .primary_destination_offset
            .checked_add(active.primary_cursor)
            .ok_or_else(|| LoadPlanError::Overflow.to_string())?;
        if let Err(error) = self.writer.write_weight(destination, bytes) {
            self.failed = true;
            return Err(error.to_string());
        }
        active.primary_cursor = end;
        self.primary_bytes = self
            .primary_bytes
            .checked_add(byte_count)
            .ok_or_else(|| LoadPlanError::Overflow.to_string())?;
        self.active = Some(active);
        Ok(())
    }

    fn aux_chunk(&mut self, bytes: &[u8]) -> Result<(), String> {
        let Some(mut active) = self.active else {
            return self.fail(LoadPlanError::Transition);
        };
        let byte_count =
            u64::try_from(bytes.len()).map_err(|_| LoadPlanError::Overflow.to_string())?;
        let end = active
            .auxiliary_cursor
            .checked_add(byte_count)
            .ok_or_else(|| LoadPlanError::Overflow.to_string())?;
        if self.failed
            || active.primary_cursor != active.entry.primary_bytes
            || end > active.entry.auxiliary_bytes
        {
            return self.fail(LoadPlanError::Bounds);
        }
        let destination = active
            .entry
            .auxiliary_destination_offset
            .checked_add(active.auxiliary_cursor)
            .ok_or_else(|| LoadPlanError::Overflow.to_string())?;
        if let Err(error) = self.writer.write_weight(destination, bytes) {
            self.failed = true;
            return Err(error.to_string());
        }
        active.auxiliary_cursor = end;
        self.auxiliary_bytes = self
            .auxiliary_bytes
            .checked_add(byte_count)
            .ok_or_else(|| LoadPlanError::Overflow.to_string())?;
        self.active = Some(active);
        Ok(())
    }

    fn finish_tensor(&mut self) -> Result<(), String> {
        let Some(active) = self.active.take() else {
            return self.fail(LoadPlanError::Transition);
        };
        if self.failed
            || active.primary_cursor != active.entry.primary_bytes
            || active.auxiliary_cursor != active.entry.auxiliary_bytes
        {
            return self.fail(LoadPlanError::Incomplete);
        }
        self.next_tensor = self
            .next_tensor
            .checked_add(1)
            .ok_or_else(|| LoadPlanError::Overflow.to_string())?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadPlanError {
    Header,
    Identity,
    Rank,
    Tensor,
    Alignment,
    Bounds,
    Overlap,
    ArenaSize,
    ArenaLayout,
    Receipt,
    Adoption,
    Transition,
    Incomplete,
    Writer,
    Overflow,
    Encoding,
    Evidence,
    Reader,
    Manifest,
    Profile,
    Capability,
    Memory,
}

impl fmt::Display for LoadPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "checkpoint load-plan validation failed: {self:?}"
        )
    }
}

impl std::error::Error for LoadPlanError {}

fn validate_header(header: &RankSetLoadPlanHeader) -> Result<(), LoadPlanError> {
    if header.tensor_count == 0
        || header.conversion_uuid == [0; 16]
        || header.staging_slot_bytes < READER_CHUNK_BYTES
        || header.staging_slots_per_rank < 2
        || [
            header.weight_policy_sha256,
            header.kernel_abi_sha256,
            header.memory_plan_sha256,
            header.codec_capability_sha256,
            header.model_config_sha256,
            header.tokenizer_bundle_sha256,
            header.chat_template_sha256,
            header.operation_manifest_sha256,
            header.tensor_catalog_sha256,
            header.profile_budget_sha256,
        ]
        .iter()
        .any(is_zero)
    {
        return Err(LoadPlanError::Header);
    }
    if header.verification_mode == LoadVerificationMode::FsVerity {
        // FS-verity is not an available first-load or restart route in v1.
        return Err(LoadPlanError::Header);
    }
    Ok(())
}

fn validate_ranks(
    header: &RankSetLoadPlanHeader,
    ranks: &[RankLoadEntry; RANK_SET_SIZE],
    tensors: &[Vec<TensorArenaEntry>; RANK_SET_SIZE],
) -> Result<(), LoadPlanError> {
    for expected_rank in 0..RANK_SET_SIZE {
        let rank = ranks[expected_rank];
        if usize::from(rank.rank) != expected_rank
            || rank.tensor_count != header.tensor_count
            || rank.file_uuid == [0; 16]
            || rank.file_payload_bytes == 0
            || rank.device_weight_arena_bytes == 0
            || [
                rank.device_identity_sha256,
                rank.manifest_sha256,
                rank.descriptor_sha256,
                rank.payload_sha256,
                rank.arena_layout_sha256,
                rank.tensor_contract_sha256,
            ]
            .iter()
            .any(is_zero)
        {
            return Err(LoadPlanError::Rank);
        }
        for prior in &ranks[..expected_rank] {
            if prior.device_identity_sha256 == rank.device_identity_sha256
                || prior.file_uuid == rank.file_uuid
            {
                return Err(LoadPlanError::Identity);
            }
        }
        validate_tensor_layout(rank, &tensors[expected_rank])?;
    }

    for tensor_index in
        0..usize::try_from(header.tensor_count).map_err(|_| LoadPlanError::Overflow)?
    {
        let reference = tensors[0][tensor_index];
        for rank_tensors in &tensors[1..] {
            let observed = rank_tensors[tensor_index];
            if observed.tensor_id != reference.tensor_id
                || observed.role_id != reference.role_id
                || observed.codec_id != reference.codec_id
                || observed.descriptor_flags != reference.descriptor_flags
                || observed.required_device_alignment != reference.required_device_alignment
            {
                return Err(LoadPlanError::Tensor);
            }
        }
    }
    Ok(())
}

fn validate_tensor_layout(
    rank: RankLoadEntry,
    tensors: &[TensorArenaEntry],
) -> Result<(), LoadPlanError> {
    if tensors.len() != usize::try_from(rank.tensor_count).map_err(|_| LoadPlanError::Overflow)? {
        return Err(LoadPlanError::Tensor);
    }
    let mut weight_intervals = Vec::with_capacity(tensors.len() * 2);
    let mut metadata_intervals = Vec::with_capacity(tensors.len());
    for (expected_tensor, tensor) in tensors.iter().copied().enumerate() {
        if tensor.tensor_id
            != u32::try_from(expected_tensor).map_err(|_| LoadPlanError::Overflow)?
            || tensor.role_id == 0
            || tensor.codec_id == 0
            || tensor.primary_bytes == 0
            || tensor.required_device_alignment == 0
            || !tensor.required_device_alignment.is_power_of_two()
        {
            return Err(LoadPlanError::Tensor);
        }
        let alignment = u64::from(tensor.required_device_alignment);
        add_interval(
            &mut weight_intervals,
            tensor.primary_destination_offset,
            tensor.primary_bytes,
            rank.device_weight_arena_bytes,
            alignment,
        )?;
        add_interval(
            &mut weight_intervals,
            tensor.auxiliary_destination_offset,
            tensor.auxiliary_bytes,
            rank.device_weight_arena_bytes,
            alignment,
        )?;
        add_interval(
            &mut metadata_intervals,
            tensor.metadata_destination_offset,
            tensor.metadata_bytes,
            rank.device_metadata_arena_bytes,
            alignment,
        )?;
    }
    require_exact_nonoverlap(&mut weight_intervals, rank.device_weight_arena_bytes)?;
    require_exact_nonoverlap(&mut metadata_intervals, rank.device_metadata_arena_bytes)?;
    if arena_layout_sha256(
        rank.rank,
        rank.device_weight_arena_bytes,
        rank.device_metadata_arena_bytes,
        tensors,
    ) != rank.arena_layout_sha256
    {
        return Err(LoadPlanError::ArenaLayout);
    }
    Ok(())
}

fn add_interval(
    intervals: &mut Vec<(u64, u64)>,
    offset: u64,
    bytes: u64,
    capacity: u64,
    alignment: u64,
) -> Result<(), LoadPlanError> {
    if bytes == 0 {
        if offset > capacity {
            return Err(LoadPlanError::Bounds);
        }
        return Ok(());
    }
    if !offset.is_multiple_of(alignment) {
        return Err(LoadPlanError::Alignment);
    }
    let end = offset.checked_add(bytes).ok_or(LoadPlanError::Overflow)?;
    if end > capacity {
        return Err(LoadPlanError::Bounds);
    }
    intervals.push((offset, end));
    Ok(())
}

fn align_up(value: u64, alignment: u64) -> Result<u64, LoadPlanError> {
    value
        .checked_add(alignment.checked_sub(1).ok_or(LoadPlanError::Alignment)?)
        .map(|rounded| rounded / alignment * alignment)
        .ok_or(LoadPlanError::Overflow)
}

fn require_exact_nonoverlap(
    intervals: &mut [(u64, u64)],
    arena_bytes: u64,
) -> Result<(), LoadPlanError> {
    if intervals.is_empty() {
        return if arena_bytes == 0 {
            Ok(())
        } else {
            Err(LoadPlanError::ArenaSize)
        };
    }
    intervals.sort_unstable();
    let mut previous_end = 0_u64;
    for &(start, end) in intervals.iter() {
        if start < previous_end {
            return Err(LoadPlanError::Overlap);
        }
        previous_end = end;
    }
    if previous_end != arena_bytes {
        return Err(LoadPlanError::ArenaSize);
    }
    Ok(())
}

fn encode_header(header: &RankSetLoadPlanHeader) -> [u8; LOAD_PLAN_HEADER_BYTES] {
    let mut output = [0_u8; LOAD_PLAN_HEADER_BYTES];
    output[0..8].copy_from_slice(b"G5LOAD1\0");
    put_u16(&mut output, 8, 1);
    put_u16(
        &mut output,
        10,
        u16::try_from(LOAD_PLAN_HEADER_BYTES).expect("constant fits"),
    );
    output[12] = header.verification_mode as u8;
    output[13] = header.profile as u8;
    output[14] = RANK_SET_SIZE as u8;
    put_u32(&mut output, 16, header.tensor_count);
    put_u32(
        &mut output,
        20,
        u32::try_from(RANK_LOAD_ENTRY_BYTES).expect("constant fits"),
    );
    put_u32(
        &mut output,
        24,
        u32::try_from(TENSOR_ARENA_ENTRY_BYTES).expect("constant fits"),
    );
    put_u32(&mut output, 28, READER_CHUNK_BYTES);
    output[32..48].copy_from_slice(&header.conversion_uuid);
    for (offset, digest) in [
        (48, header.weight_policy_sha256),
        (80, header.kernel_abi_sha256),
        (112, header.memory_plan_sha256),
        (144, header.codec_capability_sha256),
        (176, header.model_config_sha256),
        (208, header.tokenizer_bundle_sha256),
        (240, header.chat_template_sha256),
        (272, header.operation_manifest_sha256),
        (304, header.tensor_catalog_sha256),
        (336, header.profile_budget_sha256),
    ] {
        output[offset..offset + 32].copy_from_slice(&digest);
    }
    put_u32(&mut output, 368, header.staging_slot_bytes);
    put_u16(&mut output, 372, header.staging_slots_per_rank);
    output
}

fn encode_rank(rank: RankLoadEntry) -> [u8; RANK_LOAD_ENTRY_BYTES] {
    let mut output = [0_u8; RANK_LOAD_ENTRY_BYTES];
    output[0] = rank.rank;
    output[8..40].copy_from_slice(&rank.device_identity_sha256);
    output[40..56].copy_from_slice(&rank.file_uuid);
    output[56..88].copy_from_slice(&rank.manifest_sha256);
    output[88..120].copy_from_slice(&rank.descriptor_sha256);
    output[120..152].copy_from_slice(&rank.payload_sha256);
    put_u32(&mut output, 152, rank.tensor_count);
    put_u64(&mut output, 160, rank.file_payload_bytes);
    put_u64(&mut output, 168, rank.device_weight_arena_bytes);
    put_u64(&mut output, 176, rank.device_metadata_arena_bytes);
    output[184..216].copy_from_slice(&rank.arena_layout_sha256);
    output[216..248].copy_from_slice(&rank.tensor_contract_sha256);
    output
}

fn hash_domain(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    hasher.finalize().into()
}

fn is_zero<const N: usize>(bytes: &[u8; N]) -> bool {
    bytes.iter().all(|&byte| byte == 0)
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    output[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use glm_format::ValidatedTensorSemantic;

    #[derive(Debug)]
    struct MockArenaWriter {
        weight: Vec<u8>,
        metadata: Vec<u8>,
        weight_written: Vec<bool>,
        metadata_written: Vec<bool>,
        writes: usize,
        fail_on_write: Option<usize>,
        sealed: bool,
    }

    impl MockArenaWriter {
        fn new(weight_bytes: usize, metadata_bytes: usize) -> Self {
            Self {
                weight: vec![0; weight_bytes],
                metadata: vec![0; metadata_bytes],
                weight_written: vec![false; weight_bytes],
                metadata_written: vec![false; metadata_bytes],
                writes: 0,
                fail_on_write: None,
                sealed: false,
            }
        }

        fn copy(
            destination: &mut [u8],
            written: &mut [bool],
            offset: u64,
            bytes: &[u8],
        ) -> Result<(), &'static str> {
            let start = usize::try_from(offset).map_err(|_| "offset")?;
            let end = start.checked_add(bytes.len()).ok_or("overflow")?;
            let destination = destination.get_mut(start..end).ok_or("bounds")?;
            let written = written.get_mut(start..end).ok_or("bounds")?;
            if written.iter().any(|&value| value) {
                return Err("duplicate");
            }
            destination.copy_from_slice(bytes);
            written.fill(true);
            Ok(())
        }

        fn before_write(&mut self) -> Result<(), &'static str> {
            if self.sealed {
                return Err("sealed");
            }
            self.writes += 1;
            if self.fail_on_write == Some(self.writes) {
                return Err("injected");
            }
            Ok(())
        }
    }

    impl QuarantinedArenaWriter for MockArenaWriter {
        type Error = &'static str;

        fn weight_capacity(&self) -> u64 {
            self.weight.len() as u64
        }

        fn metadata_capacity(&self) -> u64 {
            self.metadata.len() as u64
        }

        fn write_weight(&mut self, offset: u64, bytes: &[u8]) -> Result<(), Self::Error> {
            self.before_write()?;
            Self::copy(&mut self.weight, &mut self.weight_written, offset, bytes)
        }

        fn write_metadata(&mut self, offset: u64, bytes: &[u8]) -> Result<(), Self::Error> {
            self.before_write()?;
            Self::copy(
                &mut self.metadata,
                &mut self.metadata_written,
                offset,
                bytes,
            )
        }

        fn drain_and_seal(&mut self) -> Result<(), Self::Error> {
            if self.sealed
                || self.weight_written.iter().any(|&written| !written)
                || self.metadata_written.iter().any(|&written| !written)
            {
                return Err("incomplete");
            }
            self.sealed = true;
            Ok(())
        }
    }

    fn digest(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn tensor_layout() -> Vec<TensorArenaEntry> {
        vec![
            TensorArenaEntry {
                tensor_id: 0,
                role_id: 1,
                codec_id: 1,
                descriptor_flags: 0,
                metadata_destination_offset: 0,
                metadata_bytes: 128,
                primary_destination_offset: 0,
                primary_bytes: 256,
                auxiliary_destination_offset: 256,
                auxiliary_bytes: 128,
                required_device_alignment: 128,
            },
            TensorArenaEntry {
                tensor_id: 1,
                role_id: 2,
                codec_id: 1,
                descriptor_flags: 1,
                metadata_destination_offset: 0,
                metadata_bytes: 0,
                primary_destination_offset: 384,
                primary_bytes: 128,
                auxiliary_destination_offset: 0,
                auxiliary_bytes: 0,
                required_device_alignment: 128,
            },
            TensorArenaEntry {
                tensor_id: 2,
                role_id: 3,
                codec_id: 2,
                descriptor_flags: 0,
                metadata_destination_offset: 128,
                metadata_bytes: 128,
                primary_destination_offset: 512,
                primary_bytes: 256,
                auxiliary_destination_offset: 768,
                auxiliary_bytes: 256,
                required_device_alignment: 128,
            },
        ]
    }

    fn plan() -> RankSetLoadPlan {
        let header = RankSetLoadPlanHeader {
            verification_mode: LoadVerificationMode::FullSha256,
            profile: LoadProfile::Nvfp4Laboratory,
            tensor_count: 3,
            conversion_uuid: [7; 16],
            weight_policy_sha256: digest(1),
            kernel_abi_sha256: digest(2),
            memory_plan_sha256: digest(3),
            codec_capability_sha256: digest(4),
            model_config_sha256: digest(5),
            tokenizer_bundle_sha256: digest(6),
            chat_template_sha256: digest(7),
            operation_manifest_sha256: digest(8),
            tensor_catalog_sha256: digest(9),
            profile_budget_sha256: digest(10),
            staging_slot_bytes: READER_CHUNK_BYTES,
            staging_slots_per_rank: 2,
        };
        let tensors: [Vec<_>; 4] = std::array::from_fn(|_| tensor_layout());
        let ranks = std::array::from_fn(|rank| {
            let rank_u8 = u8::try_from(rank).unwrap();
            RankLoadEntry {
                rank: rank_u8,
                device_identity_sha256: digest(20 + rank_u8),
                file_uuid: [30 + rank_u8; 16],
                manifest_sha256: digest(40 + rank_u8),
                descriptor_sha256: digest(50 + rank_u8),
                payload_sha256: digest(60 + rank_u8),
                tensor_count: 3,
                file_payload_bytes: 1024,
                device_weight_arena_bytes: 1024,
                device_metadata_arena_bytes: 256,
                arena_layout_sha256: arena_layout_sha256(rank_u8, 1024, 256, &tensors[rank]),
                tensor_contract_sha256: digest(70 + rank_u8),
            }
        });
        RankSetLoadPlan::new(header, ranks, tensors).unwrap()
    }

    fn descriptor(entry: TensorArenaEntry) -> TensorDescriptor {
        TensorDescriptor {
            tensor_id: entry.tensor_id,
            name_offset: 0,
            name_bytes: 1,
            role_id: entry.role_id,
            layer_id: 0,
            expert_id: -1,
            codec_id: entry.codec_id,
            logical_dtype: 1,
            stored_dtype: 1,
            tp_shard_axis: -1,
            ndim: 1,
            flags: u8::try_from(entry.descriptor_flags).unwrap(),
            logical_shape: [1; 4],
            padded_shape: [1; 4],
            payload_offset: 0,
            payload_bytes: entry.primary_bytes,
            aux_offset: 0,
            aux_bytes: entry.auxiliary_bytes,
            codec_metadata_offset: 0,
            codec_metadata_bytes: entry.metadata_bytes,
            payload_alignment: entry.required_device_alignment,
            quant_group_elements: 1,
            payload_sha256: [0; 32],
            aux_sha256: [0; 32],
            codec_metadata_sha256: [0; 32],
        }
    }

    fn semantic(entry: TensorArenaEntry) -> ValidatedTensorSemantic {
        ValidatedTensorSemantic {
            tensor_id: entry.tensor_id,
            role_id: entry.role_id,
            codec_id: entry.codec_id,
            layer_id: 0,
            expert_id: -1,
            tp_shard_axis: -1,
            ndim: 1,
            flags: u8::try_from(entry.descriptor_flags).unwrap(),
            source_binding_kind: 1,
            logical_dtype: 1,
            stored_dtype: 1,
            quant_group_elements: 1,
            rank_logical_shape: [1; 4],
            global_logical_shape: [1; 4],
            name_sha256: digest(90 + u8::try_from(entry.tensor_id).unwrap()),
            reconstruction_id: 1,
            collective_after_id: 0,
            source_dtype_id: 1,
            source_axis: -1,
        }
    }

    fn tensor_contract() -> Vec<TensorLoadContract> {
        tensor_layout()
            .into_iter()
            .map(|entry| TensorLoadContract {
                tensor_id: entry.tensor_id,
                role_id: entry.role_id,
                codec_id: entry.codec_id,
                descriptor_flags: entry.descriptor_flags,
                metadata_bytes: entry.metadata_bytes,
                primary_bytes: entry.primary_bytes,
                auxiliary_bytes: entry.auxiliary_bytes,
                required_device_alignment: entry.required_device_alignment,
            })
            .collect()
    }

    fn prepared_attempt(
        plan: &RankSetLoadPlan,
        generation_base: u64,
    ) -> (
        [RankArenaLifecycle; RANK_SET_SIZE],
        [PreparedRankReceipt; RANK_SET_SIZE],
    ) {
        let receipts = std::array::from_fn(|rank| {
            PreparedRankReceipt::test_only(
                plan,
                u8::try_from(rank).unwrap(),
                generation_base + u64::try_from(rank).unwrap(),
                digest(130 + u8::try_from(rank).unwrap()),
            )
            .unwrap()
        });
        let lifecycles = std::array::from_fn(|rank| {
            let mut lifecycle = RankArenaLifecycle::allocated(
                plan,
                u8::try_from(rank).unwrap(),
                receipts[rank].owner_allocation_generation,
            )
            .unwrap();
            lifecycle.begin_staging().unwrap();
            lifecycle.prepare(plan, receipts[rank]).unwrap();
            lifecycle
        });
        (lifecycles, receipts)
    }

    fn load_verification_evidence(
        plan: &RankSetLoadPlan,
        rank: u8,
        owner_allocation_generation: u64,
    ) -> RankLoadVerificationEvidence {
        let expected = *plan.rank(rank).unwrap();
        RankLoadVerificationEvidence::new(
            plan,
            rank,
            owner_allocation_generation,
            RankPayloadProof {
                rank: u32::from(rank),
                tensor_count: usize::try_from(expected.tensor_count).unwrap(),
                payload_bytes: expected.file_payload_bytes,
                payload_sha256: expected.payload_sha256,
                stream_chunks: 7,
                maximum_reader_scratch_bytes: READER_CHUNK_BYTES as usize,
                storage_read_nanoseconds: 11,
            },
            RankArenaUploadSummary {
                rank,
                tensor_count: expected.tensor_count,
                metadata_bytes: 256,
                primary_bytes: 640,
                auxiliary_bytes: 384,
                uploaded_bytes: 1280,
            },
            RankLoadTimingEvidence {
                storage_read_nanoseconds: 11,
                host_to_pinned_copy_nanoseconds: 12,
                h2d_submission_nanoseconds: 13,
                h2d_drain_nanoseconds: 14,
                full_arena_readback_nanoseconds: 15,
            },
            CudaArenaVerificationEvidence::test_only(
                rank,
                plan.plan_sha256(),
                owner_allocation_generation,
                expected.device_weight_arena_bytes,
                expected.device_metadata_arena_bytes,
                digest(199),
            ),
            digest(200),
        )
        .unwrap()
    }

    #[test]
    fn canonical_plan_encoding_is_exact_and_deterministic() {
        let first = plan();
        let second = plan();
        let bytes = first.canonical_preimage().unwrap();
        assert_eq!(
            bytes.len(),
            LOAD_PLAN_HEADER_BYTES
                + RANK_SET_SIZE * RANK_LOAD_ENTRY_BYTES
                + RANK_SET_SIZE * 3 * TENSOR_ARENA_ENTRY_BYTES
        );
        assert_eq!(&bytes[0..8], b"G5LOAD1\0");
        assert_eq!(&bytes[8..10], 1_u16.to_le_bytes());
        assert_eq!(&bytes[10..12], 416_u16.to_le_bytes());
        assert_eq!(bytes[12], LoadVerificationMode::FullSha256 as u8);
        assert_eq!(bytes[13], LoadProfile::Nvfp4Laboratory as u8);
        assert_eq!(bytes[14], 4);
        assert_eq!(bytes[15], 0);
        assert!(bytes[374..416].iter().all(|&byte| byte == 0));
        assert_eq!(first.plan_sha256, second.plan_sha256);
        assert_eq!(
            first.plan_sha256,
            hash_domain(LOAD_PLAN_DOMAIN, &first.canonical_preimage().unwrap())
        );
    }

    #[test]
    fn authenticated_sources_build_the_complete_plan_and_reject_consensus_drift() {
        let layout = tensor_layout();
        let descriptors: Vec<_> = layout.iter().copied().map(descriptor).collect();
        let semantics: Vec<_> = layout.iter().copied().map(semantic).collect();
        let manifests: [ValidatedRankManifest; RANK_SET_SIZE] =
            std::array::from_fn(|rank| ValidatedRankManifest {
                rank: u8::try_from(rank).unwrap(),
                profile: RankWeightProfile::CapacityExl3,
                conversion_commit: [11; 20],
                operation_manifest_sha256: digest(8),
                tensor_contract_sha256: digest(70 + u8::try_from(rank).unwrap()),
                profile_budget_sha256: digest(10),
                review_artifact_sha256: digest(11),
                format_spec_sha256: digest(12),
                engine_spec_sha256: digest(13),
                tensor_source_payload_bytes: 1024,
                source_verified_file_bytes: 4096,
                tensor_semantics: semantics.clone(),
            });
        let sources: [AuthenticatedRankLoadSource<'_>; RANK_SET_SIZE] =
            std::array::from_fn(|rank| {
                let rank_u8 = u8::try_from(rank).unwrap();
                AuthenticatedRankLoadSource {
                    rank: rank_u8,
                    conversion_uuid: [7; 16],
                    file_uuid: [30 + rank_u8; 16],
                    model_config_sha256: digest(5),
                    tokenizer_bundle_sha256: digest(6),
                    chat_template_sha256: digest(7),
                    weight_policy_sha256: digest(1),
                    kernel_abi_sha256: digest(2),
                    manifest_sha256: digest(40 + rank_u8),
                    descriptor_sha256: digest(50 + rank_u8),
                    payload_sha256: digest(60 + rank_u8),
                    file_payload_bytes: 1024,
                    descriptors: &descriptors,
                    manifest: &manifests[rank],
                }
            });
        let environment = RankSetLoadEnvironment {
            verification_mode: LoadVerificationMode::FullSha256,
            profile: LoadProfile::CapacityExl3,
            device_identity_sha256: std::array::from_fn(|rank| {
                digest(20 + u8::try_from(rank).unwrap())
            }),
            memory_plan_sha256: digest(3),
            codec_capability_sha256: digest(4),
            staging_slot_bytes: READER_CHUNK_BYTES,
            staging_slots_per_rank: 2,
        };
        let contract = tensor_contract();

        let observed =
            build_rank_set_load_plan_from_sources(sources, environment, &contract).unwrap();
        assert_eq!(observed.header.tensor_count, 3);
        assert_eq!(observed.header.conversion_uuid, [7; 16]);
        assert_eq!(
            observed.header.tensor_catalog_sha256,
            rank_invariant_tensor_catalog_sha256(&semantics).unwrap()
        );
        assert_eq!(observed.tensors, std::array::from_fn(|_| layout.clone()));
        for (rank, manifest) in manifests.iter().enumerate() {
            assert_eq!(observed.ranks[rank].rank, u8::try_from(rank).unwrap());
            assert_eq!(
                observed.ranks[rank].tensor_contract_sha256,
                manifest.tensor_contract_sha256
            );
            assert_eq!(observed.ranks[rank].device_weight_arena_bytes, 1024);
            assert_eq!(observed.ranks[rank].device_metadata_arena_bytes, 256);
        }
        assert_eq!(
            observed,
            build_rank_set_load_plan_from_sources(sources, environment, &contract).unwrap()
        );

        let mut identity_drift = sources;
        identity_drift[2].weight_policy_sha256 = digest(99);
        assert_eq!(
            build_rank_set_load_plan_from_sources(identity_drift, environment, &contract),
            Err(LoadPlanError::Identity)
        );

        let mut semantic_drift_manifest = manifests[3].clone();
        semantic_drift_manifest.tensor_semantics[1].source_axis = 0;
        let mut semantic_drift = sources;
        semantic_drift[3].manifest = &semantic_drift_manifest;
        assert_eq!(
            build_rank_set_load_plan_from_sources(semantic_drift, environment, &contract),
            Err(LoadPlanError::Manifest)
        );

        let mut wrong_profile = environment;
        wrong_profile.profile = LoadProfile::Nvfp4Laboratory;
        assert_eq!(
            build_rank_set_load_plan_from_sources(sources, wrong_profile, &contract),
            Err(LoadPlanError::Profile)
        );
    }

    #[test]
    fn descriptor_projection_derives_the_exact_quarantined_arena_layout() {
        let expected = tensor_layout();
        let descriptors: Vec<_> = expected.iter().copied().map(descriptor).collect();
        let contract = tensor_contract();
        let (observed, weight_bytes, metadata_bytes) =
            derive_tensor_arena_entries(&descriptors, &contract).unwrap();
        assert_eq!(observed, expected);
        assert_eq!(weight_bytes, 1024);
        assert_eq!(metadata_bytes, 256);

        let mut invalid = descriptors.clone();
        invalid[1].tensor_id = 7;
        assert_eq!(
            derive_tensor_arena_entries(&invalid, &contract),
            Err(LoadPlanError::Tensor)
        );
        invalid = descriptors;
        invalid[2].payload_alignment = 96;
        assert_eq!(
            derive_tensor_arena_entries(&invalid, &contract),
            Err(LoadPlanError::Tensor)
        );
    }

    #[test]
    fn pinned_capacity_contract_has_exact_full_rank_arena_arithmetic() {
        let contract = pinned_capacity_tensor_load_contract().unwrap();
        assert_eq!(contract.len(), glm_format::PINNED_RANK_TENSOR_COUNT);
        let (tensors, weight_bytes, metadata_bytes) =
            derive_tensor_arena_entries_from_contract(&contract).unwrap();
        let digest = arena_layout_sha256(0, weight_bytes, metadata_bytes, &tensors);
        let digest_hex = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(weight_bytes, 81_605_027_840);
        assert_eq!(metadata_bytes, 14_942_048);
        assert_eq!(
            digest_hex,
            "140274b8d69521115e82ffe72b83af4018dc55c6e7ac7f6bb8ce5af8f81df039"
        );
    }

    #[test]
    fn arena_overlap_and_unbounded_tail_are_rejected() {
        let valid = plan();
        let mut overlap = valid.tensors.clone();
        overlap[2][1].primary_destination_offset = 256;
        let mut ranks = valid.ranks;
        ranks[2].arena_layout_sha256 = arena_layout_sha256(2, 1024, 256, &overlap[2]);
        assert_eq!(
            RankSetLoadPlan::new(valid.header, ranks, overlap),
            Err(LoadPlanError::Overlap)
        );

        let valid = plan();
        let mut ranks = valid.ranks;
        ranks[1].device_weight_arena_bytes = 1152;
        ranks[1].arena_layout_sha256 = arena_layout_sha256(1, 1152, 256, &valid.tensors[1]);
        assert_eq!(
            RankSetLoadPlan::new(valid.header, ranks, valid.tensors),
            Err(LoadPlanError::ArenaSize)
        );
    }

    #[test]
    fn duplicate_device_and_rank_local_semantic_drift_are_rejected() {
        let valid = plan();
        let mut ranks = valid.ranks;
        ranks[3].device_identity_sha256 = ranks[0].device_identity_sha256;
        assert_eq!(
            RankSetLoadPlan::new(valid.header, ranks, valid.tensors.clone()),
            Err(LoadPlanError::Identity)
        );

        let valid = plan();
        let mut tensors = valid.tensors.clone();
        tensors[3][1].codec_id += 1;
        let mut ranks = valid.ranks;
        ranks[3].arena_layout_sha256 = arena_layout_sha256(3, 1024, 256, &tensors[3]);
        assert_eq!(
            RankSetLoadPlan::new(valid.header, ranks, tensors),
            Err(LoadPlanError::Tensor)
        );
    }

    #[test]
    fn prepared_receipts_bind_every_rank_and_exact_byte_counts() {
        let plan = plan();
        let receipts = std::array::from_fn(|rank| {
            PreparedRankReceipt::test_only(
                &plan,
                u8::try_from(rank).unwrap(),
                u64::try_from(rank + 1).unwrap(),
                digest(90 + u8::try_from(rank).unwrap()),
            )
            .unwrap()
        });
        for (rank, receipt) in receipts.iter().copied().enumerate() {
            assert_eq!(receipt.rank as usize, rank);
            assert_eq!(receipt.verified_file_payload_bytes, 1024);
            assert_eq!(receipt.uploaded_plane_metadata_bytes, 1280);
            let encoded = receipt.encode();
            assert_eq!(&encoded[0..8], b"G5PRP1\0\0");
            assert_eq!(&encoded[10..12], 256_u16.to_le_bytes());
            assert!(encoded[232..].iter().all(|&byte| byte == 0));
        }
        assert!(PreparedRankSet::new(&plan, receipts).is_ok());

        let mut invalid = receipts;
        invalid[3].uploaded_plane_metadata_bytes -= 1;
        assert_eq!(
            PreparedRankSet::new(&plan, invalid),
            Err(LoadPlanError::Receipt)
        );
    }

    #[test]
    fn rank_load_verification_evidence_is_exact_typed_and_fail_closed() {
        let plan = plan();
        let evidence = load_verification_evidence(&plan, 2, 41);
        let encoded = evidence.encode();
        assert_eq!(encoded.len(), RANK_LOAD_VERIFICATION_EVIDENCE_BYTES);
        assert_eq!(&encoded[0..8], b"G5LVE1\0\0");
        assert_eq!(&encoded[8..10], 1_u16.to_le_bytes());
        assert_eq!(&encoded[10..12], 256_u16.to_le_bytes());
        assert_eq!(encoded[12], 2);
        assert_eq!(encoded[13], LoadVerificationMode::FullSha256 as u8);
        assert!(encoded[14..16].iter().all(|&byte| byte == 0));
        assert_eq!(&encoded[16..48], &plan.plan_sha256());
        assert_eq!(&encoded[80..88], 41_u64.to_le_bytes());
        assert_eq!(
            &encoded[144..152],
            (2_u64 * u64::from(READER_CHUNK_BYTES)).to_le_bytes()
        );
        assert_eq!(
            evidence.evidence_sha256(),
            [
                0x41, 0x83, 0x93, 0x40, 0xfe, 0x94, 0xfc, 0x6d, 0xd7, 0xd5, 0xd9, 0x08, 0x44, 0x97,
                0xc2, 0x5f, 0xe6, 0x74, 0x8d, 0x83, 0x75, 0xda, 0x14, 0x9c, 0x3c, 0x50, 0xbc, 0x55,
                0x3a, 0x82, 0xa4, 0x6a,
            ]
        );
        let receipt = PreparedRankReceipt::new(&plan, 2, 41, evidence).unwrap();
        assert_eq!(
            receipt.verification_evidence_sha256,
            evidence.evidence_sha256()
        );

        let expected = *plan.rank(2).unwrap();
        let mismatched_payload = RankPayloadProof {
            rank: 2,
            tensor_count: usize::try_from(expected.tensor_count).unwrap(),
            payload_bytes: expected.file_payload_bytes - 1,
            payload_sha256: expected.payload_sha256,
            stream_chunks: 1,
            maximum_reader_scratch_bytes: 1,
            storage_read_nanoseconds: 0,
        };
        assert_eq!(
            RankLoadVerificationEvidence::new(
                &plan,
                2,
                41,
                mismatched_payload,
                RankArenaUploadSummary {
                    rank: 2,
                    tensor_count: expected.tensor_count,
                    metadata_bytes: 256,
                    primary_bytes: 640,
                    auxiliary_bytes: 384,
                    uploaded_bytes: 1280,
                },
                RankLoadTimingEvidence::default(),
                CudaArenaVerificationEvidence::test_only(
                    2,
                    plan.plan_sha256(),
                    41,
                    expected.device_weight_arena_bytes,
                    expected.device_metadata_arena_bytes,
                    digest(199),
                ),
                digest(200),
            ),
            Err(LoadPlanError::Evidence)
        );
    }

    #[test]
    fn adoption_requires_four_identical_commands_and_generations() {
        let plan = plan();
        let receipts = std::array::from_fn(|rank| {
            PreparedRankReceipt::test_only(
                &plan,
                u8::try_from(rank).unwrap(),
                u64::try_from(rank + 11).unwrap(),
                digest(100 + u8::try_from(rank).unwrap()),
            )
            .unwrap()
        });
        let prepared = PreparedRankSet::new(&plan, receipts).unwrap();
        let command = prepared.adoption_command();
        let acknowledgements =
            receipts.map(|receipt| AdoptionAcknowledgement::new(command, receipt).unwrap());
        let adopted = prepared.complete_adoption(acknowledgements).unwrap();
        assert_eq!(adopted.plan_sha256(), plan.plan_sha256());
        assert_ne!(adopted.adopted_rank_set_sha256(), [0; 32]);

        let mut divergent = acknowledgements;
        divergent[2].rank_set_receipt_sha256[0] ^= 1;
        assert_eq!(
            prepared.complete_adoption(divergent),
            Err(LoadPlanError::Adoption)
        );
    }

    #[test]
    fn coordinator_success_requires_all_prepared_and_all_adoption_acknowledgements() {
        let plan = plan();
        let (mut lifecycles, receipts) = prepared_attempt(&plan, 100);
        let generations = receipts.map(|receipt| receipt.owner_allocation_generation);
        let mut coordinator = RankSetLoadCoordinator::new(&plan, 7, generations).unwrap();
        assert_eq!(coordinator.state(), RankSetLoadCoordinatorState::Preparing);
        for (rank, receipt) in receipts.iter().copied().enumerate() {
            let action = coordinator.report_prepared(receipt);
            if rank + 1 == RANK_SET_SIZE {
                assert!(matches!(action, RankSetLoadAction::Adopt(_)));
            } else {
                assert_eq!(action, RankSetLoadAction::Wait);
            }
        }
        assert_eq!(coordinator.state(), RankSetLoadCoordinatorState::Adopting);

        let prepared = PreparedRankSet::new(&plan, receipts).unwrap();
        let acknowledgements: [AdoptionAcknowledgement; RANK_SET_SIZE] =
            std::array::from_fn(|rank| lifecycles[rank].acknowledge_adoption(&prepared).unwrap());
        let mut adopted = None;
        for (rank, acknowledgement) in acknowledgements.into_iter().enumerate() {
            let action = coordinator.report_adoption_acknowledgement(acknowledgement);
            if rank + 1 == RANK_SET_SIZE {
                let RankSetLoadAction::Complete(receipt) = action else {
                    panic!("fourth acknowledgement must complete adoption");
                };
                adopted = Some(receipt);
            } else {
                assert_eq!(action, RankSetLoadAction::Wait);
            }
        }

        let adopted = adopted.unwrap();
        assert_eq!(coordinator.state(), RankSetLoadCoordinatorState::Adopted);
        assert_eq!(coordinator.adopted_receipt(), Some(adopted));
        assert_eq!(coordinator.terminal_error(), None);
        let permits = lifecycles.map(|lifecycle| lifecycle.execution_permit(adopted).unwrap());
        assert!(
            permits
                .iter()
                .enumerate()
                .all(|(rank, permit)| usize::from(permit.rank()) == rank)
        );
    }

    #[test]
    fn every_preparation_rank_failure_emits_one_common_abort_and_no_adoption() {
        let plan = plan();
        for failed_rank in 0..RANK_SET_SIZE {
            let (mut lifecycles, receipts) = prepared_attempt(&plan, 200);
            let generations = receipts.map(|receipt| receipt.owner_allocation_generation);
            let mut coordinator =
                RankSetLoadCoordinator::new(&plan, 10 + failed_rank as u64, generations).unwrap();
            for receipt in receipts.iter().take(failed_rank).copied() {
                assert_eq!(
                    coordinator.report_prepared(receipt),
                    RankSetLoadAction::Wait
                );
            }
            let action = coordinator
                .report_rank_failure(u8::try_from(failed_rank).unwrap(), LoadPlanError::Reader);
            assert_eq!(
                action,
                RankSetLoadAction::Abort(coordinator.abort_command())
            );
            assert_eq!(coordinator.state(), RankSetLoadCoordinatorState::Aborted);
            assert_eq!(coordinator.terminal_error(), Some(LoadPlanError::Reader));
            assert_eq!(coordinator.adopted_receipt(), None);
            assert_eq!(
                coordinator.report_prepared(receipts[failed_rank]),
                RankSetLoadAction::Abort(coordinator.abort_command())
            );
            for lifecycle in &mut lifecycles {
                assert!(lifecycle.abort());
                assert!(!lifecycle.abort());
                assert_eq!(lifecycle.state(), RankArenaState::Aborted);
            }
        }
    }

    #[test]
    fn every_adoption_rank_failure_aborts_prepared_and_already_adopted_ranks() {
        let plan = plan();
        for failed_rank in 0..RANK_SET_SIZE {
            let (mut lifecycles, receipts) = prepared_attempt(&plan, 300);
            let generations = receipts.map(|receipt| receipt.owner_allocation_generation);
            let mut coordinator =
                RankSetLoadCoordinator::new(&plan, 20 + failed_rank as u64, generations).unwrap();
            for receipt in receipts {
                let _ = coordinator.report_prepared(receipt);
            }
            let prepared = PreparedRankSet::new(&plan, receipts).unwrap();
            for lifecycle in lifecycles.iter_mut().take(failed_rank) {
                let acknowledgement = lifecycle.acknowledge_adoption(&prepared).unwrap();
                assert_eq!(
                    coordinator.report_adoption_acknowledgement(acknowledgement),
                    RankSetLoadAction::Wait
                );
            }
            let action = coordinator
                .report_rank_failure(u8::try_from(failed_rank).unwrap(), LoadPlanError::Adoption);
            assert_eq!(
                action,
                RankSetLoadAction::Abort(coordinator.abort_command())
            );
            assert_eq!(coordinator.state(), RankSetLoadCoordinatorState::Aborted);
            assert_eq!(coordinator.adopted_receipt(), None);
            for lifecycle in &mut lifecycles {
                assert!(lifecycle.abort());
                assert_eq!(lifecycle.state(), RankArenaState::Aborted);
            }
        }
    }

    #[test]
    fn duplicate_or_malformed_coordinator_messages_are_terminal() {
        let plan = plan();
        let (_, receipts) = prepared_attempt(&plan, 400);
        let generations = receipts.map(|receipt| receipt.owner_allocation_generation);
        let mut stale_generation = RankSetLoadCoordinator::new(&plan, 30, generations).unwrap();
        let mut stale_receipt = receipts[0];
        stale_receipt.owner_allocation_generation += 1;
        assert_eq!(
            stale_generation.report_prepared(stale_receipt),
            RankSetLoadAction::Abort(stale_generation.abort_command())
        );
        assert_eq!(
            stale_generation.terminal_error(),
            Some(LoadPlanError::Receipt)
        );

        let mut duplicate_prepared = RankSetLoadCoordinator::new(&plan, 31, generations).unwrap();
        assert_eq!(
            duplicate_prepared.report_prepared(receipts[0]),
            RankSetLoadAction::Wait
        );
        assert_eq!(
            duplicate_prepared.report_prepared(receipts[0]),
            RankSetLoadAction::Abort(duplicate_prepared.abort_command())
        );
        assert_eq!(
            duplicate_prepared.terminal_error(),
            Some(LoadPlanError::Receipt)
        );

        let (mut lifecycles, receipts) = prepared_attempt(&plan, 500);
        let generations = receipts.map(|receipt| receipt.owner_allocation_generation);
        let mut duplicate_ack = RankSetLoadCoordinator::new(&plan, 32, generations).unwrap();
        for receipt in receipts {
            let _ = duplicate_ack.report_prepared(receipt);
        }
        let prepared = PreparedRankSet::new(&plan, receipts).unwrap();
        let acknowledgement = lifecycles[0].acknowledge_adoption(&prepared).unwrap();
        assert_eq!(
            duplicate_ack.report_adoption_acknowledgement(acknowledgement),
            RankSetLoadAction::Wait
        );
        assert_eq!(
            duplicate_ack.report_adoption_acknowledgement(acknowledgement),
            RankSetLoadAction::Abort(duplicate_ack.abort_command())
        );
        assert_eq!(
            duplicate_ack.terminal_error(),
            Some(LoadPlanError::Adoption)
        );
        for lifecycle in &mut lifecycles {
            assert!(lifecycle.abort());
        }

        assert!(matches!(
            RankSetLoadCoordinator::new(&plan, 0, [1; RANK_SET_SIZE]),
            Err(LoadPlanError::Transition)
        ));
        assert!(matches!(
            RankSetLoadCoordinator::new(&plan, 1, [0; RANK_SET_SIZE]),
            Err(LoadPlanError::Transition)
        ));
    }

    #[test]
    fn lifecycle_needs_global_adoption_before_it_can_issue_execution_permits() {
        let plan = plan();
        let receipts = std::array::from_fn(|rank| {
            PreparedRankReceipt::test_only(
                &plan,
                u8::try_from(rank).unwrap(),
                u64::try_from(rank + 21).unwrap(),
                digest(110 + u8::try_from(rank).unwrap()),
            )
            .unwrap()
        });
        let mut lifecycles: [RankArenaLifecycle; RANK_SET_SIZE] = std::array::from_fn(|rank| {
            RankArenaLifecycle::allocated(
                &plan,
                u8::try_from(rank).unwrap(),
                receipts[rank].owner_allocation_generation,
            )
            .unwrap()
        });
        for (lifecycle, receipt) in lifecycles.iter_mut().zip(receipts) {
            lifecycle.begin_staging().unwrap();
            lifecycle.prepare(&plan, receipt).unwrap();
            assert_eq!(lifecycle.state(), RankArenaState::Prepared);
        }
        let prepared = PreparedRankSet::new(&plan, receipts).unwrap();
        let acknowledgements =
            std::array::from_fn(|rank| lifecycles[rank].acknowledge_adoption(&prepared).unwrap());
        assert!(
            lifecycles
                .iter()
                .all(|lifecycle| lifecycle.state() == RankArenaState::Adopted)
        );

        let mut divergent = acknowledgements;
        divergent[3].owner_allocation_generation += 1;
        assert_eq!(
            prepared.complete_adoption(divergent),
            Err(LoadPlanError::Adoption)
        );

        let adopted = prepared.complete_adoption(acknowledgements).unwrap();
        let permits = lifecycles.map(|lifecycle| lifecycle.execution_permit(adopted).unwrap());
        for (rank, permit) in permits.into_iter().enumerate() {
            assert_eq!(permit.rank() as usize, rank);
            assert_eq!(permit.plan_sha256(), plan.plan_sha256());
            assert_eq!(
                permit.owner_allocation_generation(),
                u64::try_from(rank + 21).unwrap()
            );
            assert_eq!(
                permit.adopted_rank_set_sha256(),
                adopted.adopted_rank_set_sha256()
            );
        }
    }

    #[test]
    fn lifecycle_rejects_skips_and_abort_is_exactly_once() {
        let plan = plan();
        let receipt = PreparedRankReceipt::test_only(&plan, 0, 1, digest(120)).unwrap();
        let mut lifecycle = RankArenaLifecycle::allocated(&plan, 0, 1).unwrap();
        assert_eq!(
            lifecycle.prepare(&plan, receipt),
            Err(LoadPlanError::Transition)
        );
        lifecycle.begin_staging().unwrap();
        assert_eq!(lifecycle.begin_staging(), Err(LoadPlanError::Transition));
        assert!(lifecycle.abort());
        assert!(!lifecycle.abort());
        assert_eq!(lifecycle.state(), RankArenaState::Aborted);
        assert_eq!(
            lifecycle.prepare(&plan, receipt),
            Err(LoadPlanError::Transition)
        );
    }

    #[test]
    fn planned_stream_routes_every_plane_once_and_seals_only_when_complete() {
        let plan = plan();
        let writer = MockArenaWriter::new(1024, 256);
        let mut sink = PlannedRankTensorSink::new(&plan, 0, writer).unwrap();
        for (index, entry) in plan.tensors[0].iter().copied().enumerate() {
            let metadata = vec![entry.tensor_id as u8 + 1; entry.metadata_bytes as usize];
            sink.begin_tensor(
                0,
                index,
                "bound-by-descriptor-hash",
                &descriptor(entry),
                &metadata,
            )
            .unwrap();
            let primary = vec![entry.tensor_id as u8 + 11; entry.primary_bytes as usize];
            let split = primary.len() / 2;
            sink.primary_chunk(&primary[..split]).unwrap();
            sink.primary_chunk(&primary[split..]).unwrap();
            if entry.auxiliary_bytes != 0 {
                let auxiliary = vec![entry.tensor_id as u8 + 21; entry.auxiliary_bytes as usize];
                sink.aux_chunk(&auxiliary).unwrap();
            }
            sink.finish_tensor().unwrap();
        }
        let (writer, summary) = sink.drain_and_seal().unwrap();
        assert!(writer.sealed);
        assert_eq!(
            summary,
            RankArenaUploadSummary {
                rank: 0,
                tensor_count: 3,
                metadata_bytes: 256,
                primary_bytes: 640,
                auxiliary_bytes: 384,
                uploaded_bytes: 1280,
            }
        );
        assert!(writer.weight_written.iter().all(|&written| written));
        assert!(writer.metadata_written.iter().all(|&written| written));
    }

    #[test]
    fn planned_stream_rejects_early_aux_overrun_and_writer_failure() {
        let plan = plan();
        let entry = plan.tensors[0][0];
        let writer = MockArenaWriter::new(1024, 256);
        let mut sink = PlannedRankTensorSink::new(&plan, 0, writer).unwrap();
        sink.begin_tensor(
            0,
            0,
            "tensor",
            &descriptor(entry),
            &vec![1; entry.metadata_bytes as usize],
        )
        .unwrap();
        assert!(sink.aux_chunk(&[1]).is_err());
        assert_eq!(
            sink.drain_and_seal().map(|_| ()),
            Err(LoadPlanError::Incomplete)
        );

        let mut writer = MockArenaWriter::new(1024, 256);
        writer.fail_on_write = Some(2);
        let mut sink = PlannedRankTensorSink::new(&plan, 0, writer).unwrap();
        sink.begin_tensor(
            0,
            0,
            "tensor",
            &descriptor(entry),
            &vec![1; entry.metadata_bytes as usize],
        )
        .unwrap();
        assert!(
            sink.primary_chunk(&vec![2; entry.primary_bytes as usize])
                .is_err()
        );
        assert!(sink.finish_tensor().is_err());
    }

    #[test]
    fn planned_stream_rejects_descriptor_or_arena_capacity_drift() {
        let plan = plan();
        assert!(matches!(
            PlannedRankTensorSink::new(&plan, 0, MockArenaWriter::new(1023, 256)),
            Err(LoadPlanError::ArenaSize)
        ));

        let entry = plan.tensors[0][0];
        let writer = MockArenaWriter::new(1024, 256);
        let mut sink = PlannedRankTensorSink::new(&plan, 0, writer).unwrap();
        let mut changed = descriptor(entry);
        changed.codec_id += 1;
        assert!(
            sink.begin_tensor(
                0,
                0,
                "tensor",
                &changed,
                &vec![1; entry.metadata_bytes as usize],
            )
            .is_err()
        );
    }

    #[test]
    fn fs_verity_cannot_open_a_serving_profile_in_v1() {
        let valid = plan();
        let mut header = valid.header;
        header.verification_mode = LoadVerificationMode::FsVerity;
        header.profile = LoadProfile::HybridServe;
        assert_eq!(
            RankSetLoadPlan::new(header, valid.ranks, valid.tensors),
            Err(LoadPlanError::Header)
        );
    }

    #[test]
    fn fs_verity_is_also_closed_for_the_laboratory_first_load() {
        let valid = plan();
        let mut header = valid.header;
        header.verification_mode = LoadVerificationMode::FsVerity;
        assert_eq!(
            RankSetLoadPlan::new(header, valid.ranks, valid.tensors),
            Err(LoadPlanError::Header)
        );
    }
}
