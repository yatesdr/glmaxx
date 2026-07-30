use std::fmt;

use glm_format::{RankTensorSink, TensorDescriptor};
use sha2::{Digest, Sha256};

pub const LOAD_PLAN_HEADER_BYTES: usize = 416;
pub const RANK_LOAD_ENTRY_BYTES: usize = 248;
pub const TENSOR_ARENA_ENTRY_BYTES: usize = 64;
pub const PREPARED_RANK_RECEIPT_BYTES: usize = 256;
pub const RANK_SET_SIZE: usize = 4;
pub const READER_CHUNK_BYTES: u32 = 8 * 1024 * 1024;

const LOAD_PLAN_DOMAIN: &[u8] = b"glmaxx.rank-set-load-plan.v1\0";
const ARENA_LAYOUT_DOMAIN: &[u8] = b"glmaxx.rank-arena-layout.v1\0";
const PREPARED_RANK_DOMAIN: &[u8] = b"glmaxx.prepared-rank-receipt.v1\0";
const PREPARED_RANK_SET_DOMAIN: &[u8] = b"glmaxx.prepared-rank-set.v1\0";
const ADOPTED_RANK_SET_DOMAIN: &[u8] = b"glmaxx.adopted-rank-set.v1\0";

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

    fn uploaded_bytes(self) -> Result<u64, LoadPlanError> {
        self.metadata_bytes
            .checked_add(self.primary_bytes)
            .and_then(|bytes| bytes.checked_add(self.auxiliary_bytes))
            .ok_or(LoadPlanError::Overflow)
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
        self.tensors
            .get(usize::from(rank))
            .ok_or(LoadPlanError::Rank)?
            .iter()
            .try_fold(0_u64, |total, tensor| {
                total
                    .checked_add(tensor.uploaded_bytes()?)
                    .ok_or(LoadPlanError::Overflow)
            })
    }
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
        let expected = Self::new(
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
            PreparedRankReceipt::new(
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
    fn adoption_requires_four_identical_commands_and_generations() {
        let plan = plan();
        let receipts = std::array::from_fn(|rank| {
            PreparedRankReceipt::new(
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
    fn lifecycle_needs_global_adoption_before_it_can_issue_execution_permits() {
        let plan = plan();
        let receipts = std::array::from_fn(|rank| {
            PreparedRankReceipt::new(
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
        let receipt = PreparedRankReceipt::new(&plan, 0, 1, digest(120)).unwrap();
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
