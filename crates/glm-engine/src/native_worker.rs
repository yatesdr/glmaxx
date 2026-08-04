use std::{
    fmt, mem,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use glm_cuda::{
    KernelError, NativeRankContext, NativeRankLoadBackend,
    native_checkpoint_codec_capability_sha256,
};
use glm_format::{NativeRankReader, NativeRankReaderError, pinned_exl3_weight_policy_sha256};

use crate::{
    AcknowledgedCudaRank, AdoptedRankSetReceipt, AdoptionAcknowledgement, CollectiveSchedule,
    CudaWeightArena, LoadPlanError, LoadProfile, LoadVerificationMode, PreparedCudaRank,
    PreparedRankReceipt, PreparedRankSet, RANK_SET_SIZE, RankCheckpointLoadError,
    RankExecutionError, RankExecutor, RankExecutorFactory, RankLoadVerificationEvidence,
    RankSetAbortCommand, RankSetLoadEnvironment, RankSetLoadPlan, StepInput, StepOutput, StepPlan,
    SystemMemoryPlan, Tp4WorkerPool, WeightLoadFailure, WeightLoadOutcome, WeightShutdownFailure,
    WeightShutdownOutcome, WorkerError, WorkerExecutionPosture, build_rank_set_load_plan,
};

const NATIVE_PROGRAM_NOT_IMPLEMENTED: i32 = -1;
type RankLoadEvidenceSink = Arc<Mutex<Option<RankLoadVerificationEvidence>>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ActiveWeightLoad {
    plan_sha256: [u8; 32],
    load_attempt_generation: u64,
    owner_allocation_generation: u64,
}

enum NativeWeightState {
    Vacant,
    Prepared(PreparedCudaRank<NativeRankLoadBackend>),
    Acknowledged(AcknowledgedCudaRank<NativeRankLoadBackend>),
    Resident(CudaWeightArena<NativeRankLoadBackend>),
    CleanupFailed,
}

/// Thread-affine native checkpoint owner for one persistent SM120 rank.
///
/// This adapter intentionally does not provide a CPU execution fallback. It
/// wires the authenticated native reader and CUDA checkpoint typestates into
/// the four-rank worker transaction. Step execution remains fail-closed until
/// the target-layer CUDA program consumes the resident arena.
pub struct NativeCheckpointRankExecutor {
    // Field order is deliberate: every arena and its backend must be dropped
    // before the context-owned stream is destroyed.
    weights: NativeWeightState,
    context: NativeRankContext,
    reader: NativeRankReader,
    active_weight_load: Option<ActiveWeightLoad>,
    load_evidence_sink: Option<RankLoadEvidenceSink>,
    software_provenance_sha256: [u8; 32],
    required_hbm_bytes: u64,
    rank: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeCheckpointStartupConfig {
    pub maximum_outstanding: usize,
    pub verification_mode: LoadVerificationMode,
    pub profile: LoadProfile,
    pub memory_plan: SystemMemoryPlan,
    pub codec_capability_sha256: [u8; 32],
    pub operation_manifest_sha256: [u8; 32],
    pub profile_budget_sha256: [u8; 32],
    pub staging_slot_bytes: u32,
    pub staging_slots_per_rank: u16,
    pub software_provenance_sha256: [u8; 32],
    pub load_attempt_generation: u64,
    pub owner_allocation_generations: [u64; RANK_SET_SIZE],
    pub phase_timeout: Duration,
}

#[derive(Debug)]
pub enum NativeCheckpointStartupError {
    Config,
    Capability(KernelError),
    Reader {
        rank: u8,
        error: NativeRankReaderError,
    },
    Plan(LoadPlanError),
    Worker(WorkerError),
    Load(WeightLoadFailure),
}

impl fmt::Display for NativeCheckpointStartupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for NativeCheckpointStartupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config => None,
            Self::Capability(error) => Some(error),
            Self::Reader { error, .. } => Some(error),
            Self::Plan(error) => Some(error),
            Self::Worker(error) => Some(error),
            Self::Load(error) => Some(error),
        }
    }
}

/// A TP4 worker generation whose four immutable rank arenas were globally
/// adopted by one successful checkpoint transaction.
pub struct LoadedNativeCheckpoint {
    pool: Tp4WorkerPool,
    plan: Arc<RankSetLoadPlan>,
    load_outcome: WeightLoadOutcome,
    device_identity_sha256: [[u8; 32]; RANK_SET_SIZE],
    rank_load_verification_evidence: [RankLoadVerificationEvidence; RANK_SET_SIZE],
}

