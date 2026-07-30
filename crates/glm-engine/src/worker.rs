use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use glm_cache::{PageTableDelta, PageTableDeltaError, PageTableMirror, SequencePageTable};
use sha2::{Digest, Sha256};

#[cfg(feature = "cuda-ffi")]
use crate::RankCheckpointLoadError;
use crate::{
    AdoptedRankSetReceipt, AdoptionAcknowledgement, CollectiveSchedule, CommittedTokens,
    GLM_52_OUTPUT_VOCABULARY, LoadPlanError, OutputError, PlanError, PreparedRankReceipt,
    PreparedRankSet, RANK_SET_SIZE, RankSetAbortCommand, RankSetLoadAction, RankSetLoadCoordinator,
    RankSetLoadPlan, StepInput, StepInputError, StepMode, StepOutput, StepPlan,
};

const CPU_TOKEN_DOMAIN: &[u8] = b"glmaxx.cpu-worker-token.v1\0";
const TP_RANKS: u8 = 4;

/// Rank-local execution boundary for one persistent TP4 worker thread.
///
/// Implementations own their mutable rank state, including a CUDA context,
/// streams, graph instances, device allocations, and collective handles.
/// The worker verifies the immutable plan and collective schedule before this
/// method is entered.
pub trait RankExecutor: 'static {
    fn execute(
        &mut self,
        rank: u8,
        plan: &StepPlan,
        schedule: &CollectiveSchedule,
    ) -> Result<StepOutput, RankExecutionError>;

    /// Executes a fully bound row payload. The worker has already verified
    /// and applied the page delta to its persistent rank mirror.
    fn execute_bound(
        &mut self,
        rank: u8,
        plan: &StepPlan,
        schedule: &CollectiveSchedule,
        input: &StepInput,
    ) -> Result<StepOutput, RankExecutionError>;

    /// Prepares this rank's authenticated checkpoint in quarantined storage.
    fn prepare_weights(
        &mut self,
        _rank: u8,
        _plan: &RankSetLoadPlan,
        _load_attempt_generation: u64,
        _owner_allocation_generation: u64,
    ) -> Result<PreparedRankReceipt, LoadPlanError> {
        Err(LoadPlanError::Transition)
    }

    /// Retains quarantine while acknowledging one process-common rank set.
    fn acknowledge_weight_adoption(
        &mut self,
        _rank: u8,
        _prepared: &PreparedRankSet,
    ) -> Result<AdoptionAcknowledgement, LoadPlanError> {
        Err(LoadPlanError::Transition)
    }

    /// Makes the already-acknowledged arena executable after global adoption.
    fn finalize_weights(
        &mut self,
        _rank: u8,
        _adopted: AdoptedRankSetReceipt,
    ) -> Result<(), LoadPlanError> {
        Err(LoadPlanError::Transition)
    }

    /// Synchronizes and releases any state owned by the named load attempt.
    ///
    /// This must be idempotent for a rank whose prepare phase failed before
    /// allocating an arena. Returning success means no resource from the
    /// attempt remains live.
    fn abort_weight_load(
        &mut self,
        _rank: u8,
        _command: RankSetAbortCommand,
        _owner_allocation_generation: u64,
    ) -> Result<(), LoadPlanError> {
        Err(LoadPlanError::Transition)
    }
}

/// Constructs a thread-affine executor after the persistent rank thread has
/// started.
///
/// The factory crosses the thread boundary and is therefore `Send`; the
/// returned executor is deliberately not required to be `Send`. CUDA
/// contexts, checkpoint arenas, streams, and graph state can consequently be
/// created on and remain owned by exactly one rank thread.
pub trait RankExecutorFactory: Send + 'static {
    fn create(self: Box<Self>, rank: u8) -> Result<Box<dyn RankExecutor>, WorkerError>;
}

