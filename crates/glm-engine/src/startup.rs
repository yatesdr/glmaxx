use std::{
    collections::BTreeSet,
    fmt,
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
};

use crate::AdoptedRankSetReceipt;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum StartupState {
    Created = 0,
    HostValidated = 1,
    CudaContextsReady = 2,
    TopologyValidated = 3,
    ModulesReady = 4,
    MemoryPlanned = 5,
    WeightsLoaded = 6,
    GraphsCaptured = 7,
    KvReady = 8,
    CollectivesVoted = 9,
    Healthy = 10,
    Failed = 255,
}

impl StartupState {
    pub const NORMATIVE_ORDER: [Self; 11] = [
        Self::Created,
        Self::HostValidated,
        Self::CudaContextsReady,
        Self::TopologyValidated,
        Self::ModulesReady,
        Self::MemoryPlanned,
        Self::WeightsLoaded,
        Self::GraphsCaptured,
        Self::KvReady,
        Self::CollectivesVoted,
        Self::Healthy,
    ];

    const fn successor(self) -> Option<Self> {
        match self {
            Self::Created => Some(Self::HostValidated),
            Self::HostValidated => Some(Self::CudaContextsReady),
            Self::CudaContextsReady => Some(Self::TopologyValidated),
            Self::TopologyValidated => Some(Self::ModulesReady),
            Self::ModulesReady => Some(Self::MemoryPlanned),
            Self::MemoryPlanned => Some(Self::WeightsLoaded),
            Self::WeightsLoaded => Some(Self::GraphsCaptured),
            Self::GraphsCaptured => Some(Self::KvReady),
            Self::KvReady => Some(Self::CollectivesVoted),
            Self::CollectivesVoted => Some(Self::Healthy),
            Self::Healthy | Self::Failed => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RankStartupReport {
    pub rank: u8,
    pub reached: StartupState,
    pub weight_policy_hash: [u8; 32],
    pub graph_profile_hash: [u8; 32],
    pub collective_route_hash: [u8; 32],
    pub memory_plan_hash: [u8; 32],
    pub adopted_rank_set_hash: [u8; 32],
}

#[derive(Clone, Debug)]
pub struct StartupCoordinator {
    state: StartupState,
    consensus: Option<RankStartupReport>,
}

impl StartupCoordinator {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: StartupState::Created,
            consensus: None,
        }
    }

    #[must_use]
    pub const fn state(&self) -> StartupState {
        self.state
    }

    /// Advances only when all four ranks report the exact next stage and all
    /// process-immutable digests match. Any disagreement poisons the process.
    pub fn advance(
        &mut self,
        reports: Vec<Result<RankStartupReport, StartupError>>,
    ) -> Result<(), StartupError> {
        self.advance_inner(reports, None, false)
    }

    /// Advances `MEMORY_PLANNED -> WEIGHTS_LOADED` only after the checkpoint
    /// transaction has validated all four adoption acknowledgements.
    pub fn advance_weights_loaded(
        &mut self,
        reports: Vec<Result<RankStartupReport, StartupError>>,
        adopted: AdoptedRankSetReceipt,
    ) -> Result<(), StartupError> {
        self.advance_inner(reports, Some(adopted.adopted_rank_set_sha256()), false)
    }

    fn advance_mock_weights_loaded(
        &mut self,
        reports: Vec<Result<RankStartupReport, StartupError>>,
    ) -> Result<(), StartupError> {
        let expected = reports
            .iter()
            .find_map(|report| report.as_ref().ok())
            .map(|report| report.adopted_rank_set_hash)
            .filter(|digest| *digest != [0; 32]);
        let Some(expected) = expected else {
            return self.fail(StartupError::AdoptionRequired);
        };
        self.advance_inner(reports, Some(expected), true)
    }

    fn advance_inner(
        &mut self,
        reports: Vec<Result<RankStartupReport, StartupError>>,
        expected_adopted_rank_set: Option<[u8; 32]>,
        mock_adoption: bool,
    ) -> Result<(), StartupError> {
        if self.state == StartupState::Failed {
            return Err(StartupError::AlreadyFailed);
        }
        let Some(next) = self.state.successor() else {
            return Err(StartupError::Terminal);
        };
        if reports.len() != 4 {
            return self.fail(StartupError::RankCount);
        }
        if next == StartupState::WeightsLoaded && expected_adopted_rank_set.is_none() {
            return self.fail(StartupError::AdoptionRequired);
        }
        if next != StartupState::WeightsLoaded && expected_adopted_rank_set.is_some() {
            return self.fail(StartupError::AdoptionAgreement);
        }
        if mock_adoption && next != StartupState::WeightsLoaded {
            return self.fail(StartupError::AdoptionAgreement);
        }
        let reports = reports
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .inspect_err(|_| {
                self.state = StartupState::Failed;
            })?;
        let ranks: BTreeSet<u8> = reports.iter().map(|report| report.rank).collect();
        if ranks != BTreeSet::from([0, 1, 2, 3])
            || reports.iter().any(|report| report.reached != next)
        {
            return self.fail(StartupError::RankAgreement);
        }
        let reference = reports[0];
        if immutable_digests(reference).contains(&[0; 32])
            || reports
                .iter()
                .skip(1)
                .any(|report| immutable_digests(*report) != immutable_digests(reference))
        {
            return self.fail(StartupError::DigestAgreement);
        }
        if let Some(consensus) = self.consensus
            && immutable_digests(consensus) != immutable_digests(reference)
        {
            return self.fail(StartupError::DigestChanged);
        }
        let expected_adopted = match next {
            StartupState::Created
            | StartupState::HostValidated
            | StartupState::CudaContextsReady
            | StartupState::TopologyValidated
            | StartupState::ModulesReady
            | StartupState::MemoryPlanned => [0; 32],
            StartupState::WeightsLoaded => {
                let Some(expected) = expected_adopted_rank_set.filter(|digest| *digest != [0; 32])
                else {
                    return self.fail(StartupError::AdoptionRequired);
                };
                expected
            }
            StartupState::GraphsCaptured
            | StartupState::KvReady
            | StartupState::CollectivesVoted
            | StartupState::Healthy => {
                let expected = self
                    .consensus
                    .map(|consensus| consensus.adopted_rank_set_hash)
                    .filter(|digest| *digest != [0; 32]);
                let Some(expected) = expected else {
                    return self.fail(StartupError::AdoptionAgreement);
                };
                expected
            }
            StartupState::Failed => return self.fail(StartupError::RankAgreement),
        };
        if reports
            .iter()
            .any(|report| report.adopted_rank_set_hash != expected_adopted)
        {
            return self.fail(StartupError::AdoptionAgreement);
        }
        self.consensus = Some(reference);
        self.state = next;
        Ok(())
    }

    fn fail<T>(&mut self, error: StartupError) -> Result<T, StartupError> {
        self.state = StartupState::Failed;
        Err(error)
    }
}

impl Default for StartupCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

fn immutable_digests(report: RankStartupReport) -> [[u8; 32]; 4] {
    [
        report.weight_policy_hash,
        report.graph_profile_hash,
        report.collective_route_hash,
        report.memory_plan_hash,
    ]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MockFault {
    None,
    RankReject { rank: u8, stage: StartupState },
    DivergentCollective { rank: u8 },
}

enum WorkerCommand {
    Advance(StartupState),
    Shutdown,
}

/// Exercises the real coordinator contract with four bounded rank channels.
/// It performs no CUDA calls and is safe on a CPU-only host.
pub fn run_mock_startup(fault: MockFault) -> Result<StartupState, StartupError> {
    let (response_tx, response_rx) = mpsc::sync_channel(4);
    let mut commands = Vec::new();
    let mut handles = Vec::new();
    for rank in 0_u8..4 {
        let (command_tx, command_rx) = mpsc::sync_channel(1);
        commands.push(command_tx);
        let response = response_tx.clone();
        handles.push(thread::spawn(move || {
            mock_worker(rank, fault, command_rx, response);
        }));
    }
    drop(response_tx);
    let result = run_coordinator(&commands, &response_rx);
    for command in &commands {
        let _ = command.send(WorkerCommand::Shutdown);
    }
    for handle in handles {
        handle.join().map_err(|_| StartupError::WorkerPanic)?;
    }
    result
}

fn run_coordinator(
    commands: &[SyncSender<WorkerCommand>],
    responses: &Receiver<Result<RankStartupReport, StartupError>>,
) -> Result<StartupState, StartupError> {
    let mut coordinator = StartupCoordinator::new();
    while let Some(next) = coordinator.state().successor() {
        for command in commands {
            command
                .send(WorkerCommand::Advance(next))
                .map_err(|_| StartupError::Channel)?;
        }
        let mut reports = Vec::with_capacity(4);
        for _ in 0..4 {
            reports.push(responses.recv().map_err(|_| StartupError::Channel)?);
        }
        if next == StartupState::WeightsLoaded {
            coordinator.advance_mock_weights_loaded(reports)?;
        } else {
            coordinator.advance(reports)?;
        }
    }
    Ok(coordinator.state())
}

fn mock_worker(
    rank: u8,
    fault: MockFault,
    commands: Receiver<WorkerCommand>,
    responses: SyncSender<Result<RankStartupReport, StartupError>>,
) {
    while let Ok(command) = commands.recv() {
        match command {
            WorkerCommand::Shutdown => return,
            WorkerCommand::Advance(stage) => {
                if fault == (MockFault::RankReject { rank, stage }) {
                    let _ = responses.send(Err(StartupError::RankRejected { rank, stage }));
                    continue;
                }
                let collective = if matches!(
                    fault,
                    MockFault::DivergentCollective { rank: faulty } if faulty == rank
                ) {
                    [0x99; 32]
                } else {
                    [0x33; 32]
                };
                let _ = responses.send(Ok(RankStartupReport {
                    rank,
                    reached: stage,
                    weight_policy_hash: [0x11; 32],
                    graph_profile_hash: [0x22; 32],
                    collective_route_hash: collective,
                    memory_plan_hash: [0x44; 32],
                    adopted_rank_set_hash: if stage >= StartupState::WeightsLoaded {
                        [0x55; 32]
                    } else {
                        [0; 32]
                    },
                }));
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupError {
    RankCount,
    RankAgreement,
    DigestAgreement,
    DigestChanged,
    AdoptionRequired,
    AdoptionAgreement,
    RankRejected { rank: u8, stage: StartupState },
    AlreadyFailed,
    Terminal,
    Channel,
    WorkerPanic,
}

impl fmt::Display for StartupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for StartupError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn reports(reached: StartupState) -> Vec<Result<RankStartupReport, StartupError>> {
        (0_u8..4)
            .map(|rank| {
                Ok(RankStartupReport {
                    rank,
                    reached,
                    weight_policy_hash: [0x11; 32],
                    graph_profile_hash: [0x22; 32],
                    collective_route_hash: [0x33; 32],
                    memory_plan_hash: [0x44; 32],
                    adopted_rank_set_hash: if reached >= StartupState::WeightsLoaded {
                        [0x55; 32]
                    } else {
                        [0; 32]
                    },
                })
            })
            .collect()
    }

    #[test]
    fn startup_order_exactly_matches_the_normative_engine_sequence() {
        let mut observed = vec![StartupState::Created];
        while let Some(next) = observed.last().copied().unwrap().successor() {
            observed.push(next);
        }
        assert_eq!(observed, StartupState::NORMATIVE_ORDER);
        assert!(
            StartupState::NORMATIVE_ORDER
                .iter()
                .position(|state| *state == StartupState::MemoryPlanned)
                .unwrap()
                < StartupState::NORMATIVE_ORDER
                    .iter()
                    .position(|state| *state == StartupState::WeightsLoaded)
                    .unwrap()
        );
    }

    #[test]
    fn obsolete_weight_before_memory_sequence_fails_closed() {
        let mut coordinator = StartupCoordinator::new();
        for stage in [
            StartupState::HostValidated,
            StartupState::CudaContextsReady,
            StartupState::TopologyValidated,
            StartupState::ModulesReady,
        ] {
            coordinator.advance(reports(stage)).unwrap();
        }
        assert_eq!(
            coordinator.advance(reports(StartupState::WeightsLoaded)),
            Err(StartupError::RankAgreement)
        );
        assert_eq!(coordinator.state(), StartupState::Failed);
    }

    #[test]
    fn four_rank_mock_reaches_healthy() {
        assert_eq!(
            run_mock_startup(MockFault::None).unwrap(),
            StartupState::Healthy
        );
    }

    #[test]
    fn one_rank_failure_aborts_the_process() {
        assert_eq!(
            run_mock_startup(MockFault::RankReject {
                rank: 2,
                stage: StartupState::WeightsLoaded,
            }),
            Err(StartupError::RankRejected {
                rank: 2,
                stage: StartupState::WeightsLoaded,
            })
        );
    }

    #[test]
    fn public_weight_transition_requires_a_completed_adoption_receipt() {
        let mut coordinator = StartupCoordinator::new();
        for stage in [
            StartupState::HostValidated,
            StartupState::CudaContextsReady,
            StartupState::TopologyValidated,
            StartupState::ModulesReady,
            StartupState::MemoryPlanned,
        ] {
            coordinator.advance(reports(stage)).unwrap();
        }
        assert_eq!(
            coordinator.advance(reports(StartupState::WeightsLoaded)),
            Err(StartupError::AdoptionRequired)
        );

        let mut coordinator = StartupCoordinator::new();
        for stage in [
            StartupState::HostValidated,
            StartupState::CudaContextsReady,
            StartupState::TopologyValidated,
            StartupState::ModulesReady,
            StartupState::MemoryPlanned,
        ] {
            coordinator.advance(reports(stage)).unwrap();
        }
        coordinator
            .advance_weights_loaded(
                reports(StartupState::WeightsLoaded),
                AdoptedRankSetReceipt::test_only([0x55; 32]),
            )
            .unwrap();
        assert_eq!(coordinator.state(), StartupState::WeightsLoaded);
    }

    #[test]
    fn adoption_digest_cannot_appear_early_or_change_later() {
        let mut coordinator = StartupCoordinator::new();
        let mut early = reports(StartupState::HostValidated);
        early[2].as_mut().unwrap().adopted_rank_set_hash = [0x55; 32];
        assert_eq!(
            coordinator.advance(early),
            Err(StartupError::AdoptionAgreement)
        );

        let mut coordinator = StartupCoordinator::new();
        for stage in [
            StartupState::HostValidated,
            StartupState::CudaContextsReady,
            StartupState::TopologyValidated,
            StartupState::ModulesReady,
            StartupState::MemoryPlanned,
        ] {
            coordinator.advance(reports(stage)).unwrap();
        }
        coordinator
            .advance_weights_loaded(
                reports(StartupState::WeightsLoaded),
                AdoptedRankSetReceipt::test_only([0x55; 32]),
            )
            .unwrap();
        let mut changed = reports(StartupState::GraphsCaptured);
        changed[1].as_mut().unwrap().adopted_rank_set_hash = [0x56; 32];
        assert_eq!(
            coordinator.advance(changed),
            Err(StartupError::AdoptionAgreement)
        );
    }

    #[test]
    fn rank_local_collective_route_is_forbidden() {
        assert_eq!(
            run_mock_startup(MockFault::DivergentCollective { rank: 3 }),
            Err(StartupError::DigestAgreement)
        );
    }

    #[test]
    fn a_failed_coordinator_never_recovers() {
        let mut coordinator = StartupCoordinator::new();
        assert_eq!(
            coordinator.advance(Vec::new()),
            Err(StartupError::RankCount)
        );
        assert_eq!(coordinator.state(), StartupState::Failed);
        assert_eq!(
            coordinator.advance(Vec::new()),
            Err(StartupError::AlreadyFailed)
        );
    }
}