impl LoadedNativeCheckpoint {
    #[must_use]
    pub const fn pool(&self) -> &Tp4WorkerPool {
        &self.pool
    }

    #[must_use]
    pub fn plan(&self) -> &RankSetLoadPlan {
        &self.plan
    }

    #[must_use]
    pub const fn load_outcome(&self) -> &WeightLoadOutcome {
        &self.load_outcome
    }

    #[must_use]
    pub const fn device_identity_sha256(&self) -> [[u8; 32]; RANK_SET_SIZE] {
        self.device_identity_sha256
    }

    #[must_use]
    pub const fn rank_load_verification_evidence(
        &self,
    ) -> [RankLoadVerificationEvidence; RANK_SET_SIZE] {
        self.rank_load_verification_evidence
    }

    /// Releases all four resident arenas and joins the persistent owner
    /// threads before returning.
    ///
    /// Callers may publish a successful checkpoint-load smoke result only
    /// after this method returns four authenticated cleanup acknowledgements.
    pub fn shutdown(
        self,
        phase_timeout: Duration,
    ) -> Result<WeightShutdownOutcome, WeightShutdownFailure> {
        let Self {
            pool,
            plan,
            load_outcome,
            device_identity_sha256: _,
            rank_load_verification_evidence: _,
        } = self;
        let owner_allocation_generations = load_outcome
            .finalize_acknowledgements
            .map(|acknowledgement| acknowledgement.owner_allocation_generation());
        let result = pool.shutdown_weights(
            plan,
            load_outcome.load_attempt_generation,
            owner_allocation_generations,
            phase_timeout,
        );
        // `Tp4WorkerPool::drop` joins the dispatcher and all four rank-owner
        // threads. Cleanup acknowledgements precede this join.
        drop(pool);
        result
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        Tp4WorkerPool,
        Arc<RankSetLoadPlan>,
        WeightLoadOutcome,
        [[u8; 32]; RANK_SET_SIZE],
    ) {
        (
            self.pool,
            self.plan,
            self.load_outcome,
            self.device_identity_sha256,
        )
    }
}

impl NativeCheckpointRankExecutor {
    /// Binds this owner thread to one SM120 rank and opens its immutable image.
    pub fn open(
        rank: u8,
        rank_file: impl AsRef<Path>,
        software_provenance_sha256: [u8; 32],
        required_hbm_bytes: u64,
    ) -> Result<Self, RankCheckpointLoadError> {
        Self::open_with_evidence_sink(
            rank,
            rank_file,
            software_provenance_sha256,
            required_hbm_bytes,
            None,
        )
    }

    fn open_with_evidence_sink(
        rank: u8,
        rank_file: impl AsRef<Path>,
        software_provenance_sha256: [u8; 32],
        required_hbm_bytes: u64,
        load_evidence_sink: Option<RankLoadEvidenceSink>,
    ) -> Result<Self, RankCheckpointLoadError> {
        if usize::from(rank) >= RANK_SET_SIZE {
            return Err(LoadPlanError::Rank.into());
        }
        if software_provenance_sha256 == [0; 32] || required_hbm_bytes == 0 {
            return Err(LoadPlanError::Evidence.into());
        }
        let context = NativeRankContext::bind(rank)?;
        let reader = NativeRankReader::open(rank_file)?;
        if reader.rank != u32::from(rank) {
            return Err(LoadPlanError::Rank.into());
        }
        Ok(Self {
            weights: NativeWeightState::Vacant,
            context,
            reader,
            active_weight_load: None,
            load_evidence_sink,
            software_provenance_sha256,
            required_hbm_bytes,
            rank,
        })
    }

    #[must_use]
    pub const fn rank(&self) -> u8 {
        self.rank
    }

    #[must_use]
    pub const fn software_provenance_sha256(&self) -> [u8; 32] {
        self.software_provenance_sha256
    }

    pub fn device_identity_sha256(&self) -> Result<[u8; 32], KernelError> {
        // `stream` performs the context's owner-thread check. The identity
        // itself was captured from the successful native bind.
        let _ = self.context.stream()?;
        Ok(self.context.identity().identity_sha256())
    }

