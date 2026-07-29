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

use sha2::{Digest, Sha256};

use crate::{CollectiveSchedule, PlanError, StepPlan};

const OUTPUT_DOMAIN: &[u8] = b"glmaxx.cpu-worker-output.v1\0";
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
    ) -> Result<[u8; 32], RankExecutionError>;
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
    ) -> Result<[u8; 32], RankExecutionError> {
        let mut hasher = Sha256::new();
        hasher.update(OUTPUT_DOMAIN);
        hasher.update(plan.plan_hash);
        hasher.update(schedule.hash());
        let mut output_digest: [u8; 32] = hasher.finalize().into();
        if self.fault
            == Some(MockWorkerFault::DivergentOutput {
                rank,
                step_id: plan.step_id,
            })
        {
            output_digest[0] ^= 1;
        }
        Ok(output_digest)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RankStepAck {
    pub rank: u8,
    pub step_id: u64,
    pub plan_hash: [u8; 32],
    pub schedule_hash: [u8; 32],
    pub output_digest: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepOutcome {
    pub step_id: u64,
    pub plan_hash: [u8; 32],
    pub output_digest: [u8; 32],
    pub rank_acks: [RankStepAck; 4],
}

pub struct StepHandle {
    receiver: Receiver<Result<StepOutcome, WorkerError>>,
    outstanding: Arc<AtomicUsize>,
    released: bool,
}

impl StepHandle {
    pub fn receive(mut self) -> Result<StepOutcome, WorkerError> {
        let result = self.receiver.recv().map_err(|_| WorkerError::Closed)?;
        self.release();
        result
    }

    pub fn receive_timeout(mut self, timeout: Duration) -> Result<StepOutcome, WorkerError> {
        let result = self
            .receiver
            .recv_timeout(timeout)
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => WorkerError::Timeout,
                mpsc::RecvTimeoutError::Disconnected => WorkerError::Closed,
            })?;
        self.release();
        result
    }

    fn release(&mut self) {
        if !self.released {
            self.outstanding.fetch_sub(1, Ordering::AcqRel);
            self.released = true;
        }
    }
}

impl Drop for StepHandle {
    fn drop(&mut self) {
        self.release();
    }
}

struct DispatchCommand {
    plan: StepPlan,
    schedule: CollectiveSchedule,
    response: SyncSender<Result<StepOutcome, WorkerError>>,
}

#[derive(Clone)]
struct RankCommand {
    plan: StepPlan,
    schedule: CollectiveSchedule,
    response: SyncSender<Result<RankStepAck, WorkerError>>,
}

