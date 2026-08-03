use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use glm_cache::MODEL_POSITIONS;
use glm_engine::{StartupCoordinator, StartupState, StepSampling, WorkerExecutionPosture};
use glm_scheduler::{RequestSpec, SamplingCollective};
use glm_tokenizer::{DecodeDelta, IncrementalDecoder, PinnedTokenizer, StreamFinish};

use crate::{
    AdmissionStatus, ApiBackend, ApiBackendError, ApiCompletionEvent, ApiCompletionHandle,
    ApiHealth, ApiHealthState, ApiUsage, RequestEvent, RequestFinishReason, ServingCoordinator,
    ValidatedChatRequest, metrics::ServingMetrics,
};

const MAXIMUM_COMMAND_CAPACITY: usize = 65_536;
const MAXIMUM_COMPLETION_EVENT_CAPACITY: usize = 4_096;
const MAXIMUM_COMMANDS_PER_TICK: usize = 4_096;
const MINIMUM_COMPLETION_EVENT_CAPACITY: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoordinatorBackendConfig {
    pub command_capacity: usize,
    pub completion_event_capacity: usize,
    pub maximum_commands_per_tick: usize,
    pub idle_poll_interval: Duration,
}

impl CoordinatorBackendConfig {
    pub fn validate(self) -> Result<(), CoordinatorBackendError> {
        if self.command_capacity == 0
            || self.command_capacity > MAXIMUM_COMMAND_CAPACITY
            || self.completion_event_capacity < MINIMUM_COMPLETION_EVENT_CAPACITY
            || self.completion_event_capacity > MAXIMUM_COMPLETION_EVENT_CAPACITY
            || self.maximum_commands_per_tick == 0
            || self.maximum_commands_per_tick > MAXIMUM_COMMANDS_PER_TICK
            || self.idle_poll_interval.is_zero()
            || self.idle_poll_interval > Duration::from_millis(100)
        {
            return Err(CoordinatorBackendError::Config);
        }
        Ok(())
    }
}

impl Default for CoordinatorBackendConfig {
    fn default() -> Self {
        Self {
            command_capacity: 1_024,
            completion_event_capacity: 256,
            maximum_commands_per_tick: 256,
            idle_poll_interval: Duration::from_millis(1),
        }
    }
}

#[derive(Debug)]
pub enum CoordinatorBackendError {
    Config,
    EngineNotHealthy,
    RuntimeStartup,
    Thread(std::io::Error),
}

impl fmt::Display for CoordinatorBackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for CoordinatorBackendError {}

fn validate_production_readiness(
    startup_state: StartupState,
    execution_posture: WorkerExecutionPosture,
) -> Result<(), CoordinatorBackendError> {
    if startup_state != StartupState::Healthy
        || execution_posture != WorkerExecutionPosture::ProductionModel
    {
        return Err(CoordinatorBackendError::EngineNotHealthy);
    }
    Ok(())
}

enum BackendCommand {
    Submit {
        request_id: u64,
        tenant: u32,
        maximum_output_tokens: u32,
        mtp_depth: u8,
        sampling: StepSampling,
        request_started_at: Instant,
        enqueued_at: Instant,
        tokens: Box<[u32]>,
        decoder: Box<dyn OutputDecoder>,
        events: SyncSender<ApiCompletionEvent>,
    },
    Cancel {
        request_id: u64,
        tenant: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CancellationState {
    Requested,
    Dispatched,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeStartupFault {
    BeforeReady,
}

trait OutputDecoder: Send {
    fn push(&mut self, token_id: u32) -> Result<DecodeDelta, String>;
    fn finish(&mut self) -> Result<DecodeDelta, String>;
}

struct PinnedOutputDecoder(IncrementalDecoder);

impl OutputDecoder for PinnedOutputDecoder {
    fn push(&mut self, token_id: u32) -> Result<DecodeDelta, String> {
        self.0.push(token_id).map_err(|error| error.to_string())
    }

    fn finish(&mut self) -> Result<DecodeDelta, String> {
        self.0.finish().map_err(|error| error.to_string())
    }
}

trait RuntimeTokenizer: Send + Sync {
    fn encode_chat(&self, request: &ValidatedChatRequest) -> Result<Box<[u32]>, String>;
    fn decoder(&self, stops: Vec<String>) -> Result<Box<dyn OutputDecoder>, String>;
}

struct PinnedRuntimeTokenizer(Arc<PinnedTokenizer>);

impl RuntimeTokenizer for PinnedRuntimeTokenizer {
    fn encode_chat(&self, request: &ValidatedChatRequest) -> Result<Box<[u32]>, String> {
        let prompt = request.render_prompt().map_err(|error| error.to_string())?;
        self.0
            .encode(&prompt)
            .map(Vec::into_boxed_slice)
            .map_err(|error| error.to_string())
    }

    fn decoder(&self, stops: Vec<String>) -> Result<Box<dyn OutputDecoder>, String> {
        self.0
            .stream(stops)
            .map(PinnedOutputDecoder)
            .map(|decoder| Box::new(decoder) as Box<dyn OutputDecoder>)
            .map_err(|error| error.to_string())
    }
}

struct ActiveRequest {
    tenant: u32,
    prompt_tokens: u32,
    prompt_done: u32,
    completion_tokens: u32,
    mtp_depth: u8,
    request_started_at: Instant,
    admission_started_at: Instant,
    admitted_at: Option<Instant>,
    last_token_at: Option<Instant>,
    decoder: Box<dyn OutputDecoder>,
    events: SyncSender<ApiCompletionEvent>,
}

struct RuntimeControl<'a> {
    fatal: &'a AtomicBool,
    shutdown: &'a AtomicBool,
    owners: &'a Mutex<BTreeMap<u64, u32>>,
    cancellations: &'a Mutex<BTreeMap<u64, CancellationState>>,
    counters: &'a ServingMetrics,
    startup: SyncSender<()>,
    startup_fault: Option<RuntimeStartupFault>,
}

/// Bounded production adapter between the HTTP API and the single-owner
/// continuous-batching coordinator.
///
/// The adapter is deliberately fail-closed to greedy sampling. Exact
/// `StepInput` sampling/RNG state now reaches rank execution, but
/// probabilistic admission remains disabled until `StepOutput` returns and
/// the coordinator atomically commits the reviewed post-step RNG counter.
pub struct CoordinatorApiBackend {
    health: ApiHealth,
    fatal: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    next_request_id: AtomicU64,
    command_sender: SyncSender<BackendCommand>,
    tokenizer: Arc<dyn RuntimeTokenizer>,
    completion_event_capacity: usize,
    owners: Arc<Mutex<BTreeMap<u64, u32>>>,
    cancellations: Arc<Mutex<BTreeMap<u64, CancellationState>>>,
    counters: Arc<ServingMetrics>,
    runtime_thread: Option<JoinHandle<()>>,
}

impl CoordinatorApiBackend {
    pub fn spawn(
        config: CoordinatorBackendConfig,
        coordinator: ServingCoordinator,
        tokenizer: Arc<PinnedTokenizer>,
        startup: &StartupCoordinator,
        backend_name: &'static str,
    ) -> Result<Self, CoordinatorBackendError> {
        validate_production_readiness(startup.state(), coordinator.execution_posture())?;
        Self::spawn_with_tokenizer(
            config,
            coordinator,
            Arc::new(PinnedRuntimeTokenizer(tokenizer)),
            backend_name,
        )
    }

    fn spawn_with_tokenizer(
        config: CoordinatorBackendConfig,
        coordinator: ServingCoordinator,
        tokenizer: Arc<dyn RuntimeTokenizer>,
        backend_name: &'static str,
    ) -> Result<Self, CoordinatorBackendError> {
        Self::spawn_with_tokenizer_inner(config, coordinator, tokenizer, backend_name, None)
    }

    fn spawn_with_tokenizer_inner(
        config: CoordinatorBackendConfig,
        coordinator: ServingCoordinator,
        tokenizer: Arc<dyn RuntimeTokenizer>,
        backend_name: &'static str,
        startup_fault: Option<RuntimeStartupFault>,
    ) -> Result<Self, CoordinatorBackendError> {
        config.validate()?;
        if backend_name.is_empty() {
            return Err(CoordinatorBackendError::Config);
        }
        let (command_sender, command_receiver) = mpsc::sync_channel(config.command_capacity);
        let (startup_sender, startup_receiver) = mpsc::sync_channel(1);
        let fatal = Arc::new(AtomicBool::new(false));
        let shutdown = Arc::new(AtomicBool::new(false));
        let owners = Arc::new(Mutex::new(BTreeMap::new()));
        let cancellations = Arc::new(Mutex::new(BTreeMap::new()));
        let counters = Arc::new(ServingMetrics::new());
        let runtime_fatal = Arc::clone(&fatal);
        let runtime_shutdown = Arc::clone(&shutdown);
        let runtime_owners = Arc::clone(&owners);
        let runtime_cancellations = Arc::clone(&cancellations);
        let runtime_counters = Arc::clone(&counters);
        let runtime_thread = thread::Builder::new()
            .name("glmaxx-serving-runtime".to_owned())
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    runtime_loop(
                        coordinator,
                        command_receiver,
                        config,
                        RuntimeControl {
                            fatal: &runtime_fatal,
                            shutdown: &runtime_shutdown,
                            owners: &runtime_owners,
                            cancellations: &runtime_cancellations,
                            counters: &runtime_counters,
                            startup: startup_sender,
                            startup_fault,
                        },
                    )
                }));
                if result.is_err() || !runtime_shutdown.load(Ordering::Acquire) {
                    runtime_fatal.store(true, Ordering::Release);
                }
                if let Ok(mut owners) = runtime_owners.lock() {
                    owners.clear();
                }
                if let Ok(mut cancellations) = runtime_cancellations.lock() {
                    cancellations.clear();
                }
            })
            .map_err(CoordinatorBackendError::Thread)?;
        if startup_receiver.recv().is_err() {
            let _ = runtime_thread.join();
            return Err(CoordinatorBackendError::RuntimeStartup);
        }
        Ok(Self {
            health: ApiHealth::production_healthy(backend_name),
            fatal,
            shutdown,
            next_request_id: AtomicU64::new(1),
            command_sender,
            tokenizer,
            completion_event_capacity: config.completion_event_capacity,
            owners,
            cancellations,
            counters,
            runtime_thread: Some(runtime_thread),
        })
    }

    fn reject(&self, code: &'static str, message: impl Into<String>) -> ApiBackendError {
        self.counters.increment_rejected();
        ApiBackendError {
            code,
            message: message.into(),
        }
    }
}