    fn validate_active(
        &self,
        rank: u8,
        plan_sha256: [u8; 32],
        owner_allocation_generation: u64,
    ) -> Result<ActiveWeightLoad, LoadPlanError> {
        if rank != self.rank {
            return Err(LoadPlanError::Rank);
        }
        let active = self.active_weight_load.ok_or(LoadPlanError::Transition)?;
        if active.plan_sha256 != plan_sha256
            || active.owner_allocation_generation != owner_allocation_generation
        {
            return Err(LoadPlanError::Transition);
        }
        Ok(active)
    }

    fn fail_closed_execute(
        &self,
        rank: u8,
        _plan: &StepPlan,
        _schedule: &CollectiveSchedule,
    ) -> Result<StepOutput, RankExecutionError> {
        if rank != self.rank {
            return Err(RankExecutionError::Invariant);
        }
        let NativeWeightState::Resident(arena) = &self.weights else {
            return Err(RankExecutionError::Invariant);
        };
        if arena.rank() != rank || self.context.stream().is_err() {
            return Err(RankExecutionError::Invariant);
        }
        Err(RankExecutionError::Backend(NATIVE_PROGRAM_NOT_IMPLEMENTED))
    }
}

impl RankExecutor for NativeCheckpointRankExecutor {
    fn execute(
        &mut self,
        rank: u8,
        plan: &StepPlan,
        schedule: &CollectiveSchedule,
    ) -> Result<StepOutput, RankExecutionError> {
        self.fail_closed_execute(rank, plan, schedule)
    }

    fn execute_bound(
        &mut self,
        rank: u8,
        plan: &StepPlan,
        schedule: &CollectiveSchedule,
        _input: &StepInput,
    ) -> Result<StepOutput, RankExecutionError> {
        self.fail_closed_execute(rank, plan, schedule)
    }

    fn checkpoint_device_identity_sha256(&mut self, rank: u8) -> Result<[u8; 32], LoadPlanError> {
        if rank != self.rank {
            return Err(LoadPlanError::Rank);
        }
        self.device_identity_sha256().map_err(map_kernel_error)
    }

    fn prepare_weights(
        &mut self,
        rank: u8,
        plan: &RankSetLoadPlan,
        load_attempt_generation: u64,
        owner_allocation_generation: u64,
    ) -> Result<PreparedRankReceipt, LoadPlanError> {
        if rank != self.rank
            || load_attempt_generation == 0
            || owner_allocation_generation == 0
            || plan.rank(rank).is_none()
        {
            return Err(LoadPlanError::Rank);
        }
        if self.active_weight_load.is_some() || !matches!(self.weights, NativeWeightState::Vacant) {
            return Err(LoadPlanError::Transition);
        }
        if self.context.free_memory_bytes().map_err(map_kernel_error)? < self.required_hbm_bytes {
            return Err(LoadPlanError::Memory);
        }
        self.active_weight_load = Some(ActiveWeightLoad {
            plan_sha256: plan.plan_sha256(),
            load_attempt_generation,
            owner_allocation_generation,
        });

        let backend = self
            .context
            .checkpoint_load_backend()
            .map_err(map_kernel_error)?;
        let prepared = PreparedCudaRank::load(
            plan,
            rank,
            &self.reader,
            backend,
            owner_allocation_generation,
            self.software_provenance_sha256,
        )
        .map_err(map_checkpoint_error)?;
        let receipt = prepared.receipt();
        let verification_evidence = prepared
            .verification_evidence()
            .ok_or(LoadPlanError::Evidence)?;
        if verification_evidence.evidence_sha256() != receipt.verification_evidence_sha256 {
            return Err(LoadPlanError::Evidence);
        }
        if let Some(sink) = &self.load_evidence_sink {
            let mut slot = sink.lock().map_err(|_| LoadPlanError::Writer)?;
            if slot.is_some() {
                return Err(LoadPlanError::Transition);
            }
            *slot = Some(verification_evidence);
        }
        self.weights = NativeWeightState::Prepared(prepared);
        Ok(receipt)
    }

