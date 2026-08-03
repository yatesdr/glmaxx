//! Bounded, CPU-testable serving coordinator for the fixed TP4 execution
//! contract. Persistent device rank executors plug into `Tp4WorkerPool`
//! without changing scheduler, event, or collective-order semantics.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use glm_cache::{
    PageTableConfig, PageTableDelta, PageTableDeltaError, PageTableStats, PrefixPageAttachment,
    SequencePageError, SequencePageTable,
};
use glm_engine::{
    CollectiveKind, CollectiveSchedule, CommittedTokens, GraphProfile, MAX_ACTIVE_SEQUENCES,
    MAX_MTP_DEPTH, SequenceStepInput, StepInput, StepInputError, StepMode, StepSampling,
    StepSamplingKind, Tp4WorkerPool, WorkerError, WorkerExecutionPosture,
};
use glm_scheduler::{
    BatchCompletion, BatchKind, RequestProgress, RequestSpec, RequestState, RouteCatalog,
    SamplingCollective, ScheduledBatch, Scheduler, SchedulerConfig, SchedulerError,
    StepPlanCompiler, TenantConfig,
};
use glm_tokenizer::EOS_TOKEN_IDS;

const MAXIMUM_STEP_EVENTS: usize = MAX_ACTIVE_SEQUENCES as usize * (MAX_MTP_DEPTH as usize + 2);

mod backend;
mod cache;
mod http;
mod metrics;

use cache::PrefixReleasePlan;