pub struct Tp4WorkerPool {
    sender: Option<SyncSender<DispatchCommand>>,
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
        if maximum_outstanding == 0 {
            return Err(WorkerError::Config);
        }
        let (sender, receiver) = mpsc::sync_channel(maximum_outstanding);
        let dispatcher = thread::Builder::new()
            .name("glmaxx-step-dispatch".into())
            .spawn(move || dispatch_loop(receiver, executors))
            .map_err(WorkerError::Thread)?;
        Ok(Self {
            sender: Some(sender),
            dispatcher: Some(dispatcher),
            outstanding: Arc::new(AtomicUsize::new(0)),
            maximum_outstanding,
        })
    }

    pub fn try_submit(
        &self,
        plan: StepPlan,
        schedule: CollectiveSchedule,
    ) -> Result<StepHandle, WorkerError> {
        plan.verify(&schedule)?;
        self.reserve_slot()?;
        let (response, receiver) = mpsc::sync_channel(1);
        let command = DispatchCommand {
            plan,
            schedule,
            response,
        };
        let Some(sender) = &self.sender else {
            self.outstanding.fetch_sub(1, Ordering::AcqRel);
            return Err(WorkerError::Closed);
        };
        if let Err(error) = sender.try_send(command) {
            self.outstanding.fetch_sub(1, Ordering::AcqRel);
            return Err(match error {
                TrySendError::Full(_) => WorkerError::Saturated,
                TrySendError::Disconnected(_) => WorkerError::Closed,
            });
        }
        Ok(StepHandle {
            receiver,
            outstanding: Arc::clone(&self.outstanding),
            released: false,
        })
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

fn dispatch_loop(receiver: Receiver<DispatchCommand>, executors: [Box<dyn RankExecutor>; 4]) {
    let mut rank_senders = Vec::with_capacity(usize::from(TP_RANKS));
    let mut rank_workers = Vec::with_capacity(usize::from(TP_RANKS));
    for (rank, executor) in (0..TP_RANKS).zip(executors) {
        let (sender, rank_receiver) = mpsc::sync_channel::<RankCommand>(1);
        let builder = thread::Builder::new().name(format!("glmaxx-rank-{rank}"));
        let Ok(worker) = builder.spawn(move || rank_loop(rank, rank_receiver, executor)) else {
            return;
        };
        rank_senders.push(sender);
        rank_workers.push(worker);
    }

    let mut last_step_id = 0_u64;
    while let Ok(command) = receiver.recv() {
        let result = if command.plan.step_id <= last_step_id {
            Err(WorkerError::StepOrder)
        } else {
            last_step_id = command.plan.step_id;
            dispatch_one(&rank_senders, &command.plan, &command.schedule)
        };
        let failed = result.is_err();
        let _ = command.response.send(result);
        if failed {
            // A rank/backend/consensus failure is process-fatal for this
            // executor generation. Continuing could let ranks enter different
            // collective ordinals after one rank already abandoned the step.
            break;
        }
    }
    drop(rank_senders);
    for worker in rank_workers {
        let _ = worker.join();
    }
}

fn dispatch_one(
    rank_senders: &[SyncSender<RankCommand>],
    plan: &StepPlan,
    schedule: &CollectiveSchedule,
) -> Result<StepOutcome, WorkerError> {
    let (ack_sender, ack_receiver) = mpsc::sync_channel(usize::from(TP_RANKS));
    for sender in rank_senders {
        sender
            .send(RankCommand {
                plan: *plan,
                schedule: schedule.clone(),
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
    if acknowledgements
        .iter()
        .enumerate()
        .any(|(rank, ack)| usize::from(ack.rank) != rank)
    {
        return Err(WorkerError::RankSet);
    }
    let first = acknowledgements[0];
    if acknowledgements.iter().any(|ack| {
        ack.step_id != first.step_id
            || ack.plan_hash != first.plan_hash
            || ack.schedule_hash != first.schedule_hash
            || ack.output_digest != first.output_digest
    }) {
        return Err(WorkerError::Consensus);
    }
    let rank_acks: [RankStepAck; 4] = acknowledgements
        .try_into()
        .map_err(|_| WorkerError::RankSet)?;
    Ok(StepOutcome {
        step_id: first.step_id,
        plan_hash: first.plan_hash,
        output_digest: first.output_digest,
        rank_acks,
    })
}

fn rank_loop(rank: u8, receiver: Receiver<RankCommand>, mut executor: Box<dyn RankExecutor>) {
    let mut last_step_id = 0_u64;
    while let Ok(command) = receiver.recv() {
        let result = if command.plan.step_id <= last_step_id {
            Err(WorkerError::StepOrder)
        } else {
            last_step_id = command.plan.step_id;
            execute_rank(rank, &command.plan, &command.schedule, executor.as_mut())
        };
        let _ = command.response.send(result);
    }
}

fn execute_rank(
    rank: u8,
    plan: &StepPlan,
    schedule: &CollectiveSchedule,
    executor: &mut dyn RankExecutor,
) -> Result<RankStepAck, WorkerError> {
    plan.verify(schedule)?;
    let output_digest = executor
        .execute(rank, plan, schedule)
        .map_err(|error| WorkerError::RankExecution { rank, error })?;
    Ok(RankStepAck {
        rank,
        step_id: plan.step_id,
        plan_hash: plan.plan_hash,
        schedule_hash: schedule.hash(),
        output_digest,
    })
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
    Plan(PlanError),
    Thread(std::io::Error),
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

#[cfg(test)]
mod tests {
    use std::thread::ThreadId;

    use crate::{
        AttentionTransport, CollectiveKind, CollectiveOp, StepMode, StepPlanRequest, TP_RANK_MASK,
    };

    use super::*;

    fn step(step_id: u64) -> (StepPlan, CollectiveSchedule) {
        let schedule = CollectiveSchedule::new(vec![CollectiveOp {
            ordinal: 0,
            kind: CollectiveKind::TpReduce,
            route_id: 1,
            payload_bytes: 16,
            participant_mask: TP_RANK_MASK,
        }])
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
                sequence_table_generation: 1,
            },
            &schedule,
        )
        .unwrap();
        (plan, schedule)
    }

    struct StatefulRankExecutor {
        expected_rank: u8,
        calls: u64,
        thread: Option<ThreadId>,
        fail_code: Option<i32>,
    }

    impl RankExecutor for StatefulRankExecutor {
        fn execute(
            &mut self,
            rank: u8,
            plan: &StepPlan,
            schedule: &CollectiveSchedule,
        ) -> Result<[u8; 32], RankExecutionError> {
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
            let mut hasher = Sha256::new();
            hasher.update(b"glmaxx.stateful-rank-test.v1\0");
            hasher.update(plan.plan_hash);
            hasher.update(schedule.hash());
            Ok(hasher.finalize().into())
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
}
