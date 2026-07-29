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
    time::Duration,
};

use glm_cache::MODEL_POSITIONS;
use glm_engine::{StartupCoordinator, StartupState};
use glm_scheduler::{RequestSpec, SamplingCollective};
use glm_tokenizer::{DecodeDelta, IncrementalDecoder, PinnedTokenizer, StreamFinish};

use crate::{
    AdmissionStatus, ApiBackend, ApiBackendError, ApiCompletionEvent, ApiCompletionHandle,
    ApiHealth, ApiHealthState, ApiUsage, RequestEvent, RequestFinishReason, ServingCoordinator,
    ValidatedChatRequest,
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
    Thread(std::io::Error),
}

impl fmt::Display for CoordinatorBackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for CoordinatorBackendError {}

struct BackendCounters {
    submitted: AtomicU64,
    rejected: AtomicU64,
    completed: AtomicU64,
    cancelled: AtomicU64,
    failed: AtomicU64,
    slow_consumers: AtomicU64,
}

impl BackendCounters {
    const fn new() -> Self {
        Self {
            submitted: AtomicU64::new(0),
            rejected: AtomicU64::new(0),
            completed: AtomicU64::new(0),
            cancelled: AtomicU64::new(0),
            failed: AtomicU64::new(0),
            slow_consumers: AtomicU64::new(0),
        }
    }
}

enum BackendCommand {
    Submit {
        request_id: u64,
        tenant: u32,
        maximum_output_tokens: u32,
        mtp_depth: u8,
        tokens: Box<[u32]>,
        decoder: Box<dyn OutputDecoder>,
        events: SyncSender<ApiCompletionEvent>,
    },
    Cancel {
        request_id: u64,
        tenant: u32,
    },
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
    completion_tokens: u32,
    decoder: Box<dyn OutputDecoder>,
    events: SyncSender<ApiCompletionEvent>,
}

