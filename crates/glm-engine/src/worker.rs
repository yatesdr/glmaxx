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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MockWorkerFault {
    DivergentOutput { rank: u8, step_id: u64 },
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

pub struct CpuWorkerPool {
    sender: Option<SyncSender<DispatchCommand>>,
    dispatcher: Option<JoinHandle<()>>,
    outstanding: Arc<AtomicUsize>,
    maximum_outstanding: usize,
}

impl CpuWorkerPool {
    pub fn spawn(
        maximum_outstanding: usize,
        fault: Option<MockWorkerFault>,
    ) -> Result<Self, WorkerError> {
        if maximum_outstanding == 0 {
            return Err(WorkerError::Config);
        }
        let (sender, receiver) = mpsc::sync_channel(maximum_outstanding);
        let dispatcher = thread::Builder::new()
            .name("glmaxx-step-dispatch".into())
            .spawn(move || dispatch_loop(receiver, fault))
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

impl Drop for CpuWorkerPool {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(dispatcher) = self.dispatcher.take() {
            let _ = dispatcher.join();
        }
    }
}

fn dispatch_loop(receiver: Receiver<DispatchCommand>, fault: Option<MockWorkerFault>) {
    let mut rank_senders = Vec::with_capacity(usize::from(TP_RANKS));
    let mut rank_workers = Vec::with_capacity(usize::from(TP_RANKS));
    for rank in 0..TP_RANKS {
        let (sender, rank_receiver) = mpsc::sync_channel::<RankCommand>(1);
        let builder = thread::Builder::new().name(format!("glmaxx-rank-{rank}"));
        let Ok(worker) = builder.spawn(move || rank_loop(rank, rank_receiver, fault)) else {
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
        let _ = command.response.send(result);
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

fn rank_loop(rank: u8, receiver: Receiver<RankCommand>, fault: Option<MockWorkerFault>) {
    let mut last_step_id = 0_u64;
    while let Ok(command) = receiver.recv() {
        let result = if command.plan.step_id <= last_step_id {
            Err(WorkerError::StepOrder)
        } else {
            last_step_id = command.plan.step_id;
            execute_rank(rank, &command.plan, &command.schedule, fault)
        };
        let _ = command.response.send(result);
    }
}

fn execute_rank(
    rank: u8,
    plan: &StepPlan,
    schedule: &CollectiveSchedule,
    fault: Option<MockWorkerFault>,
) -> Result<RankStepAck, WorkerError> {
    plan.verify(schedule)?;
    let mut hasher = Sha256::new();
    hasher.update(OUTPUT_DOMAIN);
    hasher.update(plan.plan_hash);
    hasher.update(schedule.hash());
    let mut output_digest: [u8; 32] = hasher.finalize().into();
    if fault
        == Some(MockWorkerFault::DivergentOutput {
            rank,
            step_id: plan.step_id,
        })
    {
        output_digest[0] ^= 1;
    }
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

    #[test]
    fn four_workers_acknowledge_one_identical_plan() {
        let pool = CpuWorkerPool::spawn(1, None).unwrap();
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
        let pool = CpuWorkerPool::spawn(
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
        let pool = CpuWorkerPool::spawn(2, None).unwrap();
        let (plan, schedule) = step(2);
        pool.try_submit(plan, schedule).unwrap().receive().unwrap();
        let (plan, schedule) = step(1);
        assert!(matches!(
            pool.try_submit(plan, schedule).unwrap().receive(),
            Err(WorkerError::StepOrder)
        ));
    }
}
