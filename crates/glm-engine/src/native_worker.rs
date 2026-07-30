use std::{
    mem,
    path::{Path, PathBuf},
};

use glm_cuda::{KernelError, NativeRankContext, NativeRankLoadBackend};
use glm_format::NativeRankReader;

use crate::{
    AcknowledgedCudaRank, AdoptedRankSetReceipt, AdoptionAcknowledgement, CollectiveSchedule,
    CudaWeightArena, LoadPlanError, PreparedCudaRank, PreparedRankReceipt, PreparedRankSet,
    RANK_SET_SIZE, RankCheckpointLoadError, RankExecutionError, RankExecutor, RankExecutorFactory,
    RankSetAbortCommand, RankSetLoadPlan, StepInput, StepOutput, StepPlan, Tp4WorkerPool,
    WorkerError,
};

const NATIVE_PROGRAM_NOT_IMPLEMENTED: i32 = -1;

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
    software_provenance_sha256: [u8; 32],
    rank: u8,
}

impl NativeCheckpointRankExecutor {
    /// Binds this owner thread to one SM120 rank and opens its immutable image.
    pub fn open(
        rank: u8,
        rank_file: impl AsRef<Path>,
        software_provenance_sha256: [u8; 32],
    ) -> Result<Self, RankCheckpointLoadError> {
        if usize::from(rank) >= RANK_SET_SIZE {
            return Err(LoadPlanError::Rank.into());
        }
        if software_provenance_sha256 == [0; 32] {
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
            software_provenance_sha256,
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
                self.weights = NativeWeightState::Resident(arena);
                Ok(())
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
    ) -> Result<Self, WorkerError> {
        if software_provenance_sha256 == [0; 32] {
            return Err(WorkerError::Config);
        }
        let factories: [Box<dyn RankExecutorFactory>; RANK_SET_SIZE] =
            rank_files.map(|rank_file| {
                Box::new(move |rank| {
                    NativeCheckpointRankExecutor::open(rank, &rank_file, software_provenance_sha256)
                        .map(|executor| Box::new(executor) as Box<dyn RankExecutor>)
                        .map_err(|error| WorkerError::RankCheckpointLoad { rank, error })
                }) as Box<dyn RankExecutorFactory>
            });
        Self::spawn_factories(maximum_outstanding, factories)
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