/// Bounded production adapter between the HTTP API and the single-owner
/// continuous-batching coordinator.
///
/// The adapter is deliberately fail-closed to greedy sampling until the
/// reviewed `StepInput` sampling/RNG ABI is implemented by rank execution.
/// Accepting probabilistic parameters earlier would silently discard quality
/// inputs at the coordinator boundary.
pub struct CoordinatorApiBackend {
    health: ApiHealth,
    fatal: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    next_request_id: AtomicU64,
    command_sender: SyncSender<BackendCommand>,
    tokenizer: Arc<dyn RuntimeTokenizer>,
    completion_event_capacity: usize,
    owners: Arc<Mutex<BTreeMap<u64, u32>>>,
    counters: Arc<BackendCounters>,
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
        if startup.state() != StartupState::Healthy {
            return Err(CoordinatorBackendError::EngineNotHealthy);
        }
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
        config.validate()?;
        if backend_name.is_empty() {
            return Err(CoordinatorBackendError::Config);
        }
        let (command_sender, command_receiver) = mpsc::sync_channel(config.command_capacity);
        let fatal = Arc::new(AtomicBool::new(false));
        let shutdown = Arc::new(AtomicBool::new(false));
        let owners = Arc::new(Mutex::new(BTreeMap::new()));
        let counters = Arc::new(BackendCounters::new());
        let runtime_fatal = Arc::clone(&fatal);
        let runtime_shutdown = Arc::clone(&shutdown);
        let runtime_owners = Arc::clone(&owners);
        let runtime_counters = Arc::clone(&counters);
        let runtime_thread = thread::Builder::new()
            .name("glmaxx-serving-runtime".to_owned())
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    runtime_loop(
                        coordinator,
                        command_receiver,
                        config,
                        &runtime_shutdown,
                        &runtime_owners,
                        &runtime_counters,
                    )
                }));
                if result.is_err() || !runtime_shutdown.load(Ordering::Acquire) {
                    runtime_fatal.store(true, Ordering::Release);
                }
                if let Ok(mut owners) = runtime_owners.lock() {
                    owners.clear();
                }
            })
            .map_err(CoordinatorBackendError::Thread)?;
        Ok(Self {
            health: ApiHealth::production_healthy(backend_name),
            fatal,
            shutdown,
            next_request_id: AtomicU64::new(1),
            command_sender,
            tokenizer,
            completion_event_capacity: config.completion_event_capacity,
            owners,
            counters,
            runtime_thread: Some(runtime_thread),
        })
    }

    fn reject(&self, code: &'static str, message: impl Into<String>) -> ApiBackendError {
        self.counters.rejected.fetch_add(1, Ordering::Relaxed);
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
        format!(
            concat!(
                "glmaxx_backend_submitted_total {}\n",
                "glmaxx_backend_rejected_total {}\n",
                "glmaxx_backend_completed_total {}\n",
                "glmaxx_backend_cancelled_total {}\n",
                "glmaxx_backend_failed_total {}\n",
                "glmaxx_backend_slow_consumers_total {}\n",
                "glmaxx_backend_active_requests {}\n",
                "glmaxx_backend_fatal {}\n"
            ),
            self.counters.submitted.load(Ordering::Relaxed),
            self.counters.rejected.load(Ordering::Relaxed),
            self.counters.completed.load(Ordering::Relaxed),
            self.counters.cancelled.load(Ordering::Relaxed),
            self.counters.failed.load(Ordering::Relaxed),
            self.counters.slow_consumers.load(Ordering::Relaxed),
            active,
            u8::from(self.health().state == ApiHealthState::Fatal),
        )
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
        let tokens = self
            .tokenizer
            .encode_chat(&request)
            .map_err(|error| self.reject("TOKENIZATION_FAILED", error))?;
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
        {
            let mut owners = self
                .owners
                .lock()
                .map_err(|_| self.reject("ENGINE_STATE_FAILED", "request registry is poisoned"))?;
            if owners.insert(request_id, tenant).is_some() {
                return Err(self.reject("ENGINE_STATE_FAILED", "request id collision"));
            }
        }
        let command = BackendCommand::Submit {
            request_id,
            tenant,
            maximum_output_tokens: request.maximum_output_tokens,
            mtp_depth: request.mtp_depth,
            tokens,
            decoder,
            events: event_sender,
        };
        match self.command_sender.try_send(command) {
            Ok(()) => {
                self.counters.submitted.fetch_add(1, Ordering::Relaxed);
                Ok(ApiCompletionHandle { request_id, events })
            }
            Err(error) => {
                if let Ok(mut owners) = self.owners.lock() {
                    owners.remove(&request_id);
                }
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
        let owner = self
            .owners
            .lock()
            .map_err(|_| self.reject("ENGINE_STATE_FAILED", "request registry is poisoned"))?
            .get(&request_id)
            .copied();
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
        match self
            .command_sender
            .try_send(BackendCommand::Cancel { request_id, tenant })
        {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(self.reject(
                "ENGINE_OVERLOADED",
                "the bounded serving command queue is full",
            )),
            Err(TrySendError::Disconnected(_)) => {
                self.fatal.store(true, Ordering::Release);
                Err(self.reject("ENGINE_NOT_HEALTHY", "the serving runtime is disconnected"))
            }
        }
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
    shutdown: &AtomicBool,
    owners: &Mutex<BTreeMap<u64, u32>>,
    counters: &BackendCounters,
) {
    let mut active = BTreeMap::<u64, ActiveRequest>::new();
    let mut pending_admissions = BTreeSet::<u64>::new();
    while !shutdown.load(Ordering::Acquire) {
        let mut progressed = false;
        for _ in 0..config.maximum_commands_per_tick {
            match commands.try_recv() {
                Ok(command) => {
                    progressed = true;
                    process_command(
                        command,
                        &mut coordinator,
                        &mut active,
                        &mut pending_admissions,
                        owners,
                        counters,
                    );
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    fail_all(
                        &mut active,
                        owners,
                        counters,
                        "BACKEND_SHUTDOWN",
                        "the serving runtime command channel closed",
                    );
                    return;
                }
            }
        }

        for request_id in pending_admissions.iter().copied().collect::<Vec<_>>() {
            match coordinator.poll_admission(request_id) {
                Ok(AdmissionStatus::Pending) => {}
                Ok(AdmissionStatus::Admitted { .. }) => {
                    pending_admissions.remove(&request_id);
                    progressed = true;
                }
                Err(error) => {
                    pending_admissions.remove(&request_id);
                    fail_request(
                        request_id,
                        &mut active,
                        owners,
                        counters,
                        "ADMISSION_FAILED",
                        error.to_string(),
                    );
                    progressed = true;
                }
            }
        }

        match coordinator.tick() {
            Ok(did_work) => progressed |= did_work,
            Err(error) => {
                dispatch_events(
                    coordinator.drain_events(),
                    &mut coordinator,
                    &mut active,
                    &mut pending_admissions,
                    owners,
                    counters,
                );
                fail_all(
                    &mut active,
                    owners,
                    counters,
                    "ENGINE_STEP_FAILED",
                    &error.to_string(),
                );
                return;
            }
        }
        let events = coordinator.drain_events();
        if !events.is_empty() {
            progressed = true;
            dispatch_events(
                events,
                &mut coordinator,
                &mut active,
                &mut pending_admissions,
                owners,
                counters,
            );
        }

        if !progressed {
            match commands.recv_timeout(config.idle_poll_interval) {
                Ok(command) => process_command(
                    command,
                    &mut coordinator,
                    &mut active,
                    &mut pending_admissions,
                    owners,
                    counters,
                ),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    }
    fail_all(
        &mut active,
        owners,
        counters,
        "BACKEND_SHUTDOWN",
        "the serving runtime is shutting down",
    );
}

fn process_command(
    command: BackendCommand,
    coordinator: &mut ServingCoordinator,
    active: &mut BTreeMap<u64, ActiveRequest>,
    pending_admissions: &mut BTreeSet<u64>,
    owners: &Mutex<BTreeMap<u64, u32>>,
    counters: &BackendCounters,
) {
    match command {
        BackendCommand::Submit {
            request_id,
            tenant,
            maximum_output_tokens,
            mtp_depth,
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
                    return;
                }
            };
            active.insert(
                request_id,
                ActiveRequest {
                    tenant,
                    prompt_tokens,
                    completion_tokens: 0,
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
            match coordinator.begin_admit_tokens(spec, &tokens) {
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
                return;
            }
            pending_admissions.remove(&request_id);
            if coordinator.cancel(request_id).is_err() {
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

fn dispatch_events(
    events: Vec<RequestEvent>,
    coordinator: &mut ServingCoordinator,
    active: &mut BTreeMap<u64, ActiveRequest>,
    pending_admissions: &mut BTreeSet<u64>,
    owners: &Mutex<BTreeMap<u64, u32>>,
    counters: &BackendCounters,
) {
    for event in events {
        match event {
            RequestEvent::Admitted { .. } | RequestEvent::PrefillProgress { .. } => {}
            RequestEvent::Token {
                request_id,
                position,
                token_id,
                ..
            } => {
                let Some(mut request) = active.remove(&request_id) else {
                    continue;
                };
                if position != request.completion_tokens {
                    let _ = coordinator.cancel(request_id);
                    fail_sender(
                        request.events,
                        counters,
                        "OUTPUT_POSITION_MISMATCH",
                        "committed output positions are not contiguous",
                    );
                    remove_owner(owners, request_id);
                    continue;
                }
                request.completion_tokens += 1;
                match request.decoder.push(token_id) {
                    Ok(delta) => {
                        if !delta.text.is_empty()
                            && request
                                .events
                                .try_send(ApiCompletionEvent::TextDelta(delta.text))
                                .is_err()
                        {
                            counters.slow_consumers.fetch_add(1, Ordering::Relaxed);
                            let _ = coordinator.cancel(request_id);
                            remove_owner(owners, request_id);
                            continue;
                        }
                        if delta.finish.is_some() {
                            finish_request(request, request_id, "stop", counters, owners);
                            let _ = coordinator.cancel(request_id);
                        } else {
                            active.insert(request_id, request);
                        }
                    }
                    Err(error) => {
                        let _ = coordinator.cancel(request_id);
                        fail_sender(request.events, counters, "OUTPUT_DECODE_FAILED", error);
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
                            counters.slow_consumers.fetch_add(1, Ordering::Relaxed);
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
                        fail_sender(request.events, counters, "OUTPUT_DECODE_FAILED", error);
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
}

fn finish_request(
    request: ActiveRequest,
    request_id: u64,
    finish_reason: &'static str,
    counters: &BackendCounters,
    owners: &Mutex<BTreeMap<u64, u32>>,
) {
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
        counters.slow_consumers.fetch_add(1, Ordering::Relaxed);
    } else {
        counters.completed.fetch_add(1, Ordering::Relaxed);
    }
    remove_owner(owners, request_id);
}

fn fail_request(
    request_id: u64,
    active: &mut BTreeMap<u64, ActiveRequest>,
    owners: &Mutex<BTreeMap<u64, u32>>,
    counters: &BackendCounters,
    code: &'static str,
    message: impl Into<String>,
) {
    if let Some(request) = active.remove(&request_id) {
        fail_sender(request.events, counters, code, message);
    }
    remove_owner(owners, request_id);
}

fn cancel_request(
    request_id: u64,
    active: &mut BTreeMap<u64, ActiveRequest>,
    owners: &Mutex<BTreeMap<u64, u32>>,
    counters: &BackendCounters,
) {
    if let Some(request) = active.remove(&request_id) {
        let _ = request
            .events
            .try_send(ApiCompletionEvent::Failed(ApiBackendError {
                code: "REQUEST_CANCELLED",
                message: "request was cancelled".to_owned(),
            }));
        counters.cancelled.fetch_add(1, Ordering::Relaxed);
    }
    remove_owner(owners, request_id);
}

fn fail_sender(
    events: SyncSender<ApiCompletionEvent>,
    counters: &BackendCounters,
    code: &'static str,
    message: impl Into<String>,
) {
    let _ = events.try_send(ApiCompletionEvent::Failed(ApiBackendError {
        code,
        message: message.into(),
    }));
    counters.failed.fetch_add(1, Ordering::Relaxed);
}

fn fail_all(
    active: &mut BTreeMap<u64, ActiveRequest>,
    owners: &Mutex<BTreeMap<u64, u32>>,
    counters: &BackendCounters,
    code: &'static str,
    message: &str,
) {
    for (_, request) in std::mem::take(active) {
        fail_sender(request.events, counters, code, message);
    }
    if let Ok(mut owners) = owners.lock() {
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
        time::{SystemTime, UNIX_EPOCH},
    };

    use glm_cache::{
        FileTierStore, NamespaceInputs, PrefixIndex, PrefixNamespace, ResidencyConfig,
    };
    use glm_engine::{
        AttentionTransport, GraphEntry, GraphKey, GraphProfile, StepMode, Tp4WorkerPool,
    };
    use glm_scheduler::{RouteCatalog, SchedulerConfig, TenantConfig};

    use super::*;
    use crate::ChatCompletionRequest;

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
            "glmaxx-api-backend-test-{}-{nonce}",
            std::process::id()
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
            ResidencyConfig {
                hbm_bytes: 1,
                dram_bytes: 0,
            },
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
            Tp4WorkerPool::spawn_cpu(2, None).unwrap(),
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
        CoordinatorApiBackend::spawn_with_tokenizer(
            CoordinatorBackendConfig {
                command_capacity: 16,
                completion_event_capacity: 16,
                maximum_commands_per_tick: 16,
                idle_poll_interval: Duration::from_millis(1),
            },
            coordinator(store_root),
            Arc::new(FakeTokenizer),
            "cpu-test",
        )
        .unwrap()
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
        assert!(
            backend
                .metrics()
                .contains("glmaxx_backend_completed_total 1")
        );
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
}
