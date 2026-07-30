use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use glm_cache::{PageTableDelta, PageTableDeltaError, PageTableMirror, SequencePageTable};
use sha2::{Digest, Sha256};

use crate::{
    CollectiveSchedule, CommittedTokens, GLM_52_OUTPUT_VOCABULARY, OutputError, PlanError,
    StepInput, StepInputError, StepMode, StepOutput, StepPlan,
};

const CPU_TOKEN_DOMAIN: &[u8] = b"glmaxx.cpu-worker-token.v1\0";
const TP_RANKS: u8 = 4;

/// Rank-local execution boundary for one persistent TP4 worker thread.
///
/// Implementations own their mutable rank state, including a CUDA context,
/// streams, graph instances, device allocations, and collective handles.
/// The worker verifies the immutable plan and collective schedule before this
/// method is entered.
pub trait RankExecutor: Send + 'static {
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RankExecutionError {
    Backend(i32),
    Invariant,
}

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
    Execute(RankCommand),
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
        let executors =
            std::array::from_fn(|_| Box::new(CpuRankExecutor { fault }) as Box<dyn RankExecutor>);
        Self::spawn(maximum_outstanding, executors)
    }

    pub fn spawn(
        maximum_outstanding: usize,
        executors: [Box<dyn RankExecutor>; 4],
    ) -> Result<Self, WorkerError> {
        Self::spawn_inner(maximum_outstanding, executors, None)
    }

    fn spawn_inner(
        maximum_outstanding: usize,
        executors: [Box<dyn RankExecutor>; 4],
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
                dispatch_loop(receiver, executors, startup_sender, rank_spawn_fault);
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

    #[must_use]
    pub fn outstanding(&self) -> usize {
        self.outstanding.load(Ordering::Acquire)
    }

    fn reserve_slot(&self) -> Result<(), WorkerError> {
        self.outstanding
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current < self.maximum_outstanding).then_some(current + 1)
            })
            .map(|_| ())
            .map_err(|_| WorkerError::Saturated)
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
    executors: [Box<dyn RankExecutor>; 4],
    startup: SyncSender<Result<(), WorkerError>>,
    rank_spawn_fault: Option<u8>,
) {
    let mut rank_senders: Vec<SyncSender<RankCommandEnvelope>> =
        Vec::with_capacity(usize::from(TP_RANKS));
    let mut rank_workers: Vec<JoinHandle<()>> = Vec::with_capacity(usize::from(TP_RANKS));
    let (ready_sender, ready_receiver) = mpsc::sync_channel(usize::from(TP_RANKS));
    for (rank, executor) in (0..TP_RANKS).zip(executors) {
        let (sender, rank_receiver) = mpsc::sync_channel::<RankCommandEnvelope>(1);
        let builder = thread::Builder::new().name(format!("glmaxx-rank-{rank}"));
        let ready = ready_sender.clone();
        let worker = if rank_spawn_fault == Some(rank) {
            Err(std::io::Error::other(format!(
                "injected rank {rank} thread spawn failure"
            )))
        } else {
            builder.spawn(move || {
                if ready.send(rank).is_ok() {
                    rank_loop(rank, rank_receiver, executor);
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
        let Ok(rank) = ready_receiver.recv() else {
            let cleanup_panicked = shutdown_rank_workers(rank_senders, rank_workers);
            let error = if cleanup_panicked {
                WorkerError::WorkerPanic
            } else {
                WorkerError::RankStartup
            };
            let _ = startup.send(Err(error));
            return;
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
    RankExecution { rank: u8, error: RankExecutionError },
    RankOutput { rank: u8, error: OutputError },
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
        sync::{
            Arc, Barrier,
            atomic::{AtomicUsize, Ordering},
        },
        thread::ThreadId,
        time::Instant,
    };

    use glm_cache::{PageTableConfig, PageTableDelta, SequencePageTable};

    use crate::{
        AttentionTransport, CollectiveKind, CollectiveOp, SequenceStepInput, StepMode,
        StepPlanRequest, StepSampling, TP_RANK_MASK,
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
            }) as Box<dyn RankExecutor>
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
            }) as Box<dyn RankExecutor>
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
            }) as Box<dyn RankExecutor>
        });
        let pool = Tp4WorkerPool::spawn(2, executors).unwrap();
        for step_id in 1..=2 {
            let (plan, schedule) = step(step_id);
            let outcome = pool.try_submit(plan, schedule).unwrap().receive().unwrap();
            assert_eq!(outcome.step_id, step_id);
        }
    }

    #[test]
    fn one_rank_backend_failure_aborts_the_whole_step() {
        let executors = std::array::from_fn(|rank| {
            Box::new(StatefulRankExecutor {
                expected_rank: u8::try_from(rank).unwrap(),
                calls: 0,
                thread: None,
                fail_code: (rank == 3).then_some(17),
            }) as Box<dyn RankExecutor>
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
        let executors =
            std::array::from_fn(|_| Box::new(InvalidOutputExecutor) as Box<dyn RankExecutor>);
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