    fn acknowledge_weight_adoption(
        &mut self,
        rank: u8,
        prepared: &PreparedRankSet,
    ) -> Result<AdoptionAcknowledgement, LoadPlanError> {
        let receipt = *prepared.receipt(rank).ok_or(LoadPlanError::Rank)?;
        self.validate_active(
            rank,
            prepared.plan_sha256,
            receipt.owner_allocation_generation,
        )?;
        let state = mem::replace(&mut self.weights, NativeWeightState::CleanupFailed);
        let NativeWeightState::Prepared(local) = state else {
            self.weights = state;
            return Err(LoadPlanError::Transition);
        };
        match local.acknowledge_adoption(prepared) {
            Ok((acknowledged, acknowledgement)) => {
                self.weights = NativeWeightState::Acknowledged(acknowledged);
                Ok(acknowledgement)
            }
            Err(error) => {
                // The consuming typestate transition drops and cleans the
                // quarantined arena on error. Retain the attempt identity so
                // the common abort can acknowledge the now-empty rank.
                self.weights = NativeWeightState::Vacant;
                Err(map_checkpoint_error(error))
            }
        }
    }

    fn finalize_weights(
        &mut self,
        rank: u8,
        adopted: AdoptedRankSetReceipt,
    ) -> Result<(), LoadPlanError> {
        let active = self.active_weight_load.ok_or(LoadPlanError::Transition)?;
        self.validate_active(
            rank,
            adopted.plan_sha256(),
            active.owner_allocation_generation,
        )?;
        let state = mem::replace(&mut self.weights, NativeWeightState::CleanupFailed);
        let NativeWeightState::Acknowledged(local) = state else {
            self.weights = state;
            return Err(LoadPlanError::Transition);
        };
        match local.adopt(adopted) {
            Ok(arena) => {
                if let Err(validation_error) =
                    validate_resident_tensor_bindings(&arena, &self.reader)
                {
                    match arena.shutdown() {
                        Ok(()) => {
                            self.weights = NativeWeightState::Vacant;
                            Err(validation_error)
                        }
                        Err(cleanup_error) => {
                            self.weights = NativeWeightState::CleanupFailed;
                            Err(map_kernel_error(cleanup_error))
                        }
                    }
                } else {
                    self.weights = NativeWeightState::Resident(arena);
                    Ok(())
                }
            }
            Err(error) => {
                // A failed consuming adoption releases the quarantined arena.
                // The common abort still needs the attempt identity.
                self.weights = NativeWeightState::Vacant;
                Err(map_checkpoint_error(error))
            }
        }
    }

    fn abort_weight_load(
        &mut self,
        rank: u8,
        command: RankSetAbortCommand,
        owner_allocation_generation: u64,
    ) -> Result<(), LoadPlanError> {
        let active =
            self.validate_active(rank, command.plan_sha256(), owner_allocation_generation)?;
        if active.load_attempt_generation != command.load_attempt_generation() {
            return Err(LoadPlanError::Transition);
        }

        let state = mem::replace(&mut self.weights, NativeWeightState::CleanupFailed);
        let cleanup = match state {
            NativeWeightState::Vacant => Ok(()),
            NativeWeightState::Prepared(local) => {
                local.abort_and_release().map_err(map_checkpoint_error)
            }
            NativeWeightState::Acknowledged(local) => {
                local.abort_and_release().map_err(map_checkpoint_error)
            }
            NativeWeightState::Resident(arena) => arena.shutdown().map_err(map_kernel_error),
            NativeWeightState::CleanupFailed => Err(LoadPlanError::Writer),
        };
        match cleanup {
            Ok(()) => {
                if let Some(sink) = &self.load_evidence_sink {
                    let Ok(mut slot) = sink.lock() else {
                        self.weights = NativeWeightState::CleanupFailed;
                        return Err(LoadPlanError::Writer);
                    };
                    *slot = None;
                }
                self.weights = NativeWeightState::Vacant;
                self.active_weight_load = None;
                Ok(())
            }
            Err(error) => {
                // Never forge a cleanup acknowledgement after any physical
                // release failure, even if the native RAII object retained or
                // deliberately leaked the unsafe handle.
                self.weights = NativeWeightState::CleanupFailed;
                Err(error)
            }
        }
    }
}