pub use backend::{CoordinatorApiBackend, CoordinatorBackendConfig, CoordinatorBackendError};
pub use cache::{
    PrefixRestoreCoordinator, PrefixRestoreError, PrefixRestoreStatus, RestoredPrefix,
};
pub use http::{
    ApiBackend, ApiBackendError, ApiCompletionEvent, ApiCompletionHandle, ApiErrorBody, ApiHealth,
    ApiHealthState, ApiHttpServer, ApiServerConfig, ApiUsage, ChatCompletionRequest, ChatMessage,
    GLMAXX_MODEL_REVISION, SamplingParameters, StopSequences, ValidatedChatRequest,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServingConfig {
    pub epoch: u64,
    pub event_capacity: usize,
    pub maximum_retained_prompt_bytes: u64,
    pub page_table: PageTableConfig,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServingRequest {
    pub spec: RequestSpec,
    pub cached_prompt_tokens: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionStatus {
    Pending,
    Admitted { cached_prompt_tokens: u32 },
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
        draft_ordinal: Option<u8>,
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CollectivePayloadObservation {
    pub tp_reduce_bytes: u64,
    pub dcp_packed_ckv_bytes: u64,
    pub dcp_query_gather_bytes: u64,
    pub dcp_candidate_exchange_bytes: u64,
    pub dcp_partial_state_return_bytes: u64,
    pub sampling_bytes: u64,
    pub tp_route_id: u16,
    pub dcp_packed_ckv_route_id: u16,
    pub dcp_query_route_id: u16,
    pub dcp_candidate_route_id: u16,
    pub dcp_partial_state_route_id: u16,
    pub sampling_route_id: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServingStepObservation {
    pub step_id: u64,
    pub mode: StepMode,
    pub graph_id: u32,
    pub real_sequences: u16,
    pub bucket_sequences: u16,
    pub real_query_rows: u32,
    pub bucket_query_rows: u32,
    pub scheduled_prompt_tokens: u32,
    pub mtp_depth: u8,
    pub collective_count: u16,
    pub collective_schedule_hash: [u8; 32],
    pub collectives: CollectivePayloadObservation,
    pub worker_round_trip: Duration,
    pub coordinator_overhead: Duration,
    pub total_step_time: Duration,
}

pub struct ServingCoordinator {
    scheduler: Scheduler,
    compiler: StepPlanCompiler,
    workers: Tp4WorkerPool,
    sequence_table_generation: u64,
    active_pages: SequencePageTable,
    event_capacity: usize,
    maximum_retained_prompt_bytes: u64,
    retained_prompt_bytes: u64,
    events: VecDeque<RequestEvent>,
    terminal_events: BTreeSet<u64>,
    prefix_cache: Option<PrefixRestoreCoordinator>,
    prefix_leases: BTreeMap<u64, RestoredPrefix>,
    pending_admissions: BTreeMap<u64, PendingAdmission>,
    request_tokens: BTreeMap<u64, Box<[u32]>>,
    request_sampling: BTreeMap<u64, StepSampling>,
}

struct PendingAdmission {
    spec: RequestSpec,
    tokens: Box<[u32]>,
    sampling: StepSampling,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RequestReleaseMode {
    Tokens,
    Prefix,
}

struct RequestReleasePlan {
    requests: BTreeMap<u64, RequestReleaseMode>,
    retained_prompt_bytes: u64,
    prefix: Option<PrefixReleasePlan>,
    active_pages: SequencePageTable,
    sequence_table_generation: u64,
    rank_state: RankReleaseState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RankReleaseState {
    Pending,
    Acknowledged,
    WorkerRetired,
}

struct SuccessfulStepPublication {
    events: StagedEvents,
    releases: RequestReleasePlan,
}

struct StagedEvents {
    entries: [Option<RequestEvent>; MAXIMUM_STEP_EVENTS],
    len: usize,
}

impl StagedEvents {
    fn new() -> Self {
        Self {
            entries: [None; MAXIMUM_STEP_EVENTS],
            len: 0,
        }
    }

    fn push(&mut self, event: RequestEvent) -> Result<(), ServingError> {
        let slot = self
            .entries
            .get_mut(self.len)
            .ok_or(ServingError::Overflow)?;
        *slot = Some(event);
        self.len += 1;
        Ok(())
    }
}

struct StagedRequestReleases {
    entries: [Option<(u64, RequestReleaseMode)>; MAX_ACTIVE_SEQUENCES as usize],
    len: usize,
}

impl StagedRequestReleases {
    fn new() -> Self {
        Self {
            entries: [None; MAX_ACTIVE_SEQUENCES as usize],
            len: 0,
        }
    }

    fn push(&mut self, release: (u64, RequestReleaseMode)) -> Result<(), ServingError> {
        let slot = self
            .entries
            .get_mut(self.len)
            .ok_or(ServingError::Overflow)?;
        *slot = Some(release);
        self.len += 1;
        Ok(())
    }

    fn iter(&self) -> impl Iterator<Item = (u64, RequestReleaseMode)> + '_ {
        self.entries[..self.len].iter().flatten().copied()
    }
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
        if config.event_capacity < MAXIMUM_STEP_EVENTS
            || config.maximum_retained_prompt_bytes < u64::from(u32::BITS / 8)
        {
            return Err(ServingError::Config);
        }
        let active_pages = SequencePageTable::new(config.page_table)?;
        workers.initialize_page_table(Arc::new(active_pages.clone()), 1)?;
        Ok(Self {
            scheduler: Scheduler::new(scheduler_config, profile, tenants)?,
            compiler: StepPlanCompiler::new(config.epoch, routes)?,
            workers,
            sequence_table_generation: 1,
            active_pages,
            event_capacity: config.event_capacity,
            maximum_retained_prompt_bytes: config.maximum_retained_prompt_bytes,
            retained_prompt_bytes: 0,
            events: VecDeque::new(),
            terminal_events: BTreeSet::new(),
            prefix_cache: None,
            prefix_leases: BTreeMap::new(),
            pending_admissions: BTreeMap::new(),
            request_tokens: BTreeMap::new(),
            request_sampling: BTreeMap::new(),
        })
    }

    /// Returns the engine-owned execution posture of the four rank workers.
    /// Public serving must never infer model readiness from startup health
    /// alone because the coordinator and startup receipt are separate values.
    #[must_use]
    pub const fn execution_posture(&self) -> WorkerExecutionPosture {
        self.workers.execution_posture()
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

    /// Test/internal admission path without a prefix lease. Production callers
    /// must use `admit_tokens`, which derives and restores exact prefix
    /// attachments inside this coordinator.
    #[cfg(test)]
    pub(crate) fn admit_prevalidated(
        &mut self,
        request: ServingRequest,
    ) -> Result<(), ServingError> {
        self.admit_active_sequence(
            request.spec,
            request.cached_prompt_tokens,
            &[],
            request.cached_prompt_tokens,
            default_step_sampling(request.spec),
        )
    }

    fn admit_active_sequence(
        &mut self,
        spec: RequestSpec,
        cached_prompt_tokens: u32,
        prefix_pages: &[PrefixPageAttachment],
        private_cached_tokens: u32,
        sampling: StepSampling,
    ) -> Result<(), ServingError> {
        validate_context_limit(spec)?;
        sampling.validate()?;
        if sampling.kind != sampling_kind(spec.sampling) {
            return Err(ServingError::Request);
        }
        self.require_event_space(1)?;
        if self.pending_admissions.contains_key(&spec.id)
            || self.prefix_leases.contains_key(&spec.id)
            || self.request_tokens.contains_key(&spec.id)
            || self.request_sampling.contains_key(&spec.id)
        {
            return Err(ServingError::Backpressure);
        }
        let prefix_tokens = u32::try_from(prefix_pages.len())
            .ok()
            .and_then(|pages| pages.checked_mul(u32::try_from(glm_cache::PAGE_TOKENS).ok()?))
            .ok_or(ServingError::Overflow)?;
        if prefix_tokens
            .checked_add(private_cached_tokens)
            .is_none_or(|tokens| tokens != cached_prompt_tokens)
        {
            return Err(ServingError::Request);
        }
        let next_generation = self.next_sequence_generation()?;
        let mut active_pages = self.active_pages.clone();
        active_pages.admit_with_prefix(spec.id, spec.mtp_depth != 0, prefix_pages)?;
        active_pages.append_committed(spec.id, u64::from(private_cached_tokens))?;
        let delta = Arc::new(PageTableDelta::between(
            &self.active_pages,
            &active_pages,
            self.sequence_table_generation,
            next_generation,
        )?);
        let mut scheduler = self.scheduler.clone();
        scheduler.admit_with_prefix(spec, cached_prompt_tokens)?;
        self.workers.apply_page_delta(delta)?;
        self.scheduler = scheduler;
        self.active_pages = active_pages;
        self.sequence_table_generation = next_generation;
        self.request_sampling.insert(spec.id, sampling);
        self.events.push_back(RequestEvent::Admitted {
            request_id: spec.id,
            cached_prompt_tokens,
        });
        Ok(())
    }

    pub fn admit_tokens(&mut self, spec: RequestSpec, tokens: &[u32]) -> Result<(), ServingError> {
        self.admit_tokens_with_sampling(spec, default_step_sampling(spec), tokens)
    }

    pub fn admit_tokens_with_sampling(
        &mut self,
        spec: RequestSpec,
        sampling: StepSampling,
        tokens: &[u32],
    ) -> Result<(), ServingError> {
        match self.begin_admit_tokens_with_sampling(spec, sampling, tokens)? {
            AdmissionStatus::Admitted { .. } => return Ok(()),
            AdmissionStatus::Pending => {}
        }
        loop {
            match self.poll_admission(spec.id)? {
                AdmissionStatus::Admitted { .. } => return Ok(()),
                AdmissionStatus::Pending => thread::park_timeout(Duration::from_millis(1)),
            }
        }
    }

    pub fn begin_admit_tokens(
        &mut self,
        spec: RequestSpec,
        tokens: &[u32],
    ) -> Result<AdmissionStatus, ServingError> {
        self.begin_admit_tokens_with_sampling(spec, default_step_sampling(spec), tokens)
    }

    pub fn begin_admit_tokens_with_sampling(
        &mut self,
        spec: RequestSpec,
        sampling: StepSampling,
        tokens: &[u32],
    ) -> Result<AdmissionStatus, ServingError> {
        validate_context_limit(spec)?;
        sampling.validate()?;
        if sampling.kind != sampling_kind(spec.sampling) {
            return Err(ServingError::Request);
        }
        if usize::try_from(spec.prompt_tokens).ok() != Some(tokens.len()) {
            return Err(ServingError::Request);
        }
        if self.pending_admissions.contains_key(&spec.id)
            || self.scheduler.request_state(spec.id).is_some()
            || self.prefix_leases.contains_key(&spec.id)
            || self.request_tokens.contains_key(&spec.id)
            || self.request_sampling.contains_key(&spec.id)
            || self.pending_admissions.len() >= self.event_capacity
        {
            return Err(ServingError::Backpressure);
        }
        let prompt_bytes = prompt_bytes(tokens.len())?;
        let prior_retained_prompt_bytes = self.retained_prompt_bytes;
        let retained_prompt_bytes = self
            .retained_prompt_bytes
            .checked_add(prompt_bytes)
            .ok_or(ServingError::Overflow)?;
        if retained_prompt_bytes > self.maximum_retained_prompt_bytes {
            return Err(ServingError::Backpressure);
        }
        let status = self
            .prefix_cache
            .as_mut()
            .ok_or(ServingError::CacheUnavailable)?
            .begin_restore_longest_with_capability(spec.id, tokens, spec.mtp_depth != 0)?;
        self.retained_prompt_bytes = retained_prompt_bytes;
        match status {
            PrefixRestoreStatus::Pending => {
                self.pending_admissions.insert(
                    spec.id,
                    PendingAdmission {
                        spec,
                        tokens: tokens.to_vec().into_boxed_slice(),
                        sampling,
                    },
                );
                Ok(AdmissionStatus::Pending)
            }
            PrefixRestoreStatus::Ready(restored) => self.finish_token_admission(
                spec,
                tokens.to_vec().into_boxed_slice(),
                restored,
                prior_retained_prompt_bytes,
                sampling,
            ),
        }
    }

    pub fn poll_admission(&mut self, request_id: u64) -> Result<AdmissionStatus, ServingError> {
        if !self.pending_admissions.contains_key(&request_id) {
            return Err(ServingError::UnknownAdmission);
        }
        let retained_prompt_bytes = self.retained_prompt_bytes_after_release(
            self.pending_admissions
                .get(&request_id)
                .ok_or(ServingError::UnknownAdmission)?
                .tokens
                .len(),
        )?;
        let cache = self
            .prefix_cache
            .as_mut()
            .ok_or(ServingError::CacheUnavailable)?;
        let status = match cache.poll_restore(request_id) {
            Ok(status) => status,
            Err(error) => {
                if !cache.has_pending_restore(request_id)
                    && self.pending_admissions.remove(&request_id).is_some()
                {
                    self.retained_prompt_bytes = retained_prompt_bytes;
                }
                return Err(error.into());
            }
        };
        match status {
            PrefixRestoreStatus::Pending => Ok(AdmissionStatus::Pending),
            PrefixRestoreStatus::Ready(restored) => {
                let pending = self
                    .pending_admissions
                    .remove(&request_id)
                    .expect("pending admission was preflighted under exclusive serving access");
                self.finish_token_admission(
                    pending.spec,
                    pending.tokens,
                    restored,
                    retained_prompt_bytes,
                    pending.sampling,
                )
            }
        }
    }

    fn finish_token_admission(
        &mut self,
        spec: RequestSpec,
        tokens: Box<[u32]>,
        mut restored: RestoredPrefix,
        retained_prompt_bytes: u64,
        sampling: StepSampling,
    ) -> Result<AdmissionStatus, ServingError> {
        if let Err(error) = restored.validate() {
            self.prefix_cache
                .as_mut()
                .ok_or(ServingError::CacheUnavailable)?
                .release(restored.page_keys())?;
            self.retained_prompt_bytes = retained_prompt_bytes;
            return Err(error.into());
        }
        if spec.mtp_depth != 0
            && restored
                .page_attachments()
                .iter()
                .any(|attachment| !attachment.has_draft())
        {
            self.prefix_cache
                .as_mut()
                .ok_or(ServingError::CacheUnavailable)?
                .release(restored.page_keys())?;
            restored = RestoredPrefix::empty();
        }
        let matched_tokens = restored.matched_tokens;
        let result = self.admit_active_sequence(
            spec,
            matched_tokens,
            restored.page_attachments(),
            0,
            sampling,
        );
        if let Err(error) = result {
            self.prefix_cache
                .as_mut()
                .ok_or(ServingError::CacheUnavailable)?
                .release(restored.page_keys())?;
            self.retained_prompt_bytes = retained_prompt_bytes;
            return Err(error);
        }
        self.prefix_leases.insert(spec.id, restored);
        if matched_tokens == spec.prompt_tokens {
            self.retained_prompt_bytes = retained_prompt_bytes;
        } else {
            self.request_tokens.insert(spec.id, tokens);
        }
        Ok(AdmissionStatus::Admitted {
            cached_prompt_tokens: matched_tokens,
        })
    }

    pub fn cancel(&mut self, request_id: u64) -> Result<(), ServingError> {
        if self.pending_admissions.contains_key(&request_id) {
            self.require_event_space(1)?;
            let retained_prompt_bytes = self.retained_prompt_bytes_after_release(
                self.pending_admissions
                    .get(&request_id)
                    .ok_or(ServingError::UnknownAdmission)?
                    .tokens
                    .len(),
            )?;
            self.prefix_cache
                .as_mut()
                .ok_or(ServingError::CacheUnavailable)?
                .cancel_restore(request_id)?;
            self.pending_admissions
                .remove(&request_id)
                .expect("pending admission was preflighted under exclusive serving access");
            self.retained_prompt_bytes = retained_prompt_bytes;
            self.events
                .push_back(RequestEvent::Cancelled { request_id });
            self.terminal_events.insert(request_id);
            return Ok(());
        }
        self.next_sequence_generation()?;
        self.scheduler.cancel(request_id)?;
        Ok(())
    }

    /// Executes at most one collective-safe scheduler iteration. Events must
    /// be drained by the API layer; if their bounded queue lacks room, no new
    /// batch is selected.
    pub fn tick(&mut self) -> Result<bool, ServingError> {
        self.tick_observed()
            .map(|observation| observation.is_some())
    }

    /// Executes one scheduler iteration and returns the exact immutable
    /// graph/collective selection plus host-observable timing for a committed
    /// step. Device executors must replace `worker_round_trip` with their
    /// qualified kernel/TP/DCP split before performance qualification.
    pub fn tick_observed(&mut self) -> Result<Option<ServingStepObservation>, ServingError> {
        let step_start = Instant::now();
        self.require_event_space(MAXIMUM_STEP_EVENTS)?;
        self.scheduler.apply_cancellations_at_boundary()?;
        self.emit_terminal_transitions()?;
        self.require_event_space(MAXIMUM_STEP_EVENTS)?;
        let Some(batch) = self.scheduler.next_batch()? else {
            return Ok(None);
        };
        let entry = match self.scheduler.graph_entry(batch.graph_id).cloned() {
            Some(entry) => entry,
            None => return self.fail_selected_step(&batch, ServingError::Graph),
        };
        let starting_progress = batch
            .rows
            .iter()
            .map(|row| {
                self.scheduler
                    .request_progress(row.request_id)
                    .map(|progress| (row.request_id, progress))
                    .ok_or(ServingError::Request)
            })
            .collect::<Result<Vec<_>, _>>();
        let starting_progress = match starting_progress {
            Ok(progress) => progress,
            Err(error) => return self.fail_selected_step(&batch, error),
        };
        let mut active_pages = match self.reserve_active_step(&batch, &starting_progress) {
            Ok(active_pages) => active_pages,
            Err(error) => return self.fail_selected_step(&batch, error),
        };
        let reservation_generation = match self.next_sequence_generation() {
            Ok(generation) => generation,
            Err(error) => return self.fail_selected_step(&batch, error),
        };
        let reservation_delta = match PageTableDelta::between(
            &self.active_pages,
            &active_pages,
            self.sequence_table_generation,
            reservation_generation,
        ) {
            Ok(delta) => Arc::new(delta),
            Err(error) => return self.fail_selected_step(&batch, error.into()),
        };
        let compiled = match self
            .compiler
            .compile(&batch, &entry, reservation_generation)
        {
            Ok(compiled) => compiled,
            Err(error) => {
                return self.fail_selected_step(&batch, ServingError::Compile(error));
            }
        };
        let plan = compiled.plan;
        let input = match self.build_step_input(
            &batch,
            &starting_progress,
            &plan,
            &compiled.schedule,
            &reservation_delta,
        ) {
            Ok(input) => Arc::new(input),
            Err(error) => return self.fail_selected_step(&batch, error),
        };
        let collective_count = match u16::try_from(compiled.schedule.operations().len()) {
            Ok(count) => count,
            Err(_) => return self.fail_selected_step(&batch, ServingError::Overflow),
        };
        let collectives = observe_collectives(&compiled.schedule);
        let worker_start = Instant::now();
        let handle = match self.workers.try_submit_bound(
            plan,
            compiled.schedule,
            input,
            Arc::clone(&reservation_delta),
        ) {
            Ok(handle) => handle,
            Err(error) => {
                return self.fail_selected_step(&batch, ServingError::Worker(error));
            }
        };
        let outcome = match handle.receive() {
            Ok(outcome) => outcome,
            Err(error) => {
                return self
                    .fail_selected_step_after_worker_fatal(&batch, ServingError::Worker(error));
            }
        };
        let worker_round_trip = worker_start.elapsed();
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
                            progress
                                .maximum_new_tokens
                                .checked_sub(progress.generated)
                                .is_some_and(|remaining| u32::from(output.count()) <= remaining)
                                && output_has_valid_termination(output)
                        })
            }
        };
        let reserved_pages = active_pages.clone();
        if !output_fits_requests {
            if let Err(error) =
                self.rollback_rank_reservation(&reserved_pages, reservation_generation)
            {
                return self.fail_selected_step_after_worker_fatal(&batch, error);
            }
            return self.fail_selected_step(&batch, ServingError::Output);
        }
        if let Err(error) = Self::commit_active_step(&mut active_pages, &batch, output_rows) {
            if let Err(rollback) =
                self.rollback_rank_reservation(&reserved_pages, reservation_generation)
            {
                return self.fail_selected_step_after_worker_fatal(&batch, rollback);
            }
            return self.fail_selected_step(&batch, error);
        }
        let mut publication = match self.plan_successful_step_publication(
            &batch,
            &starting_progress,
            output_rows,
            active_pages,
        ) {
            Ok(publication) => publication,
            Err(error) => {
                if let Err(rollback) =
                    self.rollback_rank_reservation(&reserved_pages, reservation_generation)
                {
                    return self.fail_selected_step_after_worker_fatal(&batch, rollback);
                }
                return self.fail_selected_step(&batch, error);
            }
        };
        let mut committed_scheduler = self.scheduler.clone();
        if let Err(error) = committed_scheduler.complete_batch_with_results(true, &completions) {
            if let Err(rollback) =
                self.rollback_rank_reservation(&reserved_pages, reservation_generation)
            {
                return self.fail_selected_step_after_worker_fatal(&batch, rollback);
            }
            return self.fail_selected_step(&batch, ServingError::Scheduler(error));
        }
        if matches!(batch.kind, BatchKind::Prefill) {
            publication.releases.sequence_table_generation = reservation_generation;
        } else {
            let commit_generation = match reservation_generation.checked_add(1) {
                Some(generation) => generation,
                None => {
                    return self
                        .fail_selected_step_after_worker_fatal(&batch, ServingError::Overflow);
                }
            };
            if let Err(error) =
                bind_reuse_quarantine(&mut publication.releases.active_pages, commit_generation)
            {
                if let Err(rollback) =
                    self.rollback_rank_reservation(&reserved_pages, reservation_generation)
                {
                    return self.fail_selected_step_after_worker_fatal(&batch, rollback);
                }
                return self.fail_selected_step(&batch, error);
            }
            let commit_delta = match PageTableDelta::between(
                &reserved_pages,
                &publication.releases.active_pages,
                reservation_generation,
                commit_generation,
            ) {
                Ok(delta) => Arc::new(delta),
                Err(error) => {
                    if let Err(rollback) =
                        self.rollback_rank_reservation(&reserved_pages, reservation_generation)
                    {
                        return self.fail_selected_step_after_worker_fatal(&batch, rollback);
                    }
                    return self.fail_selected_step(&batch, error.into());
                }
            };
            if let Err(error) = self.workers.apply_page_delta(commit_delta) {
                return self
                    .fail_selected_step_after_worker_fatal(&batch, ServingError::Worker(error));
            }
            publication.releases.sequence_table_generation = commit_generation;
        }
        self.scheduler = committed_scheduler;
        publication.releases.rank_state = RankReleaseState::Acknowledged;
        self.commit_request_releases(publication.releases)?;
        self.events.extend(
            publication.events.entries[..publication.events.len]
                .iter()
                .flatten()
                .copied(),
        );
        let total_step_time = step_start.elapsed();
        Ok(Some(ServingStepObservation {
            step_id: plan.step_id,
            mode: plan.mode,
            graph_id: plan.graph_id,
            real_sequences: plan.active_sequences,
            bucket_sequences: plan.sequence_bucket,
            real_query_rows: plan.query_rows,
            bucket_query_rows: entry.maximum_query_rows,
            scheduled_prompt_tokens: plan.scheduled_prompt_tokens,
            mtp_depth: plan.mtp_depth,
            collective_count,
            collective_schedule_hash: plan.collective_schedule_hash,
            collectives,
            worker_round_trip,
            coordinator_overhead: total_step_time.saturating_sub(worker_round_trip),
            total_step_time,
        }))
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

    #[must_use]
    pub const fn retained_prompt_bytes(&self) -> u64 {
        self.retained_prompt_bytes
    }

    #[must_use]
    pub fn has_pending_admission(&self, request_id: u64) -> bool {
        self.pending_admissions.contains_key(&request_id)
    }

    pub fn active_page_stats(&self) -> Result<PageTableStats, ServingError> {
        self.active_pages.stats().map_err(Into::into)
    }

    #[must_use]
    pub fn active_committed_tokens(&self, request_id: u64) -> Option<u64> {
        self.active_pages.committed_tokens(request_id)
    }

    fn plan_successful_step_publication(
        &self,
        batch: &ScheduledBatch,
        starting_progress: &[(u64, RequestProgress)],
        output_rows: &[CommittedTokens],
        active_pages: SequencePageTable,
    ) -> Result<SuccessfulStepPublication, ServingError> {
        if starting_progress.len() != batch.rows.len()
            || (!matches!(batch.kind, BatchKind::Prefill) && output_rows.len() != batch.rows.len())
        {
            return Err(ServingError::Output);
        }
        let mut events = StagedEvents::new();
        let mut releases = StagedRequestReleases::new();
        for (row_index, row) in batch.rows.iter().enumerate() {
            let (request_id, starting) = starting_progress
                .get(row_index)
                .copied()
                .ok_or(ServingError::Request)?;
            if request_id != row.request_id {
                return Err(ServingError::Request);
            }
            match batch.kind {
                BatchKind::Prefill => {
                    let prompt_done = starting
                        .prompt_done
                        .checked_add(row.prompt_tokens)
                        .ok_or(ServingError::Overflow)?;
                    if prompt_done > starting.prompt_tokens {
                        return Err(ServingError::Request);
                    }
                    events.push(RequestEvent::PrefillProgress {
                        request_id,
                        prompt_done,
                        prompt_tokens: starting.prompt_tokens,
                    })?;
                    if prompt_done == starting.prompt_tokens {
                        releases.push((request_id, RequestReleaseMode::Tokens))?;
                    }
                }
                BatchKind::Decode | BatchKind::Verify { .. } => {
                    let output = output_rows.get(row_index).ok_or(ServingError::Output)?;
                    for (offset, &token_id) in output.token_ids().iter().enumerate() {
                        let offset = u32::try_from(offset).map_err(|_| ServingError::Overflow)?;
                        let draft_ordinal = (matches!(batch.kind, BatchKind::Verify { .. })
                            && offset < u32::from(output.accepted_draft_count()))
                        .then(|| u8::try_from(offset).map_err(|_| ServingError::Overflow))
                        .transpose()?;
                        let position = starting
                            .generated
                            .checked_add(offset)
                            .ok_or(ServingError::Overflow)?;
                        events.push(RequestEvent::Token {
                            request_id,
                            position,
                            token_id,
                            speculative: draft_ordinal.is_some(),
                            draft_ordinal,
                        })?;
                    }
                    let generated = starting
                        .generated
                        .checked_add(u32::from(output.count()))
                        .ok_or(ServingError::Overflow)?;
                    if generated > starting.maximum_new_tokens {
                        return Err(ServingError::Output);
                    }
                    let stopped = output
                        .token_ids()
                        .last()
                        .is_some_and(|token_id| EOS_TOKEN_IDS.contains(token_id));
                    if stopped || generated == starting.maximum_new_tokens {
                        releases.push((request_id, RequestReleaseMode::Prefix))?;
                        events.push(RequestEvent::Finished {
                            request_id,
                            reason: if stopped {
                                RequestFinishReason::Stop
                            } else {
                                RequestFinishReason::Length
                            },
                        })?;
                    }
                }
            }
        }
        self.require_event_space(events.len)?;
        Ok(SuccessfulStepPublication {
            events,
            releases: self.plan_request_releases(releases.iter(), active_pages)?,
        })
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
        if cancelled.is_empty() {
            return Ok(());
        }
        self.require_event_space(cancelled.len())?;
        let releases = self.plan_request_releases(
            cancelled
                .iter()
                .copied()
                .map(|request_id| (request_id, RequestReleaseMode::Prefix)),
            self.active_pages.clone(),
        )?;
        self.commit_request_releases(releases)?;
        for request_id in cancelled {
            self.events
                .push_back(RequestEvent::Cancelled { request_id });
            self.terminal_events.insert(request_id);
        }
        Ok(())
    }

    fn fail_selected_step<T>(
        &mut self,
        batch: &ScheduledBatch,
        error: ServingError,
    ) -> Result<T, ServingError> {
        let releases = self.plan_request_releases(
            batch
                .rows
                .iter()
                .map(|row| (row.request_id, RequestReleaseMode::Prefix)),
            self.active_pages.clone(),
        );
        let event_space = self.require_event_space(batch.rows.len());
        self.scheduler.complete_batch(false)?;
        let releases = releases?;
        event_space?;
        self.commit_request_releases(releases)?;
        self.events
            .extend(batch.rows.iter().map(|row| RequestEvent::Failed {
                request_id: row.request_id,
            }));
        Err(error)
    }

    fn fail_selected_step_after_worker_fatal<T>(
        &mut self,
        batch: &ScheduledBatch,
        error: ServingError,
    ) -> Result<T, ServingError> {
        let releases = self.plan_request_releases(
            batch
                .rows
                .iter()
                .map(|row| (row.request_id, RequestReleaseMode::Prefix)),
            self.active_pages.clone(),
        );
        let event_space = self.require_event_space(batch.rows.len());
        self.scheduler.complete_batch(false)?;
        let mut releases = releases?;
        event_space?;
        // Rank state is no longer usable, so host cleanup must not attempt a
        // second command that could mask the original fatal worker error.
        releases.rank_state = RankReleaseState::WorkerRetired;
        self.commit_request_releases(releases)?;
        self.events
            .extend(batch.rows.iter().map(|row| RequestEvent::Failed {
                request_id: row.request_id,
            }));
        Err(error)
    }

    fn rollback_rank_reservation(
        &mut self,
        reserved_pages: &SequencePageTable,
        reservation_generation: u64,
    ) -> Result<(), ServingError> {
        let rollback_generation = reservation_generation
            .checked_add(1)
            .ok_or(ServingError::Overflow)?;
        let delta = Arc::new(PageTableDelta::between(
            reserved_pages,
            &self.active_pages,
            reservation_generation,
            rollback_generation,
        )?);
        self.workers.apply_page_delta(delta)?;
        self.sequence_table_generation = rollback_generation;
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

    fn next_sequence_generation(&self) -> Result<u64, ServingError> {
        self.sequence_table_generation
            .checked_add(1)
            .ok_or(ServingError::Overflow)
    }

    fn plan_request_releases(
        &self,
        releases: impl IntoIterator<Item = (u64, RequestReleaseMode)>,
        mut active_pages: SequencePageTable,
    ) -> Result<RequestReleasePlan, ServingError> {
        let mut requests = BTreeMap::new();
        for (request_id, mode) in releases {
            requests
                .entry(request_id)
                .and_modify(|prior| {
                    if mode == RequestReleaseMode::Prefix {
                        *prior = RequestReleaseMode::Prefix;
                    }
                })
                .or_insert(mode);
        }
        let retained_prompt_bytes =
            requests
                .keys()
                .try_fold(self.retained_prompt_bytes, |retained, request_id| {
                    self.request_tokens
                        .get(request_id)
                        .map_or(Ok(retained), |tokens| {
                            retained
                                .checked_sub(prompt_bytes(tokens.len())?)
                                .ok_or(ServingError::Overflow)
                        })
                })?;
        let page_sets: Vec<_> = requests
            .iter()
            .filter(|(_, mode)| **mode == RequestReleaseMode::Prefix)
            .filter_map(|(request_id, _)| {
                self.prefix_leases
                    .get(request_id)
                    .map(RestoredPrefix::page_keys)
            })
            .collect();
        let prefix = if page_sets.is_empty() {
            None
        } else {
            Some(
                self.prefix_cache
                    .as_ref()
                    .ok_or(ServingError::CacheUnavailable)?
                    .plan_release_many(page_sets)?,
            )
        };
        for (&request_id, &mode) in &requests {
            if mode == RequestReleaseMode::Prefix {
                active_pages.remove_sequence(request_id)?;
            }
        }
        let has_page_removal = requests
            .values()
            .any(|mode| *mode == RequestReleaseMode::Prefix);
        Ok(RequestReleasePlan {
            requests,
            retained_prompt_bytes,
            prefix,
            active_pages,
            sequence_table_generation: if has_page_removal {
                self.next_sequence_generation()?
            } else {
                self.sequence_table_generation
            },
            rank_state: RankReleaseState::Pending,
        })
    }

    fn commit_request_releases(
        &mut self,
        mut plan: RequestReleasePlan,
    ) -> Result<(), ServingError> {
        if plan.sequence_table_generation != self.sequence_table_generation {
            bind_reuse_quarantine(&mut plan.active_pages, plan.sequence_table_generation)?;
            if plan.rank_state == RankReleaseState::Pending {
                let delta = Arc::new(PageTableDelta::between(
                    &self.active_pages,
                    &plan.active_pages,
                    self.sequence_table_generation,
                    plan.sequence_table_generation,
                )?);
                self.workers.apply_page_delta(delta)?;
                plan.rank_state = RankReleaseState::Acknowledged;
            }
            if plan.rank_state == RankReleaseState::Acknowledged
                && plan
                    .active_pages
                    .reuse_quarantine_stats()
                    .bound_generation
                    .is_some()
            {
                plan.active_pages
                    .acknowledge_reuse_quarantine(plan.sequence_table_generation)?;
            }
        }
        self.active_pages = plan.active_pages;
        self.sequence_table_generation = plan.sequence_table_generation;
        if let Some(prefix) = plan.prefix {
            self.prefix_cache
                .as_mut()
                .expect("prefix cache was preflighted under exclusive serving access")
                .commit_release(prefix);
        }
        for (request_id, mode) in plan.requests {
            if mode == RequestReleaseMode::Prefix {
                self.prefix_leases.remove(&request_id);
                self.request_sampling.remove(&request_id);
            }
            self.request_tokens.remove(&request_id);
        }
        self.retained_prompt_bytes = plan.retained_prompt_bytes;
        Ok(())
    }

    fn reserve_active_step(
        &self,
        batch: &ScheduledBatch,
        starting_progress: &[(u64, RequestProgress)],
    ) -> Result<SequencePageTable, ServingError> {
        if batch.rows.len() != starting_progress.len() {
            return Err(ServingError::Request);
        }
        let mut active_pages = self.active_pages.clone();
        for (row, &(request_id, progress)) in batch.rows.iter().zip(starting_progress) {
            if row.request_id != request_id {
                return Err(ServingError::Request);
            }
            let expected_committed = u64::from(progress.prompt_done)
                .checked_add(u64::from(progress.generated))
                .ok_or(ServingError::Overflow)?;
            if active_pages.committed_tokens(request_id) != Some(expected_committed) {
                return Err(ServingError::PageTable);
            }
            match batch.kind {
                BatchKind::Prefill => {
                    active_pages.append_committed(request_id, u64::from(row.prompt_tokens))?;
                }
                BatchKind::Decode => active_pages.begin_tentative(request_id, 1)?,
                BatchKind::Verify { depth } => {
                    let reserved = depth.checked_add(1).ok_or(ServingError::Overflow)?;
                    active_pages.begin_tentative(request_id, reserved)?;
                }
            }
        }
        Ok(active_pages)
    }

    fn commit_active_step(
        active_pages: &mut SequencePageTable,
        batch: &ScheduledBatch,
        output_rows: &[CommittedTokens],
    ) -> Result<(), ServingError> {
        match batch.kind {
            BatchKind::Prefill => {
                if !output_rows.is_empty() {
                    return Err(ServingError::Output);
                }
            }
            BatchKind::Decode | BatchKind::Verify { .. } => {
                if output_rows.len() != batch.rows.len() {
                    return Err(ServingError::Output);
                }
                for (row, output) in batch.rows.iter().zip(output_rows) {
                    active_pages.commit_tentative(row.request_id, output.count())?;
                }
            }
        }
        Ok(())
    }

    fn build_step_input(
        &self,
        batch: &ScheduledBatch,
        starting_progress: &[(u64, RequestProgress)],
        plan: &glm_engine::StepPlan,
        schedule: &CollectiveSchedule,
        delta: &PageTableDelta,
    ) -> Result<StepInput, ServingError> {
        if batch.rows.len() != starting_progress.len() {
            return Err(ServingError::Request);
        }
        let mut rows = Vec::with_capacity(batch.rows.len());
        let mut prompt_token_ids = Vec::new();
        for (row, &(request_id, progress)) in batch.rows.iter().zip(starting_progress) {
            if row.request_id != request_id {
                return Err(ServingError::Request);
            }
            let prompt_payload_offset =
                u32::try_from(prompt_token_ids.len()).map_err(|_| ServingError::Overflow)?;
            if row.prompt_tokens != 0 {
                let start =
                    usize::try_from(progress.prompt_done).map_err(|_| ServingError::Overflow)?;
                let count =
                    usize::try_from(row.prompt_tokens).map_err(|_| ServingError::Overflow)?;
                let end = start.checked_add(count).ok_or(ServingError::Overflow)?;
                if let Some(tokens) = self.request_tokens.get(&request_id) {
                    prompt_token_ids
                        .extend_from_slice(tokens.get(start..end).ok_or(ServingError::Request)?);
                } else {
                    #[cfg(test)]
                    prompt_token_ids.resize(
                        prompt_token_ids
                            .len()
                            .checked_add(count)
                            .ok_or(ServingError::Overflow)?,
                        0,
                    );
                    #[cfg(not(test))]
                    return Err(ServingError::Request);
                }
            }
            let context_tokens_before = progress
                .prompt_done
                .checked_add(progress.generated)
                .ok_or(ServingError::Overflow)?;
            let effective_mtp_depth = match batch.kind {
                BatchKind::Prefill | BatchKind::Decode => 0,
                BatchKind::Verify { depth } => depth,
            };
            rows.push(SequenceStepInput {
                request_id,
                context_tokens_before,
                generated_tokens_before: progress.generated,
                maximum_new_tokens: progress.maximum_new_tokens,
                prompt_payload_offset,
                prompt_tokens_this_step: row.prompt_tokens,
                configured_mtp_depth: progress.mtp_depth,
                effective_mtp_depth,
                sampling: *self
                    .request_sampling
                    .get(&request_id)
                    .ok_or(ServingError::Request)?,
            });
        }
        StepInput::new(plan, schedule, delta, rows, prompt_token_ids).map_err(Into::into)
    }

    fn retained_prompt_bytes_after_release(&self, token_count: usize) -> Result<u64, ServingError> {
        self.retained_prompt_bytes
            .checked_sub(prompt_bytes(token_count)?)
            .ok_or(ServingError::Overflow)
    }
}

fn prompt_bytes(token_count: usize) -> Result<u64, ServingError> {
    u64::try_from(token_count)
        .ok()
        .and_then(|count| count.checked_mul(u64::from(u32::BITS / 8)))
        .ok_or(ServingError::Overflow)
}

fn bind_reuse_quarantine(
    pages: &mut SequencePageTable,
    generation: u64,
) -> Result<(), ServingError> {
    let quarantine = pages.reuse_quarantine_stats();
    if quarantine.is_empty() {
        if quarantine.bound_generation.is_some() {
            return Err(ServingError::PageTable);
        }
        return Ok(());
    }
    match quarantine.bound_generation {
        None => {
            if !pages.bind_reuse_quarantine(generation)? {
                return Err(ServingError::PageTable);
            }
        }
        Some(bound) if bound == generation => {}
        Some(_) => return Err(ServingError::PageTable),
    }
    Ok(())
}

const fn sampling_kind(collective: SamplingCollective) -> StepSamplingKind {
    match collective {
        SamplingCollective::Greedy => StepSamplingKind::Greedy,
        SamplingCollective::TopK => StepSamplingKind::TopK,
        SamplingCollective::Mass => StepSamplingKind::Mass,
    }
}

fn default_step_sampling(spec: RequestSpec) -> StepSampling {
    match spec.sampling {
        SamplingCollective::Greedy => StepSampling::greedy(spec.id),
        SamplingCollective::TopK => StepSampling {
            kind: StepSamplingKind::TopK,
            temperature_bits: 1.0_f32.to_bits(),
            top_p_bits: 1.0_f32.to_bits(),
            top_k: 256,
            seed: spec.id,
            rng_counter_before: 0,
        },
        SamplingCollective::Mass => StepSampling {
            kind: StepSamplingKind::Mass,
            temperature_bits: 1.0_f32.to_bits(),
            top_p_bits: 1.0_f32.to_bits(),
            top_k: 0,
            seed: spec.id,
            rng_counter_before: 0,
        },
    }
}

fn validate_context_limit(spec: RequestSpec) -> Result<(), ServingError> {
    u64::from(spec.prompt_tokens)
        .checked_add(u64::from(spec.maximum_new_tokens))
        .filter(|&tokens| tokens <= glm_cache::MAXIMUM_CONTEXT_TOKENS)
        .map(|_| ())
        .ok_or(ServingError::Request)
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

fn observe_collectives(schedule: &CollectiveSchedule) -> CollectivePayloadObservation {
    let mut observation = CollectivePayloadObservation::default();
    for operation in schedule.operations() {
        let bytes = u64::from(operation.payload_bytes);
        match operation.kind {
            CollectiveKind::TpReduce => {
                observation.tp_reduce_bytes += bytes;
                observation.tp_route_id = operation.route_id;
            }
            CollectiveKind::DcpPackedCkv => {
                observation.dcp_packed_ckv_bytes += bytes;
                observation.dcp_packed_ckv_route_id = operation.route_id;
            }
            CollectiveKind::DcpQueryGather => {
                observation.dcp_query_gather_bytes += bytes;
                observation.dcp_query_route_id = operation.route_id;
            }
            CollectiveKind::DcpCandidateExchange => {
                observation.dcp_candidate_exchange_bytes += bytes;
                observation.dcp_candidate_route_id = operation.route_id;
            }
            CollectiveKind::DcpPartialStateReturn => {
                observation.dcp_partial_state_return_bytes += bytes;
                observation.dcp_partial_state_route_id = operation.route_id;
            }
            CollectiveKind::LogitsArgmax
            | CollectiveKind::LogitsTopK
            | CollectiveKind::LogitsMass => {
                observation.sampling_bytes += bytes;
                observation.sampling_route_id = operation.route_id;
            }
        }
    }
    observation
}

#[derive(Debug)]
pub enum ServingError {
    Config,
    Backpressure,
    Graph,
    Request,
    UnknownAdmission,
    Output,
    Overflow,
    StepLimit,
    CacheUnavailable,
    Cache(PrefixRestoreError),
    Scheduler(SchedulerError),
    Compile(glm_scheduler::CompileError),
    Worker(WorkerError),
    PageTable,
    Pages(SequencePageError),
    Delta(PageTableDeltaError),
    StepInput(StepInputError),
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

impl From<SequencePageError> for ServingError {
    fn from(value: SequencePageError) -> Self {
        Self::Pages(value)
    }
}

impl From<PageTableDeltaError> for ServingError {
    fn from(value: PageTableDeltaError) -> Self {
        Self::Delta(value)
    }
}

impl From<StepInputError> for ServingError {
    fn from(value: StepInputError) -> Self {
        Self::StepInput(value)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
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

    #[test]
    fn cpu_reference_coordinator_is_not_production_execution_ready() {
        let coordinator = coordinator(None);
        assert_eq!(
            coordinator.execution_posture(),
            WorkerExecutionPosture::CpuReference
        );
        assert_ne!(
            coordinator.execution_posture(),
            WorkerExecutionPosture::ProductionModel
        );
    }

    fn coordinator_with_workers(workers: Tp4WorkerPool) -> ServingCoordinator {
        coordinator_with_page_config(
            workers,
            PageTableConfig {
                target_pages_per_rank: 256,
                draft_pages_per_rank: 256,
            },
        )
    }

    fn coordinator_with_page_config(
        workers: Tp4WorkerPool,
        page_table: PageTableConfig,
    ) -> ServingCoordinator {
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
                maximum_retained_prompt_bytes: 64 * 1024 * 1024,
                page_table,
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

    fn release_prefix(
        serving: &mut ServingCoordinator,
        request_id: u64,
    ) -> Result<(), ServingError> {
        let plan = serving.plan_request_releases(
            std::iter::once((request_id, RequestReleaseMode::Prefix)),
            serving.active_pages.clone(),
        )?;
        serving.commit_request_releases(plan)?;
        Ok(())
    }

    struct CountingRankExecutor {
        calls: Arc<AtomicUsize>,
    }

    struct BoundInputRankExecutor {
        calls: Arc<AtomicUsize>,
        expected_seed: u64,
    }

    impl RankExecutor for CountingRankExecutor {
        fn execute(
            &mut self,
            _rank: u8,
            plan: &StepPlan,
            _schedule: &CollectiveSchedule,
        ) -> Result<StepOutput, RankExecutionError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match plan.mode {
                StepMode::Prefill => Ok(StepOutput::empty()),
                StepMode::Decode | StepMode::Verify => {
                    let token =
                        CommittedTokens::target(1).map_err(|_| RankExecutionError::Invariant)?;
                    StepOutput::new(&vec![token; usize::from(plan.active_sequences)])
                        .map_err(|_| RankExecutionError::Invariant)
                }
                StepMode::Mixed | StepMode::CacheOnly => Err(RankExecutionError::Invariant),
            }
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

    impl RankExecutor for BoundInputRankExecutor {
        fn execute(
            &mut self,
            _rank: u8,
            _plan: &StepPlan,
            _schedule: &CollectiveSchedule,
        ) -> Result<StepOutput, RankExecutionError> {
            Err(RankExecutionError::Invariant)
        }

        fn execute_bound(
            &mut self,
            _rank: u8,
            plan: &StepPlan,
            _schedule: &CollectiveSchedule,
            input: &StepInput,
        ) -> Result<StepOutput, RankExecutionError> {
            let row = input.rows().first().ok_or(RankExecutionError::Invariant)?;
            if row.sampling.seed != self.expected_seed
                || row.context_tokens_before != 64
                || row.generated_tokens_before != 0
                || row.prompt_tokens_this_step != 0
                || !input.prompt_token_ids().is_empty()
            {
                return Err(RankExecutionError::Invariant);
            }
            self.calls.fetch_add(1, Ordering::SeqCst);
            let token = CommittedTokens::target(1).map_err(|_| RankExecutionError::Invariant)?;
            StepOutput::new(&vec![token; usize::from(plan.active_sequences)])
                .map_err(|_| RankExecutionError::Invariant)
        }
    }

    #[test]
    fn step_observation_captures_exact_graph_routes_bytes_and_host_split() {
        let mut serving = coordinator(None);
        serving
            .admit_prevalidated(ServingRequest {
                spec: RequestSpec {
                    id: 91,
                    tenant: 1,
                    prompt_tokens: 64,
                    maximum_new_tokens: 1,
                    mtp_depth: 0,
                    sampling: SamplingCollective::Greedy,
                },
                cached_prompt_tokens: 0,
            })
            .unwrap();
        assert_eq!(serving.active_committed_tokens(91), Some(0));
        assert_eq!(serving.active_page_stats().unwrap().active_sequences, 1);
        assert_eq!(serving.sequence_table_generation, 2);
        let _ = serving.drain_events();

        let prefill = serving.tick_observed().unwrap().unwrap();
        assert_eq!(prefill.mode, StepMode::Prefill);
        assert_eq!(prefill.graph_id, 1);
        assert_eq!((prefill.real_sequences, prefill.bucket_sequences), (1, 4));
        assert_eq!(
            (prefill.real_query_rows, prefill.bucket_query_rows),
            (64, 64)
        );
        assert_eq!(prefill.scheduled_prompt_tokens, 64);
        assert_eq!(prefill.collective_count, 3);
        assert_eq!(prefill.collectives.dcp_query_gather_bytes, 2_048);
        assert_eq!(prefill.collectives.dcp_partial_state_return_bytes, 2_048);
        assert_eq!(prefill.collectives.tp_reduce_bytes, 2_048);
        assert_eq!(prefill.collectives.sampling_bytes, 0);
        assert_eq!(prefill.collectives.dcp_query_route_id, 3);
        assert_eq!(prefill.collectives.dcp_partial_state_route_id, 5);
        assert_eq!(prefill.collectives.tp_route_id, 1);
        assert_eq!(
            prefill
                .worker_round_trip
                .saturating_add(prefill.coordinator_overhead),
            prefill.total_step_time
        );
        assert_eq!(serving.active_committed_tokens(91), Some(64));
        assert_eq!(serving.sequence_table_generation, 3);
        let _ = serving.drain_events();

        let decode = serving.tick_observed().unwrap().unwrap();
        assert_eq!(decode.mode, StepMode::Decode);
        assert_eq!(decode.graph_id, 2);
        assert_eq!((decode.real_sequences, decode.bucket_sequences), (1, 4));
        assert_eq!((decode.real_query_rows, decode.bucket_query_rows), (1, 4));
        assert_eq!(decode.scheduled_prompt_tokens, 0);
        assert_eq!(decode.collective_count, 5);
        assert_eq!(decode.collectives.dcp_query_gather_bytes, 32);
        assert_eq!(decode.collectives.dcp_candidate_exchange_bytes, 32);
        assert_eq!(decode.collectives.dcp_partial_state_return_bytes, 32);
        assert_eq!(decode.collectives.tp_reduce_bytes, 32);
        assert_eq!(decode.collectives.sampling_bytes, 8);
        assert_eq!(decode.collectives.dcp_candidate_route_id, 4);
        assert_eq!(decode.collectives.sampling_route_id, 6);
        assert_eq!(serving.active_committed_tokens(91), None);
        assert_eq!(serving.active_page_stats().unwrap().active_sequences, 0);
        assert_eq!(serving.sequence_table_generation, 5);
    }

    #[test]
    fn serving_delivers_exact_sampling_and_context_to_all_four_bound_executors() {
        let calls = Arc::new(AtomicUsize::new(0));
        let expected_seed = 0xdead_beef_cafe_babe;
        let executors = std::array::from_fn(|_| {
            Box::new(BoundInputRankExecutor {
                calls: Arc::clone(&calls),
                expected_seed,
            }) as Box<dyn RankExecutor + Send>
        });
        let mut serving = coordinator_with_workers(Tp4WorkerPool::spawn(2, executors).unwrap());
        let spec = RequestSpec {
            id: 98,
            tenant: 1,
            prompt_tokens: 64,
            maximum_new_tokens: 1,
            mtp_depth: 0,
            sampling: SamplingCollective::Greedy,
        };
        serving
            .admit_active_sequence(spec, 64, &[], 64, StepSampling::greedy(expected_seed))
            .unwrap();
        let _ = serving.drain_events();
        assert!(serving.tick().unwrap());
        assert_eq!(calls.load(Ordering::SeqCst), 4);
        assert_eq!(serving.active_committed_tokens(98), None);
    }

    #[test]
    fn page_capacity_failure_is_atomic_and_never_reaches_rank_workers() {
        let calls = Arc::new(AtomicUsize::new(0));
        let executors = std::array::from_fn(|_| {
            Box::new(CountingRankExecutor {
                calls: Arc::clone(&calls),
            }) as Box<dyn RankExecutor + Send>
        });
        let workers = Tp4WorkerPool::spawn(2, executors).unwrap();
        let mut serving = coordinator_with_page_config(
            workers,
            PageTableConfig {
                target_pages_per_rank: 1,
                draft_pages_per_rank: 1,
            },
        );
        serving
            .admit_prevalidated(ServingRequest {
                spec: RequestSpec {
                    id: 94,
                    tenant: 1,
                    prompt_tokens: 257,
                    maximum_new_tokens: 1,
                    mtp_depth: 0,
                    sampling: SamplingCollective::Greedy,
                },
                cached_prompt_tokens: 0,
            })
            .unwrap();
        let _ = serving.drain_events();
        for expected in [64_u64, 128, 192, 256] {
            assert!(serving.tick().unwrap());
            assert_eq!(serving.active_committed_tokens(94), Some(expected));
        }
        assert_eq!(calls.load(Ordering::SeqCst), 16);
        assert!(matches!(
            serving.tick(),
            Err(ServingError::Pages(SequencePageError::Capacity))
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 16);
        assert_eq!(
            serving.request_progress(94).unwrap().state,
            RequestState::Failed
        );
        assert_eq!(serving.active_committed_tokens(94), None);
        assert_eq!(serving.active_page_stats().unwrap().active_sequences, 0);
        assert!(
            serving
                .drain_events()
                .contains(&RequestEvent::Failed { request_id: 94 })
        );
    }

    #[test]
    fn exact_one_million_context_is_admitted_accounted_executed_and_released() {
        let calls = Arc::new(AtomicUsize::new(0));
        let executors = std::array::from_fn(|_| {
            Box::new(CountingRankExecutor {
                calls: Arc::clone(&calls),
            }) as Box<dyn RankExecutor + Send>
        });
        let workers = Tp4WorkerPool::spawn(2, executors).unwrap();
        let mut serving = coordinator_with_page_config(
            workers,
            PageTableConfig {
                target_pages_per_rank: 4_096,
                draft_pages_per_rank: 4_096,
            },
        );
        let prompt_tokens = u32::try_from(glm_cache::MAXIMUM_CONTEXT_TOKENS - 1).unwrap();
        serving
            .admit_prevalidated(ServingRequest {
                spec: RequestSpec {
                    id: 95,
                    tenant: 1,
                    prompt_tokens,
                    maximum_new_tokens: 1,
                    mtp_depth: 0,
                    sampling: SamplingCollective::Greedy,
                },
                cached_prompt_tokens: prompt_tokens,
            })
            .unwrap();
        assert_eq!(
            serving.active_committed_tokens(95),
            Some(glm_cache::MAXIMUM_CONTEXT_TOKENS - 1)
        );
        assert_eq!(
            serving.active_page_stats().unwrap(),
            PageTableStats {
                target_pages_used: [4_096; 4],
                draft_pages_used: [0; 4],
                active_sequences: 1,
                active_positions: glm_cache::MAXIMUM_CONTEXT_TOKENS - 1,
                maximum_target_only_sequence_tokens: glm_cache::MAXIMUM_CONTEXT_TOKENS,
                maximum_mtp_sequence_tokens: glm_cache::MAXIMUM_CONTEXT_TOKENS,
            }
        );
        let _ = serving.drain_events();

        assert!(serving.tick().unwrap());
        assert_eq!(calls.load(Ordering::SeqCst), 4);
        assert_eq!(serving.active_committed_tokens(95), None);
        assert_eq!(
            serving.active_page_stats().unwrap(),
            PageTableStats {
                target_pages_used: [0; 4],
                draft_pages_used: [0; 4],
                active_sequences: 0,
                active_positions: 0,
                maximum_target_only_sequence_tokens: glm_cache::MAXIMUM_CONTEXT_TOKENS,
                maximum_mtp_sequence_tokens: glm_cache::MAXIMUM_CONTEXT_TOKENS,
            }
        );

        serving
            .admit_prevalidated(ServingRequest {
                spec: RequestSpec {
                    id: 96,
                    tenant: 1,
                    prompt_tokens,
                    maximum_new_tokens: 1,
                    mtp_depth: 6,
                    sampling: SamplingCollective::Greedy,
                },
                cached_prompt_tokens: prompt_tokens,
            })
            .unwrap();
        assert_eq!(
            serving.active_page_stats().unwrap(),
            PageTableStats {
                target_pages_used: [4_096; 4],
                draft_pages_used: [4_096; 4],
                active_sequences: 1,
                active_positions: glm_cache::MAXIMUM_CONTEXT_TOKENS - 1,
                maximum_target_only_sequence_tokens: glm_cache::MAXIMUM_CONTEXT_TOKENS,
                maximum_mtp_sequence_tokens: glm_cache::MAXIMUM_CONTEXT_TOKENS,
            }
        );
        let _ = serving.drain_events();
        assert_eq!(
            serving.tick_observed().unwrap().unwrap().mode,
            StepMode::Decode
        );
        assert_eq!(calls.load(Ordering::SeqCst), 8);
        assert_eq!(serving.active_committed_tokens(96), None);
        assert_eq!(serving.active_page_stats().unwrap().active_sequences, 0);

        assert!(matches!(
            serving.admit_prevalidated(ServingRequest {
                spec: RequestSpec {
                    id: 97,
                    tenant: 1,
                    prompt_tokens: u32::try_from(glm_cache::MAXIMUM_CONTEXT_TOKENS).unwrap(),
                    maximum_new_tokens: 1,
                    mtp_depth: 0,
                    sampling: SamplingCollective::Greedy,
                },
                cached_prompt_tokens: u32::try_from(glm_cache::MAXIMUM_CONTEXT_TOKENS).unwrap(),
            }),
            Err(ServingError::Request)
        ));
        assert_eq!(serving.active_page_stats().unwrap().active_sequences, 0);
    }

    #[test]
    fn maximum_verify_publication_fits_the_fixed_event_boundary_exactly() {
        let mut serving = coordinator(None);
        let batch = ScheduledBatch {
            step_id: 1,
            kind: BatchKind::Verify { depth: 6 },
            graph_id: 3,
            rows: (1..=glm_engine::MAX_ACTIVE_SEQUENCES)
                .map(|request_id| glm_scheduler::BatchRow {
                    request_id: u64::from(request_id),
                    prompt_tokens: 0,
                })
                .collect(),
            query_rows: u32::from(glm_engine::MAX_ACTIVE_SEQUENCES) * 7,
            sampling: SamplingCollective::Greedy,
        };
        let starting_progress: Vec<_> = batch
            .rows
            .iter()
            .map(|row| {
                (
                    row.request_id,
                    RequestProgress {
                        state: RequestState::Decoding,
                        prompt_tokens: 64,
                        prompt_done: 64,
                        maximum_new_tokens: 7,
                        generated: 0,
                        mtp_depth: 6,
                    },
                )
            })
            .collect();
        let committed = CommittedTokens::verify(&[1, 2, 3, 4, 5, 6], Some(7)).unwrap();
        let output_rows = vec![committed; usize::from(glm_engine::MAX_ACTIVE_SEQUENCES)];
        for row in &batch.rows {
            serving
                .active_pages
                .admit_with_prefix(row.request_id, true, &[])
                .unwrap();
            serving
                .active_pages
                .append_committed(row.request_id, 64)
                .unwrap();
        }

        let publication = serving
            .plan_successful_step_publication(
                &batch,
                &starting_progress,
                &output_rows,
                serving.active_pages.clone(),
            )
            .unwrap();
        assert_eq!(publication.events.len, MAXIMUM_STEP_EVENTS);
        assert_eq!(
            publication.releases.requests.len(),
            usize::from(glm_engine::MAX_ACTIVE_SEQUENCES)
        );
    }

    #[test]
    fn corrupt_generation_fails_closed_without_forging_rank_cleanup() {
        let mut serving = coordinator(None);
        serving
            .admit_prevalidated(ServingRequest {
                spec: RequestSpec {
                    id: 92,
                    tenant: 1,
                    prompt_tokens: 64,
                    maximum_new_tokens: 1,
                    mtp_depth: 0,
                    sampling: SamplingCollective::Greedy,
                },
                cached_prompt_tokens: 0,
            })
            .unwrap();
        let _ = serving.drain_events();
        serving.sequence_table_generation = 0;

        assert!(matches!(
            serving.tick_observed(),
            Err(ServingError::Delta(PageTableDeltaError::Generation))
        ));
        assert_eq!(
            serving.request_progress(92).unwrap().state,
            RequestState::Failed
        );
        assert!(serving.drain_events().is_empty());
        assert_eq!(serving.active_committed_tokens(92), Some(0));
        assert!(!serving.tick().unwrap());
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

    fn fixed_mtp_workers() -> Tp4WorkerPool {
        let executors =
            std::array::from_fn(|_| Box::new(FixedMtpRankExecutor) as Box<dyn RankExecutor + Send>);
        Tp4WorkerPool::spawn(2, executors).unwrap()
    }

    fn accepted_draft_eos_workers() -> Tp4WorkerPool {
        let executors = std::array::from_fn(|_| {
            Box::new(AcceptedDraftEosRankExecutor) as Box<dyn RankExecutor + Send>
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
                    maximum_new_tokens: 7,
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
            9
        );
        assert!(events.iter().any(|event| matches!(
            event,
            RequestEvent::Token {
                request_id: 20,
                speculative: true,
                ..
            }
        )));
        assert_eq!(serving.active_page_stats().unwrap().active_sequences, 0);
        assert_eq!(
            serving.active_page_stats().unwrap().target_pages_used,
            [0; 4]
        );
        assert_eq!(
            serving.active_page_stats().unwrap().draft_pages_used,
            [0; 4]
        );
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
        serving
            .admit_prevalidated(ServingRequest {
                spec: RequestSpec {
                    id: 20,
                    tenant: 2,
                    prompt_tokens: 64,
                    maximum_new_tokens: 2,
                    mtp_depth: 0,
                    sampling: SamplingCollective::Greedy,
                },
                cached_prompt_tokens: 64,
            })
            .unwrap();
        serving.cancel(10).unwrap();
        let _ = serving.drain_events();
        assert!(serving.tick().unwrap());
        let events = serving.drain_events();
        assert!(events.contains(&RequestEvent::Cancelled { request_id: 10 }));
        assert!(events.iter().any(|event| matches!(
            event,
            RequestEvent::Token {
                request_id: 20,
                position: 0,
                speculative: false,
                draft_ordinal: None,
                ..
            }
        )));
        assert_eq!(serving.active_committed_tokens(10), None);
        assert_eq!(serving.active_committed_tokens(20), Some(65));
        assert_eq!(
            serving.request_progress(10).unwrap().state,
            RequestState::Cancelled
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
        assert_eq!(serving.active_committed_tokens(10), None);
        assert_eq!(serving.active_page_stats().unwrap().active_sequences, 0);
    }

    #[test]
    fn mtp_tail_falls_back_to_decode_at_the_request_generation_limit() {
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
        assert!(serving.tick().unwrap());
        let events = serving.drain_events();
        assert!(events.contains(&RequestEvent::Token {
            request_id: 10,
            position: 0,
            token_id: 43,
            speculative: false,
            draft_ordinal: None,
        }));
        assert!(events.contains(&RequestEvent::Finished {
            request_id: 10,
            reason: RequestFinishReason::Length,
        }));
        assert_eq!(
            serving.request_progress(10).unwrap().state,
            RequestState::Finished
        );
        assert_eq!(serving.active_committed_tokens(10), None);
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
                    draft_ordinal: Some(0),
                },
                RequestEvent::Token {
                    request_id: 10,
                    position: 1,
                    token_id: EOS_TOKEN_IDS[0],
                    speculative: true,
                    draft_ordinal: Some(1),
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
        serving.maximum_retained_prompt_bytes = 255;
        assert!(matches!(
            serving.begin_admit_tokens(
                RequestSpec {
                    id: 76,
                    tenant: 1,
                    prompt_tokens: 64,
                    maximum_new_tokens: 1,
                    mtp_depth: 0,
                    sampling: SamplingCollective::Greedy,
                },
                &tokens,
            ),
            Err(ServingError::Backpressure)
        ));
        assert_eq!(serving.retained_prompt_bytes(), 0);
        serving.maximum_retained_prompt_bytes = 64 * 1024 * 1024;
        assert_eq!(
            serving
                .begin_admit_tokens(
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
                .unwrap(),
            AdmissionStatus::Pending
        );
        assert_eq!(serving.retained_prompt_bytes(), 256);
        assert!(serving.drain_events().is_empty());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match serving.poll_admission(77).unwrap() {
                AdmissionStatus::Pending => {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "admission restore did not complete"
                    );
                    std::thread::yield_now();
                }
                AdmissionStatus::Admitted {
                    cached_prompt_tokens,
                } => {
                    assert_eq!(cached_prompt_tokens, 64);
                    break;
                }
            }
        }
        assert_eq!(serving.retained_prompt_bytes(), 0);
        assert_eq!(serving.active_committed_tokens(77), Some(64));
        assert_eq!(
            serving.active_page_stats().unwrap().target_pages_used,
            [1, 0, 0, 0]
        );
        assert_eq!(
            serving.drain_events(),
            vec![RequestEvent::Admitted {
                request_id: 77,
                cached_prompt_tokens: 64,
            }]
        );
        serving
            .prefix_cache
            .as_mut()
            .unwrap()
            .release(&[key])
            .unwrap();
        assert!(matches!(
            release_prefix(&mut serving, 77),
            Err(ServingError::Cache(PrefixRestoreError::Residency(
                glm_cache::ResidencyError::State
            )))
        ));
        assert!(serving.prefix_leases.contains_key(&77));
        let repaired = serving
            .prefix_cache
            .as_mut()
            .unwrap()
            .restore_longest(999, &tokens)
            .unwrap();
        assert_eq!(repaired.page_keys(), [key]);
        serving
            .prefix_cache
            .as_mut()
            .unwrap()
            .release(repaired.page_keys())
            .unwrap();
        serving.prefix_leases.remove(&77);
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
        assert_eq!(serving.active_committed_tokens(77), None);

        assert_eq!(
            serving
                .begin_admit_tokens(
                    RequestSpec {
                        id: 78,
                        tenant: 1,
                        prompt_tokens: 64,
                        maximum_new_tokens: 1,
                        mtp_depth: 6,
                        sampling: SamplingCollective::Greedy,
                    },
                    &tokens,
                )
                .unwrap(),
            AdmissionStatus::Admitted {
                cached_prompt_tokens: 0,
            }
        );
        assert_eq!(serving.retained_prompt_bytes(), 256);
        assert_eq!(
            serving.drain_events(),
            vec![RequestEvent::Admitted {
                request_id: 78,
                cached_prompt_tokens: 0,
            }]
        );
        serving.cancel(78).unwrap();
        assert!(!serving.tick().unwrap());
        assert_eq!(serving.retained_prompt_bytes(), 0);
        assert_eq!(
            serving.drain_events(),
            vec![RequestEvent::Cancelled { request_id: 78 }]
        );
        drop(serving);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_restore_rollback_retains_pending_admission_until_cancel() {
        let root = temporary_store("serving-restore-rollback");
        let namespace = PrefixNamespace::new(NamespaceInputs {
            model_revision_sha256: [21; 32],
            tokenizer_sha256: [22; 32],
            chat_template_sha256: [23; 32],
            weight_policy_hash: [24; 32],
            target_kv_abi_sha256: [25; 32],
            draft_kv_abi_sha256: [26; 32],
            rope_parameters_sha256: [27; 32],
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
            1,
        )
        .unwrap();
        prefix.register_prefix(&tokens, vec![record]).unwrap();
        let mut serving = coordinator(None);
        serving.attach_prefix_cache(prefix).unwrap();
        assert_eq!(
            serving
                .begin_admit_tokens(
                    RequestSpec {
                        id: 210,
                        tenant: 1,
                        prompt_tokens: 64,
                        maximum_new_tokens: 1,
                        mtp_depth: 0,
                        sampling: SamplingCollective::Greedy,
                    },
                    &tokens,
                )
                .unwrap(),
            AdmissionStatus::Pending
        );
        assert!(matches!(
            serving.admit_prevalidated(ServingRequest {
                spec: RequestSpec {
                    id: 210,
                    tenant: 1,
                    prompt_tokens: 64,
                    maximum_new_tokens: 1,
                    mtp_depth: 0,
                    sampling: SamplingCollective::Greedy,
                },
                cached_prompt_tokens: 0,
            }),
            Err(ServingError::Backpressure)
        ));
        assert!(serving.request_progress(210).is_none());
        serving
            .prefix_cache
            .as_mut()
            .unwrap()
            .abort_pending_page_for_test(210, 0)
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match serving.poll_admission(210) {
                Ok(AdmissionStatus::Pending) => {
                    assert!(Instant::now() < deadline, "restore fault did not arrive");
                    thread::yield_now();
                }
                Err(ServingError::Cache(PrefixRestoreError::Residency(
                    glm_cache::ResidencyError::State,
                ))) => break,
                result => panic!("unexpected restore rollback result: {result:?}"),
            }
        }
        assert!(serving.pending_admissions.contains_key(&210));
        assert!(
            serving
                .prefix_cache
                .as_ref()
                .unwrap()
                .has_pending_restore(210)
        );
        assert_eq!(serving.retained_prompt_bytes(), 256);
        assert!(serving.drain_events().is_empty());

        serving
            .prefix_cache
            .as_mut()
            .unwrap()
            .repair_pending_page_identity_for_test(210, 0)
            .unwrap();
        serving.cancel(210).unwrap();
        assert!(!serving.pending_admissions.contains_key(&210));
        assert!(
            !serving
                .prefix_cache
                .as_ref()
                .unwrap()
                .has_pending_restore(210)
        );
        assert_eq!(serving.retained_prompt_bytes(), 0);
        assert_eq!(
            serving.drain_events(),
            vec![RequestEvent::Cancelled { request_id: 210 }]
        );
        drop(serving);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn late_terminal_cleanup_failure_does_not_partially_publish_the_batch() {
        let root = temporary_store("serving-terminal-cleanup");
        let namespace = PrefixNamespace::new(NamespaceInputs {
            model_revision_sha256: [11; 32],
            tokenizer_sha256: [12; 32],
            chat_template_sha256: [13; 32],
            weight_policy_hash: [14; 32],
            target_kv_abi_sha256: [15; 32],
            draft_kv_abi_sha256: [16; 32],
            rope_parameters_sha256: [17; 32],
        })
        .unwrap();
        let tokens_a: Vec<u32> = (0..64).collect();
        let tokens_b: Vec<u32> = (1_000..1_064).collect();
        let index = PrefixIndex::new(namespace);
        let key_a = index.derive_keys(&tokens_a)[0];
        let key_b = index.derive_keys(&tokens_b)[0];
        let mut store = FileTierStore::open(&root).unwrap();
        let records: Vec<_> = [key_a, key_b]
            .into_iter()
            .enumerate()
            .map(|(record_index, key)| {
                store
                    .publish(DurablePageRequest {
                        namespace: namespace.0,
                        page_key: key.0,
                        generation: 1,
                        mtp: false,
                        pieces: [TierPiece::TargetKv, TierPiece::TargetIndexer]
                            .into_iter()
                            .map(|piece| PagePieceBytes {
                                piece,
                                bytes: vec![
                                    u8::try_from(record_index + 1).unwrap();
                                    piece.expected_bytes() as usize
                                ],
                            })
                            .collect(),
                    })
                    .unwrap()
            })
            .collect();
        let page_bytes: u64 = records[0]
            .pieces
            .iter()
            .map(|piece| piece.byte_length)
            .sum();
        drop(store);

        let mut prefix = PrefixRestoreCoordinator::new(
            index,
            &root,
            ResidencyConfig {
                hbm_bytes: page_bytes * 2,
                dram_bytes: page_bytes * 2,
            },
            2,
        )
        .unwrap();
        prefix
            .register_prefix(&tokens_a, vec![records[0].clone()])
            .unwrap();
        prefix
            .register_prefix(&tokens_b, vec![records[1].clone()])
            .unwrap();
        let mut serving = coordinator(None);
        serving.attach_prefix_cache(prefix).unwrap();
        for (request_id, tokens) in [
            (100, tokens_a.as_slice()),
            (101, tokens_a.as_slice()),
            (102, tokens_b.as_slice()),
        ] {
            serving
                .admit_tokens(
                    RequestSpec {
                        id: request_id,
                        tenant: 1,
                        prompt_tokens: 64,
                        maximum_new_tokens: 1,
                        mtp_depth: 0,
                        sampling: SamplingCollective::Greedy,
                    },
                    tokens,
                )
                .unwrap();
        }
        let _ = serving.drain_events();

        serving
            .prefix_cache
            .as_mut()
            .unwrap()
            .release(&[key_b])
            .unwrap();
        assert!(matches!(
            serving.tick_observed(),
            Err(ServingError::Cache(PrefixRestoreError::Residency(
                glm_cache::ResidencyError::State
            )))
        ));
        for request_id in [100, 101, 102] {
            assert_eq!(
                serving.request_progress(request_id).unwrap().state,
                RequestState::Failed
            );
            assert!(serving.prefix_leases.contains_key(&request_id));
        }
        assert!(serving.drain_events().is_empty());
        assert!(!serving.tick().unwrap());

        serving
            .prefix_cache
            .as_mut()
            .unwrap()
            .release(&[key_a])
            .expect("the earlier valid row must retain its pin");
        serving
            .prefix_cache
            .as_mut()
            .unwrap()
            .release(&[key_a])
            .expect("both shared-prefix pins must survive the failed plan");
        let repaired_a_first = serving
            .prefix_cache
            .as_mut()
            .unwrap()
            .restore_longest(900, &tokens_a)
            .unwrap();
        let repaired_a_second = serving
            .prefix_cache
            .as_mut()
            .unwrap()
            .restore_longest(902, &tokens_a)
            .unwrap();
        let repaired_b = serving
            .prefix_cache
            .as_mut()
            .unwrap()
            .restore_longest(901, &tokens_b)
            .unwrap();
        assert_eq!(repaired_a_first.page_keys(), [key_a]);
        assert_eq!(repaired_a_second.page_keys(), [key_a]);
        assert_eq!(repaired_b.page_keys(), [key_b]);
        release_prefix(&mut serving, 100).unwrap();
        release_prefix(&mut serving, 101).unwrap();
        release_prefix(&mut serving, 102).unwrap();

        for (request_id, tokens) in [(103, tokens_a.as_slice()), (104, tokens_b.as_slice())] {
            serving
                .admit_tokens(
                    RequestSpec {
                        id: request_id,
                        tenant: 1,
                        prompt_tokens: 64,
                        maximum_new_tokens: 1,
                        mtp_depth: 0,
                        sampling: SamplingCollective::Greedy,
                    },
                    tokens,
                )
                .unwrap();
            serving.cancel(request_id).unwrap();
        }
        let _ = serving.drain_events();
        serving
            .prefix_cache
            .as_mut()
            .unwrap()
            .release(&[key_b])
            .unwrap();
        assert!(matches!(
            serving.tick(),
            Err(ServingError::Cache(PrefixRestoreError::Residency(
                glm_cache::ResidencyError::State
            )))
        ));
        for request_id in [103, 104] {
            assert_eq!(
                serving.request_progress(request_id).unwrap().state,
                RequestState::Cancelled
            );
            assert!(serving.prefix_leases.contains_key(&request_id));
        }
        assert!(serving.drain_events().is_empty());
        serving
            .prefix_cache
            .as_mut()
            .unwrap()
            .release(&[key_a])
            .expect("cancellation preflight must retain every earlier pin");
        let _ = serving
            .prefix_cache
            .as_mut()
            .unwrap()
            .restore_longest(903, &tokens_a)
            .unwrap();
        let _ = serving
            .prefix_cache
            .as_mut()
            .unwrap()
            .restore_longest(904, &tokens_b)
            .unwrap();
        assert!(!serving.tick().unwrap());
        assert_eq!(
            serving.drain_events(),
            vec![
                RequestEvent::Cancelled { request_id: 103 },
                RequestEvent::Cancelled { request_id: 104 },
            ]
        );
        assert!(!serving.prefix_leases.contains_key(&103));
        assert!(!serving.prefix_leases.contains_key(&104));
        drop(serving);
        fs::remove_dir_all(root).unwrap();
    }
}