impl ApiBackend for CoordinatorApiBackend {
    fn health(&self) -> ApiHealth {
        let mut health = self.health.clone();
        if self.fatal.load(Ordering::Acquire) || self.shutdown.load(Ordering::Acquire) {
            health.state = ApiHealthState::Fatal;
        }
        health
    }

    fn metrics(&self) -> String {
        let active = self.owners.lock().map_or(0, |owners| owners.len());
        self.counters
            .render(active, self.health().state == ApiHealthState::Fatal)
    }

    fn submit_chat(
        &self,
        tenant: u32,
        request: ValidatedChatRequest,
    ) -> Result<ApiCompletionHandle, ApiBackendError> {
        if !self.health().is_production_healthy() {
            return Err(self.reject("ENGINE_NOT_HEALTHY", "the serving runtime is not healthy"));
        }
        if tenant == 0 {
            return Err(self.reject("INVALID_TENANT", "tenant must be nonzero"));
        }
        if request.sampling.temperature != 0.0
            || request.sampling.top_p != 1.0
            || request.sampling.top_k.is_some()
        {
            return Err(self.reject(
                "SAMPLING_ABI_NOT_PROMOTED",
                "the current backend admits greedy requests only until StepInput sampling is promoted",
            ));
        }
        let request_started_at = Instant::now();
        let tokenization_start = Instant::now();
        let tokens = match self.tokenizer.encode_chat(&request) {
            Ok(tokens) => {
                self.counters
                    .observe_tokenization(tokenization_start.elapsed());
                tokens
            }
            Err(error) => {
                self.counters
                    .observe_tokenization(tokenization_start.elapsed());
                return Err(self.reject("TOKENIZATION_FAILED", error));
            }
        };
        let prompt_tokens = u32::try_from(tokens.len())
            .map_err(|_| self.reject("CONTEXT_LENGTH_EXCEEDED", "prompt token count overflow"))?;
        if prompt_tokens == 0
            || prompt_tokens
                .checked_add(request.maximum_output_tokens)
                .is_none_or(|total| u64::from(total) > MODEL_POSITIONS)
        {
            return Err(self.reject(
                "CONTEXT_LENGTH_EXCEEDED",
                "prompt plus maximum output must fit the 1048576-token context",
            ));
        }
        let decoder = self
            .tokenizer
            .decoder(request.stop.clone())
            .map_err(|error| self.reject("TOKENIZATION_FAILED", error))?;
        let request_id = self
            .next_request_id
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_add(1)
            })
            .map_err(|_| self.reject("REQUEST_ID_EXHAUSTED", "request id space exhausted"))?;
        let (event_sender, events) = mpsc::sync_channel(self.completion_event_capacity);
        let command = BackendCommand::Submit {
            request_id,
            tenant,
            maximum_output_tokens: request.maximum_output_tokens,
            mtp_depth: request.mtp_depth,
            sampling: StepSampling::greedy(request.sampling.seed.unwrap_or(request_id)),
            request_started_at,
            enqueued_at: Instant::now(),
            tokens,
            decoder,
            events: event_sender,
        };
        // Hold the registry gate through the nonblocking enqueue. A terminal
        // runtime sets `fatal` before taking the same gate to drain commands,
        // so no request can pass the initial health check and enter the queue
        // after the terminal drain.
        let mut owners = self
            .owners
            .lock()
            .map_err(|_| self.reject("ENGINE_STATE_FAILED", "request registry is poisoned"))?;
        if self.fatal.load(Ordering::Acquire) || self.shutdown.load(Ordering::Acquire) {
            return Err(self.reject(
                "ENGINE_NOT_HEALTHY",
                "the serving runtime stopped during request admission",
            ));
        }
        if owners.insert(request_id, tenant).is_some() {
            return Err(self.reject("ENGINE_STATE_FAILED", "request id collision"));
        }
        match self.command_sender.try_send(command) {
            Ok(()) => {
                self.counters.increment_submitted();
                Ok(ApiCompletionHandle { request_id, events })
            }
            Err(error) => {
                owners.remove(&request_id);
                let (code, message) = match error {
                    TrySendError::Full(_) => (
                        "ENGINE_OVERLOADED",
                        "the bounded serving command queue is full",
                    ),
                    TrySendError::Disconnected(_) => {
                        self.fatal.store(true, Ordering::Release);
                        ("ENGINE_NOT_HEALTHY", "the serving runtime is disconnected")
                    }
                };
                Err(self.reject(code, message))
            }
        }
    }

    fn cancel(&self, tenant: u32, request_id: u64) -> Result<(), ApiBackendError> {
        if request_id == 0 || tenant == 0 {
            return Err(self.reject("UNKNOWN_REQUEST", "request is not active"));
        }
        let owners = self
            .owners
            .lock()
            .map_err(|_| self.reject("ENGINE_STATE_FAILED", "request registry is poisoned"))?;
        let owner = owners.get(&request_id).copied();
        match owner {
            None => return Err(self.reject("UNKNOWN_REQUEST", "request is not active")),
            Some(owner) if owner != tenant => {
                return Err(self.reject(
                    "TENANT_MISMATCH",
                    "request does not belong to the authenticated tenant",
                ));
            }
            Some(_) => {}
        }
        if self.fatal.load(Ordering::Acquire) || self.shutdown.load(Ordering::Acquire) {
            return Err(self.reject(
                "ENGINE_NOT_HEALTHY",
                "the serving runtime stopped before cancellation",
            ));
        }
        self.cancellations
            .lock()
            .map_err(|_| self.reject("ENGINE_STATE_FAILED", "cancellation registry is poisoned"))?
            .entry(request_id)
            .or_insert(CancellationState::Requested);
        Ok(())
    }
}

impl Drop for CoordinatorApiBackend {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(runtime_thread) = self.runtime_thread.take() {
            let _ = runtime_thread.join();
        }
    }
}