/// Opens, authenticates, plans, allocates, verifies, and globally adopts one
/// four-rank native checkpoint.
///
/// The preflight plan uses synthetic unique device identities only to prove
/// the four files and all non-device plan inputs before native startup. The
/// published plan is rebuilt from identities reported by the same persistent
/// rank executors that perform the allocation.
pub fn load_native_checkpoint(
    rank_files: [PathBuf; RANK_SET_SIZE],
    config: NativeCheckpointStartupConfig,
) -> Result<LoadedNativeCheckpoint, NativeCheckpointStartupError> {
    if config.maximum_outstanding == 0
        || config.software_provenance_sha256 == [0; 32]
        || config.operation_manifest_sha256 == [0; 32]
        || config.profile_budget_sha256 == [0; 32]
        || config.load_attempt_generation == 0
        || config.owner_allocation_generations.contains(&0)
        || config.phase_timeout.is_zero()
    {
        return Err(NativeCheckpointStartupError::Config);
    }
    config
        .memory_plan
        .validate()
        .map_err(|_| NativeCheckpointStartupError::Plan(LoadPlanError::Memory))?;
    if config
        .memory_plan
        .ranks
        .iter()
        .any(|rank| rank.profile != crate::ProfileClass::CapacityExl3)
    {
        return Err(NativeCheckpointStartupError::Plan(LoadPlanError::Profile));
    }
    let memory_plan_sha256 = config
        .memory_plan
        .artifact_sha256()
        .map_err(|_| NativeCheckpointStartupError::Plan(LoadPlanError::Memory))?;
    let required_hbm_bytes =
        std::array::from_fn(|rank| config.memory_plan.ranks[rank].required_bytes);
    let linked_codec_capability_sha256 = native_checkpoint_codec_capability_sha256()
        .map_err(NativeCheckpointStartupError::Capability)?;
    if config.codec_capability_sha256 != linked_codec_capability_sha256 {
        return Err(NativeCheckpointStartupError::Plan(
            LoadPlanError::Capability,
        ));
    }

    let mut opened = Vec::with_capacity(RANK_SET_SIZE);
    for (rank, path) in rank_files.iter().enumerate() {
        let rank_u8 = u8::try_from(rank).map_err(|_| NativeCheckpointStartupError::Config)?;
        let reader =
            NativeRankReader::open(path).map_err(|error| NativeCheckpointStartupError::Reader {
                rank: rank_u8,
                error,
            })?;
        if reader.rank != u32::from(rank_u8) {
            return Err(NativeCheckpointStartupError::Plan(LoadPlanError::Rank));
        }
        let manifest = reader
            .validated_manifest()
            .ok_or(NativeCheckpointStartupError::Plan(LoadPlanError::Manifest))?;
        if manifest.operation_manifest_sha256 != config.operation_manifest_sha256
            || manifest.profile_budget_sha256 != config.profile_budget_sha256
            || reader.weight_policy_sha256 != pinned_exl3_weight_policy_sha256()
        {
            return Err(NativeCheckpointStartupError::Plan(LoadPlanError::Identity));
        }
        opened.push(reader);
    }
    let readers: [NativeRankReader; RANK_SET_SIZE] = opened
        .try_into()
        .map_err(|_| NativeCheckpointStartupError::Config)?;
    let reader_refs = [&readers[0], &readers[1], &readers[2], &readers[3]];

    let preflight_identities =
        std::array::from_fn(|rank| [u8::try_from(rank).expect("four ranks fit") + 1; 32]);
    let preflight_plan = build_rank_set_load_plan(
        reader_refs,
        config.rank_set_environment(preflight_identities, memory_plan_sha256),
    )
    .map_err(NativeCheckpointStartupError::Plan)?;
    validate_checkpoint_arena_budget(&preflight_plan, &config.memory_plan)
        .map_err(NativeCheckpointStartupError::Plan)?;

    let evidence_sinks: [RankLoadEvidenceSink; RANK_SET_SIZE] =
        std::array::from_fn(|_| Arc::new(Mutex::new(None)));
    let pool = Tp4WorkerPool::spawn_native_checkpoint_loaders_with_evidence(
        config.maximum_outstanding,
        rank_files,
        config.software_provenance_sha256,
        required_hbm_bytes,
        std::array::from_fn(|rank| Some(Arc::clone(&evidence_sinks[rank]))),
    )
    .map_err(NativeCheckpointStartupError::Worker)?;
    let device_identity_sha256 = pool
        .checkpoint_device_identities(config.phase_timeout)
        .map_err(NativeCheckpointStartupError::Worker)?;
    let plan = Arc::new(
        build_rank_set_load_plan(
            reader_refs,
            config.rank_set_environment(device_identity_sha256, memory_plan_sha256),
        )
        .map_err(NativeCheckpointStartupError::Plan)?,
    );
    let load_outcome = pool
        .load_weights(
            Arc::clone(&plan),
            config.load_attempt_generation,
            config.owner_allocation_generations,
            config.phase_timeout,
        )
        .map_err(NativeCheckpointStartupError::Load)?;
    let mut rank_evidence = Vec::with_capacity(RANK_SET_SIZE);
    for (rank, sink) in evidence_sinks.iter().enumerate() {
        let evidence = sink
            .lock()
            .map_err(|_| NativeCheckpointStartupError::Plan(LoadPlanError::Writer))?
            .ok_or(NativeCheckpointStartupError::Plan(LoadPlanError::Evidence))?;
        let receipt = load_outcome.prepared_receipts[rank];
        if evidence.rank()
            != u8::try_from(rank).map_err(|_| NativeCheckpointStartupError::Config)?
            || evidence.plan_sha256() != plan.plan_sha256()
            || evidence.owner_allocation_generation() != receipt.owner_allocation_generation
            || evidence.evidence_sha256() != receipt.verification_evidence_sha256
        {
            return Err(NativeCheckpointStartupError::Plan(LoadPlanError::Evidence));
        }
        rank_evidence.push(evidence);
    }
    let rank_load_verification_evidence = rank_evidence
        .try_into()
        .map_err(|_| NativeCheckpointStartupError::Config)?;
    Ok(LoadedNativeCheckpoint {
        pool,
        plan,
        load_outcome,
        device_identity_sha256,
        rank_load_verification_evidence,
    })
}

