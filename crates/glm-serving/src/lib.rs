//! Bounded, CPU-testable serving coordinator for the fixed TP4 execution
//! contract. Persistent device rank executors plug into `Tp4WorkerPool`
//! without changing scheduler, event, or collective-order semantics.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
};

use glm_cache::PrefixPageKey;
use glm_engine::{GraphProfile, Tp4WorkerPool, WorkerError};
#[cfg(test)]
use glm_scheduler::SamplingCollective;
use glm_scheduler::{
    BatchCompletion, BatchKind, RequestProgress, RequestSpec, RequestState, RouteCatalog,
    Scheduler, SchedulerConfig, SchedulerError, StepPlanCompiler, TenantConfig,
};
use glm_tokenizer::EOS_TOKEN_IDS;

const MAXIMUM_STEP_EVENTS: usize = 512;

mod cache;
mod http;

pub use cache::{PrefixRestoreCoordinator, PrefixRestoreError, RestoredPrefix};
pub use http::{
    ApiBackend, ApiBackendError, ApiCompletionEvent, ApiCompletionHandle, ApiErrorBody, ApiHealth,
    ApiHealthState, ApiHttpServer, ApiServerConfig, ApiUsage, ChatCompletionRequest, ChatMessage,
    SamplingParameters, StopSequences, ValidatedChatRequest,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServingConfig {
    pub epoch: u64,
    pub event_capacity: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServingRequest {
    pub spec: RequestSpec,
    pub cached_prompt_tokens: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestFinishReason {
    Stop,
    Length,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestEvent {
    Admitted {
        request_id: u64,
        cached_prompt_tokens: u32,
    },
    PrefillProgress {
        request_id: u64,
        prompt_done: u32,
        prompt_tokens: u32,
    },
    Token {
        request_id: u64,
        position: u32,
        token_id: u32,
        speculative: bool,
    },
    Finished {
        request_id: u64,
        reason: RequestFinishReason,
    },
    Cancelled {
        request_id: u64,
    },
    Failed {
        request_id: u64,
    },
}

pub struct ServingCoordinator {
    scheduler: Scheduler,
    compiler: StepPlanCompiler,
    workers: Tp4WorkerPool,
    sequence_table_generation: u64,
    event_capacity: usize,
    events: VecDeque<RequestEvent>,
    terminal_events: BTreeSet<u64>,
    prefix_cache: Option<PrefixRestoreCoordinator>,
    prefix_leases: BTreeMap<u64, Vec<PrefixPageKey>>,
}

impl ServingCoordinator {
    pub fn new(
        config: ServingConfig,
        scheduler_config: SchedulerConfig,
        profile: GraphProfile,
        tenants: Vec<TenantConfig>,
        routes: RouteCatalog,
        workers: Tp4WorkerPool,
    ) -> Result<Self, ServingError> {
        if config.event_capacity < MAXIMUM_STEP_EVENTS {
            return Err(ServingError::Config);
        }
        Ok(Self {
            scheduler: Scheduler::new(scheduler_config, profile, tenants)?,
            compiler: StepPlanCompiler::new(config.epoch, routes)?,
            workers,
            sequence_table_generation: 1,
            event_capacity: config.event_capacity,
            events: VecDeque::new(),
            terminal_events: BTreeSet::new(),
            prefix_cache: None,
            prefix_leases: BTreeMap::new(),
        })
    }

    pub fn attach_prefix_cache(
        &mut self,
        cache: PrefixRestoreCoordinator,
    ) -> Result<(), ServingError> {
        if self.prefix_cache.is_some() || !self.scheduler.request_ids().is_empty() {
            return Err(ServingError::Config);
        }
        self.prefix_cache = Some(cache);
        Ok(())
    }

    /// Admission for a cache result already proved by an embedding caller.
    /// Normal serving should use `admit_tokens`, which derives and restores
    /// prefix pages inside this coordinator.
    pub fn admit_prevalidated(&mut self, request: ServingRequest) -> Result<(), ServingError> {
        self.require_event_space(1)?;
        self.scheduler
            .admit_with_prefix(request.spec, request.cached_prompt_tokens)?;
        self.bump_sequence_generation()?;
        self.events.push_back(RequestEvent::Admitted {
            request_id: request.spec.id,
            cached_prompt_tokens: request.cached_prompt_tokens,
        });
        Ok(())
    }

    pub fn admit_tokens(&mut self, spec: RequestSpec, tokens: &[u32]) -> Result<(), ServingError> {
        if usize::try_from(spec.prompt_tokens).ok() != Some(tokens.len()) {
            return Err(ServingError::Request);
        }
        let restored = self
            .prefix_cache
            .as_mut()
            .ok_or(ServingError::CacheUnavailable)?
            .restore_longest(spec.id, tokens)?;
        let page_keys = restored.page_keys;
        let result = self.admit_prevalidated(ServingRequest {
            spec,
            cached_prompt_tokens: restored.matched_tokens,
        });
        if let Err(error) = result {
            self.prefix_cache
                .as_mut()
                .ok_or(ServingError::CacheUnavailable)?
                .release(&page_keys)?;
            return Err(error);
        }
        self.prefix_leases.insert(spec.id, page_keys);
        Ok(())
    }

    pub fn cancel(&mut self, request_id: u64) -> Result<(), ServingError> {
        self.scheduler.cancel(request_id)?;
        self.bump_sequence_generation()
    }

    /// Executes at most one collective-safe scheduler iteration. Events must
    /// be drained by the API layer; if their bounded queue lacks room, no new
    /// batch is selected.
    pub fn tick(&mut self) -> Result<bool, ServingError> {
        self.require_event_space(MAXIMUM_STEP_EVENTS)?;
        let Some(batch) = self.scheduler.next_batch()? else {
            self.emit_terminal_transitions()?;
            return Ok(false);
        };
        let entry = self
            .scheduler
            .graph_entry(batch.graph_id)
            .cloned()
            .ok_or(ServingError::Graph)?;
        let compiled = self
            .compiler
            .compile(&batch, &entry, self.sequence_table_generation)?;
        let starting_progress: Vec<_> = batch
            .rows
            .iter()
            .map(|row| {
                self.scheduler
                    .request_progress(row.request_id)
                    .map(|progress| (row.request_id, progress))
                    .ok_or(ServingError::Request)
            })
            .collect::<Result<_, _>>()?;
        let outcome = match self
            .workers
            .try_submit(compiled.plan, compiled.schedule)?
            .receive()
        {
            Ok(outcome) => outcome,
            Err(error) => {
                self.scheduler.complete_batch(false)?;
                self.emit_failed_rows(&batch)?;
                return Err(ServingError::Worker(error));
            }
        };
        let output_rows = outcome.output.sequences();
        let completions: Vec<_> = starting_progress
            .iter()
            .zip(output_rows)
            .map(|((request_id, _), output)| BatchCompletion {
                request_id: *request_id,
                committed_tokens: output.count(),
                terminal: output
                    .token_ids()
                    .last()
                    .is_some_and(|token_id| EOS_TOKEN_IDS.contains(token_id)),
            })
            .collect();
        let output_fits_requests = match batch.kind {
            BatchKind::Prefill => output_rows.is_empty(),
            BatchKind::Decode | BatchKind::Verify { .. } => {
                output_rows.len() == starting_progress.len()
                    && starting_progress
                        .iter()
                        .zip(output_rows)
                        .all(|((_, progress), output)| {
                            u32::from(output.count())
                                <= progress.maximum_new_tokens - progress.generated
                                && output_has_valid_termination(output)
                        })
            }
        };
        if !output_fits_requests {
            self.scheduler.complete_batch(false)?;
            self.emit_failed_rows(&batch)?;
            return Err(ServingError::Output);
        }
        self.scheduler
            .complete_batch_with_results(true, &completions)?;
        for (row_index, row) in batch.rows.iter().enumerate() {
            let starting = starting_progress
                .get(row_index)
                .map(|(_, progress)| *progress)
                .ok_or(ServingError::Request)?;
            let progress = self
                .scheduler
                .request_progress(row.request_id)
                .ok_or(ServingError::Request)?;
            match batch.kind {
                BatchKind::Prefill => self.events.push_back(RequestEvent::PrefillProgress {
                    request_id: row.request_id,
                    prompt_done: progress.prompt_done,
                    prompt_tokens: progress.prompt_tokens,
                }),
                BatchKind::Decode | BatchKind::Verify { .. } => {
                    let output = output_rows.get(row_index).ok_or(ServingError::Output)?;
                    for (offset, &token_id) in output.token_ids().iter().enumerate() {
                        let offset = u32::try_from(offset).map_err(|_| ServingError::Overflow)?;
                        let position = starting
                            .generated
                            .checked_add(offset)
                            .ok_or(ServingError::Overflow)?;
                        self.events.push_back(RequestEvent::Token {
                            request_id: row.request_id,
                            position,
                            token_id,
                            // A verify result commits N accepted draft tokens
                            // followed by an optional target/residual/bonus
                            // token. Accepted EOS is the draft-only case.
                            speculative: matches!(batch.kind, BatchKind::Verify { .. })
                                && offset < u32::from(output.accepted_draft_count()),
                        });
                    }
                }
            }
            if progress.state == RequestState::Finished {
                let stopped = output_rows
                    .get(row_index)
                    .and_then(|output| output.token_ids().last())
                    .is_some_and(|token_id| EOS_TOKEN_IDS.contains(token_id));
                self.release_request_prefix(row.request_id)?;
                self.events.push_back(RequestEvent::Finished {
                    request_id: row.request_id,
                    reason: if stopped {
                        RequestFinishReason::Stop
                    } else {
                        RequestFinishReason::Length
                    },
                });
            }
        }
        Ok(true)
    }

    pub fn run_until_idle(&mut self, maximum_steps: u64) -> Result<u64, ServingError> {
        let mut completed = 0_u64;
        while completed < maximum_steps {
            if !self.tick()? {
                return Ok(completed);
            }
            completed = completed.checked_add(1).ok_or(ServingError::Overflow)?;
            // A real API drains on a different task. The reference driver
            // must explicitly call `drain_events`; it never silently drops.
            if self.event_capacity - self.events.len() < MAXIMUM_STEP_EVENTS {
                return Err(ServingError::Backpressure);
            }
        }
        Err(ServingError::StepLimit)
    }

    #[must_use]
    pub fn drain_events(&mut self) -> Vec<RequestEvent> {
        self.events.drain(..).collect()
    }

    #[must_use]
    pub fn request_progress(&self, request_id: u64) -> Option<RequestProgress> {
        self.scheduler.request_progress(request_id)
    }

    fn emit_terminal_transitions(&mut self) -> Result<(), ServingError> {
        // Cancellations are made visible by Scheduler::next_batch before it
        // reports idle. Keep a compact terminal marker so draining the event
        // queue cannot cause duplicate cancellation events.
        let cancelled: Vec<_> = self
            .scheduler
            .request_ids()
            .into_iter()
            .filter(|&id| self.scheduler.request_state(id) == Some(RequestState::Cancelled))
            .filter(|id| !self.terminal_events.contains(id))
            .collect();
        self.require_event_space(cancelled.len())?;
        for request_id in cancelled {
            self.release_request_prefix(request_id)?;
            self.events
                .push_back(RequestEvent::Cancelled { request_id });
            self.terminal_events.insert(request_id);
        }
        Ok(())
    }

    fn emit_failed_rows(
        &mut self,
        batch: &glm_scheduler::ScheduledBatch,
    ) -> Result<(), ServingError> {
        self.require_event_space(batch.rows.len())?;
        for row in &batch.rows {
            self.release_request_prefix(row.request_id)?;
            self.events.push_back(RequestEvent::Failed {
                request_id: row.request_id,
            });
        }
        Ok(())
    }

    fn require_event_space(&self, needed: usize) -> Result<(), ServingError> {
        if self
            .events
            .len()
            .checked_add(needed)
            .is_none_or(|total| total > self.event_capacity)
        {
            return Err(ServingError::Backpressure);
        }
        Ok(())
    }

    fn bump_sequence_generation(&mut self) -> Result<(), ServingError> {
        self.sequence_table_generation = self
            .sequence_table_generation
            .checked_add(1)
            .ok_or(ServingError::Overflow)?;
        Ok(())
    }

    fn release_request_prefix(&mut self, request_id: u64) -> Result<(), ServingError> {
        let Some(page_keys) = self.prefix_leases.remove(&request_id) else {
            return Ok(());
        };
        self.prefix_cache
            .as_mut()
            .ok_or(ServingError::CacheUnavailable)?
            .release(&page_keys)?;
        Ok(())
    }
}

fn output_has_valid_termination(output: &glm_engine::CommittedTokens) -> bool {
    let token_ids = output.token_ids();
    let first_eos = token_ids
        .iter()
        .position(|token_id| EOS_TOKEN_IDS.contains(token_id));
    if first_eos.is_some_and(|position| position + 1 != token_ids.len()) {
        return false;
    }
    output.target_token_present()
        || token_ids
            .last()
            .is_some_and(|token_id| EOS_TOKEN_IDS.contains(token_id))
}

#[derive(Debug)]
pub enum ServingError {
    Config,
    Backpressure,
    Graph,
    Request,
    Output,
    Overflow,
    StepLimit,
    CacheUnavailable,
    Cache(PrefixRestoreError),
    Scheduler(SchedulerError),
    Compile(glm_scheduler::CompileError),
    Worker(WorkerError),
}

impl fmt::Display for ServingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ServingError {}

impl From<SchedulerError> for ServingError {
    fn from(value: SchedulerError) -> Self {
        Self::Scheduler(value)
    }
}

impl From<glm_scheduler::CompileError> for ServingError {
    fn from(value: glm_scheduler::CompileError) -> Self {
        Self::Compile(value)
    }
}

impl From<WorkerError> for ServingError {
    fn from(value: WorkerError) -> Self {
        Self::Worker(value)
    }
}

impl From<PrefixRestoreError> for ServingError {
    fn from(value: PrefixRestoreError) -> Self {
        Self::Cache(value)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use glm_cache::{
        DurablePageRequest, FileTierStore, NamespaceInputs, PagePieceBytes, PrefixIndex,
        PrefixNamespace, ResidencyConfig, TierPiece,
    };
    use glm_engine::{
        AttentionTransport, CollectiveSchedule, CommittedTokens, GraphEntry, GraphKey,
        MockWorkerFault, RankExecutionError, RankExecutor, StepMode, StepOutput, StepPlan,
    };

    use super::*;

    fn routes() -> RouteCatalog {
        RouteCatalog {
            tp_route_id: 1,
            dcp_ckv_route_id: 2,
            dcp_query_route_id: 3,
            dcp_candidate_route_id: 4,
            dcp_partial_route_id: 5,
            greedy_route_id: 6,
            top_k_route_id: 7,
            mass_route_id: 8,
            packed_ckv_bytes_per_row: 32,
            query_bytes_per_row: 32,
            candidate_bytes_per_row: 32,
            partial_state_bytes_per_row: 32,
            tp_reduce_bytes_per_row: 32,
            greedy_bytes_per_row: 8,
            top_k_bytes_per_row: 64,
            mass_bytes_per_row: 16,
        }
    }

    fn entry(graph_id: u32, mode: StepMode, sequences: u16, rows: u32, depth: u8) -> GraphEntry {
        GraphEntry {
            graph_id,
            key: GraphKey {
                mode,
                sequence_bucket: sequences,
                verifier_row_bucket: if mode == StepMode::Prefill { 0 } else { rows },
                mtp_depth: depth,
                attention_transport: if mode == StepMode::Prefill {
                    AttentionTransport::PrefillQuery
                } else {
                    AttentionTransport::DecodeQueryLse
                },
            },
            maximum_active_sequences: sequences,
            maximum_prompt_tokens: if mode == StepMode::Prefill { rows } else { 0 },
            maximum_query_rows: rows,
            compatible_tp_routes: vec![1],
            compatible_dcp_routes: vec![3, 4, 5],
            compatible_sampling_routes: if mode == StepMode::Prefill {
                vec![]
            } else {
                vec![6, 7, 8]
            },
            maximum_scratch_bytes: 1,
            argument_bytes: 1,
            graph_object_bytes: 1,
            resident_module_bytes: 1,
            admission_slo_class: 1,
        }
    }

    fn coordinator(fault: Option<MockWorkerFault>) -> ServingCoordinator {
        coordinator_with_workers(Tp4WorkerPool::spawn_cpu(2, fault).unwrap())
    }

    fn coordinator_with_workers(workers: Tp4WorkerPool) -> ServingCoordinator {
        let profile = GraphProfile::new(vec![
            entry(1, StepMode::Prefill, 4, 64, 0),
            entry(2, StepMode::Decode, 4, 4, 0),
            entry(3, StepMode::Verify, 4, 28, 6),
        ])
        .unwrap();
        ServingCoordinator::new(
            ServingConfig {
                epoch: 1,
                event_capacity: 1024,
            },
            SchedulerConfig {
                maximum_batch_sequences: 4,
                maximum_prefill_tokens: 64,
                maximum_decode_burst: 2,
            },
            profile,
            vec![
                TenantConfig {
                    tenant: 1,
                    weight: 1,
                    maximum_active_requests: 4,
                },
                TenantConfig {
                    tenant: 2,
                    weight: 2,
                    maximum_active_requests: 4,
                },
            ],
            routes(),
            workers,
        )
        .unwrap()
    }

    struct FixedMtpRankExecutor;
    struct AcceptedDraftEosRankExecutor;

    impl RankExecutor for FixedMtpRankExecutor {
        fn execute(
            &mut self,
            _rank: u8,
            plan: &StepPlan,
            _schedule: &CollectiveSchedule,
        ) -> Result<StepOutput, RankExecutionError> {
            if plan.mode == StepMode::Prefill {
                return Ok(StepOutput::empty());
            }
            let sequence = if plan.mode == StepMode::Verify {
                CommittedTokens::verify(&[41], Some(42))
            } else {
                CommittedTokens::target(43)
            }
            .map_err(|_| RankExecutionError::Invariant)?;
            StepOutput::new(&vec![sequence; usize::from(plan.active_sequences)])
                .map_err(|_| RankExecutionError::Invariant)
        }
    }

    impl RankExecutor for AcceptedDraftEosRankExecutor {
        fn execute(
            &mut self,
            _rank: u8,
            plan: &StepPlan,
            _schedule: &CollectiveSchedule,
        ) -> Result<StepOutput, RankExecutionError> {
            if plan.mode == StepMode::Prefill {
                return Ok(StepOutput::empty());
            }
            let sequence = if plan.mode == StepMode::Verify {
                CommittedTokens::verify(&[41, EOS_TOKEN_IDS[0]], None)
            } else {
                CommittedTokens::target(EOS_TOKEN_IDS[0])
            }
            .map_err(|_| RankExecutionError::Invariant)?;
            StepOutput::new(&vec![sequence; usize::from(plan.active_sequences)])
                .map_err(|_| RankExecutionError::Invariant)
        }
    }

    fn fixed_mtp_workers() -> Tp4WorkerPool {
        let executors =
            std::array::from_fn(|_| Box::new(FixedMtpRankExecutor) as Box<dyn RankExecutor>);
        Tp4WorkerPool::spawn(2, executors).unwrap()
    }

    fn accepted_draft_eos_workers() -> Tp4WorkerPool {
        let executors = std::array::from_fn(|_| {
            Box::new(AcceptedDraftEosRankExecutor) as Box<dyn RankExecutor>
        });
        Tp4WorkerPool::spawn(2, executors).unwrap()
    }

    fn temporary_store(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("glmaxx-{name}-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn multi_user_prefix_mtp_and_streaming_lifecycle_runs_end_to_end() {
        let mut serving = coordinator_with_workers(fixed_mtp_workers());
        serving
            .admit_prevalidated(ServingRequest {
                spec: RequestSpec {
                    id: 10,
                    tenant: 1,
                    prompt_tokens: 128,
                    maximum_new_tokens: 2,
                    mtp_depth: 0,
                    sampling: SamplingCollective::Greedy,
                },
                cached_prompt_tokens: 64,
            })
            .unwrap();
        serving
            .admit_prevalidated(ServingRequest {
                spec: RequestSpec {
                    id: 20,
                    tenant: 2,
                    prompt_tokens: 64,
                    maximum_new_tokens: 2,
                    mtp_depth: 6,
                    sampling: SamplingCollective::Greedy,
                },
                cached_prompt_tokens: 64,
            })
            .unwrap();
        let mut events = serving.drain_events();
        for _ in 0..16 {
            if !serving.tick().unwrap() {
                break;
            }
            events.extend(serving.drain_events());
        }
        assert_eq!(
            serving.request_progress(10).unwrap().state,
            RequestState::Finished
        );
        assert_eq!(
            serving.request_progress(20).unwrap().state,
            RequestState::Finished
        );
        assert!(events.contains(&RequestEvent::PrefillProgress {
            request_id: 10,
            prompt_done: 128,
            prompt_tokens: 128,
        }));
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, RequestEvent::Token { .. }))
                .count(),
            4
        );
        assert!(events.iter().any(|event| matches!(
            event,
            RequestEvent::Token {
                request_id: 20,
                speculative: true,
                ..
            }
        )));
    }

    #[test]
    fn cancellation_and_event_backpressure_are_step_boundary_safe() {
        let mut serving = coordinator(None);
        serving
            .admit_prevalidated(ServingRequest {
                spec: RequestSpec {
                    id: 10,
                    tenant: 1,
                    prompt_tokens: 64,
                    maximum_new_tokens: 2,
                    mtp_depth: 0,
                    sampling: SamplingCollective::Greedy,
                },
                cached_prompt_tokens: 0,
            })
            .unwrap();
        serving.cancel(10).unwrap();
        let _ = serving.drain_events();
        assert!(!serving.tick().unwrap());
        assert_eq!(
            serving.drain_events(),
            vec![RequestEvent::Cancelled { request_id: 10 }]
        );
    }

    #[test]
    fn rank_divergence_fails_every_row_in_the_selected_batch() {
        let mut serving = coordinator(Some(MockWorkerFault::DivergentOutput {
            rank: 3,
            step_id: 1,
        }));
        serving
            .admit_prevalidated(ServingRequest {
                spec: RequestSpec {
                    id: 10,
                    tenant: 1,
                    prompt_tokens: 64,
                    maximum_new_tokens: 1,
                    mtp_depth: 0,
                    sampling: SamplingCollective::Greedy,
                },
                cached_prompt_tokens: 64,
            })
            .unwrap();
        let _ = serving.drain_events();
        assert!(matches!(
            serving.tick(),
            Err(ServingError::Worker(WorkerError::Consensus))
        ));
        assert_eq!(
            serving.drain_events(),
            vec![RequestEvent::Failed { request_id: 10 }]
        );
    }

    #[test]
    fn backend_cannot_commit_past_a_request_generation_limit() {
        let mut serving = coordinator_with_workers(fixed_mtp_workers());
        serving
            .admit_prevalidated(ServingRequest {
                spec: RequestSpec {
                    id: 10,
                    tenant: 1,
                    prompt_tokens: 64,
                    maximum_new_tokens: 1,
                    mtp_depth: 6,
                    sampling: SamplingCollective::Greedy,
                },
                cached_prompt_tokens: 64,
            })
            .unwrap();
        let _ = serving.drain_events();
        assert!(matches!(serving.tick(), Err(ServingError::Output)));
        assert_eq!(
            serving.drain_events(),
            vec![RequestEvent::Failed { request_id: 10 }]
        );
    }

    #[test]
    fn accepted_draft_eos_finishes_early_without_a_fake_target_token() {
        let mut serving = coordinator_with_workers(accepted_draft_eos_workers());
        serving
            .admit_prevalidated(ServingRequest {
                spec: RequestSpec {
                    id: 10,
                    tenant: 1,
                    prompt_tokens: 64,
                    maximum_new_tokens: 10,
                    mtp_depth: 6,
                    sampling: SamplingCollective::Greedy,
                },
                cached_prompt_tokens: 64,
            })
            .unwrap();
        let _ = serving.drain_events();
        assert!(serving.tick().unwrap());
        assert_eq!(
            serving.request_progress(10),
            Some(RequestProgress {
                state: RequestState::Finished,
                prompt_tokens: 64,
                prompt_done: 64,
                maximum_new_tokens: 10,
                generated: 2,
                mtp_depth: 6,
            })
        );
        assert_eq!(
            serving.drain_events(),
            vec![
                RequestEvent::Token {
                    request_id: 10,
                    position: 0,
                    token_id: 41,
                    speculative: true,
                },
                RequestEvent::Token {
                    request_id: 10,
                    position: 1,
                    token_id: EOS_TOKEN_IDS[0],
                    speculative: true,
                },
                RequestEvent::Finished {
                    request_id: 10,
                    reason: RequestFinishReason::Stop,
                },
            ]
        );
    }

    #[test]
    fn prefix_admission_restores_real_durable_bytes_before_skipping_prefill() {
        let root = temporary_store("serving-prefix");
        let namespace = PrefixNamespace::new(NamespaceInputs {
            model_revision_sha256: [1; 32],
            tokenizer_sha256: [2; 32],
            chat_template_sha256: [3; 32],
            weight_policy_hash: [4; 32],
            target_kv_abi_sha256: [5; 32],
            draft_kv_abi_sha256: [6; 32],
            rope_parameters_sha256: [7; 32],
        })
        .unwrap();
        let tokens: Vec<u32> = (0..64).collect();
        let index = PrefixIndex::new(namespace);
        let key = index.derive_keys(&tokens)[0];
        let mut store = FileTierStore::open(&root).unwrap();
        let record = store
            .publish(DurablePageRequest {
                namespace: namespace.0,
                page_key: key.0,
                generation: 1,
                mtp: false,
                pieces: [TierPiece::TargetKv, TierPiece::TargetIndexer]
                    .into_iter()
                    .map(|piece| PagePieceBytes {
                        piece,
                        bytes: vec![piece as u8; piece.expected_bytes() as usize],
                    })
                    .collect(),
            })
            .unwrap();
        let page_bytes = record.pieces.iter().map(|piece| piece.byte_length).sum();
        drop(store);

        let mut prefix = PrefixRestoreCoordinator::new(
            index,
            &root,
            ResidencyConfig {
                hbm_bytes: page_bytes,
                dram_bytes: page_bytes,
            },
            2,
        )
        .unwrap();
        prefix.register_prefix(&tokens, vec![record]).unwrap();
        let mut serving = coordinator(None);
        serving.attach_prefix_cache(prefix).unwrap();
        serving
            .admit_tokens(
                RequestSpec {
                    id: 77,
                    tenant: 1,
                    prompt_tokens: 64,
                    maximum_new_tokens: 1,
                    mtp_depth: 0,
                    sampling: SamplingCollective::Greedy,
                },
                &tokens,
            )
            .unwrap();
        assert_eq!(
            serving.drain_events(),
            vec![RequestEvent::Admitted {
                request_id: 77,
                cached_prompt_tokens: 64,
            }]
        );
        assert!(serving.tick().unwrap());
        let events = serving.drain_events();
        assert!(
            events
                .iter()
                .all(|event| !matches!(event, RequestEvent::PrefillProgress { .. }))
        );
        assert!(events.contains(&RequestEvent::Finished {
            request_id: 77,
            reason: RequestFinishReason::Length,
        }));
        drop(serving);
        fs::remove_dir_all(root).unwrap();
    }
}