impl<F> RankExecutorFactory for F
where
    F: FnOnce(u8) -> Result<Box<dyn RankExecutor>, WorkerError> + Send + 'static,
{
    fn create(self: Box<Self>, rank: u8) -> Result<Box<dyn RankExecutor>, WorkerError> {
        (*self)(rank)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RankExecutionError {
    Backend(i32),
    Invariant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RankWeightPhase {
    Prepare = 1,
    Acknowledge = 2,
    Finalize = 3,
    Abort = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeightLoadFailureCause {
    Config,
    Saturated,
    Closed,
    Timeout {
        phase: RankWeightPhase,
    },
    RankSet {
        phase: RankWeightPhase,
    },
    Rank {
        rank: u8,
        phase: RankWeightPhase,
        error: LoadPlanError,
    },
    Coordinator(LoadPlanError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RankWeightFinalizeAck {
    rank: u8,
    plan_sha256: [u8; 32],
    owner_allocation_generation: u64,
    adopted_rank_set_sha256: [u8; 32],
}

impl RankWeightFinalizeAck {
    fn new(
        rank: u8,
        plan_sha256: [u8; 32],
        owner_allocation_generation: u64,
        adopted: AdoptedRankSetReceipt,
    ) -> Result<Self, LoadPlanError> {
        if usize::from(rank) >= RANK_SET_SIZE
            || owner_allocation_generation == 0
            || plan_sha256 != adopted.plan_sha256()
        {
            return Err(LoadPlanError::Adoption);
        }
        Ok(Self {
            rank,
            plan_sha256,
            owner_allocation_generation,
            adopted_rank_set_sha256: adopted.adopted_rank_set_sha256(),
        })
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

    #[must_use]
    pub const fn adopted_rank_set_sha256(self) -> [u8; 32] {
        self.adopted_rank_set_sha256
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RankWeightCleanupAck {
    rank: u8,
    plan_sha256: [u8; 32],
    load_attempt_generation: u64,
    owner_allocation_generation: u64,
}

impl RankWeightCleanupAck {
    fn new(
        rank: u8,
        command: RankSetAbortCommand,
        owner_allocation_generation: u64,
    ) -> Result<Self, LoadPlanError> {
        if usize::from(rank) >= RANK_SET_SIZE || owner_allocation_generation == 0 {
            return Err(LoadPlanError::Transition);
        }
        Ok(Self {
            rank,
            plan_sha256: command.plan_sha256(),
            load_attempt_generation: command.load_attempt_generation(),
            owner_allocation_generation,
        })
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
    pub const fn load_attempt_generation(self) -> u64 {
        self.load_attempt_generation
    }

    #[must_use]
    pub const fn owner_allocation_generation(self) -> u64 {
        self.owner_allocation_generation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WeightLoadOutcome {
    pub plan_sha256: [u8; 32],
    pub load_attempt_generation: u64,
    pub prepared_receipts: [PreparedRankReceipt; RANK_SET_SIZE],
    pub adoption_acknowledgements: [AdoptionAcknowledgement; RANK_SET_SIZE],
    pub adopted_receipt: AdoptedRankSetReceipt,
    pub finalize_acknowledgements: [RankWeightFinalizeAck; RANK_SET_SIZE],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WeightLoadFailure {
    pub cause: WeightLoadFailureCause,
    pub cleanup_failure: Option<WeightLoadFailureCause>,
    pub cleanup_acknowledgements: Box<[Option<RankWeightCleanupAck>; RANK_SET_SIZE]>,
}

impl fmt::Display for WeightLoadFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for WeightLoadFailure {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MockWorkerFault {
    DivergentOutput { rank: u8, step_id: u64 },
}

struct CpuRankExecutor {
    fault: Option<MockWorkerFault>,
}

impl RankExecutor for CpuRankExecutor {
    fn execute(
        &mut self,
        rank: u8,
        plan: &StepPlan,
        schedule: &CollectiveSchedule,
    ) -> Result<StepOutput, RankExecutionError> {
        let mut output = cpu_output(plan, schedule)?;
        if self.fault
            == Some(MockWorkerFault::DivergentOutput {
                rank,
                step_id: plan.step_id,
            })
        {
            let mut sequences = output.sequences().to_vec();
            let first = sequences.first_mut().ok_or(RankExecutionError::Invariant)?;
            let divergent = (first.token_ids()[0] + 1) % GLM_52_OUTPUT_VOCABULARY;
            *first = CommittedTokens::target(divergent)?;
            output = StepOutput::new(&sequences)?;
        }
        Ok(output)
    }

    fn execute_bound(
        &mut self,
        rank: u8,
        plan: &StepPlan,
        schedule: &CollectiveSchedule,
        input: &StepInput,
    ) -> Result<StepOutput, RankExecutionError> {
        let mut output = cpu_bound_output(plan, schedule, input)?;
        if self.fault
            == Some(MockWorkerFault::DivergentOutput {
                rank,
                step_id: plan.step_id,
            })
        {
            let mut sequences = output.sequences().to_vec();
            let first = sequences.first_mut().ok_or(RankExecutionError::Invariant)?;
            let divergent = (first.token_ids()[0] + 1) % GLM_52_OUTPUT_VOCABULARY;
            *first = CommittedTokens::target(divergent)?;
            output = StepOutput::new(&sequences)?;
        }
        Ok(output)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RankStepAck {
    pub rank: u8,
    pub step_id: u64,
    pub plan_hash: [u8; 32],
    pub schedule_hash: [u8; 32],
    pub input_hash: [u8; 32],
    pub page_table_global_digest: [u8; 32],
    pub page_table_local_digest: [u8; 32],
    pub output_digest: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepOutcome {
    pub step_id: u64,
    pub plan_hash: [u8; 32],
    pub output_digest: [u8; 32],
    pub output: StepOutput,
    pub rank_acks: [RankStepAck; 4],
}

pub struct StepHandle {
    receiver: Receiver<Result<StepOutcome, WorkerError>>,
}

impl StepHandle {
    pub fn receive(self) -> Result<StepOutcome, WorkerError> {
        self.receiver.recv().map_err(|_| WorkerError::Closed)?
    }

    pub fn receive_timeout(self, timeout: Duration) -> Result<StepOutcome, WorkerError> {
        self.receiver
            .recv_timeout(timeout)
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => WorkerError::Timeout,
                mpsc::RecvTimeoutError::Disconnected => WorkerError::Closed,
            })?
    }
}

struct OutstandingPermit {
    outstanding: Arc<AtomicUsize>,
}

struct ExclusivePermit {
    outstanding: Arc<AtomicUsize>,
}

impl Drop for ExclusivePermit {
    fn drop(&mut self) {
        let prior = self.outstanding.swap(0, Ordering::AcqRel);
        debug_assert_eq!(
            prior,
            usize::MAX,
            "exclusive worker operation lost ownership"
        );
    }
}

impl Drop for OutstandingPermit {
    fn drop(&mut self) {
        let released =
            self.outstanding
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                    current.checked_sub(1)
                });
        debug_assert!(released.is_ok(), "step outstanding counter underflow");
    }
}

struct DispatchCommand {
    plan: StepPlan,
    schedule: CollectiveSchedule,
    binding: Option<StepBinding>,
    response: SyncSender<Result<StepOutcome, WorkerError>>,
    permit: OutstandingPermit,
}

#[derive(Clone)]
struct RankCommand {
    plan: StepPlan,
    schedule: CollectiveSchedule,
    binding: Option<StepBinding>,
    response: SyncSender<Result<RankResult, WorkerError>>,
}

#[derive(Clone)]
struct StepBinding {
    input: Arc<StepInput>,
    delta: Arc<PageTableDelta>,
}

struct WeightLoadCommand {
    plan: Arc<RankSetLoadPlan>,
    load_attempt_generation: u64,
    owner_allocation_generations: [u64; RANK_SET_SIZE],
    phase_timeout: Duration,
    response: SyncSender<Result<WeightLoadOutcome, WeightLoadFailure>>,
    permit: ExclusivePermit,
}

enum PoolCommand {
    Initialize {
        table: Arc<SequencePageTable>,
        generation: u64,
        response: SyncSender<Result<(), WorkerError>>,
    },
    ApplyDelta {
        delta: Arc<PageTableDelta>,
        response: SyncSender<Result<[PageDeltaAck; 4], WorkerError>>,
    },
    LoadWeights(WeightLoadCommand),
    Execute(DispatchCommand),
}

#[derive(Clone)]
enum RankCommandEnvelope {
    Initialize {
        table: Arc<SequencePageTable>,
        generation: u64,
        response: SyncSender<Result<u8, WorkerError>>,
    },
    ApplyDelta {
        delta: Arc<PageTableDelta>,
        response: SyncSender<Result<PageDeltaAck, WorkerError>>,
    },
    PrepareWeights {
        plan: Arc<RankSetLoadPlan>,
        load_attempt_generation: u64,
        owner_allocation_generation: u64,
        response: SyncSender<RankPreparedResult>,
    },
    AcknowledgeWeights {
        prepared: Arc<PreparedRankSet>,
        response: SyncSender<RankAdoptionResult>,
    },
    FinalizeWeights {
        adopted: AdoptedRankSetReceipt,
        owner_allocation_generation: u64,
        response: SyncSender<RankFinalizeResult>,
    },
    AbortWeights {
        command: RankSetAbortCommand,
        owner_allocation_generation: u64,
        response: SyncSender<RankCleanupResult>,
    },
    Execute(RankCommand),
}

struct RankPreparedResult {
    rank: u8,
    result: Result<PreparedRankReceipt, LoadPlanError>,
}

struct RankAdoptionResult {
    rank: u8,
    result: Result<AdoptionAcknowledgement, LoadPlanError>,
}

struct RankFinalizeResult {
    rank: u8,
    result: Result<RankWeightFinalizeAck, LoadPlanError>,
}

struct RankCleanupResult {
    rank: u8,
    result: Result<RankWeightCleanupAck, LoadPlanError>,
}

enum RankExecutorSource {
    Transferred(Box<dyn RankExecutor + Send>),
    Factory(Box<dyn RankExecutorFactory>),
}

impl RankExecutorSource {
    fn initialize(self, rank: u8) -> Result<Box<dyn RankExecutor>, WorkerError> {
        match self {
            Self::Transferred(executor) => Ok(executor),
            Self::Factory(factory) => factory.create(rank),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageDeltaAck {
    pub rank: u8,
    pub generation: u64,
    pub global_digest: [u8; 32],
    pub local_digest: [u8; 32],
}

pub struct Tp4WorkerPool {
    sender: Option<SyncSender<PoolCommand>>,
    dispatcher: Option<JoinHandle<()>>,
    outstanding: Arc<AtomicUsize>,
    maximum_outstanding: usize,
}

impl Tp4WorkerPool {
    pub fn spawn_cpu(
        maximum_outstanding: usize,
        fault: Option<MockWorkerFault>,
    ) -> Result<Self, WorkerError> {
        let executors = std::array::from_fn(|_| {
            Box::new(CpuRankExecutor { fault }) as Box<dyn RankExecutor + Send>
        });
        Self::spawn(maximum_outstanding, executors)
    }

    pub fn spawn(
        maximum_outstanding: usize,
        executors: [Box<dyn RankExecutor + Send>; 4],
    ) -> Result<Self, WorkerError> {
        Self::spawn_inner(maximum_outstanding, executors, None)
    }

    pub fn spawn_factories(
        maximum_outstanding: usize,
        factories: [Box<dyn RankExecutorFactory>; 4],
    ) -> Result<Self, WorkerError> {
        let sources = factories.map(RankExecutorSource::Factory);
        Self::spawn_sources(maximum_outstanding, sources, None)
    }

    fn spawn_inner(
        maximum_outstanding: usize,
        executors: [Box<dyn RankExecutor + Send>; 4],
        rank_spawn_fault: Option<u8>,
    ) -> Result<Self, WorkerError> {
        let sources = executors.map(RankExecutorSource::Transferred);
        Self::spawn_sources(maximum_outstanding, sources, rank_spawn_fault)
    }

    fn spawn_sources(
        maximum_outstanding: usize,
        sources: [RankExecutorSource; 4],
        rank_spawn_fault: Option<u8>,
    ) -> Result<Self, WorkerError> {
        if maximum_outstanding == 0 {
            return Err(WorkerError::Config);
        }
        let (sender, receiver) = mpsc::sync_channel(maximum_outstanding);
        let (startup_sender, startup_receiver) = mpsc::sync_channel(1);
        let dispatcher = thread::Builder::new()
            .name("glmaxx-step-dispatch".into())
            .spawn(move || {
                dispatch_loop(receiver, sources, startup_sender, rank_spawn_fault);
            })
            .map_err(WorkerError::Thread)?;
        match startup_receiver.recv() {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                let _ = dispatcher.join();
                return Err(error);
            }
            Err(_) => {
                return Err(if dispatcher.join().is_err() {
                    WorkerError::WorkerPanic
                } else {
                    WorkerError::Closed
                });
            }
        }
        Ok(Self {
            sender: Some(sender),
            dispatcher: Some(dispatcher),
            outstanding: Arc::new(AtomicUsize::new(0)),
            maximum_outstanding,
        })
    }

    #[cfg(test)]
    pub fn try_submit(
        &self,
        plan: StepPlan,
        schedule: CollectiveSchedule,
    ) -> Result<StepHandle, WorkerError> {
        plan.verify(&schedule)?;
        self.try_submit_inner(plan, schedule, None)
    }

    pub fn try_submit_bound(
        &self,
        plan: StepPlan,
        schedule: CollectiveSchedule,
        input: Arc<StepInput>,
        delta: Arc<PageTableDelta>,
    ) -> Result<StepHandle, WorkerError> {
        input.verify(&plan, &schedule, &delta)?;
        self.try_submit_inner(plan, schedule, Some(StepBinding { input, delta }))
    }

    fn try_submit_inner(
        &self,
        plan: StepPlan,
        schedule: CollectiveSchedule,
        binding: Option<StepBinding>,
    ) -> Result<StepHandle, WorkerError> {
        self.reserve_slot()?;
        let (response, receiver) = mpsc::sync_channel(1);
        let command = PoolCommand::Execute(DispatchCommand {
            plan,
            schedule,
            binding,
            response,
            permit: OutstandingPermit {
                outstanding: Arc::clone(&self.outstanding),
            },
        });
        let Some(sender) = &self.sender else {
            return Err(WorkerError::Closed);
        };
        if let Err(error) = sender.try_send(command) {
            return Err(match error {
                TrySendError::Full(_) => WorkerError::Saturated,
                TrySendError::Disconnected(_) => WorkerError::Closed,
            });
        }
        Ok(StepHandle { receiver })
    }

    pub fn initialize_page_table(
        &self,
        table: Arc<SequencePageTable>,
        generation: u64,
    ) -> Result<(), WorkerError> {
        if self.outstanding() != 0 {
            return Err(WorkerError::Saturated);
        }
        let (response, receiver) = mpsc::sync_channel(1);
        self.sender
            .as_ref()
            .ok_or(WorkerError::Closed)?
            .send(PoolCommand::Initialize {
                table,
                generation,
                response,
            })
            .map_err(|_| WorkerError::Closed)?;
        receiver.recv().map_err(|_| WorkerError::Closed)?
    }

    pub fn apply_page_delta(
        &self,
        delta: Arc<PageTableDelta>,
    ) -> Result<[PageDeltaAck; 4], WorkerError> {
        if self.outstanding() != 0 {
            return Err(WorkerError::Saturated);
        }
        delta.verify()?;
        let (response, receiver) = mpsc::sync_channel(1);
        self.sender
            .as_ref()
            .ok_or(WorkerError::Closed)?
            .send(PoolCommand::ApplyDelta { delta, response })
            .map_err(|_| WorkerError::Closed)?;
        receiver.recv().map_err(|_| WorkerError::Closed)?
    }

    pub fn load_weights(
        &self,
        plan: Arc<RankSetLoadPlan>,
        load_attempt_generation: u64,
        owner_allocation_generations: [u64; RANK_SET_SIZE],
        phase_timeout: Duration,
    ) -> Result<WeightLoadOutcome, WeightLoadFailure> {
        if load_attempt_generation == 0
            || owner_allocation_generations.contains(&0)
            || phase_timeout.is_zero()
        {
            return Err(WeightLoadFailure {
                cause: WeightLoadFailureCause::Config,
                cleanup_failure: None,
                cleanup_acknowledgements: Box::new([None; RANK_SET_SIZE]),
            });
        }
        let permit = self.reserve_exclusive()?;
        let (response, receiver) = mpsc::sync_channel(1);
        self.sender
            .as_ref()
            .ok_or(WeightLoadFailure {
                cause: WeightLoadFailureCause::Closed,
                cleanup_failure: None,
                cleanup_acknowledgements: Box::new([None; RANK_SET_SIZE]),
            })?
            .send(PoolCommand::LoadWeights(WeightLoadCommand {
                plan,
                load_attempt_generation,
                owner_allocation_generations,
                phase_timeout,
                response,
                permit,
            }))
            .map_err(|_| WeightLoadFailure {
                cause: WeightLoadFailureCause::Closed,
                cleanup_failure: None,
                cleanup_acknowledgements: Box::new([None; RANK_SET_SIZE]),
            })?;
        receiver.recv().map_err(|_| WeightLoadFailure {
            cause: WeightLoadFailureCause::Closed,
            cleanup_failure: None,
            cleanup_acknowledgements: Box::new([None; RANK_SET_SIZE]),
        })?
    }

    #[must_use]
    pub fn outstanding(&self) -> usize {
        self.outstanding
            .load(Ordering::Acquire)
            .min(self.maximum_outstanding)
    }

    fn reserve_slot(&self) -> Result<(), WorkerError> {
        self.outstanding
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                if current < self.maximum_outstanding {
                    current.checked_add(1)
                } else {
                    None
                }
            })
            .map(|_| ())
            .map_err(|_| WorkerError::Saturated)
    }

    fn reserve_exclusive(&self) -> Result<ExclusivePermit, WeightLoadFailure> {
        self.outstanding
            .compare_exchange(0, usize::MAX, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| WeightLoadFailure {
                cause: WeightLoadFailureCause::Saturated,
                cleanup_failure: None,
                cleanup_acknowledgements: Box::new([None; RANK_SET_SIZE]),
            })?;
        Ok(ExclusivePermit {
            outstanding: Arc::clone(&self.outstanding),
        })
    }
}

impl Drop for Tp4WorkerPool {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(dispatcher) = self.dispatcher.take() {
            let _ = dispatcher.join();
        }
    }
}

fn dispatch_loop(
    receiver: Receiver<PoolCommand>,
    sources: [RankExecutorSource; 4],
    startup: SyncSender<Result<(), WorkerError>>,
    rank_spawn_fault: Option<u8>,
) {
    let mut rank_senders: Vec<SyncSender<RankCommandEnvelope>> =
        Vec::with_capacity(usize::from(TP_RANKS));
    let mut rank_workers: Vec<JoinHandle<()>> = Vec::with_capacity(usize::from(TP_RANKS));
    let (ready_sender, ready_receiver) =
        mpsc::sync_channel::<Result<u8, WorkerError>>(usize::from(TP_RANKS));
    for (rank, source) in (0..TP_RANKS).zip(sources) {
        let (sender, rank_receiver) = mpsc::sync_channel::<RankCommandEnvelope>(1);
        let builder = thread::Builder::new().name(format!("glmaxx-rank-{rank}"));
        let ready = ready_sender.clone();
        let worker = if rank_spawn_fault == Some(rank) {
            Err(std::io::Error::other(format!(
                "injected rank {rank} thread spawn failure"
            )))
        } else {
            builder.spawn(move || match source.initialize(rank) {
                Ok(executor) => {
                    if ready.send(Ok(rank)).is_ok() {
                        rank_loop(rank, rank_receiver, executor);
                    }
                }
                Err(error) => {
                    let _ = ready.send(Err(error));
                }
            })
        };
        let worker = match worker {
            Ok(worker) => worker,
            Err(error) => {
                let cleanup_panicked = shutdown_rank_workers(rank_senders, rank_workers);
                let startup_error = if cleanup_panicked {
                    WorkerError::WorkerPanic
                } else {
                    WorkerError::Thread(error)
                };
                let _ = startup.send(Err(startup_error));
                return;
            }
        };
        rank_senders.push(sender);
        rank_workers.push(worker);
    }
    drop(ready_sender);
    let mut ready_mask = 0_u8;
    for _ in 0..TP_RANKS {
        let rank = match ready_receiver.recv() {
            Ok(Ok(rank)) => rank,
            Ok(Err(error)) => {
                let cleanup_panicked = shutdown_rank_workers(rank_senders, rank_workers);
                let error = if cleanup_panicked {
                    WorkerError::WorkerPanic
                } else {
                    error
                };
                let _ = startup.send(Err(error));
                return;
            }
            Err(_) => {
                let cleanup_panicked = shutdown_rank_workers(rank_senders, rank_workers);
                let error = if cleanup_panicked {
                    WorkerError::WorkerPanic
                } else {
                    WorkerError::RankStartup
                };
                let _ = startup.send(Err(error));
                return;
            }
        };
        let Some(bit) = 1_u8.checked_shl(u32::from(rank)) else {
            let cleanup_panicked = shutdown_rank_workers(rank_senders, rank_workers);
            let error = if cleanup_panicked {
                WorkerError::WorkerPanic
            } else {
                WorkerError::RankStartup
            };
            let _ = startup.send(Err(error));
            return;
        };
        if rank >= TP_RANKS || ready_mask & bit != 0 {
            let cleanup_panicked = shutdown_rank_workers(rank_senders, rank_workers);
            let error = if cleanup_panicked {
                WorkerError::WorkerPanic
            } else {
                WorkerError::RankStartup
            };
            let _ = startup.send(Err(error));
            return;
        }
        ready_mask |= bit;
    }
    if ready_mask != (1_u8 << TP_RANKS) - 1 || startup.send(Ok(())).is_err() {
        let _ = shutdown_rank_workers(rank_senders, rank_workers);
        return;
    }

    let mut last_step_id = 0_u64;
    let mut initialized = false;
    let mut weights_loaded = false;
    while let Ok(command) = receiver.recv() {
        let (failed, terminal) = match command {
            PoolCommand::Initialize {
                table,
                generation,
                response,
            } => {
                let result = if initialized {
                    Err(WorkerError::PageTableInitialized)
                } else {
                    initialize_rank_page_tables(&rank_senders, table, generation)
                };
                initialized |= result.is_ok();
                let failed = result.is_err();
                let _ = response.send(result);
                (failed, failed)
            }
            PoolCommand::ApplyDelta { delta, response } => {
                let result = if initialized {
                    apply_rank_page_delta(&rank_senders, delta)
                } else {
                    Err(WorkerError::PageTableUninitialized)
                };
                let failed = result.is_err();
                let _ = response.send(result);
                (failed, failed)
            }
            PoolCommand::LoadWeights(command) => {
                let WeightLoadCommand {
                    plan,
                    load_attempt_generation,
                    owner_allocation_generations,
                    phase_timeout,
                    response,
                    permit,
                } = command;
                let result = if weights_loaded {
                    Err(WeightLoadFailure {
                        cause: WeightLoadFailureCause::Coordinator(LoadPlanError::Transition),
                        cleanup_failure: None,
                        cleanup_acknowledgements: Box::new([None; RANK_SET_SIZE]),
                    })
                } else {
                    load_rank_weights(
                        &rank_senders,
                        plan,
                        load_attempt_generation,
                        owner_allocation_generations,
                        phase_timeout,
                    )
                };
                weights_loaded |= result.is_ok();
                let failed = result.is_err();
                drop(permit);
                let _ = response.send(result);
                (failed, failed)
            }
            PoolCommand::Execute(command) => {
                let DispatchCommand {
                    plan,
                    schedule,
                    binding,
                    response,
                    permit,
                } = command;
                let bound_without_initialization = binding.is_some() && !initialized;
                let result = if plan.step_id <= last_step_id {
                    Err(WorkerError::StepOrder)
                } else if bound_without_initialization {
                    Err(WorkerError::PageTableUninitialized)
                } else {
                    last_step_id = plan.step_id;
                    dispatch_one(&rank_senders, &plan, &schedule, binding)
                };
                // Quota belongs to the queued/running TP4 operation, not its
                // response handle.
                drop(permit);
                let failed = result.is_err();
                let _ = response.send(result);
                (failed, failed)
            }
        };
        if terminal {
            debug_assert!(failed);
            // Any rank, mirror, backend, or consensus failure is fatal for
            // this worker generation.
            break;
        }
    }
    let _ = shutdown_rank_workers(rank_senders, rank_workers);
}

fn shutdown_rank_workers(
    rank_senders: Vec<SyncSender<RankCommandEnvelope>>,
    rank_workers: Vec<JoinHandle<()>>,
) -> bool {
    drop(rank_senders);
    let mut panicked = false;
    for worker in rank_workers {
        panicked |= worker.join().is_err();
    }
    panicked
}

fn initialize_rank_page_tables(
    rank_senders: &[SyncSender<RankCommandEnvelope>],
    table: Arc<SequencePageTable>,
    generation: u64,
) -> Result<(), WorkerError> {
    let (ack_sender, ack_receiver) = mpsc::sync_channel(usize::from(TP_RANKS));
    for sender in rank_senders {
        sender
            .send(RankCommandEnvelope::Initialize {
                table: Arc::clone(&table),
                generation,
                response: ack_sender.clone(),
            })
            .map_err(|_| WorkerError::Closed)?;
    }
    drop(ack_sender);
    let mut rank_mask = 0_u8;
    for _ in 0..TP_RANKS {
        let rank = ack_receiver.recv().map_err(|_| WorkerError::Closed)??;
        let bit = 1_u8
            .checked_shl(u32::from(rank))
            .ok_or(WorkerError::RankSet)?;
        if rank >= TP_RANKS || rank_mask & bit != 0 {
            return Err(WorkerError::RankSet);
        }
        rank_mask |= bit;
    }
    if rank_mask != (1_u8 << TP_RANKS) - 1 {
        return Err(WorkerError::RankSet);
    }
    Ok(())
}

fn apply_rank_page_delta(
    rank_senders: &[SyncSender<RankCommandEnvelope>],
    delta: Arc<PageTableDelta>,
) -> Result<[PageDeltaAck; 4], WorkerError> {
    delta.verify()?;
    let (ack_sender, ack_receiver) = mpsc::sync_channel(usize::from(TP_RANKS));
    for sender in rank_senders {
        sender
            .send(RankCommandEnvelope::ApplyDelta {
                delta: Arc::clone(&delta),
                response: ack_sender.clone(),
            })
            .map_err(|_| WorkerError::Closed)?;
    }
    drop(ack_sender);
    let mut acknowledgements = Vec::with_capacity(usize::from(TP_RANKS));
    for _ in 0..TP_RANKS {
        acknowledgements.push(ack_receiver.recv().map_err(|_| WorkerError::Closed)??);
    }
    acknowledgements.sort_by_key(|ack| ack.rank);
    for (rank, ack) in acknowledgements.iter().enumerate() {
        if usize::from(ack.rank) != rank
            || ack.generation != delta.generation_after()
            || ack.global_digest != delta.global_digest()
            || ack.local_digest != delta.rank_local_digest(ack.rank)?
        {
            return Err(WorkerError::Consensus);
        }
    }
    acknowledgements
        .try_into()
        .map_err(|_| WorkerError::RankSet)
}

fn apply_page_delta_on_rank(
    rank: u8,
    page_table: Option<&mut PageTableMirror>,
    delta: &PageTableDelta,
) -> Result<PageDeltaAck, WorkerError> {
    let page_table = page_table.ok_or(WorkerError::PageTableUninitialized)?;
    page_table.apply(delta)?;
    Ok(PageDeltaAck {
        rank,
        generation: page_table.generation(),
        global_digest: delta.global_digest(),
        local_digest: delta.rank_local_digest(rank)?,
    })
}

fn load_rank_weights(
    rank_senders: &[SyncSender<RankCommandEnvelope>],
    plan: Arc<RankSetLoadPlan>,
    load_attempt_generation: u64,
    owner_allocation_generations: [u64; RANK_SET_SIZE],
    phase_timeout: Duration,
) -> Result<WeightLoadOutcome, WeightLoadFailure> {
    let mut coordinator =
        RankSetLoadCoordinator::new(&plan, load_attempt_generation, owner_allocation_generations)
            .map_err(|error| WeightLoadFailure {
            cause: WeightLoadFailureCause::Coordinator(error),
            cleanup_failure: None,
            cleanup_acknowledgements: Box::new([None; RANK_SET_SIZE]),
        })?;
    let abort_command = coordinator.abort_command();

    let prepared_messages = match prepare_rank_weights(
        rank_senders,
        Arc::clone(&plan),
        load_attempt_generation,
        owner_allocation_generations,
        phase_timeout,
    ) {
        Ok(messages) => messages,
        Err(cause) => {
            return Err(abort_weight_load(
                rank_senders,
                cause,
                abort_command,
                owner_allocation_generations,
                phase_timeout,
            ));
        }
    };
    let mut prepared_receipts = Vec::with_capacity(RANK_SET_SIZE);
    let mut prepare_failure = None;
    let mut prepare_route = RankSetLoadAction::Wait;
    for message in prepared_messages {
        match message.result {
            Ok(receipt) => {
                prepare_route = coordinator.report_prepared(receipt);
                prepared_receipts.push(receipt);
            }
            Err(error) => {
                prepare_failure.get_or_insert(WeightLoadFailureCause::Rank {
                    rank: message.rank,
                    phase: RankWeightPhase::Prepare,
                    error,
                });
                prepare_route = coordinator.report_rank_failure(message.rank, error);
            }
        }
    }
    if let Some(cause) = prepare_failure {
        return Err(abort_weight_load(
            rank_senders,
            cause,
            abort_command,
            owner_allocation_generations,
            phase_timeout,
        ));
    }
    let prepared_receipts: [PreparedRankReceipt; RANK_SET_SIZE] = match prepared_receipts.try_into()
    {
        Ok(receipts) => receipts,
        Err(_) => {
            return Err(abort_weight_load(
                rank_senders,
                WeightLoadFailureCause::RankSet {
                    phase: RankWeightPhase::Prepare,
                },
                abort_command,
                owner_allocation_generations,
                phase_timeout,
            ));
        }
    };
    let prepared = match PreparedRankSet::new(&plan, prepared_receipts) {
        Ok(prepared) => prepared,
        Err(error) => {
            return Err(abort_weight_load(
                rank_senders,
                WeightLoadFailureCause::Coordinator(error),
                abort_command,
                owner_allocation_generations,
                phase_timeout,
            ));
        }
    };
    if prepare_route != RankSetLoadAction::Adopt(prepared.adoption_command()) {
        let cause = WeightLoadFailureCause::Coordinator(
            coordinator
                .terminal_error()
                .unwrap_or(LoadPlanError::Transition),
        );
        return Err(abort_weight_load(
            rank_senders,
            cause,
            abort_command,
            owner_allocation_generations,
            phase_timeout,
        ));
    }

    let adoption_messages =
        match acknowledge_rank_weights(rank_senders, Arc::new(prepared), phase_timeout) {
            Ok(messages) => messages,
            Err(cause) => {
                return Err(abort_weight_load(
                    rank_senders,
                    cause,
                    abort_command,
                    owner_allocation_generations,
                    phase_timeout,
                ));
            }
        };
    let mut adoption_acknowledgements = Vec::with_capacity(RANK_SET_SIZE);
    let mut adoption_failure = None;
    let mut adoption_route = RankSetLoadAction::Wait;
    for message in adoption_messages {
        match message.result {
            Ok(acknowledgement) => {
                adoption_route = coordinator.report_adoption_acknowledgement(acknowledgement);
                adoption_acknowledgements.push(acknowledgement);
            }
            Err(error) => {
                adoption_failure.get_or_insert(WeightLoadFailureCause::Rank {
                    rank: message.rank,
                    phase: RankWeightPhase::Acknowledge,
                    error,
                });
                adoption_route = coordinator.report_rank_failure(message.rank, error);
            }
        }
    }
    if let Some(cause) = adoption_failure {
        return Err(abort_weight_load(
            rank_senders,
            cause,
            abort_command,
            owner_allocation_generations,
            phase_timeout,
        ));
    }
    let adoption_acknowledgements: [AdoptionAcknowledgement; RANK_SET_SIZE] =
        match adoption_acknowledgements.try_into() {
            Ok(acknowledgements) => acknowledgements,
            Err(_) => {
                return Err(abort_weight_load(
                    rank_senders,
                    WeightLoadFailureCause::RankSet {
                        phase: RankWeightPhase::Acknowledge,
                    },
                    abort_command,
                    owner_allocation_generations,
                    phase_timeout,
                ));
            }
        };
    let RankSetLoadAction::Complete(adopted_receipt) = adoption_route else {
        let cause = WeightLoadFailureCause::Coordinator(
            coordinator
                .terminal_error()
                .unwrap_or(LoadPlanError::Transition),
        );
        return Err(abort_weight_load(
            rank_senders,
            cause,
            abort_command,
            owner_allocation_generations,
            phase_timeout,
        ));
    };

    let finalize_messages = match finalize_rank_weights(
        rank_senders,
        adopted_receipt,
        owner_allocation_generations,
        phase_timeout,
    ) {
        Ok(messages) => messages,
        Err(cause) => {
            let _ = coordinator.report_rank_failure(0, LoadPlanError::Transition);
            return Err(abort_weight_load(
                rank_senders,
                cause,
                abort_command,
                owner_allocation_generations,
                phase_timeout,
            ));
        }
    };
    let mut finalize_acknowledgements = Vec::with_capacity(RANK_SET_SIZE);
    let mut finalize_failure = None;
    for message in finalize_messages {
        match message.result {
            Ok(acknowledgement) => {
                if acknowledgement.rank() != message.rank
                    || acknowledgement.plan_sha256() != plan.plan_sha256()
                    || acknowledgement.owner_allocation_generation()
                        != owner_allocation_generations[usize::from(message.rank)]
                    || acknowledgement.adopted_rank_set_sha256()
                        != adopted_receipt.adopted_rank_set_sha256()
                {
                    finalize_failure.get_or_insert(WeightLoadFailureCause::RankSet {
                        phase: RankWeightPhase::Finalize,
                    });
                } else {
                    finalize_acknowledgements.push(acknowledgement);
                }
            }
            Err(error) => {
                finalize_failure.get_or_insert(WeightLoadFailureCause::Rank {
                    rank: message.rank,
                    phase: RankWeightPhase::Finalize,
                    error,
                });
                let _ = coordinator.report_rank_failure(message.rank, error);
            }
        }
    }
    if let Some(cause) = finalize_failure {
        return Err(abort_weight_load(
            rank_senders,
            cause,
            abort_command,
            owner_allocation_generations,
            phase_timeout,
        ));
    }
    let finalize_acknowledgements: [RankWeightFinalizeAck; RANK_SET_SIZE] =
        match finalize_acknowledgements.try_into() {
            Ok(acknowledgements) => acknowledgements,
            Err(_) => {
                return Err(abort_weight_load(
                    rank_senders,
                    WeightLoadFailureCause::RankSet {
                        phase: RankWeightPhase::Finalize,
                    },
                    abort_command,
                    owner_allocation_generations,
                    phase_timeout,
                ));
            }
        };
    Ok(WeightLoadOutcome {
        plan_sha256: plan.plan_sha256(),
        load_attempt_generation,
        prepared_receipts,
        adoption_acknowledgements,
        adopted_receipt,
        finalize_acknowledgements,
    })
}

fn prepare_rank_weights(
    rank_senders: &[SyncSender<RankCommandEnvelope>],
    plan: Arc<RankSetLoadPlan>,
    load_attempt_generation: u64,
    owner_allocation_generations: [u64; RANK_SET_SIZE],
    phase_timeout: Duration,
) -> Result<Vec<RankPreparedResult>, WeightLoadFailureCause> {
    let (response, receiver) = mpsc::sync_channel(RANK_SET_SIZE);
    let mut send_failed = false;
    for (rank, sender) in rank_senders.iter().enumerate() {
        send_failed |= sender
            .send(RankCommandEnvelope::PrepareWeights {
                plan: Arc::clone(&plan),
                load_attempt_generation,
                owner_allocation_generation: owner_allocation_generations[rank],
                response: response.clone(),
            })
            .is_err();
    }
    drop(response);
    if send_failed {
        return Err(WeightLoadFailureCause::Closed);
    }
    collect_rank_messages(
        receiver,
        RankWeightPhase::Prepare,
        phase_timeout,
        |message| message.rank,
    )
}

fn acknowledge_rank_weights(
    rank_senders: &[SyncSender<RankCommandEnvelope>],
    prepared: Arc<PreparedRankSet>,
    phase_timeout: Duration,
) -> Result<Vec<RankAdoptionResult>, WeightLoadFailureCause> {
    let (response, receiver) = mpsc::sync_channel(RANK_SET_SIZE);
    let mut send_failed = false;
    for sender in rank_senders {
        send_failed |= sender
            .send(RankCommandEnvelope::AcknowledgeWeights {
                prepared: Arc::clone(&prepared),
                response: response.clone(),
            })
            .is_err();
    }
    drop(response);
    if send_failed {
        return Err(WeightLoadFailureCause::Closed);
    }
    collect_rank_messages(
        receiver,
        RankWeightPhase::Acknowledge,
        phase_timeout,
        |message| message.rank,
    )
}

fn finalize_rank_weights(
    rank_senders: &[SyncSender<RankCommandEnvelope>],
    adopted: AdoptedRankSetReceipt,
    owner_allocation_generations: [u64; RANK_SET_SIZE],
    phase_timeout: Duration,
) -> Result<Vec<RankFinalizeResult>, WeightLoadFailureCause> {
    let (response, receiver) = mpsc::sync_channel(RANK_SET_SIZE);
    let mut send_failed = false;
    for (rank, sender) in rank_senders.iter().enumerate() {
        send_failed |= sender
            .send(RankCommandEnvelope::FinalizeWeights {
                adopted,
                owner_allocation_generation: owner_allocation_generations[rank],
                response: response.clone(),
            })
            .is_err();
    }
    drop(response);
    if send_failed {
        return Err(WeightLoadFailureCause::Closed);
    }
    collect_rank_messages(
        receiver,
        RankWeightPhase::Finalize,
        phase_timeout,
        |message| message.rank,
    )
}

fn abort_weight_load(
    rank_senders: &[SyncSender<RankCommandEnvelope>],
    cause: WeightLoadFailureCause,
    command: RankSetAbortCommand,
    owner_allocation_generations: [u64; RANK_SET_SIZE],
    phase_timeout: Duration,
) -> WeightLoadFailure {
    let (cleanup_acknowledgements, cleanup_failure) = cleanup_rank_weights(
        rank_senders,
        command,
        owner_allocation_generations,
        phase_timeout,
    );
    WeightLoadFailure {
        cause,
        cleanup_failure,
        cleanup_acknowledgements: Box::new(cleanup_acknowledgements),
    }
}

fn cleanup_rank_weights(
    rank_senders: &[SyncSender<RankCommandEnvelope>],
    command: RankSetAbortCommand,
    owner_allocation_generations: [u64; RANK_SET_SIZE],
    phase_timeout: Duration,
) -> (
    [Option<RankWeightCleanupAck>; RANK_SET_SIZE],
    Option<WeightLoadFailureCause>,
) {
    let (response, receiver) = mpsc::sync_channel(RANK_SET_SIZE);
    let mut send_failed = false;
    for (rank, sender) in rank_senders.iter().enumerate() {
        send_failed |= sender
            .send(RankCommandEnvelope::AbortWeights {
                command,
                owner_allocation_generation: owner_allocation_generations[rank],
                response: response.clone(),
            })
            .is_err();
    }
    drop(response);
    let mut cleanup_failure = send_failed.then_some(WeightLoadFailureCause::Closed);
    let Some(deadline) = Instant::now().checked_add(phase_timeout) else {
        return ([None; RANK_SET_SIZE], Some(WeightLoadFailureCause::Config));
    };
    let mut messages = Vec::with_capacity(RANK_SET_SIZE);
    for _ in 0..RANK_SET_SIZE {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            cleanup_failure.get_or_insert(WeightLoadFailureCause::Timeout {
                phase: RankWeightPhase::Abort,
            });
            break;
        };
        match receiver.recv_timeout(remaining) {
            Ok(message) => messages.push(message),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                cleanup_failure.get_or_insert(WeightLoadFailureCause::Timeout {
                    phase: RankWeightPhase::Abort,
                });
                break;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                cleanup_failure.get_or_insert(WeightLoadFailureCause::Closed);
                break;
            }
        }
    }
    messages.sort_by_key(|message| message.rank);
    let mut acknowledgements = [None; RANK_SET_SIZE];
    for message in messages {
        let rank = usize::from(message.rank);
        if rank >= RANK_SET_SIZE || acknowledgements[rank].is_some() {
            cleanup_failure.get_or_insert(WeightLoadFailureCause::RankSet {
                phase: RankWeightPhase::Abort,
            });
            continue;
        }
        let acknowledgement = match message.result {
            Ok(acknowledgement) => acknowledgement,
            Err(error) => {
                cleanup_failure.get_or_insert(WeightLoadFailureCause::Rank {
                    rank: message.rank,
                    phase: RankWeightPhase::Abort,
                    error,
                });
                continue;
            }
        };
        if acknowledgement.rank() != message.rank
            || acknowledgement.plan_sha256() != command.plan_sha256()
            || acknowledgement.load_attempt_generation() != command.load_attempt_generation()
            || acknowledgement.owner_allocation_generation() != owner_allocation_generations[rank]
        {
            cleanup_failure.get_or_insert(WeightLoadFailureCause::RankSet {
                phase: RankWeightPhase::Abort,
            });
            continue;
        }
        acknowledgements[rank] = Some(acknowledgement);
    }
    if acknowledgements.iter().any(Option::is_none) && cleanup_failure.is_none() {
        cleanup_failure = Some(WeightLoadFailureCause::RankSet {
            phase: RankWeightPhase::Abort,
        });
    }
    (acknowledgements, cleanup_failure)
}

fn collect_rank_messages<T>(
    receiver: Receiver<T>,
    phase: RankWeightPhase,
    phase_timeout: Duration,
    rank: impl Fn(&T) -> u8,
) -> Result<Vec<T>, WeightLoadFailureCause> {
    let deadline = Instant::now()
        .checked_add(phase_timeout)
        .ok_or(WeightLoadFailureCause::Config)?;
    let mut messages = Vec::with_capacity(RANK_SET_SIZE);
    for _ in 0..RANK_SET_SIZE {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(WeightLoadFailureCause::Timeout { phase })?;
        let message = receiver
            .recv_timeout(remaining)
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => WeightLoadFailureCause::Timeout { phase },
                mpsc::RecvTimeoutError::Disconnected => WeightLoadFailureCause::Closed,
            })?;
        messages.push(message);
    }
    messages.sort_by_key(&rank);
    for (expected_rank, message) in messages.iter().enumerate() {
        if usize::from(rank(message)) != expected_rank {
            return Err(WeightLoadFailureCause::RankSet { phase });
        }
    }
    Ok(messages)
}

fn dispatch_one(
    rank_senders: &[SyncSender<RankCommandEnvelope>],
    plan: &StepPlan,
    schedule: &CollectiveSchedule,
    binding: Option<StepBinding>,
) -> Result<StepOutcome, WorkerError> {
    let (ack_sender, ack_receiver) = mpsc::sync_channel(usize::from(TP_RANKS));
    for sender in rank_senders {
        sender
            .send(RankCommandEnvelope::Execute(RankCommand {
                plan: *plan,
                schedule: schedule.clone(),
                binding: binding.clone(),
                response: ack_sender.clone(),
            }))
            .map_err(|_| WorkerError::Closed)?;
    }
    drop(ack_sender);
    let mut acknowledgements = Vec::with_capacity(usize::from(TP_RANKS));
    for _ in 0..TP_RANKS {
        acknowledgements.push(ack_receiver.recv().map_err(|_| WorkerError::Closed)??);
    }
    acknowledgements.sort_by_key(|result| result.ack.rank);
    if acknowledgements
        .iter()
        .enumerate()
        .any(|(rank, result)| usize::from(result.ack.rank) != rank)
    {
        return Err(WorkerError::RankSet);
    }
    if let Some(binding) = &binding {
        for result in &acknowledgements {
            if result.ack.input_hash != binding.input.canonical_hash()
                || result.ack.page_table_global_digest != binding.delta.global_digest()
                || result.ack.page_table_local_digest
                    != binding.delta.rank_local_digest(result.ack.rank)?
            {
                return Err(WorkerError::Consensus);
            }
        }
    }
    let first = &acknowledgements[0];
    if acknowledgements.iter().any(|result| {
        result.ack.step_id != first.ack.step_id
            || result.ack.plan_hash != first.ack.plan_hash
            || result.ack.schedule_hash != first.ack.schedule_hash
            || result.ack.input_hash != first.ack.input_hash
            || result.ack.page_table_global_digest != first.ack.page_table_global_digest
            || result.ack.output_digest != first.ack.output_digest
            || result.output != first.output
    }) {
        return Err(WorkerError::Consensus);
    }
    let step_id = first.ack.step_id;
    let plan_hash = first.ack.plan_hash;
    let output_digest = first.ack.output_digest;
    let output = first.output.clone();
    let rank_acks: [RankStepAck; 4] = acknowledgements
        .into_iter()
        .map(|result| result.ack)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| WorkerError::RankSet)?;
    Ok(StepOutcome {
        step_id,
        plan_hash,
        output_digest,
        output,
        rank_acks,
    })
}

fn rank_loop(
    rank: u8,
    receiver: Receiver<RankCommandEnvelope>,
    mut executor: Box<dyn RankExecutor>,
) {
    let mut last_step_id = 0_u64;
    let mut page_table = None;
    while let Ok(command) = receiver.recv() {
        match command {
            RankCommandEnvelope::Initialize {
                table,
                generation,
                response,
            } => {
                let result = if page_table.is_some() {
                    Err(WorkerError::PageTableInitialized)
                } else {
                    PageTableMirror::from_table(&table, generation)
                        .map(|mirror| {
                            page_table = Some(mirror);
                            rank
                        })
                        .map_err(WorkerError::PageDelta)
                };
                let failed = result.is_err();
                let _ = response.send(result);
                if failed {
                    break;
                }
            }
            RankCommandEnvelope::ApplyDelta { delta, response } => {
                let result = apply_page_delta_on_rank(rank, page_table.as_mut(), &delta);
                let failed = result.is_err();
                let _ = response.send(result);
                if failed {
                    break;
                }
            }
            RankCommandEnvelope::PrepareWeights {
                plan,
                load_attempt_generation,
                owner_allocation_generation,
                response,
            } => {
                let result = executor.prepare_weights(
                    rank,
                    &plan,
                    load_attempt_generation,
                    owner_allocation_generation,
                );
                let _ = response.send(RankPreparedResult { rank, result });
            }
            RankCommandEnvelope::AcknowledgeWeights { prepared, response } => {
                let result = executor.acknowledge_weight_adoption(rank, &prepared);
                let _ = response.send(RankAdoptionResult { rank, result });
            }
            RankCommandEnvelope::FinalizeWeights {
                adopted,
                owner_allocation_generation,
                response,
            } => {
                let result = executor.finalize_weights(rank, adopted).and_then(|()| {
                    RankWeightFinalizeAck::new(
                        rank,
                        adopted.plan_sha256(),
                        owner_allocation_generation,
                        adopted,
                    )
                });
                let _ = response.send(RankFinalizeResult { rank, result });
            }
            RankCommandEnvelope::AbortWeights {
                command,
                owner_allocation_generation,
                response,
            } => {
                let result = executor
                    .abort_weight_load(rank, command, owner_allocation_generation)
                    .and_then(|()| {
                        RankWeightCleanupAck::new(rank, command, owner_allocation_generation)
                    });
                let failed = result.is_err();
                let _ = response.send(RankCleanupResult { rank, result });
                if failed {
                    break;
                }
            }
            RankCommandEnvelope::Execute(command) => {
                let result = if command.plan.step_id <= last_step_id {
                    Err(WorkerError::StepOrder)
                } else {
                    last_step_id = command.plan.step_id;
                    execute_rank(
                        rank,
                        &command.plan,
                        &command.schedule,
                        command.binding.as_ref(),
                        page_table.as_mut(),
                        executor.as_mut(),
                    )
                };
                let failed = result.is_err();
                let _ = command.response.send(result);
                if failed {
                    break;
                }
            }
        }
    }
}

fn execute_rank(
    rank: u8,
    plan: &StepPlan,
    schedule: &CollectiveSchedule,
    binding: Option<&StepBinding>,
    page_table: Option<&mut PageTableMirror>,
    executor: &mut dyn RankExecutor,
) -> Result<RankResult, WorkerError> {
    plan.verify(schedule)?;
    let (input_hash, page_table_global_digest, page_table_local_digest) =
        if let Some(binding) = binding {
            binding.input.verify(plan, schedule, &binding.delta)?;
            let page_table = page_table.ok_or(WorkerError::PageTableUninitialized)?;
            page_table.apply(&binding.delta)?;
            (
                binding.input.canonical_hash(),
                binding.delta.global_digest(),
                binding.delta.rank_local_digest(rank)?,
            )
        } else {
            ([0; 32], [0; 32], [0; 32])
        };
    let output = if let Some(binding) = binding {
        executor.execute_bound(rank, plan, schedule, &binding.input)
    } else {
        executor.execute(rank, plan, schedule)
    }
    .map_err(|error| WorkerError::RankExecution { rank, error })?;
    output
        .validate(plan)
        .map_err(|error| WorkerError::RankOutput { rank, error })?;
    let output_digest = output.canonical_digest();
    Ok(RankResult {
        ack: RankStepAck {
            rank,
            step_id: plan.step_id,
            plan_hash: plan.plan_hash,
            schedule_hash: schedule.hash(),
            input_hash,
            page_table_global_digest,
            page_table_local_digest,
            output_digest,
        },
        output,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RankResult {
    ack: RankStepAck,
    output: StepOutput,
}

fn cpu_output(
    plan: &StepPlan,
    schedule: &CollectiveSchedule,
) -> Result<StepOutput, RankExecutionError> {
    if matches!(plan.mode, StepMode::Prefill | StepMode::CacheOnly) {
        return Ok(StepOutput::empty());
    }
    let sequences = (0..plan.active_sequences)
        .map(|row| {
            let mut hasher = Sha256::new();
            hasher.update(CPU_TOKEN_DOMAIN);
            hasher.update(plan.plan_hash);
            hasher.update(schedule.hash());
            hasher.update(row.to_le_bytes());
            let digest: [u8; 32] = hasher.finalize().into();
            let token_id = u32::from_le_bytes(digest[..4].try_into().expect("bounded"))
                % GLM_52_OUTPUT_VOCABULARY;
            CommittedTokens::target(token_id)
        })
        .collect::<Result<Vec<_>, _>>()?;
    StepOutput::new(&sequences).map_err(Into::into)
}

fn cpu_bound_output(
    plan: &StepPlan,
    schedule: &CollectiveSchedule,
    input: &StepInput,
) -> Result<StepOutput, RankExecutionError> {
    if matches!(plan.mode, StepMode::Prefill | StepMode::CacheOnly) {
        return Ok(StepOutput::empty());
    }
    let sequences = input
        .rows()
        .iter()
        .enumerate()
        .map(|(row, input_row)| {
            let mut hasher = Sha256::new();
            hasher.update(CPU_TOKEN_DOMAIN);
            hasher.update(plan.plan_hash);
            hasher.update(schedule.hash());
            hasher.update(input.canonical_hash());
            hasher.update(input_row.request_id.to_le_bytes());
            hasher.update(
                u16::try_from(row)
                    .expect("validated StepInput row count fits u16")
                    .to_le_bytes(),
            );
            let digest: [u8; 32] = hasher.finalize().into();
            let token_id = u32::from_le_bytes(digest[..4].try_into().expect("bounded"))
                % GLM_52_OUTPUT_VOCABULARY;
            CommittedTokens::target(token_id)
        })
        .collect::<Result<Vec<_>, _>>()?;
    StepOutput::new(&sequences).map_err(Into::into)
}

#[derive(Debug)]
pub enum WorkerError {
    Config,
    Saturated,
    Closed,
    Timeout,
    StepOrder,
    RankSet,
    Consensus,
    RankExecution {
        rank: u8,
        error: RankExecutionError,
    },
    #[cfg(feature = "cuda-ffi")]
    RankCheckpointLoad {
        rank: u8,
        error: RankCheckpointLoadError,
    },
    RankOutput {
        rank: u8,
        error: OutputError,
    },
    StepInput(StepInputError),
    PageDelta(PageTableDeltaError),
    PageTableUninitialized,
    PageTableInitialized,
    Plan(PlanError),
    Thread(std::io::Error),
    RankStartup,
    WorkerPanic,
}

impl fmt::Display for WorkerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for WorkerError {}

impl From<PlanError> for WorkerError {
    fn from(value: PlanError) -> Self {
        Self::Plan(value)
    }
}

impl From<StepInputError> for WorkerError {
    fn from(value: StepInputError) -> Self {
        Self::StepInput(value)
    }
}

impl From<PageTableDeltaError> for WorkerError {
    fn from(value: PageTableDeltaError) -> Self {
        Self::PageDelta(value)
    }
}

impl From<OutputError> for RankExecutionError {
    fn from(_: OutputError) -> Self {
        Self::Invariant
    }
}

#[cfg(test)]
mod tests {
    use std::{
        rc::Rc,
        sync::{
            Arc, Barrier, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        thread::ThreadId,
        time::Instant,
    };

    use glm_cache::{PageTableConfig, PageTableDelta, SequencePageTable};

    use crate::{
        AttentionTransport, CollectiveKind, CollectiveOp, LoadProfile, LoadVerificationMode,
        READER_CHUNK_BYTES, RankLoadEntry, RankSetLoadPlanHeader, SequenceStepInput, StepMode,
        StepPlanRequest, StepSampling, TP_RANK_MASK, TensorArenaEntry, arena_layout_sha256,
    };

    use super::*;

    fn step(step_id: u64) -> (StepPlan, CollectiveSchedule) {
        step_at_generation(step_id, 1)
    }

    fn step_at_generation(
        step_id: u64,
        sequence_table_generation: u64,
    ) -> (StepPlan, CollectiveSchedule) {
        let schedule = CollectiveSchedule::new(vec![
            CollectiveOp {
                ordinal: 0,
                kind: CollectiveKind::TpReduce,
                route_id: 1,
                payload_bytes: 16,
                participant_mask: TP_RANK_MASK,
            },
            CollectiveOp {
                ordinal: 1,
                kind: CollectiveKind::LogitsArgmax,
                route_id: 1,
                payload_bytes: 8,
                participant_mask: TP_RANK_MASK,
            },
        ])
        .unwrap();
        let plan = StepPlan::build(
            StepPlanRequest {
                epoch: 1,
                step_id,
                mode: StepMode::Decode,
                active_sequences: 1,
                sequence_bucket: 1,
                scheduled_prompt_tokens: 0,
                query_rows: 1,
                verifier_row_bucket: 1,
                mtp_depth: 0,
                graph_id: 1,
                tp_route_id: 1,
                dcp_route_id: 1,
                attention_transport: AttentionTransport::DecodeQueryLse,
                sampling_route_id: 1,
                sequence_table_generation,
            },
            &schedule,
        )
        .unwrap();
        (plan, schedule)
    }

    fn weight_load_plan() -> RankSetLoadPlan {
        let tensor = TensorArenaEntry {
            tensor_id: 0,
            role_id: 1,
            codec_id: 1,
            descriptor_flags: 0,
            metadata_destination_offset: 0,
            metadata_bytes: 256,
            primary_destination_offset: 0,
            primary_bytes: 1024,
            auxiliary_destination_offset: 0,
            auxiliary_bytes: 0,
            required_device_alignment: 256,
        };
        let tensors = std::array::from_fn(|_| vec![tensor]);
        let ranks = std::array::from_fn(|rank| {
            let rank = u8::try_from(rank).unwrap();
            RankLoadEntry {
                rank,
                device_identity_sha256: [rank + 1; 32],
                file_uuid: [rank + 1; 16],
                manifest_sha256: [11; 32],
                descriptor_sha256: [12; 32],
                payload_sha256: [13; 32],
                tensor_count: 1,
                file_payload_bytes: 1024,
                device_weight_arena_bytes: 1024,
                device_metadata_arena_bytes: 256,
                arena_layout_sha256: arena_layout_sha256(
                    rank,
                    1024,
                    256,
                    &tensors[usize::from(rank)],
                ),
                tensor_contract_sha256: [14 + rank; 32],
            }
        });
        RankSetLoadPlan::new(
            RankSetLoadPlanHeader {
                verification_mode: LoadVerificationMode::FullSha256,
                profile: LoadProfile::Nvfp4Laboratory,
                tensor_count: 1,
                conversion_uuid: [1; 16],
                weight_policy_sha256: [2; 32],
                kernel_abi_sha256: [3; 32],
                memory_plan_sha256: [4; 32],
                codec_capability_sha256: [5; 32],
                model_config_sha256: [6; 32],
                tokenizer_bundle_sha256: [7; 32],
                chat_template_sha256: [8; 32],
                operation_manifest_sha256: [9; 32],
                tensor_catalog_sha256: [10; 32],
                profile_budget_sha256: [11; 32],
                staging_slot_bytes: READER_CHUNK_BYTES,
                staging_slots_per_rank: 2,
            },
            ranks,
            tensors,
        )
        .unwrap()
    }

    type TransactionalWeightPoolHarness = (
        Tp4WorkerPool,
        Arc<Mutex<[MockWeightState; RANK_SET_SIZE]>>,
        Arc<[AtomicUsize; RANK_SET_SIZE]>,
    );

    fn transactional_weight_pool(fault: MockWeightFaultConfig) -> TransactionalWeightPoolHarness {
        let states = Arc::new(Mutex::new([MockWeightState::Empty; RANK_SET_SIZE]));
        let cleanup_counts = Arc::new(std::array::from_fn(|_| AtomicUsize::new(0)));
        let factories = std::array::from_fn(|expected_rank| {
            let states = Arc::clone(&states);
            let cleanup_counts = Arc::clone(&cleanup_counts);
            Box::new(move |rank| {
                if usize::from(rank) != expected_rank {
                    return Err(WorkerError::RankStartup);
                }
                Ok(Box::new(TransactionalWeightExecutor {
                    rank,
                    fault,
                    states,
                    cleanup_counts,
                    receipt: None,
                    plan_sha256: [0; 32],
                    load_attempt_generation: 0,
                    owner_allocation_generation: 0,
                }) as Box<dyn RankExecutor>)
            }) as Box<dyn RankExecutorFactory>
        });
        (
            Tp4WorkerPool::spawn_factories(1, factories).unwrap(),
            states,
            cleanup_counts,
        )
    }

    fn active_table() -> SequencePageTable {
        let mut table = SequencePageTable::new(PageTableConfig {
            target_pages_per_rank: 8,
            draft_pages_per_rank: 8,
        })
        .unwrap();
        table.admit_with_prefix(7, false, &[]).unwrap();
        table.append_committed(7, 64).unwrap();
        table
    }

    struct StatefulRankExecutor {
        expected_rank: u8,
        calls: u64,
        thread: Option<ThreadId>,
        fail_code: Option<i32>,
    }

    struct InvalidOutputExecutor;

    struct FirstStepBlockingRankExecutor {
        entered: Arc<Barrier>,
        release: Arc<Barrier>,
        calls: u64,
    }

    struct DropCountingRankExecutor {
        drops: Arc<AtomicUsize>,
    }

    struct ThreadLocalRankExecutor {
        rank: u8,
        owner: ThreadId,
        calls: u64,
        _not_send: Rc<()>,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum MockWeightState {
        Empty,
        Preparing,
        Prepared,
        Acknowledged,
        Resident,
        Aborted,
    }

    #[derive(Clone, Copy)]
    struct MockWeightFaultConfig {
        phase: Option<(u8, RankWeightPhase)>,
        cleanup_rank: Option<u8>,
        delayed_prepare: Option<(u8, Duration)>,
    }

    struct TransactionalWeightExecutor {
        rank: u8,
        fault: MockWeightFaultConfig,
        states: Arc<Mutex<[MockWeightState; RANK_SET_SIZE]>>,
        cleanup_counts: Arc<[AtomicUsize; RANK_SET_SIZE]>,
        receipt: Option<PreparedRankReceipt>,
        plan_sha256: [u8; 32],
        load_attempt_generation: u64,
        owner_allocation_generation: u64,
    }

    impl RankExecutor for InvalidOutputExecutor {
        fn execute(
            &mut self,
            _rank: u8,
            _plan: &StepPlan,
            _schedule: &CollectiveSchedule,
        ) -> Result<StepOutput, RankExecutionError> {
            Ok(StepOutput::empty())
        }

        fn execute_bound(
            &mut self,
            rank: u8,
            plan: &StepPlan,
            schedule: &CollectiveSchedule,
            _input: &StepInput,
        ) -> Result<StepOutput, RankExecutionError> {
            self.execute(rank, plan, schedule)
        }
    }

    impl RankExecutor for FirstStepBlockingRankExecutor {
        fn execute(
            &mut self,
            _rank: u8,
            plan: &StepPlan,
            schedule: &CollectiveSchedule,
        ) -> Result<StepOutput, RankExecutionError> {
            self.calls += 1;
            if self.calls == 1 {
                self.entered.wait();
                self.release.wait();
            }
            cpu_output(plan, schedule)
        }

        fn execute_bound(
            &mut self,
            rank: u8,
            plan: &StepPlan,
            schedule: &CollectiveSchedule,
            _input: &StepInput,
        ) -> Result<StepOutput, RankExecutionError> {
            self.execute(rank, plan, schedule)
        }
    }

    impl RankExecutor for DropCountingRankExecutor {
        fn execute(
            &mut self,
            _rank: u8,
            plan: &StepPlan,
            schedule: &CollectiveSchedule,
        ) -> Result<StepOutput, RankExecutionError> {
            cpu_output(plan, schedule)
        }

        fn execute_bound(
            &mut self,
            rank: u8,
            plan: &StepPlan,
            schedule: &CollectiveSchedule,
            _input: &StepInput,
        ) -> Result<StepOutput, RankExecutionError> {
            self.execute(rank, plan, schedule)
        }
    }

    impl Drop for DropCountingRankExecutor {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::AcqRel);
        }
    }

    impl RankExecutor for StatefulRankExecutor {
        fn execute(
            &mut self,
            rank: u8,
            plan: &StepPlan,
            schedule: &CollectiveSchedule,
        ) -> Result<StepOutput, RankExecutionError> {
            let thread = std::thread::current().id();
            if rank != self.expected_rank
                || plan.step_id != self.calls + 1
                || self.thread.is_some_and(|expected| expected != thread)
            {
                return Err(RankExecutionError::Invariant);
            }
            self.thread = Some(thread);
            self.calls += 1;
            if let Some(code) = self.fail_code {
                return Err(RankExecutionError::Backend(code));
            }
            cpu_output(plan, schedule)
        }

        fn execute_bound(
            &mut self,
            rank: u8,
            plan: &StepPlan,
            schedule: &CollectiveSchedule,
            _input: &StepInput,
        ) -> Result<StepOutput, RankExecutionError> {
            self.execute(rank, plan, schedule)
        }
    }

    impl RankExecutor for ThreadLocalRankExecutor {
        fn execute(
            &mut self,
            rank: u8,
            plan: &StepPlan,
            schedule: &CollectiveSchedule,
        ) -> Result<StepOutput, RankExecutionError> {
            if rank != self.rank || thread::current().id() != self.owner {
                return Err(RankExecutionError::Invariant);
            }
            self.calls += 1;
            cpu_output(plan, schedule)
        }

        fn execute_bound(
            &mut self,
            rank: u8,
            plan: &StepPlan,
            schedule: &CollectiveSchedule,
            input: &StepInput,
        ) -> Result<StepOutput, RankExecutionError> {
            if rank != self.rank || thread::current().id() != self.owner {
                return Err(RankExecutionError::Invariant);
            }
            self.calls += 1;
            cpu_bound_output(plan, schedule, input)
        }
    }

    impl TransactionalWeightExecutor {
        fn set_state(&self, state: MockWeightState) {
            self.states.lock().unwrap()[usize::from(self.rank)] = state;
        }

        fn state(&self) -> MockWeightState {
            self.states.lock().unwrap()[usize::from(self.rank)]
        }

        fn fails(&self, phase: RankWeightPhase) -> bool {
            self.fault.phase == Some((self.rank, phase))
        }
    }

    impl RankExecutor for TransactionalWeightExecutor {
        fn execute(
            &mut self,
            rank: u8,
            plan: &StepPlan,
            schedule: &CollectiveSchedule,
        ) -> Result<StepOutput, RankExecutionError> {
            if rank != self.rank || self.state() != MockWeightState::Resident {
                return Err(RankExecutionError::Invariant);
            }
            cpu_output(plan, schedule)
        }

        fn execute_bound(
            &mut self,
            rank: u8,
            plan: &StepPlan,
            schedule: &CollectiveSchedule,
            input: &StepInput,
        ) -> Result<StepOutput, RankExecutionError> {
            if rank != self.rank || self.state() != MockWeightState::Resident {
                return Err(RankExecutionError::Invariant);
            }
            cpu_bound_output(plan, schedule, input)
        }

        fn prepare_weights(
            &mut self,
            rank: u8,
            plan: &RankSetLoadPlan,
            load_attempt_generation: u64,
            owner_allocation_generation: u64,
        ) -> Result<PreparedRankReceipt, LoadPlanError> {
            if rank != self.rank
                || self.state() != MockWeightState::Empty
                || plan.rank(rank).is_none()
                || load_attempt_generation == 0
                || owner_allocation_generation == 0
            {
                return Err(LoadPlanError::Transition);
            }
            self.plan_sha256 = plan.plan_sha256();
            self.load_attempt_generation = load_attempt_generation;
            self.owner_allocation_generation = owner_allocation_generation;
            self.set_state(MockWeightState::Preparing);
            if let Some((delayed_rank, delay)) = self.fault.delayed_prepare
                && delayed_rank == rank
            {
                thread::sleep(delay);
            }
            if self.fails(RankWeightPhase::Prepare) {
                return Err(LoadPlanError::Writer);
            }
            let receipt = PreparedRankReceipt::test_only(
                plan,
                rank,
                owner_allocation_generation,
                [0x80 + rank; 32],
            )?;
            self.receipt = Some(receipt);
            self.set_state(MockWeightState::Prepared);
            Ok(receipt)
        }

        fn acknowledge_weight_adoption(
            &mut self,
            rank: u8,
            prepared: &PreparedRankSet,
        ) -> Result<AdoptionAcknowledgement, LoadPlanError> {
            if rank != self.rank
                || self.state() != MockWeightState::Prepared
                || prepared.plan_sha256 != self.plan_sha256
            {
                return Err(LoadPlanError::Transition);
            }
            self.set_state(MockWeightState::Acknowledged);
            if self.fails(RankWeightPhase::Acknowledge) {
                return Err(LoadPlanError::Adoption);
            }
            AdoptionAcknowledgement::new(
                prepared.adoption_command(),
                self.receipt.ok_or(LoadPlanError::Receipt)?,
            )
        }

        fn finalize_weights(
            &mut self,
            rank: u8,
            adopted: AdoptedRankSetReceipt,
        ) -> Result<(), LoadPlanError> {
            if rank != self.rank
                || self.state() != MockWeightState::Acknowledged
                || adopted.plan_sha256() != self.plan_sha256
            {
                return Err(LoadPlanError::Transition);
            }
            // Model the hardest partial failure: the rank has made its arena
            // resident before reporting a failed final acknowledgement.
            self.set_state(MockWeightState::Resident);
            if self.fails(RankWeightPhase::Finalize) {
                return Err(LoadPlanError::Adoption);
            }
            Ok(())
        }

        fn abort_weight_load(
            &mut self,
            rank: u8,
            command: RankSetAbortCommand,
            owner_allocation_generation: u64,
        ) -> Result<(), LoadPlanError> {
            if rank != self.rank
                || command.plan_sha256() != self.plan_sha256
                || command.load_attempt_generation() != self.load_attempt_generation
                || owner_allocation_generation != self.owner_allocation_generation
            {
                return Err(LoadPlanError::Transition);
            }
            if self.fault.cleanup_rank == Some(rank) || self.fails(RankWeightPhase::Abort) {
                return Err(LoadPlanError::Writer);
            }
            if self.state() != MockWeightState::Aborted {
                self.set_state(MockWeightState::Aborted);
                self.cleanup_counts[usize::from(rank)].fetch_add(1, Ordering::SeqCst);
            }
            Ok(())
        }
    }

    #[test]
    fn four_workers_acknowledge_one_identical_plan() {
        let pool = Tp4WorkerPool::spawn_cpu(1, None).unwrap();
        let (plan, schedule) = step(1);
        let handle = pool.try_submit(plan, schedule).unwrap();
        assert_eq!(pool.outstanding(), 1);
        let outcome = handle.receive().unwrap();
        assert_eq!(outcome.step_id, 1);
        assert_eq!(outcome.rank_acks.map(|ack| ack.rank), [0, 1, 2, 3]);
        assert_eq!(pool.outstanding(), 0);
    }

    #[test]
    fn bound_step_applies_one_delta_to_every_persistent_rank_mirror() {
        let pool = Tp4WorkerPool::spawn_cpu(1, None).unwrap();
        let before = active_table();
        pool.initialize_page_table(Arc::new(before.clone()), 4)
            .unwrap();

        let mut reserved = before.clone();
        reserved.begin_tentative(7, 1).unwrap();
        let reservation_delta =
            Arc::new(PageTableDelta::between(&before, &reserved, 4, 5).unwrap());
        let (plan, schedule) = step_at_generation(1, 5);
        let input = Arc::new(
            StepInput::new(
                &plan,
                &schedule,
                &reservation_delta,
                vec![SequenceStepInput {
                    request_id: 7,
                    context_tokens_before: 64,
                    generated_tokens_before: 0,
                    maximum_new_tokens: 1,
                    prompt_payload_offset: 0,
                    prompt_tokens_this_step: 0,
                    configured_mtp_depth: 0,
                    effective_mtp_depth: 0,
                    sampling: StepSampling::greedy(99),
                }],
                vec![],
            )
            .unwrap(),
        );
        let input_hash = input.canonical_hash();
        let outcome = pool
            .try_submit_bound(
                plan,
                schedule,
                Arc::clone(&input),
                Arc::clone(&reservation_delta),
            )
            .unwrap()
            .receive()
            .unwrap();
        for ack in outcome.rank_acks {
            assert_eq!(ack.input_hash, input_hash);
            assert_eq!(
                ack.page_table_global_digest,
                reservation_delta.global_digest()
            );
            assert_eq!(
                ack.page_table_local_digest,
                reservation_delta.rank_local_digest(ack.rank).unwrap()
            );
        }

        let mut committed = reserved.clone();
        committed.commit_tentative(7, 1).unwrap();
        let commit_delta = Arc::new(PageTableDelta::between(&reserved, &committed, 5, 6).unwrap());
        let acknowledgements = pool.apply_page_delta(Arc::clone(&commit_delta)).unwrap();
        for ack in acknowledgements {
            assert_eq!(ack.generation, 6);
            assert_eq!(ack.global_digest, commit_delta.global_digest());
            assert_eq!(
                ack.local_digest,
                commit_delta.rank_local_digest(ack.rank).unwrap()
            );
        }
    }

    #[test]
    fn bound_step_requires_one_exact_initial_mirror_generation() {
        let pool = Tp4WorkerPool::spawn_cpu(1, None).unwrap();
        let before = active_table();
        let mut reserved = before.clone();
        reserved.begin_tentative(7, 1).unwrap();
        let delta = Arc::new(PageTableDelta::between(&before, &reserved, 4, 5).unwrap());
        let (plan, schedule) = step_at_generation(1, 5);
        let input = Arc::new(
            StepInput::new(
                &plan,
                &schedule,
                &delta,
                vec![SequenceStepInput {
                    request_id: 7,
                    context_tokens_before: 64,
                    generated_tokens_before: 0,
                    maximum_new_tokens: 1,
                    prompt_payload_offset: 0,
                    prompt_tokens_this_step: 0,
                    configured_mtp_depth: 0,
                    effective_mtp_depth: 0,
                    sampling: StepSampling::greedy(1),
                }],
                vec![],
            )
            .unwrap(),
        );
        assert!(matches!(
            pool.try_submit_bound(plan, schedule, input, delta)
                .unwrap()
                .receive(),
            Err(WorkerError::PageTableUninitialized)
        ));
    }

    #[test]
    fn step_quota_is_owned_by_operation_after_handle_abandonment() {
        let entered = Arc::new(Barrier::new(5));
        let release = Arc::new(Barrier::new(5));
        let executors = std::array::from_fn(|_| {
            Box::new(FirstStepBlockingRankExecutor {
                entered: Arc::clone(&entered),
                release: Arc::clone(&release),
                calls: 0,
            }) as Box<dyn RankExecutor + Send>
        });
        let pool = Tp4WorkerPool::spawn(1, executors).unwrap();
        let (plan, schedule) = step(1);
        let handle = pool.try_submit(plan, schedule).unwrap();
        entered.wait();

        drop(handle);
        let outstanding_after_abandonment = pool.outstanding();
        assert!(matches!(
            pool.initialize_page_table(Arc::new(active_table()), 1),
            Err(WorkerError::Saturated)
        ));
        let (replacement_plan, replacement_schedule) = step(2);
        let replacement = pool.try_submit(replacement_plan, replacement_schedule);
        let replacement_was_saturated = matches!(&replacement, Err(WorkerError::Saturated));

        // Always release the blocked rank calls before asserting, so the
        // regression also terminates cleanly against the former ownership.
        release.wait();
        if let Ok(handle) = replacement {
            let _ = handle.receive();
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while pool.outstanding() != 0 {
            assert!(
                Instant::now() < deadline,
                "abandoned TP4 operation did not drain"
            );
            thread::yield_now();
        }

        assert_eq!(outstanding_after_abandonment, 1);
        assert!(replacement_was_saturated);
        let (next_plan, next_schedule) = step(2);
        pool.try_submit(next_plan, next_schedule)
            .unwrap()
            .receive()
            .unwrap();
    }

    #[test]
    fn pool_spawn_waits_for_all_four_ranks_and_cleans_partial_startup() {
        let drops = Arc::new(AtomicUsize::new(0));
        let executors = std::array::from_fn(|_| {
            Box::new(DropCountingRankExecutor {
                drops: Arc::clone(&drops),
            }) as Box<dyn RankExecutor + Send>
        });
        assert!(matches!(
            Tp4WorkerPool::spawn_inner(1, executors, Some(2)),
            Err(WorkerError::Thread(_))
        ));
        assert_eq!(drops.load(Ordering::Acquire), 4);
    }

    #[test]
    fn queue_is_bounded_and_rank_divergence_fails_the_step() {
        let pool = Tp4WorkerPool::spawn_cpu(
            1,
            Some(MockWorkerFault::DivergentOutput {
                rank: 2,
                step_id: 1,
            }),
        )
        .unwrap();
        let (plan, schedule) = step(1);
        let handle = pool.try_submit(plan, schedule).unwrap();
        let (next_plan, next_schedule) = step(2);
        assert!(matches!(
            pool.try_submit(next_plan, next_schedule),
            Err(WorkerError::Saturated)
        ));
        assert!(matches!(handle.receive(), Err(WorkerError::Consensus)));
    }

    #[test]
    fn non_monotonic_step_ids_are_rejected_before_rank_execution() {
        let pool = Tp4WorkerPool::spawn_cpu(2, None).unwrap();
        let (plan, schedule) = step(2);
        pool.try_submit(plan, schedule).unwrap().receive().unwrap();
        let (plan, schedule) = step(1);
        assert!(matches!(
            pool.try_submit(plan, schedule).unwrap().receive(),
            Err(WorkerError::StepOrder)
        ));
    }

    #[test]
    fn custom_rank_executors_are_mutable_persistent_and_thread_affine() {
        let executors = std::array::from_fn(|rank| {
            Box::new(StatefulRankExecutor {
                expected_rank: u8::try_from(rank).unwrap(),
                calls: 0,
                thread: None,
                fail_code: None,
            }) as Box<dyn RankExecutor + Send>
        });
        let pool = Tp4WorkerPool::spawn(2, executors).unwrap();
        for step_id in 1..=2 {
            let (plan, schedule) = step(step_id);
            let outcome = pool.try_submit(plan, schedule).unwrap().receive().unwrap();
            assert_eq!(outcome.step_id, step_id);
        }
    }

    #[test]
    fn factories_create_non_send_executors_on_their_persistent_rank_threads() {
        let coordinator_thread = thread::current().id();
        let factories = std::array::from_fn(|expected_rank| {
            Box::new(move |rank| {
                let owner = thread::current().id();
                if owner == coordinator_thread || usize::from(rank) != expected_rank {
                    return Err(WorkerError::RankStartup);
                }
                Ok(Box::new(ThreadLocalRankExecutor {
                    rank,
                    owner,
                    calls: 0,
                    _not_send: Rc::new(()),
                }) as Box<dyn RankExecutor>)
            }) as Box<dyn RankExecutorFactory>
        });
        let pool = Tp4WorkerPool::spawn_factories(1, factories).unwrap();
        let (plan, schedule) = step(1);
        let outcome = pool.try_submit(plan, schedule).unwrap().receive().unwrap();
        assert_eq!(outcome.step_id, 1);
    }

    #[test]
    fn weight_load_requires_all_four_finalize_acks_before_execution() {
        let (pool, states, cleanup_counts) = transactional_weight_pool(MockWeightFaultConfig {
            phase: None,
            cleanup_rank: None,
            delayed_prepare: None,
        });
        let plan = Arc::new(weight_load_plan());
        let owner_generations = [41, 42, 43, 44];
        let outcome = pool
            .load_weights(
                Arc::clone(&plan),
                17,
                owner_generations,
                Duration::from_secs(1),
            )
            .unwrap();
        assert_eq!(outcome.plan_sha256, plan.plan_sha256());
        assert_eq!(outcome.load_attempt_generation, 17);
        for rank in 0..RANK_SET_SIZE {
            assert_eq!(usize::from(outcome.prepared_receipts[rank].rank), rank);
            assert_eq!(
                outcome.prepared_receipts[rank].owner_allocation_generation,
                owner_generations[rank]
            );
            assert_eq!(
                usize::from(outcome.adoption_acknowledgements[rank].rank),
                rank
            );
            assert_eq!(
                usize::from(outcome.finalize_acknowledgements[rank].rank()),
                rank
            );
            assert_eq!(
                outcome.finalize_acknowledgements[rank].adopted_rank_set_sha256(),
                outcome.adopted_receipt.adopted_rank_set_sha256()
            );
            assert_eq!(cleanup_counts[rank].load(Ordering::SeqCst), 0);
        }
        assert_eq!(
            *states.lock().unwrap(),
            [MockWeightState::Resident; RANK_SET_SIZE]
        );

        let (step, schedule) = step(1);
        assert_eq!(
            pool.try_submit(step, schedule)
                .unwrap()
                .receive()
                .unwrap()
                .step_id,
            1
        );
    }

    #[test]
    fn weight_load_owns_exclusive_pool_capacity_until_transaction_completion() {
        let (pool, _states, _cleanup_counts) = transactional_weight_pool(MockWeightFaultConfig {
            phase: None,
            cleanup_rank: None,
            delayed_prepare: Some((2, Duration::from_millis(75))),
        });
        let pool = Arc::new(pool);
        let loading_pool = Arc::clone(&pool);
        let loader = thread::spawn(move || {
            loading_pool.load_weights(
                Arc::new(weight_load_plan()),
                17,
                [41, 42, 43, 44],
                Duration::from_secs(1),
            )
        });
        let deadline = Instant::now() + Duration::from_secs(1);
        while pool.outstanding() == 0 {
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        let (step, schedule) = step(1);
        assert!(matches!(
            pool.try_submit(step, schedule),
            Err(WorkerError::Saturated)
        ));
        loader.join().unwrap().unwrap();
        assert_eq!(pool.outstanding(), 0);
    }

    #[test]
    fn every_prepare_adoption_and_finalize_rank_failure_gets_four_cleanup_acks() {
        for phase in [
            RankWeightPhase::Prepare,
            RankWeightPhase::Acknowledge,
            RankWeightPhase::Finalize,
        ] {
            for failed_rank in 0..u8::try_from(RANK_SET_SIZE).unwrap() {
                let (pool, states, cleanup_counts) =
                    transactional_weight_pool(MockWeightFaultConfig {
                        phase: Some((failed_rank, phase)),
                        cleanup_rank: None,
                        delayed_prepare: None,
                    });
                let failure = pool
                    .load_weights(
                        Arc::new(weight_load_plan()),
                        17,
                        [41, 42, 43, 44],
                        Duration::from_secs(1),
                    )
                    .unwrap_err();
                assert_eq!(
                    failure.cause,
                    WeightLoadFailureCause::Rank {
                        rank: failed_rank,
                        phase,
                        error: if phase == RankWeightPhase::Prepare {
                            LoadPlanError::Writer
                        } else {
                            LoadPlanError::Adoption
                        },
                    }
                );
                assert_eq!(failure.cleanup_failure, None);
                let cleanup = failure.cleanup_acknowledgements;
                for rank in 0..RANK_SET_SIZE {
                    let acknowledgement = cleanup[rank].unwrap();
                    assert_eq!(usize::from(acknowledgement.rank()), rank);
                    assert_eq!(acknowledgement.load_attempt_generation(), 17);
                    assert_eq!(
                        acknowledgement.owner_allocation_generation(),
                        41 + u64::try_from(rank).unwrap()
                    );
                    assert_eq!(cleanup_counts[rank].load(Ordering::SeqCst), 1);
                }
                assert_eq!(
                    *states.lock().unwrap(),
                    [MockWeightState::Aborted; RANK_SET_SIZE]
                );
            }
        }
    }

    #[test]
    fn every_cleanup_rank_failure_is_explicit_and_never_forges_four_acks() {
        for cleanup_rank in 0..u8::try_from(RANK_SET_SIZE).unwrap() {
            let primary_rank = (cleanup_rank + 1) % u8::try_from(RANK_SET_SIZE).unwrap();
            let (pool, states, cleanup_counts) = transactional_weight_pool(MockWeightFaultConfig {
                phase: Some((primary_rank, RankWeightPhase::Prepare)),
                cleanup_rank: Some(cleanup_rank),
                delayed_prepare: None,
            });
            let failure = pool
                .load_weights(
                    Arc::new(weight_load_plan()),
                    17,
                    [41, 42, 43, 44],
                    Duration::from_secs(1),
                )
                .unwrap_err();
            assert_eq!(
                failure.cause,
                WeightLoadFailureCause::Rank {
                    rank: primary_rank,
                    phase: RankWeightPhase::Prepare,
                    error: LoadPlanError::Writer,
                }
            );
            assert_eq!(
                failure.cleanup_failure,
                Some(WeightLoadFailureCause::Rank {
                    rank: cleanup_rank,
                    phase: RankWeightPhase::Abort,
                    error: LoadPlanError::Writer,
                })
            );
            for rank in 0..RANK_SET_SIZE {
                assert_eq!(
                    failure.cleanup_acknowledgements[rank].is_some(),
                    rank != usize::from(cleanup_rank)
                );
                assert_eq!(
                    cleanup_counts[rank].load(Ordering::SeqCst),
                    usize::from(rank != usize::from(cleanup_rank))
                );
            }
            assert_ne!(
                states.lock().unwrap()[usize::from(cleanup_rank)],
                MockWeightState::Aborted
            );
        }
    }

    #[test]
    fn phase_timeout_triggers_common_abort_and_reports_incomplete_cleanup() {
        let (pool, _states, _cleanup_counts) = transactional_weight_pool(MockWeightFaultConfig {
            phase: None,
            cleanup_rank: None,
            delayed_prepare: Some((2, Duration::from_millis(75))),
        });
        let failure = pool
            .load_weights(
                Arc::new(weight_load_plan()),
                17,
                [41, 42, 43, 44],
                Duration::from_millis(5),
            )
            .unwrap_err();
        assert_eq!(
            failure.cause,
            WeightLoadFailureCause::Timeout {
                phase: RankWeightPhase::Prepare
            }
        );
        assert_eq!(
            failure.cleanup_failure,
            Some(WeightLoadFailureCause::Timeout {
                phase: RankWeightPhase::Abort
            })
        );
        assert_eq!(
            failure
                .cleanup_acknowledgements
                .iter()
                .filter(|acknowledgement| acknowledgement.is_some())
                .count(),
            RANK_SET_SIZE - 1
        );
    }

    #[test]
    fn one_rank_backend_failure_aborts_the_whole_step() {
        let executors = std::array::from_fn(|rank| {
            Box::new(StatefulRankExecutor {
                expected_rank: u8::try_from(rank).unwrap(),
                calls: 0,
                thread: None,
                fail_code: (rank == 3).then_some(17),
            }) as Box<dyn RankExecutor + Send>
        });
        let pool = Tp4WorkerPool::spawn(1, executors).unwrap();
        let (plan, schedule) = step(1);
        assert!(matches!(
            pool.try_submit(plan, schedule).unwrap().receive(),
            Err(WorkerError::RankExecution {
                rank: 3,
                error: RankExecutionError::Backend(17),
            })
        ));
    }

    #[test]
    fn malformed_rank_output_never_reaches_consensus() {
        let executors = std::array::from_fn(|_| {
            Box::new(InvalidOutputExecutor) as Box<dyn RankExecutor + Send>
        });
        let pool = Tp4WorkerPool::spawn(1, executors).unwrap();
        let (plan, schedule) = step(1);
        assert!(matches!(
            pool.try_submit(plan, schedule).unwrap().receive(),
            Err(WorkerError::RankOutput {
                error: OutputError::SequenceCount,
                ..
            })
        ));
    }
}