fn validate_resident_tensor_bindings(
    arena: &CudaWeightArena<NativeRankLoadBackend>,
    reader: &NativeRankReader,
) -> Result<(), LoadPlanError> {
    let manifest = reader.validated_manifest().ok_or(LoadPlanError::Manifest)?;
    if arena.rank() != u8::try_from(reader.rank).map_err(|_| LoadPlanError::Rank)?
        || arena.tensor_count() != reader.tensor_count()
        || manifest.tensor_semantics.len() != reader.tensor_count()
    {
        return Err(LoadPlanError::Tensor);
    }
    for (index, (descriptor, semantic)) in reader
        .descriptors
        .iter()
        .zip(&manifest.tensor_semantics)
        .enumerate()
    {
        let tensor_id = u32::try_from(index).map_err(|_| LoadPlanError::Overflow)?;
        let binding = arena.tensor_binding(tensor_id)?;
        let primary = binding.primary();
        let metadata = binding.metadata();
        let auxiliary = binding.auxiliary();
        if descriptor.tensor_id != tensor_id
            || semantic.tensor_id != tensor_id
            || binding.tensor_id() != tensor_id
            || descriptor.role_id != semantic.role_id
            || binding.role_id() != descriptor.role_id
            || descriptor.codec_id != semantic.codec_id
            || binding.codec_id() != descriptor.codec_id
            || binding.descriptor_flags() != u32::from(descriptor.flags)
            || binding.required_device_alignment() != descriptor.payload_alignment
            || primary.pointer() == 0
            || primary.bytes() != descriptor.payload_bytes
            || metadata.map(|span| span.bytes()).unwrap_or(0) != descriptor.codec_metadata_bytes
            || auxiliary.map(|span| span.bytes()).unwrap_or(0) != descriptor.aux_bytes
            || metadata.is_some_and(|span| span.pointer() == 0)
            || auxiliary.is_some_and(|span| span.pointer() == 0)
        {
            return Err(LoadPlanError::Tensor);
        }
    }
    Ok(())
}

fn validate_checkpoint_arena_budget(
    plan: &RankSetLoadPlan,
    memory_plan: &SystemMemoryPlan,
) -> Result<(), LoadPlanError> {
    for rank in 0..RANK_SET_SIZE {
        let load = plan.ranks.get(rank).ok_or(LoadPlanError::Rank)?;
        let memory = memory_plan.ranks.get(rank).ok_or(LoadPlanError::Memory)?;
        let physical_arena_bytes = load
            .device_weight_arena_bytes
            .checked_add(load.device_metadata_arena_bytes)
            .ok_or(LoadPlanError::Overflow)?;
        let weight_and_metadata_budget = memory
            .terms
            .weights
            .checked_add(memory.terms.model_metadata)
            .ok_or(LoadPlanError::Overflow)?;
        if usize::from(load.rank) != rank
            || usize::from(memory.rank) != rank
            || load.file_payload_bytes != memory.terms.weights
            || physical_arena_bytes > weight_and_metadata_budget
        {
            return Err(LoadPlanError::Memory);
        }
    }
    Ok(())
}