fn runtime_loop(
    mut coordinator: ServingCoordinator,
    commands: Receiver<BackendCommand>,
    config: CoordinatorBackendConfig,
    control: RuntimeControl<'_>,
) {
    let RuntimeControl {
        fatal,
        shutdown,
        owners,
        cancellations,
        counters,
        startup,
        startup_fault,
    } = control;
    let mut active = BTreeMap::<u64, ActiveRequest>::new();
    let mut pending_admissions = BTreeSet::<u64>::new();
    if startup_fault == Some(RuntimeStartupFault::BeforeReady) {
        panic!("injected serving runtime startup failure");
    }
    if startup.send(()).is_err() {
        return;
    }
    while !shutdown.load(Ordering::Acquire) {
        let mut progressed = false;
        for _ in 0..config.maximum_commands_per_tick {
            match commands.try_recv() {
                Ok(command) => {
                    progressed = true;
                    if let Err(error) = process_command(
                        command,
                        &mut coordinator,
                        &mut active,
                        &mut pending_admissions,
                        owners,
                        counters,
                    ) {
                        fatal.store(true, Ordering::Release);
                        fail_all(
                            &mut active,
                            &commands,
                            owners,
                            counters,
                            error.code,
                            &error.message,
                        );
                        return;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    fail_all(
                        &mut active,
                        &commands,
                        owners,
                        counters,
                        "BACKEND_SHUTDOWN",
                        "the serving runtime command channel closed",
                    );
                    return;
                }
            }
        }

        match process_cancellation_requests(
            &mut coordinator,
            &mut active,
            &mut pending_admissions,
            owners,
            cancellations,
            counters,
            config.maximum_commands_per_tick,
        ) {
            Ok(cancel_progressed) => progressed |= cancel_progressed,
            Err(error) => {
                fatal.store(true, Ordering::Release);
                fail_all(
                    &mut active,
                    &commands,
                    owners,
                    counters,
                    error.code,
                    &error.message,
                );
                return;
            }
        }

        for request_id in pending_admissions.iter().copied().collect::<Vec<_>>() {
            match poll_pending_admission(
                request_id,
                &mut coordinator,
                &mut active,
                &mut pending_admissions,
                owners,
                counters,
            ) {
                Ok(poll_progressed) => progressed |= poll_progressed,
                Err(error) => {
                    fatal.store(true, Ordering::Release);
                    fail_all(
                        &mut active,
                        &commands,
                        owners,
                        counters,
                        error.code,
                        &error.message,
                    );
                    return;
                }
            }
        }

        let admission_events = coordinator.drain_events();
        if !admission_events.is_empty() {
            progressed = true;
            if let Err(error) = dispatch_events(
                admission_events,
                &mut coordinator,
                &mut active,
                &mut pending_admissions,
                owners,
                counters,
            ) {
                fatal.store(true, Ordering::Release);
                fail_all(
                    &mut active,
                    &commands,
                    owners,
                    counters,
                    error.code,
                    &error.message,
                );
                return;
            }
        }

        match coordinator.tick_observed() {
            Ok(Some(observation)) => {
                counters.observe_step(&observation);
                progressed = true;
            }
            Ok(None) => {}
            Err(error) => {
                fatal.store(true, Ordering::Release);
                let dispatch_error = dispatch_events(
                    coordinator.drain_events(),
                    &mut coordinator,
                    &mut active,
                    &mut pending_admissions,
                    owners,
                    counters,
                )
                .err();
                if let Some(dispatch_error) = dispatch_error {
                    fail_all(
                        &mut active,
                        &commands,
                        owners,
                        counters,
                        dispatch_error.code,
                        &dispatch_error.message,
                    );
                } else {
                    fail_all(
                        &mut active,
                        &commands,
                        owners,
                        counters,
                        "ENGINE_STEP_FAILED",
                        &error.to_string(),
                    );
                }
                return;
            }
        }
        let events = coordinator.drain_events();
        if !events.is_empty() {
            progressed = true;
            if let Err(error) = dispatch_events(
                events,
                &mut coordinator,
                &mut active,
                &mut pending_admissions,
                owners,
                counters,
            ) {
                fatal.store(true, Ordering::Release);
                fail_all(
                    &mut active,
                    &commands,
                    owners,
                    counters,
                    error.code,
                    &error.message,
                );
                return;
            }
        }

        if !progressed {
            match commands.recv_timeout(config.idle_poll_interval) {
                Ok(command) => {
                    if let Err(error) = process_command(
                        command,
                        &mut coordinator,
                        &mut active,
                        &mut pending_admissions,
                        owners,
                        counters,
                    ) {
                        fatal.store(true, Ordering::Release);
                        fail_all(
                            &mut active,
                            &commands,
                            owners,
                            counters,
                            error.code,
                            &error.message,
                        );
                        return;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    }
    fail_all(
        &mut active,
        &commands,
        owners,
        counters,
        "BACKEND_SHUTDOWN",
        "the serving runtime is shutting down",
    );
}

fn process_cancellation_requests(
    coordinator: &mut ServingCoordinator,
    active: &mut BTreeMap<u64, ActiveRequest>,
    pending_admissions: &mut BTreeSet<u64>,
    owners: &Mutex<BTreeMap<u64, u32>>,
    cancellations: &Mutex<BTreeMap<u64, CancellationState>>,
    counters: &ServingMetrics,
    maximum_cancellations: usize,
) -> Result<bool, ApiBackendError> {
    let mut progressed = false;
    for _ in 0..maximum_cancellations {
        let next = {
            let owners = owners.lock().map_err(|_| ApiBackendError {
                code: "ENGINE_STATE_FAILED",
                message: "request registry is poisoned".to_owned(),
            })?;
            let mut cancellations = cancellations.lock().map_err(|_| ApiBackendError {
                code: "ENGINE_STATE_FAILED",
                message: "cancellation registry is poisoned".to_owned(),
            })?;
            let requested = cancellations.iter().find_map(|(&request_id, &state)| {
                (state == CancellationState::Requested)
                    .then(|| {
                        active
                            .get(&request_id)
                            .map(|request| (request_id, request.tenant))
                    })
                    .flatten()
            });
            if let Some((request_id, tenant)) = requested {
                if owners.get(&request_id) != Some(&tenant) {
                    return Err(ApiBackendError {
                        code: "ENGINE_STATE_FAILED",
                        message: "cancellation ownership disagrees with active request".to_owned(),
                    });
                }
                Some(Some((request_id, tenant)))
            } else if let Some(request_id) = cancellations
                .keys()
                .find(|request_id| !owners.contains_key(request_id))
                .copied()
            {
                cancellations.remove(&request_id);
                Some(None)
            } else {
                None
            }
        };
        let Some(next) = next else {
            break;
        };
        let Some((request_id, tenant)) = next else {
            progressed = true;
            continue;
        };
        process_command(
            BackendCommand::Cancel { request_id, tenant },
            coordinator,
            active,
            pending_admissions,
            owners,
            counters,
        )?;
        let mut cancellations = cancellations.lock().map_err(|_| ApiBackendError {
            code: "ENGINE_STATE_FAILED",
            message: "cancellation registry is poisoned".to_owned(),
        })?;
        if let Some(state) = cancellations.get_mut(&request_id) {
            *state = CancellationState::Dispatched;
        }
        progressed = true;
    }
    Ok(progressed)
}

fn poll_pending_admission(
    request_id: u64,
    coordinator: &mut ServingCoordinator,
    active: &mut BTreeMap<u64, ActiveRequest>,
    pending_admissions: &mut BTreeSet<u64>,
    owners: &Mutex<BTreeMap<u64, u32>>,
    counters: &ServingMetrics,
) -> Result<bool, ApiBackendError> {
    match coordinator.poll_admission(request_id) {
        Ok(AdmissionStatus::Pending) => Ok(false),
        Ok(AdmissionStatus::Admitted { .. }) => {
            pending_admissions.remove(&request_id);
            Ok(true)
        }
        Err(error) if coordinator.has_pending_admission(request_id) => Err(ApiBackendError {
            code: "ADMISSION_ROLLBACK_FAILED",
            message: error.to_string(),
        }),
        Err(error) => {
            pending_admissions.remove(&request_id);
            fail_request(
                request_id,
                active,
                owners,
                counters,
                "ADMISSION_FAILED",
                error.to_string(),
            );
            Ok(true)
        }
    }
}

fn process_command(
    command: BackendCommand,
    coordinator: &mut ServingCoordinator,
    active: &mut BTreeMap<u64, ActiveRequest>,
    pending_admissions: &mut BTreeSet<u64>,
    owners: &Mutex<BTreeMap<u64, u32>>,
    counters: &ServingMetrics,
) -> Result<(), ApiBackendError> {
    match command {
        BackendCommand::Submit {
            request_id,
            tenant,
            maximum_output_tokens,
            mtp_depth,
            sampling,
            request_started_at,
            enqueued_at,
            tokens,
            decoder,
            events,
        } => {
            let prompt_tokens = match u32::try_from(tokens.len()) {
                Ok(prompt_tokens) => prompt_tokens,
                Err(_) => {
                    fail_sender(
                        events,
                        counters,
                        "CONTEXT_LENGTH_EXCEEDED",
                        "prompt token count overflow",
                    );
                    remove_owner(owners, request_id);
                    return Ok(());
                }
            };
            let admission_started_at = Instant::now();
            counters.observe_queue(admission_started_at.saturating_duration_since(enqueued_at));
            active.insert(
                request_id,
                ActiveRequest {
                    tenant,
                    prompt_tokens,
                    prompt_done: 0,
                    completion_tokens: 0,
                    mtp_depth,
                    request_started_at,
                    admission_started_at,
                    admitted_at: None,
                    last_token_at: None,
                    decoder,
                    events,
                },
            );
            let spec = RequestSpec {
                id: request_id,
                tenant,
                prompt_tokens,
                maximum_new_tokens: maximum_output_tokens,
                mtp_depth,
                sampling: SamplingCollective::Greedy,
            };
            match coordinator.begin_admit_tokens_with_sampling(spec, sampling, &tokens) {
                Ok(AdmissionStatus::Pending) => {
                    pending_admissions.insert(request_id);
                }
                Ok(AdmissionStatus::Admitted { .. }) => {}
                Err(error) => fail_request(
                    request_id,
                    active,
                    owners,
                    counters,
                    "ADMISSION_FAILED",
                    error.to_string(),
                ),
            }
        }
        BackendCommand::Cancel { request_id, tenant } => {
            let matches_owner = active
                .get(&request_id)
                .is_some_and(|request| request.tenant == tenant);
            if !matches_owner {
                return Ok(());
            }
            match coordinator.cancel(request_id) {
                Ok(()) => {
                    pending_admissions.remove(&request_id);
                }
                Err(error) if coordinator.has_pending_admission(request_id) => {
                    return Err(ApiBackendError {
                        code: "CANCELLATION_ROLLBACK_FAILED",
                        message: error.to_string(),
                    });
                }
                Err(_) => {
                    pending_admissions.remove(&request_id);
                    fail_request(
                        request_id,
                        active,
                        owners,
                        counters,
                        "CANCELLATION_FAILED",
                        "the coordinator rejected cancellation",
                    );
                }
            }
        }
    }
    Ok(())
}

fn dispatch_events(
    events: Vec<RequestEvent>,
    coordinator: &mut ServingCoordinator,
    active: &mut BTreeMap<u64, ActiveRequest>,
    pending_admissions: &mut BTreeSet<u64>,
    owners: &Mutex<BTreeMap<u64, u32>>,
    counters: &ServingMetrics,
) -> Result<(), ApiBackendError> {
    for event in events {
        match event {
            RequestEvent::Admitted {
                request_id,
                cached_prompt_tokens,
            } => {
                let Some(mut request) = active.remove(&request_id) else {
                    continue;
                };
                if request.admitted_at.is_some() || cached_prompt_tokens > request.prompt_tokens {
                    let request =
                        cancel_dispatch_request(coordinator, active, request_id, request)?;
                    fail_active_request(
                        request,
                        counters,
                        "ADMISSION_EVENT_MISMATCH",
                        "admission event is duplicated or exceeds the prompt",
                    );
                    remove_owner(owners, request_id);
                    continue;
                }
                let now = Instant::now();
                counters.observe_prefix_resolution(
                    now.saturating_duration_since(request.admission_started_at),
                );
                counters.add_prefix_restored(cached_prompt_tokens, request.mtp_depth != 0);
                counters.observe_admitted_mtp_depth(request.mtp_depth);
                request.prompt_done = cached_prompt_tokens;
                request.admitted_at = Some(now);
                active.insert(request_id, request);
            }
            RequestEvent::PrefillProgress {
                request_id,
                prompt_done,
                prompt_tokens,
            } => {
                let Some(mut request) = active.remove(&request_id) else {
                    continue;
                };
                if request.admitted_at.is_none()
                    || prompt_tokens != request.prompt_tokens
                    || prompt_done < request.prompt_done
                    || prompt_done > request.prompt_tokens
                {
                    let request =
                        cancel_dispatch_request(coordinator, active, request_id, request)?;
                    fail_active_request(
                        request,
                        counters,
                        "PREFILL_PROGRESS_MISMATCH",
                        "prefill progress is nonmonotonic or disagrees with admission",
                    );
                    remove_owner(owners, request_id);
                    continue;
                }
                let computed = prompt_done - request.prompt_done;
                counters.add_prompt_computed(computed, request.mtp_depth != 0);
                request.prompt_done = prompt_done;
                active.insert(request_id, request);
            }
            RequestEvent::Token {
                request_id,
                position,
                token_id,
                speculative,
                draft_ordinal,
            } => {
                let Some(mut request) = active.remove(&request_id) else {
                    continue;
                };
                if request.admitted_at.is_none()
                    || position != request.completion_tokens
                    || speculative != draft_ordinal.is_some()
                    || draft_ordinal.is_some_and(|ordinal| ordinal >= request.mtp_depth)
                {
                    let request =
                        cancel_dispatch_request(coordinator, active, request_id, request)?;
                    fail_active_request(
                        request,
                        counters,
                        "OUTPUT_POSITION_MISMATCH",
                        "committed output positions are not contiguous",
                    );
                    remove_owner(owners, request_id);
                    continue;
                }
                let now = Instant::now();
                request.completion_tokens += 1;
                match request.decoder.push(token_id) {
                    Ok(delta) => {
                        if let Some(previous) = request.last_token_at {
                            counters.observe_itl(now.saturating_duration_since(previous));
                        } else {
                            counters.observe_ttft(
                                now.saturating_duration_since(request.request_started_at),
                            );
                            if let Some(admitted_at) = request.admitted_at {
                                counters.observe_admission_to_first_token(
                                    now.saturating_duration_since(admitted_at),
                                );
                            }
                        }
                        request.last_token_at = Some(now);
                        counters.observe_output_token(
                            speculative,
                            request.mtp_depth,
                            draft_ordinal,
                        );
                        if !delta.text.is_empty()
                            && request
                                .events
                                .try_send(ApiCompletionEvent::TextDelta(delta.text))
                                .is_err()
                        {
                            let request =
                                cancel_dispatch_request(coordinator, active, request_id, request)?;
                            counters.increment_slow_consumers();
                            counters.increment_cancelled();
                            counters.observe_request_time(
                                now.saturating_duration_since(request.request_started_at),
                            );
                            remove_owner(owners, request_id);
                            continue;
                        }
                        if delta.finish.is_some() {
                            let request =
                                cancel_dispatch_request(coordinator, active, request_id, request)?;
                            finish_request(request, request_id, "stop", counters, owners);
                        } else {
                            active.insert(request_id, request);
                        }
                    }
                    Err(error) => {
                        let request =
                            cancel_dispatch_request(coordinator, active, request_id, request)?;
                        fail_active_request(request, counters, "OUTPUT_DECODE_FAILED", error);
                        remove_owner(owners, request_id);
                    }
                }
            }
            RequestEvent::Finished { request_id, reason } => {
                pending_admissions.remove(&request_id);
                let Some(mut request) = active.remove(&request_id) else {
                    continue;
                };
                match request.decoder.finish() {
                    Ok(delta) => {
                        if !delta.text.is_empty()
                            && request
                                .events
                                .try_send(ApiCompletionEvent::TextDelta(delta.text))
                                .is_err()
                        {
                            counters.increment_slow_consumers();
                            counters.increment_cancelled();
                            counters.observe_request_time(
                                Instant::now()
                                    .saturating_duration_since(request.request_started_at),
                            );
                            remove_owner(owners, request_id);
                            continue;
                        }
                        let stopped_by_decoder =
                            matches!(delta.finish, Some(StreamFinish::StopString(_)));
                        let finish_reason =
                            if reason == RequestFinishReason::Stop || stopped_by_decoder {
                                "stop"
                            } else {
                                "length"
                            };
                        finish_request(request, request_id, finish_reason, counters, owners);
                    }
                    Err(error) => {
                        fail_active_request(request, counters, "OUTPUT_DECODE_FAILED", error);
                        remove_owner(owners, request_id);
                    }
                }
            }
            RequestEvent::Cancelled { request_id } => {
                pending_admissions.remove(&request_id);
                cancel_request(request_id, active, owners, counters);
            }
            RequestEvent::Failed { request_id } => {
                pending_admissions.remove(&request_id);
                fail_request(
                    request_id,
                    active,
                    owners,
                    counters,
                    "ENGINE_REQUEST_FAILED",
                    "request failed at a collective-safe step boundary",
                );
            }
        }
    }
    Ok(())
}

fn cancel_dispatch_request(
    coordinator: &mut ServingCoordinator,
    active: &mut BTreeMap<u64, ActiveRequest>,
    request_id: u64,
    request: ActiveRequest,
) -> Result<ActiveRequest, ApiBackendError> {
    match coordinator.cancel(request_id) {
        Ok(()) => Ok(request),
        Err(error) => {
            if active.insert(request_id, request).is_some() {
                panic!("dispatch request identity reappeared under exclusive backend access");
            }
            Err(ApiBackendError {
                code: "EVENT_CANCELLATION_FAILED",
                message: error.to_string(),
            })
        }
    }
}

fn finish_request(
    request: ActiveRequest,
    request_id: u64,
    finish_reason: &'static str,
    counters: &ServingMetrics,
    owners: &Mutex<BTreeMap<u64, u32>>,
) {
    counters
        .observe_request_time(Instant::now().saturating_duration_since(request.request_started_at));
    counters.observe_termination(finish_reason);
    let Some(total_tokens) = request.prompt_tokens.checked_add(request.completion_tokens) else {
        fail_sender(
            request.events,
            counters,
            "OUTPUT_USAGE_OVERFLOW",
            "completion usage overflowed u32",
        );
        remove_owner(owners, request_id);
        return;
    };
    let usage = ApiUsage {
        prompt_tokens: request.prompt_tokens,
        completion_tokens: request.completion_tokens,
        total_tokens,
    };
    if request
        .events
        .try_send(ApiCompletionEvent::Finished {
            finish_reason: finish_reason.to_owned(),
            usage,
        })
        .is_err()
    {
        counters.increment_slow_consumers();
    } else {
        counters.increment_completed();
    }
    remove_owner(owners, request_id);
}

fn fail_request(
    request_id: u64,
    active: &mut BTreeMap<u64, ActiveRequest>,
    owners: &Mutex<BTreeMap<u64, u32>>,
    counters: &ServingMetrics,
    code: &'static str,
    message: impl Into<String>,
) {
    if let Some(request) = active.remove(&request_id) {
        fail_active_request(request, counters, code, message);
    }
    remove_owner(owners, request_id);
}

fn cancel_request(
    request_id: u64,
    active: &mut BTreeMap<u64, ActiveRequest>,
    owners: &Mutex<BTreeMap<u64, u32>>,
    counters: &ServingMetrics,
) {
    if let Some(request) = active.remove(&request_id) {
        counters.observe_request_time(
            Instant::now().saturating_duration_since(request.request_started_at),
        );
        let _ = request
            .events
            .try_send(ApiCompletionEvent::Failed(ApiBackendError {
                code: "REQUEST_CANCELLED",
                message: "request was cancelled".to_owned(),
            }));
        counters.increment_cancelled();
    }
    remove_owner(owners, request_id);
}

fn fail_sender(
    events: SyncSender<ApiCompletionEvent>,
    counters: &ServingMetrics,
    code: &'static str,
    message: impl Into<String>,
) {
    let _ = events.try_send(ApiCompletionEvent::Failed(ApiBackendError {
        code,
        message: message.into(),
    }));
    counters.increment_failed();
}

fn fail_active_request(
    request: ActiveRequest,
    counters: &ServingMetrics,
    code: &'static str,
    message: impl Into<String>,
) {
    counters
        .observe_request_time(Instant::now().saturating_duration_since(request.request_started_at));
    fail_sender(request.events, counters, code, message);
}

fn fail_all(
    active: &mut BTreeMap<u64, ActiveRequest>,
    commands: &Receiver<BackendCommand>,
    owners: &Mutex<BTreeMap<u64, u32>>,
    counters: &ServingMetrics,
    code: &'static str,
    message: &str,
) {
    for (_, request) in std::mem::take(active) {
        fail_active_request(request, counters, code, message);
    }
    if let Ok(mut owners) = owners.lock() {
        while let Ok(command) = commands.try_recv() {
            if let BackendCommand::Submit {
                request_id,
                request_started_at,
                enqueued_at,
                events,
                ..
            } = command
            {
                let now = Instant::now();
                counters.observe_queue(now.saturating_duration_since(enqueued_at));
                counters.observe_request_time(now.saturating_duration_since(request_started_at));
                fail_sender(events, counters, code, message);
                owners.remove(&request_id);
            }
        }
        owners.clear();
    }
}

fn remove_owner(owners: &Mutex<BTreeMap<u64, u32>>, request_id: u64) {
    if let Ok(mut owners) = owners.lock() {
        owners.remove(&request_id);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::{Barrier, atomic::AtomicUsize},
        time::{SystemTime, UNIX_EPOCH},
    };

    use glm_cache::{
        DurablePageRequest, FileTierStore, NamespaceInputs, PagePieceBytes, PageTableConfig,
        PrefixIndex, PrefixNamespace, ResidencyConfig, TierPiece,
    };
    use glm_engine::{
        AttentionTransport, CollectiveSchedule, CommittedTokens, GraphEntry, GraphKey,
        GraphProfile, RankExecutionError, RankExecutor, StepInput, StepMode, StepOutput, StepPlan,
        Tp4WorkerPool,
    };
    use glm_scheduler::{RouteCatalog, SchedulerConfig, TenantConfig};

    use super::*;
    use crate::ChatCompletionRequest;

    static NEXT_TEMPORARY_STORE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn production_backend_requires_healthy_startup_and_production_workers() {
        assert!(
            validate_production_readiness(
                StartupState::Healthy,
                WorkerExecutionPosture::ProductionModel,
            )
            .is_ok()
        );

        for posture in [
            WorkerExecutionPosture::CpuReference,
            WorkerExecutionPosture::CustomUnverified,
            WorkerExecutionPosture::NativeWeightsOnly,
        ] {
            assert!(matches!(
                validate_production_readiness(StartupState::Healthy, posture),
                Err(CoordinatorBackendError::EngineNotHealthy)
            ));
        }

        for state in StartupState::NORMATIVE_ORDER
            .into_iter()
            .filter(|state| *state != StartupState::Healthy)
            .chain([StartupState::Failed])
        {
            assert!(matches!(
                validate_production_readiness(state, WorkerExecutionPosture::ProductionModel),
                Err(CoordinatorBackendError::EngineNotHealthy)
            ));
        }
    }

    struct FakeTokenizer;

    impl RuntimeTokenizer for FakeTokenizer {
        fn encode_chat(&self, _request: &ValidatedChatRequest) -> Result<Box<[u32]>, String> {
            Ok(vec![17].into_boxed_slice())
        }

        fn decoder(&self, stops: Vec<String>) -> Result<Box<dyn OutputDecoder>, String> {
            Ok(Box::new(FakeDecoder {
                stop_on_x: stops.iter().any(|stop| stop == "x"),
                finished: false,
            }))
        }
    }

    struct FakeDecoder {
        stop_on_x: bool,
        finished: bool,
    }

    impl OutputDecoder for FakeDecoder {
        fn push(&mut self, _token_id: u32) -> Result<DecodeDelta, String> {
            if self.finished {
                return Err("decoder already finished".to_owned());
            }
            if self.stop_on_x {
                self.finished = true;
                Ok(DecodeDelta {
                    text: String::new(),
                    finish: Some(StreamFinish::StopString(0)),
                })
            } else {
                Ok(DecodeDelta {
                    text: "x".to_owned(),
                    finish: None,
                })
            }
        }

        fn finish(&mut self) -> Result<DecodeDelta, String> {
            if self.finished {
                return Err("decoder already finished".to_owned());
            }
            self.finished = true;
            Ok(DecodeDelta {
                text: String::new(),
                finish: Some(StreamFinish::EndOfStream),
            })
        }
    }

    fn temporary_store() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "glmaxx-api-backend-test-{}-{nonce}-{}",
            std::process::id(),
            NEXT_TEMPORARY_STORE.fetch_add(1, Ordering::Relaxed),
        ))
    }

    fn graph_entry(graph_id: u32, mode: StepMode, rows: u32) -> GraphEntry {
        GraphEntry {
            graph_id,
            key: GraphKey {
                mode,
                sequence_bucket: 4,
                verifier_row_bucket: if mode == StepMode::Prefill { 0 } else { rows },
                mtp_depth: 0,
                attention_transport: if mode == StepMode::Prefill {
                    AttentionTransport::PrefillQuery
                } else {
                    AttentionTransport::DecodeQueryLse
                },
            },
            maximum_active_sequences: 4,
            maximum_prompt_tokens: if mode == StepMode::Prefill { rows } else { 0 },
            maximum_query_rows: rows,
            compatible_tp_routes: vec![1],
            compatible_dcp_routes: vec![3, 4, 5],
            compatible_sampling_routes: if mode == StepMode::Prefill {
                Vec::new()
            } else {
                vec![6]
            },
            maximum_scratch_bytes: 1,
            argument_bytes: 1,
            graph_object_bytes: 1,
            resident_module_bytes: 1,
            admission_slo_class: 1,
        }
    }

    fn coordinator(store_root: &std::path::Path) -> ServingCoordinator {
        coordinator_with_workers(store_root, Tp4WorkerPool::spawn_cpu(2, None).unwrap())
    }

    fn coordinator_with_workers(
        store_root: &std::path::Path,
        workers: Tp4WorkerPool,
    ) -> ServingCoordinator {
        coordinator_with_workers_and_capacity(
            store_root,
            workers,
            ResidencyConfig {
                hbm_bytes: 1,
                dram_bytes: 0,
            },
        )
    }

    fn coordinator_with_workers_and_capacity(
        store_root: &std::path::Path,
        workers: Tp4WorkerPool,
        residency: ResidencyConfig,
    ) -> ServingCoordinator {
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
        drop(FileTierStore::open(store_root).unwrap());
        let prefix = crate::PrefixRestoreCoordinator::new(
            PrefixIndex::new(namespace),
            store_root,
            residency,
            1,
        )
        .unwrap();
        let profile = GraphProfile::new(vec![
            graph_entry(1, StepMode::Prefill, 64),
            graph_entry(2, StepMode::Decode, 4),
        ])
        .unwrap();
        let routes = RouteCatalog {
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
        };
        let mut coordinator = ServingCoordinator::new(
            crate::ServingConfig {
                epoch: 1,
                event_capacity: 1_024,
                maximum_retained_prompt_bytes: 1_024 * 1_024,
                page_table: PageTableConfig {
                    target_pages_per_rank: 256,
                    draft_pages_per_rank: 256,
                },
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
                    weight: 1,
                    maximum_active_requests: 4,
                },
            ],
            routes,
            workers,
        )
        .unwrap();
        coordinator.attach_prefix_cache(prefix).unwrap();
        coordinator
    }

    fn validated(maximum_output_tokens: u32, stop: Option<&str>) -> ValidatedChatRequest {
        let stop = stop.map_or_else(String::new, |value| format!(r#","stop":"{value}""#));
        serde_json::from_str::<ChatCompletionRequest>(&format!(
            concat!(
                r#"{{"model":"glm-5.2","messages":[{{"role":"user","content":"hi"}}],"#,
                r#""temperature":0,"max_tokens":{}{}}}"#
            ),
            maximum_output_tokens, stop
        ))
        .unwrap()
        .validate()
        .unwrap()
    }

    fn backend(store_root: &std::path::Path) -> CoordinatorApiBackend {
        backend_with(
            coordinator(store_root),
            CoordinatorBackendConfig {
                command_capacity: 16,
                completion_event_capacity: 16,
                maximum_commands_per_tick: 16,
                idle_poll_interval: Duration::from_millis(1),
            },
        )
    }

    fn backend_with(
        coordinator: ServingCoordinator,
        config: CoordinatorBackendConfig,
    ) -> CoordinatorApiBackend {
        CoordinatorApiBackend::spawn_with_tokenizer(
            config,
            coordinator,
            Arc::new(FakeTokenizer),
            "cpu-test",
        )
        .unwrap()
    }

    fn terminal_event(handle: &ApiCompletionHandle) -> ApiCompletionEvent {
        loop {
            match handle.events.recv_timeout(Duration::from_secs(2)).unwrap() {
                ApiCompletionEvent::TextDelta(_) => {}
                terminal => return terminal,
            }
        }
    }

    struct GatedFailExecutor {
        entered: Arc<Barrier>,
        release: Arc<Barrier>,
    }

    struct BarrierReleaseGuard {
        release: Arc<Barrier>,
        released: bool,
    }

    impl BarrierReleaseGuard {
        fn new(release: Arc<Barrier>) -> Self {
            Self {
                release,
                released: false,
            }
        }

        fn release(mut self) {
            self.release.wait();
            self.released = true;
        }
    }

    impl Drop for BarrierReleaseGuard {
        fn drop(&mut self) {
            if !self.released {
                self.release.wait();
            }
        }
    }

    impl RankExecutor for GatedFailExecutor {
        fn execute(
            &mut self,
            _rank: u8,
            _plan: &StepPlan,
            _schedule: &CollectiveSchedule,
        ) -> Result<StepOutput, RankExecutionError> {
            self.entered.wait();
            self.release.wait();
            Err(RankExecutionError::Backend(-1))
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

    struct FirstStepBlockingExecutor {
        entered: Arc<Barrier>,
        release: Arc<Barrier>,
        blocked: bool,
    }

    impl RankExecutor for FirstStepBlockingExecutor {
        fn execute(
            &mut self,
            _rank: u8,
            plan: &StepPlan,
            _schedule: &CollectiveSchedule,
        ) -> Result<StepOutput, RankExecutionError> {
            if !self.blocked {
                self.blocked = true;
                self.entered.wait();
                self.release.wait();
            }
            match plan.mode {
                StepMode::Prefill => Ok(StepOutput::empty()),
                StepMode::Decode => StepOutput::new(&[
                    CommittedTokens::target(1).map_err(|_| RankExecutionError::Backend(-2))?
                ])
                .map_err(|_| RankExecutionError::Backend(-2)),
                _ => Err(RankExecutionError::Backend(-2)),
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

    struct DropCountingExecutor {
        drops: Arc<AtomicUsize>,
    }

    impl RankExecutor for DropCountingExecutor {
        fn execute(
            &mut self,
            _rank: u8,
            _plan: &StepPlan,
            _schedule: &CollectiveSchedule,
        ) -> Result<StepOutput, RankExecutionError> {
            Err(RankExecutionError::Backend(-1))
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

    impl Drop for DropCountingExecutor {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::AcqRel);
        }
    }

    #[test]
    fn backend_waits_for_runtime_readiness_and_cleans_failed_startup() {
        let root = temporary_store();
        let drops = Arc::new(AtomicUsize::new(0));
        let executors = std::array::from_fn(|_| {
            Box::new(DropCountingExecutor {
                drops: Arc::clone(&drops),
            }) as Box<dyn RankExecutor + Send>
        });
        let workers = Tp4WorkerPool::spawn(2, executors).unwrap();
        let result = CoordinatorApiBackend::spawn_with_tokenizer_inner(
            CoordinatorBackendConfig {
                command_capacity: 16,
                completion_event_capacity: 16,
                maximum_commands_per_tick: 16,
                idle_poll_interval: Duration::from_millis(1),
            },
            coordinator_with_workers(&root, workers),
            Arc::new(FakeTokenizer),
            "cpu-test",
            Some(RuntimeStartupFault::BeforeReady),
        );
        assert!(matches!(
            result,
            Err(CoordinatorBackendError::RuntimeStartup)
        ));
        assert_eq!(drops.load(Ordering::Acquire), 4);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn retained_admission_rollback_forces_fatal_signal_without_losing_owner() {
        let root = temporary_store();
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

        let mut coordinator = coordinator_with_workers_and_capacity(
            &root,
            Tp4WorkerPool::spawn_cpu(2, None).unwrap(),
            ResidencyConfig {
                hbm_bytes: page_bytes,
                dram_bytes: page_bytes,
            },
        );
        coordinator
            .prefix_cache
            .as_mut()
            .unwrap()
            .register_prefix(&tokens, vec![record])
            .unwrap();

        let counters = ServingMetrics::new();
        let owners = Mutex::new(BTreeMap::from([(50, 1)]));
        let mut active = BTreeMap::new();
        let mut pending_admissions = BTreeSet::new();
        let (events, completion) = mpsc::sync_channel(4);
        process_command(
            BackendCommand::Submit {
                request_id: 50,
                tenant: 1,
                maximum_output_tokens: 1,
                mtp_depth: 0,
                sampling: StepSampling::greedy(50),
                request_started_at: Instant::now(),
                enqueued_at: Instant::now(),
                tokens: tokens.clone().into_boxed_slice(),
                decoder: Box::new(FakeDecoder {
                    stop_on_x: false,
                    finished: false,
                }),
                events,
            },
            &mut coordinator,
            &mut active,
            &mut pending_admissions,
            &owners,
            &counters,
        )
        .unwrap();
        assert!(pending_admissions.contains(&50));
        assert!(coordinator.has_pending_admission(50));
        coordinator
            .prefix_cache
            .as_mut()
            .unwrap()
            .abort_pending_page_for_test(50, 0)
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        let poll_error = loop {
            match poll_pending_admission(
                50,
                &mut coordinator,
                &mut active,
                &mut pending_admissions,
                &owners,
                &counters,
            ) {
                Ok(false) => {
                    assert!(Instant::now() < deadline, "restore fault did not arrive");
                    thread::yield_now();
                }
                Err(error) => break error,
                Ok(true) => panic!("corrupt pending admission was forgotten"),
            }
        };
        assert_eq!(poll_error.code, "ADMISSION_ROLLBACK_FAILED");
        assert!(active.contains_key(&50));
        assert!(pending_admissions.contains(&50));
        assert_eq!(owners.lock().unwrap().get(&50), Some(&1));
        assert!(coordinator.has_pending_admission(50));
        assert_eq!(coordinator.retained_prompt_bytes(), 256);

        let cancel_error = process_command(
            BackendCommand::Cancel {
                request_id: 50,
                tenant: 1,
            },
            &mut coordinator,
            &mut active,
            &mut pending_admissions,
            &owners,
            &counters,
        )
        .unwrap_err();
        assert_eq!(cancel_error.code, "CANCELLATION_ROLLBACK_FAILED");
        assert!(active.contains_key(&50));
        assert!(pending_admissions.contains(&50));
        assert_eq!(owners.lock().unwrap().get(&50), Some(&1));
        assert!(coordinator.has_pending_admission(50));

        coordinator
            .prefix_cache
            .as_mut()
            .unwrap()
            .repair_pending_page_identity_for_test(50, 0)
            .unwrap();
        process_command(
            BackendCommand::Cancel {
                request_id: 50,
                tenant: 1,
            },
            &mut coordinator,
            &mut active,
            &mut pending_admissions,
            &owners,
            &counters,
        )
        .unwrap();
        assert!(!pending_admissions.contains(&50));
        assert!(!coordinator.has_pending_admission(50));
        dispatch_events(
            coordinator.drain_events(),
            &mut coordinator,
            &mut active,
            &mut pending_admissions,
            &owners,
            &counters,
        )
        .unwrap();
        assert!(!active.contains_key(&50));
        assert!(!owners.lock().unwrap().contains_key(&50));
        assert_eq!(coordinator.retained_prompt_bytes(), 0);
        assert!(matches!(
            completion.recv_timeout(Duration::from_secs(1)).unwrap(),
            ApiCompletionEvent::Failed(ApiBackendError {
                code: "REQUEST_CANCELLED",
                ..
            })
        ));

        drop(coordinator);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dispatch_cancellation_failure_preserves_request_for_fatal_drain() {
        let root = temporary_store();
        let mut coordinator = coordinator(&root);
        let counters = ServingMetrics::new();
        let owners = Mutex::new(BTreeMap::from([(60, 1)]));
        let mut active = BTreeMap::new();
        let mut pending_admissions = BTreeSet::new();
        let (events, completion) = mpsc::sync_channel(4);
        process_command(
            BackendCommand::Submit {
                request_id: 60,
                tenant: 1,
                maximum_output_tokens: 1,
                mtp_depth: 0,
                sampling: StepSampling::greedy(60),
                request_started_at: Instant::now(),
                enqueued_at: Instant::now(),
                tokens: vec![17].into_boxed_slice(),
                decoder: Box::new(FakeDecoder {
                    stop_on_x: false,
                    finished: false,
                }),
                events,
            },
            &mut coordinator,
            &mut active,
            &mut pending_admissions,
            &owners,
            &counters,
        )
        .unwrap();
        dispatch_events(
            coordinator.drain_events(),
            &mut coordinator,
            &mut active,
            &mut pending_admissions,
            &owners,
            &counters,
        )
        .unwrap();
        assert!(active.get(&60).unwrap().admitted_at.is_some());
        assert_eq!(owners.lock().unwrap().get(&60), Some(&1));

        coordinator.sequence_table_generation = u64::MAX;
        let error = dispatch_events(
            vec![RequestEvent::Token {
                request_id: 60,
                position: 1,
                token_id: 99,
                speculative: false,
                draft_ordinal: None,
            }],
            &mut coordinator,
            &mut active,
            &mut pending_admissions,
            &owners,
            &counters,
        )
        .unwrap_err();
        assert_eq!(error.code, "EVENT_CANCELLATION_FAILED");
        assert!(active.contains_key(&60));
        assert_eq!(owners.lock().unwrap().get(&60), Some(&1));
        assert!(coordinator.request_progress(60).is_some());
        assert!(matches!(completion.try_recv(), Err(TryRecvError::Empty)));

        coordinator.sequence_table_generation = 2;
        process_command(
            BackendCommand::Cancel {
                request_id: 60,
                tenant: 1,
            },
            &mut coordinator,
            &mut active,
            &mut pending_admissions,
            &owners,
            &counters,
        )
        .unwrap();
        assert!(!coordinator.tick().unwrap());
        dispatch_events(
            coordinator.drain_events(),
            &mut coordinator,
            &mut active,
            &mut pending_admissions,
            &owners,
            &counters,
        )
        .unwrap();
        assert!(!active.contains_key(&60));
        assert!(!owners.lock().unwrap().contains_key(&60));
        assert!(matches!(
            completion.recv_timeout(Duration::from_secs(1)).unwrap(),
            ApiCompletionEvent::Failed(ApiBackendError {
                code: "REQUEST_CANCELLED",
                ..
            })
        ));

        drop(coordinator);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn bounded_backend_runs_greedy_request_to_exact_length() {
        let root = temporary_store();
        let backend = backend(&root);
        let handle = backend.submit_chat(1, validated(2, None)).unwrap();
        let mut text = String::new();
        let usage = loop {
            match handle.events.recv_timeout(Duration::from_secs(2)).unwrap() {
                ApiCompletionEvent::TextDelta(delta) => text.push_str(&delta),
                ApiCompletionEvent::Finished {
                    finish_reason,
                    usage,
                } => {
                    assert_eq!(finish_reason, "length");
                    break usage;
                }
                ApiCompletionEvent::Failed(error) => panic!("unexpected failure: {error:?}"),
            }
        };
        assert_eq!(text, "xx");
        assert_eq!(
            usage,
            ApiUsage {
                prompt_tokens: 1,
                completion_tokens: 2,
                total_tokens: 3,
            }
        );
        let metrics = backend.metrics();
        for expected in [
            "glmaxx_backend_completed_total 1\n",
            "glmaxx_prefix_cached_tokens_total 0\n",
            "glmaxx_prompt_computed_tokens_total 1\n",
            "glmaxx_output_tokens_total 2\n",
            "glmaxx_collective_tp_bytes_total 96\n",
            "glmaxx_collective_dcp_query_bytes_total 96\n",
            "glmaxx_collective_dcp_candidate_bytes_total 64\n",
            "glmaxx_collective_dcp_partial_bytes_total 96\n",
            "glmaxx_collective_sampling_bytes_total 16\n",
            "glmaxx_scheduler_real_sequence_rows_total 3\n",
            "glmaxx_scheduler_bucket_sequence_rows_total 12\n",
            "glmaxx_scheduler_real_query_rows_total 3\n",
            "glmaxx_scheduler_bucket_query_rows_total 72\n",
            "glmaxx_tokenization_time_us_count 1\n",
            "glmaxx_queue_time_us_count 1\n",
            "glmaxx_prefix_resolution_time_us_count 1\n",
            "glmaxx_ttft_us_count 1\n",
            "glmaxx_itl_us_count 1\n",
            "glmaxx_request_time_us_count 1\n",
            "glmaxx_step_worker_round_trip_us_prefill_count 1\n",
            "glmaxx_step_worker_round_trip_us_decode_count 2\n",
            "glmaxx_graph_selections_total{graph_id=\"1\",mode=\"prefill\"} 1\n",
            "glmaxx_graph_selections_total{graph_id=\"2\",mode=\"decode\"} 2\n",
        ] {
            assert!(metrics.contains(expected), "missing metric: {expected}");
        }
        drop(backend);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn text_stop_cancels_remaining_model_work_without_leaking_stop() {
        let root = temporary_store();
        let backend = backend(&root);
        let handle = backend.submit_chat(1, validated(100, Some("x"))).unwrap();
        assert_eq!(
            handle.events.recv_timeout(Duration::from_secs(2)).unwrap(),
            ApiCompletionEvent::Finished {
                finish_reason: "stop".to_owned(),
                usage: ApiUsage {
                    prompt_tokens: 1,
                    completion_tokens: 1,
                    total_tokens: 2,
                },
            }
        );
        let metrics = backend.metrics();
        assert!(metrics.contains("glmaxx_output_tokens_total 1\n"));
        assert!(metrics.contains("glmaxx_termination_stop_total 1\n"));
        drop(backend);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cancellation_is_tenant_bound_and_reaches_completion_waiter() {
        let root = temporary_store();
        let backend = backend(&root);
        let handle = backend.submit_chat(1, validated(1_000, None)).unwrap();
        let error = backend.cancel(2, handle.request_id).unwrap_err();
        assert_eq!(error.code, "TENANT_MISMATCH");
        backend.cancel(1, handle.request_id).unwrap();
        let terminal = loop {
            match handle.events.recv_timeout(Duration::from_secs(2)).unwrap() {
                ApiCompletionEvent::TextDelta(_) => {}
                terminal => break terminal,
            }
        };
        assert!(matches!(
            terminal,
            ApiCompletionEvent::Failed(ApiBackendError {
                code: "REQUEST_CANCELLED",
                ..
            })
        ));
        drop(backend);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cancellation_survives_a_saturated_submission_queue() {
        let root = temporary_store();
        let entered = Arc::new(Barrier::new(5));
        let release = Arc::new(Barrier::new(5));
        let executors = std::array::from_fn(|_| {
            Box::new(FirstStepBlockingExecutor {
                entered: Arc::clone(&entered),
                release: Arc::clone(&release),
                blocked: false,
            }) as Box<dyn RankExecutor + Send>
        });
        let workers = Tp4WorkerPool::spawn(2, executors).unwrap();
        let backend = backend_with(
            coordinator_with_workers(&root, workers),
            CoordinatorBackendConfig {
                command_capacity: 1,
                completion_event_capacity: 16,
                maximum_commands_per_tick: 1,
                idle_poll_interval: Duration::from_millis(1),
            },
        );
        let first = backend.submit_chat(1, validated(1, None)).unwrap();
        entered.wait();
        let queued = backend.submit_chat(2, validated(100, None)).unwrap();

        backend.cancel(2, queued.request_id).unwrap();
        backend.cancel(2, queued.request_id).unwrap();
        assert_eq!(
            backend
                .cancellations
                .lock()
                .unwrap()
                .get(&queued.request_id),
            Some(&CancellationState::Requested)
        );
        assert_eq!(backend.cancellations.lock().unwrap().len(), 1);
        release.wait();

        assert!(matches!(
            terminal_event(&queued),
            ApiCompletionEvent::Failed(ApiBackendError {
                code: "REQUEST_CANCELLED",
                ..
            })
        ));
        assert!(matches!(
            terminal_event(&first),
            ApiCompletionEvent::Finished {
                finish_reason,
                usage: ApiUsage {
                    completion_tokens: 1,
                    ..
                },
            } if finish_reason == "length"
        ));
        let deadline = Instant::now() + Duration::from_secs(2);
        while !backend.cancellations.lock().unwrap().is_empty() {
            assert!(
                Instant::now() < deadline,
                "delivered cancellation was not pruned"
            );
            thread::yield_now();
        }
        assert!(
            backend
                .metrics()
                .contains("glmaxx_backend_active_requests 0\n")
        );
        drop(backend);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn probabilistic_requests_fail_closed_before_admission() {
        let root = temporary_store();
        let backend = backend(&root);
        let request: ChatCompletionRequest = serde_json::from_str(
            r#"{"model":"glm-5.2","messages":[{"role":"user","content":"hi"}],"temperature":1}"#,
        )
        .unwrap();
        let error = match backend.submit_chat(1, request.validate().unwrap()) {
            Ok(_) => panic!("probabilistic request must fail closed"),
            Err(error) => error,
        };
        assert_eq!(error.code, "SAMPLING_ABI_NOT_PROMOTED");
        assert!(
            backend
                .metrics()
                .contains("glmaxx_backend_submitted_total 0")
        );
        drop(backend);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn slow_completion_receiver_is_cancelled_without_blocking_runtime() {
        let root = temporary_store();
        let backend = backend(&root);
        let _handle = backend.submit_chat(1, validated(1_000, None)).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !backend
            .metrics()
            .contains("glmaxx_backend_slow_consumers_total 1")
        {
            assert!(
                std::time::Instant::now() < deadline,
                "slow receiver was not isolated before the deadline"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(
            backend
                .metrics()
                .contains("glmaxx_backend_active_requests 0")
        );
        drop(backend);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn concurrent_tenants_complete_with_exact_lifecycle_totals() {
        let root = temporary_store();
        let backend = backend(&root);
        let handles = [
            backend.submit_chat(1, validated(1, None)).unwrap(),
            backend.submit_chat(2, validated(2, None)).unwrap(),
            backend.submit_chat(1, validated(3, None)).unwrap(),
            backend.submit_chat(2, validated(4, None)).unwrap(),
        ];
        for (handle, expected_completion_tokens) in handles.iter().zip(1_u32..=4) {
            assert_eq!(
                terminal_event(handle),
                ApiCompletionEvent::Finished {
                    finish_reason: "length".to_owned(),
                    usage: ApiUsage {
                        prompt_tokens: 1,
                        completion_tokens: expected_completion_tokens,
                        total_tokens: expected_completion_tokens + 1,
                    },
                }
            );
        }

        let metrics = backend.metrics();
        for expected in [
            "glmaxx_backend_submitted_total 4\n",
            "glmaxx_backend_completed_total 4\n",
            "glmaxx_backend_cancelled_total 0\n",
            "glmaxx_backend_failed_total 0\n",
            "glmaxx_backend_active_requests 0\n",
            "glmaxx_output_tokens_total 10\n",
            "glmaxx_ttft_us_count 4\n",
            "glmaxx_itl_us_count 6\n",
            "glmaxx_request_time_us_count 4\n",
        ] {
            assert!(metrics.contains(expected), "missing metric: {expected}");
        }
        drop(backend);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn slow_consumer_does_not_block_a_concurrent_peer() {
        let root = temporary_store();
        let backend = backend_with(
            coordinator(&root),
            CoordinatorBackendConfig {
                command_capacity: 16,
                completion_event_capacity: 8,
                maximum_commands_per_tick: 16,
                idle_poll_interval: Duration::from_millis(1),
            },
        );
        let _slow = backend.submit_chat(1, validated(1_000, None)).unwrap();
        let peer = backend.submit_chat(2, validated(4, None)).unwrap();
        assert_eq!(
            terminal_event(&peer),
            ApiCompletionEvent::Finished {
                finish_reason: "length".to_owned(),
                usage: ApiUsage {
                    prompt_tokens: 1,
                    completion_tokens: 4,
                    total_tokens: 5,
                },
            }
        );

        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let metrics = backend.metrics();
            if metrics.contains("glmaxx_backend_slow_consumers_total 1\n")
                && metrics.contains("glmaxx_backend_active_requests 0\n")
            {
                assert!(metrics.contains("glmaxx_backend_completed_total 1\n"));
                assert!(metrics.contains("glmaxx_backend_cancelled_total 1\n"));
                assert!(metrics.contains("glmaxx_backend_failed_total 0\n"));
                break;
            }
            assert!(
                Instant::now() < deadline,
                "slow consumer was not isolated while its peer completed"
            );
            std::thread::yield_now();
        }
        drop(backend);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fatal_step_fails_active_and_queued_requests_with_structured_events() {
        let root = temporary_store();
        let entered = Arc::new(Barrier::new(5));
        let release = Arc::new(Barrier::new(5));
        let executors = std::array::from_fn(|_| {
            Box::new(GatedFailExecutor {
                entered: Arc::clone(&entered),
                release: Arc::clone(&release),
            }) as Box<dyn RankExecutor + Send>
        });
        let workers = Tp4WorkerPool::spawn(2, executors).unwrap();
        let backend = backend_with(
            coordinator_with_workers(&root, workers),
            CoordinatorBackendConfig {
                command_capacity: 16,
                completion_event_capacity: 16,
                maximum_commands_per_tick: 1,
                idle_poll_interval: Duration::from_millis(1),
            },
        );
        let first = backend.submit_chat(1, validated(8, None)).unwrap();
        entered.wait();
        let release_guard = BarrierReleaseGuard::new(Arc::clone(&release));
        let handles = [
            first,
            backend.submit_chat(2, validated(8, None)).unwrap(),
            backend.submit_chat(1, validated(8, None)).unwrap(),
            backend.submit_chat(2, validated(8, None)).unwrap(),
        ];
        release_guard.release();

        for (index, handle) in handles.iter().enumerate() {
            let terminal = terminal_event(handle);
            let expected_code = if index == 0 {
                "ENGINE_REQUEST_FAILED"
            } else {
                "ENGINE_STEP_FAILED"
            };
            assert!(
                matches!(
                    terminal,
                    ApiCompletionEvent::Failed(ApiBackendError { code, .. }) if code == expected_code
                ),
                "unexpected terminal event: {terminal:?}"
            );
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while backend.health().state != ApiHealthState::Fatal {
            assert!(
                Instant::now() < deadline,
                "fatal worker result did not poison backend health"
            );
            std::thread::yield_now();
        }
        let metrics = backend.metrics();
        for expected in [
            "glmaxx_backend_submitted_total 4\n",
            "glmaxx_backend_completed_total 0\n",
            "glmaxx_backend_cancelled_total 0\n",
            "glmaxx_backend_failed_total 4\n",
            "glmaxx_backend_active_requests 0\n",
            "glmaxx_request_time_us_count 4\n",
            "glmaxx_step_total_time_us_prefill_count 0\n",
            "glmaxx_backend_fatal 1\n",
        ] {
            assert!(metrics.contains(expected), "missing metric: {expected}");
        }
        drop(backend);
        fs::remove_dir_all(root).unwrap();
    }
}