impl NativeCheckpointStartupConfig {
    fn rank_set_environment(
        &self,
        device_identity_sha256: [[u8; 32]; RANK_SET_SIZE],
        memory_plan_sha256: [u8; 32],
    ) -> RankSetLoadEnvironment {
        RankSetLoadEnvironment {
            verification_mode: self.verification_mode,
            profile: self.profile,
            device_identity_sha256,
            memory_plan_sha256,
            codec_capability_sha256: self.codec_capability_sha256,
            staging_slot_bytes: self.staging_slot_bytes,
            staging_slots_per_rank: self.staging_slots_per_rank,
        }
    }
}

impl Tp4WorkerPool {
    /// Creates four persistent native rank owners.
    ///
    /// Construction binds and opens each rank image on its eventual owner
    /// thread. The returned pool can execute the atomic checkpoint load
    /// transaction; step execution remains fail-closed until the native
    /// target-layer program is integrated.
    pub fn spawn_native_checkpoint_loaders(
        maximum_outstanding: usize,
        rank_files: [PathBuf; RANK_SET_SIZE],
        software_provenance_sha256: [u8; 32],
        required_hbm_bytes: [u64; RANK_SET_SIZE],
    ) -> Result<Self, WorkerError> {
        Self::spawn_native_checkpoint_loaders_with_evidence(
            maximum_outstanding,
            rank_files,
            software_provenance_sha256,
            required_hbm_bytes,
            std::array::from_fn(|_| None),
        )
    }

    fn spawn_native_checkpoint_loaders_with_evidence(
        maximum_outstanding: usize,
        rank_files: [PathBuf; RANK_SET_SIZE],
        software_provenance_sha256: [u8; 32],
        required_hbm_bytes: [u64; RANK_SET_SIZE],
        evidence_sinks: [Option<RankLoadEvidenceSink>; RANK_SET_SIZE],
    ) -> Result<Self, WorkerError> {
        if software_provenance_sha256 == [0; 32] || required_hbm_bytes.contains(&0) {
            return Err(WorkerError::Config);
        }
        let factories: [Box<dyn RankExecutorFactory>; RANK_SET_SIZE] =
            std::array::from_fn(|rank| {
                let rank_file = rank_files[rank].clone();
                let required_hbm_bytes = required_hbm_bytes[rank];
                let evidence_sink = evidence_sinks[rank].clone();
                Box::new(move |rank| {
                    NativeCheckpointRankExecutor::open_with_evidence_sink(
                        rank,
                        &rank_file,
                        software_provenance_sha256,
                        required_hbm_bytes,
                        evidence_sink,
                    )
                    .map(|executor| Box::new(executor) as Box<dyn RankExecutor>)
                    .map_err(|error| WorkerError::RankCheckpointLoad { rank, error })
                }) as Box<dyn RankExecutorFactory>
            });
        Self::spawn_factories_with_posture(
            maximum_outstanding,
            factories,
            WorkerExecutionPosture::NativeWeightsOnly,
        )
    }
}

fn map_checkpoint_error(error: RankCheckpointLoadError) -> LoadPlanError {
    match error {
        RankCheckpointLoadError::Plan(error) => error,
        RankCheckpointLoadError::Reader(_) => LoadPlanError::Reader,
        RankCheckpointLoadError::Kernel(error) => map_kernel_error(error),
    }
}

fn map_kernel_error(error: KernelError) -> LoadPlanError {
    match error {
        KernelError::Topology | KernelError::DeviceValidation(_) => LoadPlanError::Identity,
        KernelError::Alignment => LoadPlanError::Alignment,
        KernelError::Overflow => LoadPlanError::Overflow,
        KernelError::Abi
        | KernelError::Path
        | KernelError::Shape
        | KernelError::Null
        | KernelError::Workspace { .. }
        | KernelError::Driver(_)
        | KernelError::Async(_) => LoadPlanError::Writer,
    }
}
