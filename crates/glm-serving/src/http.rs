use std::{
    collections::BTreeMap,
    fmt,
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use glm_scheduler::SamplingCollective;
pub use glm_tokenizer::{ChatMessage, ChatRole, OrderedValue, ReasoningEffort};
use glm_tokenizer::{ChatTemplateError, ChatTemplateOptions, render_chat};

pub const GLMAXX_MODEL_ID: &str = "glm-5.2";
const MAXIMUM_MESSAGES: usize = 4_096;
const MAXIMUM_STOP_SEQUENCES: usize = 16;
const MAXIMUM_STOP_BYTES: usize = 256;
const MAXIMUM_TOOLS: usize = 128;
const MAXIMUM_TOP_K: u16 = 256;
const MAXIMUM_OUTPUT_TOKENS: u32 = 1_048_576;
const MAXIMUM_HEADER_BYTES: usize = 32 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiHealthState {
    Healthy,
    Fatal,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ApiHealth {
    pub state: ApiHealthState,
    pub model: &'static str,
    pub model_revision: &'static str,
    pub backend: &'static str,
    pub tp: u8,
    pub sm: u16,
}

impl ApiHealth {
    #[must_use]
    pub const fn production_healthy(backend: &'static str) -> Self {
        Self {
            state: ApiHealthState::Healthy,
            model: GLMAXX_MODEL_ID,
            model_revision: "b4734de4facf877f85769a911abafc5283eab3d9",
            backend,
            tp: 4,
            sm: 120,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(untagged)]
pub enum StopSequences {
    One(String),
    Many(Vec<String>),
}

impl StopSequences {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(value) => vec![value],
            Self::Many(values) => values,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default, alias = "max_completion_tokens")]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub top_k: Option<u16>,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub mtp_depth: Option<u8>,
    #[serde(default)]
    pub stop: Option<StopSequences>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub tools: Option<Vec<OrderedValue>>,
    #[serde(default)]
    pub tool_choice: Option<OrderedValue>,
    #[serde(default)]
    pub reasoning_effort: Option<ReasoningEffort>,
    #[serde(default)]
    pub enable_thinking: Option<bool>,
    #[serde(default)]
    pub clear_thinking: Option<bool>,
    #[serde(default)]
    pub user: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplingParameters {
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: Option<u16>,
    pub seed: Option<u64>,
}

impl SamplingParameters {
    #[must_use]
    pub fn collective(self) -> SamplingCollective {
        if self.temperature == 0.0 {
            SamplingCollective::Greedy
        } else if self.top_k.is_some() {
            SamplingCollective::TopK
        } else {
            SamplingCollective::Mass
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedChatRequest {
    pub messages: Vec<ChatMessage>,
    pub maximum_output_tokens: u32,
    pub sampling: SamplingParameters,
    pub mtp_depth: u8,
    pub stop: Vec<String>,
    pub stream: bool,
    pub tools: Option<Vec<OrderedValue>>,
    pub tool_choice: Option<OrderedValue>,
    pub template_options: ChatTemplateOptions,
    pub user: Option<String>,
}

impl ValidatedChatRequest {
    pub fn render_prompt(&self) -> Result<String, ChatTemplateError> {
        render_chat(&self.messages, self.tools.as_deref(), self.template_options)
    }
}

impl ChatCompletionRequest {
    pub fn validate(self) -> Result<ValidatedChatRequest, ApiRequestError> {
        if self.model != GLMAXX_MODEL_ID {
            return Err(ApiRequestError::new(
                404,
                "MODEL_NOT_FOUND",
                Some("model"),
                "only the pinned glm-5.2 model is available",
            ));
        }
        if self.messages.is_empty() || self.messages.len() > MAXIMUM_MESSAGES {
            return Err(ApiRequestError::new(
                400,
                "INVALID_MESSAGES",
                Some("messages"),
                "messages must contain between 1 and 4096 entries",
            ));
        }
        if self
            .messages
            .iter()
            .any(|message| message.content.contains('\0'))
            || self.messages.iter().any(|message| {
                message.content.is_empty()
                    && (message.role != ChatRole::Assistant || message.tool_calls.is_empty())
            })
        {
            return Err(ApiRequestError::new(
                400,
                "INVALID_MESSAGES",
                Some("messages"),
                "message content must be nonempty and contain no NUL bytes",
            ));
        }
        if self
            .tools
            .as_ref()
            .is_some_and(|tools| tools.is_empty() || tools.len() > MAXIMUM_TOOLS)
        {
            return Err(ApiRequestError::new(
                400,
                "INVALID_TOOLS",
                Some("tools"),
                "tools must contain between 1 and 128 definitions",
            ));
        }
        let maximum_output_tokens = self.max_tokens.unwrap_or(1_024);
        if maximum_output_tokens == 0 || maximum_output_tokens > MAXIMUM_OUTPUT_TOKENS {
            return Err(ApiRequestError::new(
                400,
                "INVALID_MAX_TOKENS",
                Some("max_tokens"),
                "max_tokens must be in 1..=1048576",
            ));
        }
        let temperature = self.temperature.unwrap_or(1.0);
        if !temperature.is_finite() || !(0.0..=2.0).contains(&temperature) {
            return Err(ApiRequestError::new(
                400,
                "INVALID_TEMPERATURE",
                Some("temperature"),
                "temperature must be finite and in 0..=2",
            ));
        }
        let top_p = self.top_p.unwrap_or(1.0);
        if !(top_p.is_finite() && 0.0 < top_p && top_p <= 1.0) {
            return Err(ApiRequestError::new(
                400,
                "INVALID_TOP_P",
                Some("top_p"),
                "top_p must be finite and in (0,1]",
            ));
        }
        if self
            .top_k
            .is_some_and(|top_k| top_k == 0 || top_k > MAXIMUM_TOP_K)
        {
            return Err(ApiRequestError::new(
                400,
                "INVALID_TOP_K",
                Some("top_k"),
                "top_k must be in 1..=256",
            ));
        }
        if top_p < 1.0 && self.top_k.is_none() {
            return Err(ApiRequestError::new(
                400,
                "UNBOUNDED_TOP_P_UNSUPPORTED",
                Some("top_k"),
                "top_p below 1 requires an explicit top_k in 1..=256",
            ));
        }
        let mtp_depth = self.mtp_depth.unwrap_or(0);
        if mtp_depth > 6 {
            return Err(ApiRequestError::new(
                400,
                "INVALID_MTP_DEPTH",
                Some("mtp_depth"),
                "mtp_depth must be in 0..=6",
            ));
        }
        let stop = self.stop.map_or_else(Vec::new, StopSequences::into_vec);
        if stop.len() > MAXIMUM_STOP_SEQUENCES
            || stop
                .iter()
                .any(|value| value.is_empty() || value.len() > MAXIMUM_STOP_BYTES)
        {
            return Err(ApiRequestError::new(
                400,
                "INVALID_STOP",
                Some("stop"),
                "stop must contain at most 16 nonempty strings of at most 256 bytes",
            ));
        }
        let validated = ValidatedChatRequest {
            messages: self.messages,
            maximum_output_tokens,
            sampling: SamplingParameters {
                temperature,
                top_p,
                top_k: self.top_k,
                seed: self.seed,
            },
            mtp_depth,
            stop,
            stream: self.stream,
            tools: self.tools,
            tool_choice: self.tool_choice,
            template_options: ChatTemplateOptions {
                reasoning_effort: self.reasoning_effort.unwrap_or_default(),
                enable_thinking: self.enable_thinking.unwrap_or(true),
                clear_thinking: self.clear_thinking,
                add_generation_prompt: true,
            },
            user: self.user,
        };
        validated.render_prompt().map_err(|error| {
            ApiRequestError::new(
                400,
                "INVALID_CHAT_TEMPLATE_INPUT",
                Some("messages"),
                error.to_string(),
            )
        })?;
        Ok(validated)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ApiUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApiCompletionEvent {
    TextDelta(String),
    Finished {
        finish_reason: String,
        usage: ApiUsage,
    },
    Failed(ApiBackendError),
}

pub struct ApiCompletionHandle {
    pub request_id: u64,
    pub events: Receiver<ApiCompletionEvent>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiBackendError {
    pub code: &'static str,
    pub message: String,
}

pub trait ApiBackend: Send + Sync + 'static {
    fn health(&self) -> ApiHealth;
    fn metrics(&self) -> String;
    fn submit_chat(
        &self,
        tenant: u32,
        request: ValidatedChatRequest,
    ) -> Result<ApiCompletionHandle, ApiBackendError>;
    fn cancel(&self, tenant: u32, request_id: u64) -> Result<(), ApiBackendError>;
}

#[derive(Clone, Debug)]
pub struct ApiServerConfig {
    pub bind: SocketAddr,
    pub api_keys: BTreeMap<String, u32>,
    pub connection_workers: usize,
    pub connection_queue_capacity: usize,
    pub maximum_body_bytes: usize,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub request_timeout: Duration,
    pub maximum_buffered_response_bytes: usize,
}

impl ApiServerConfig {
    pub fn validate(&self) -> Result<(), ApiServerError> {
        if self.api_keys.is_empty()
            || self
                .api_keys
                .iter()
                .any(|(key, &tenant)| key.is_empty() || tenant == 0)
            || self.connection_workers == 0
            || self.connection_queue_capacity == 0
            || self.maximum_body_bytes == 0
            || self.maximum_body_bytes > 16 * 1024 * 1024
            || self.read_timeout.is_zero()
            || self.write_timeout.is_zero()
            || self.request_timeout.is_zero()
            || self.request_timeout > Duration::from_secs(24 * 60 * 60)
            || self.maximum_buffered_response_bytes == 0
            || self.maximum_buffered_response_bytes > 256 * 1024 * 1024
        {
            return Err(ApiServerError::Config);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ApiErrorBody {
    pub error: ApiErrorDetail,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ApiErrorDetail {
    pub message: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub param: Option<String>,
    pub code: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiRequestError {
    status: u16,
    body: ApiErrorBody,
}

impl ApiRequestError {
    fn new(
        status: u16,
        code: &'static str,
        param: Option<&str>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            status,
            body: ApiErrorBody {
                error: ApiErrorDetail {
                    message: message.into(),
                    kind: "invalid_request_error",
                    param: param.map(str::to_owned),
                    code,
                },
            },
        }
    }
}

pub struct ApiHttpServer {
    local_addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    accept_thread: Option<JoinHandle<()>>,
    worker_threads: Vec<JoinHandle<()>>,
}

impl ApiHttpServer {
    pub fn bind(
        config: ApiServerConfig,
        backend: Arc<dyn ApiBackend>,
    ) -> Result<Self, ApiServerError> {
        config.validate()?;
        if backend.health().state != ApiHealthState::Healthy {
            return Err(ApiServerError::BackendNotHealthy);
        }
        let listener = TcpListener::bind(config.bind)?;
        let local_addr = listener.local_addr()?;
        let (sender, receiver) = mpsc::sync_channel(config.connection_queue_capacity);
        let receiver = Arc::new(Mutex::new(receiver));
        let shutdown = Arc::new(AtomicBool::new(false));
        let shared_config = Arc::new(config);

        let mut worker_threads = Vec::with_capacity(shared_config.connection_workers);
        for worker_id in 0..shared_config.connection_workers {
            let worker_receiver = Arc::clone(&receiver);
            let worker_backend = Arc::clone(&backend);
            let worker_config = Arc::clone(&shared_config);
            worker_threads.push(
                thread::Builder::new()
                    .name(format!("glmaxx-http-{worker_id}"))
                    .spawn(move || {
                        worker_loop(worker_receiver, worker_backend, worker_config);
                    })?,
            );
        }

        let accept_shutdown = Arc::clone(&shutdown);
        let accept_config = Arc::clone(&shared_config);
        let accept_thread = thread::Builder::new()
            .name("glmaxx-http-accept".to_owned())
            .spawn(move || {
                accept_loop(listener, sender, accept_shutdown, accept_config);
            })?;
        Ok(Self {
            local_addr,
            shutdown,
            accept_thread: Some(accept_thread),
            worker_threads,
        })
    }

    #[must_use]
    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

impl Drop for ApiHttpServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        let _ = TcpStream::connect_timeout(&self.local_addr, Duration::from_millis(100));
        if let Some(thread) = self.accept_thread.take() {
            let _ = thread.join();
        }
        for thread in self.worker_threads.drain(..) {
            let _ = thread.join();
        }
    }
}

fn accept_loop(
    listener: TcpListener,
    sender: SyncSender<TcpStream>,
    shutdown: Arc<AtomicBool>,
    config: Arc<ApiServerConfig>,
) {
    while !shutdown.load(Ordering::Acquire) {
        let Ok((stream, _)) = listener.accept() else {
            continue;
        };
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        let _ = stream.set_read_timeout(Some(config.read_timeout));
        let _ = stream.set_write_timeout(Some(config.write_timeout));
        let _ = stream.set_nodelay(true);
        match sender.try_send(stream) {
            Ok(()) => {}
            Err(TrySendError::Full(mut rejected)) => {
                let error = ApiRequestError::new(
                    503,
                    "SERVER_OVERLOADED",
                    None,
                    "the bounded connection queue is full",
                );
                let _ = write_error(&mut rejected, &error);
            }
            Err(TrySendError::Disconnected(_)) => break,
        }
    }
}

fn worker_loop(
    receiver: Arc<Mutex<Receiver<TcpStream>>>,
    backend: Arc<dyn ApiBackend>,
    config: Arc<ApiServerConfig>,
) {
    loop {
        let stream = {
            let Ok(receiver) = receiver.lock() else {
                return;
            };
            receiver.recv()
        };
        let Ok(mut stream) = stream else {
            return;
        };
        if let Err(error) = handle_connection(&mut stream, backend.as_ref(), &config) {
            let _ = write_error(&mut stream, &error);
        }
    }
}

fn handle_connection(
    stream: &mut TcpStream,
    backend: &dyn ApiBackend,
    config: &ApiServerConfig,
) -> Result<(), ApiRequestError> {
    let request = read_http_request(stream, config.maximum_body_bytes)?;
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/health") => {
            let health = backend.health();
            let status = if health.state == ApiHealthState::Healthy {
                200
            } else {
                503
            };
            write_json(stream, status, &health).map_err(internal_io)
        }
        ("GET", "/metrics") => {
            if backend.health().state != ApiHealthState::Healthy {
                return Err(backend_unavailable());
            }
            write_response(
                stream,
                200,
                "text/plain; version=0.0.4",
                backend.metrics().as_bytes(),
            )
            .map_err(internal_io)
        }
        ("POST", "/v1/chat/completions") => {
            if backend.health().state != ApiHealthState::Healthy {
                return Err(backend_unavailable());
            }
            let tenant = authenticate(&request.headers, &config.api_keys)?;
            let wire: ChatCompletionRequest =
                serde_json::from_slice(&request.body).map_err(|_| {
                    ApiRequestError::new(
                        400,
                        "INVALID_JSON",
                        None,
                        "request body is not the supported chat-completions schema",
                    )
                })?;
            let validated = wire.validate()?;
            let stream_response = validated.stream;
            let handle = backend
                .submit_chat(tenant, validated)
                .map_err(backend_request_error)?;
            if stream_response {
                write_streaming_completion(stream, backend, tenant, handle, config.request_timeout)
            } else {
                write_buffered_completion(
                    stream,
                    backend,
                    tenant,
                    handle,
                    config.request_timeout,
                    config.maximum_buffered_response_bytes,
                )
            }
        }
        ("DELETE", path) if path.starts_with("/v1/requests/") => {
            let tenant = authenticate(&request.headers, &config.api_keys)?;
            let request_id = path
                .strip_prefix("/v1/requests/")
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|&value| value != 0)
                .ok_or_else(|| {
                    ApiRequestError::new(
                        400,
                        "INVALID_REQUEST_ID",
                        Some("request_id"),
                        "request id must be a nonzero unsigned integer",
                    )
                })?;
            backend
                .cancel(tenant, request_id)
                .map_err(backend_request_error)?;
            write_json(stream, 200, &json!({"id": request_id, "cancelled": true}))
                .map_err(internal_io)
        }
        _ => Err(ApiRequestError::new(
            404,
            "NOT_FOUND",
            None,
            "endpoint not found",
        )),
    }
}

fn write_buffered_completion(
    stream: &mut TcpStream,
    backend: &dyn ApiBackend,
    tenant: u32,
    handle: ApiCompletionHandle,
    timeout: Duration,
    maximum_response_bytes: usize,
) -> Result<(), ApiRequestError> {
    let request_id = handle.request_id;
    let deadline = Instant::now() + timeout;
    let mut content = String::new();
    loop {
        match recv_before(&handle.events, deadline) {
            Ok(ApiCompletionEvent::TextDelta(delta)) => {
                if content
                    .len()
                    .checked_add(delta.len())
                    .is_none_or(|bytes| bytes > maximum_response_bytes)
                {
                    let _ = backend.cancel(tenant, request_id);
                    return Err(ApiRequestError::new(
                        500,
                        "BACKEND_OUTPUT_LIMIT",
                        None,
                        "buffered completion exceeded its configured byte limit",
                    ));
                }
                content.push_str(&delta);
            }
            Ok(ApiCompletionEvent::Finished {
                finish_reason,
                usage,
            }) => {
                return write_json(
                    stream,
                    200,
                    &json!({
                        "id": format!("chatcmpl-{request_id}"),
                        "object": "chat.completion",
                        "created": unix_seconds(),
                        "model": GLMAXX_MODEL_ID,
                        "choices": [{
                            "index": 0,
                            "message": {"role": "assistant", "content": content},
                            "finish_reason": finish_reason
                        }],
                        "usage": usage
                    }),
                )
                .map_err(internal_io);
            }
            Ok(ApiCompletionEvent::Failed(error)) => {
                return Err(backend_request_error(error));
            }
            Err(error) => {
                let _ = backend.cancel(tenant, request_id);
                return Err(error);
            }
        }
    }
}

fn write_streaming_completion(
    stream: &mut TcpStream,
    backend: &dyn ApiBackend,
    tenant: u32,
    handle: ApiCompletionHandle,
    timeout: Duration,
) -> Result<(), ApiRequestError> {
    write_stream_headers(stream).map_err(internal_io)?;
    let request_id = handle.request_id;
    let created = unix_seconds();
    let first = json!({
        "id": format!("chatcmpl-{request_id}"),
        "object": "chat.completion.chunk",
        "created": created,
        "model": GLMAXX_MODEL_ID,
        "choices": [{"index": 0, "delta": {"role": "assistant"}, "finish_reason": null}]
    });
    if write_sse(stream, &first).is_err() {
        let _ = backend.cancel(tenant, request_id);
        return Ok(());
    }
    let deadline = Instant::now() + timeout;
    loop {
        match recv_before(&handle.events, deadline) {
            Ok(ApiCompletionEvent::TextDelta(delta)) => {
                let chunk = json!({
                    "id": format!("chatcmpl-{request_id}"),
                    "object": "chat.completion.chunk",
                    "created": created,
                    "model": GLMAXX_MODEL_ID,
                    "choices": [{"index": 0, "delta": {"content": delta}, "finish_reason": null}]
                });
                if write_sse(stream, &chunk).is_err() {
                    let _ = backend.cancel(tenant, request_id);
                    return Ok(());
                }
            }
            Ok(ApiCompletionEvent::Finished {
                finish_reason,
                usage,
            }) => {
                let final_chunk = json!({
                    "id": format!("chatcmpl-{request_id}"),
                    "object": "chat.completion.chunk",
                    "created": created,
                    "model": GLMAXX_MODEL_ID,
                    "choices": [{"index": 0, "delta": {}, "finish_reason": finish_reason}],
                    "usage": usage
                });
                let _ = write_sse(stream, &final_chunk);
                let _ = stream.write_all(b"data: [DONE]\n\n");
                let _ = stream.flush();
                return Ok(());
            }
            Ok(ApiCompletionEvent::Failed(error)) => {
                let payload = json!({"error": {
                    "message": error.message,
                    "type": "server_error",
                    "param": null,
                    "code": error.code
                }});
                let _ = write_sse(stream, &payload);
                let _ = stream.write_all(b"data: [DONE]\n\n");
                let _ = stream.flush();
                return Ok(());
            }
            Err(error) => {
                let _ = backend.cancel(tenant, request_id);
                let _ = write_sse(stream, &json!(error.body));
                let _ = stream.write_all(b"data: [DONE]\n\n");
                let _ = stream.flush();
                return Ok(());
            }
        }
    }
}

fn recv_before(
    receiver: &Receiver<ApiCompletionEvent>,
    deadline: Instant,
) -> Result<ApiCompletionEvent, ApiRequestError> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(timeout_error());
    }
    match receiver.recv_timeout(remaining) {
        Ok(event) => Ok(event),
        Err(RecvTimeoutError::Timeout) => Err(timeout_error()),
        Err(RecvTimeoutError::Disconnected) => Err(ApiRequestError::new(
            500,
            "BACKEND_DISCONNECTED",
            None,
            "completion backend closed without a terminal event",
        )),
    }
}

struct HttpRequest {
    method: String,
    path: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

fn read_http_request(
    stream: &mut TcpStream,
    maximum_body_bytes: usize,
) -> Result<HttpRequest, ApiRequestError> {
    let mut bytes = Vec::with_capacity(4_096);
    let mut buffer = [0_u8; 4_096];
    let header_end = loop {
        if let Some(offset) = find_bytes(&bytes, b"\r\n\r\n") {
            break offset + 4;
        }
        if bytes.len() >= MAXIMUM_HEADER_BYTES {
            return Err(ApiRequestError::new(
                431,
                "HEADERS_TOO_LARGE",
                None,
                "HTTP headers exceed 32768 bytes",
            ));
        }
        let read = stream.read(&mut buffer).map_err(bad_http_io)?;
        if read == 0 {
            return Err(ApiRequestError::new(
                400,
                "TRUNCATED_HTTP",
                None,
                "connection closed before HTTP headers completed",
            ));
        }
        bytes.extend_from_slice(&buffer[..read]);
    };
    let header = std::str::from_utf8(&bytes[..header_end - 4]).map_err(|_| {
        ApiRequestError::new(
            400,
            "INVALID_HTTP",
            None,
            "HTTP headers must be valid UTF-8",
        )
    })?;
    let mut lines = header.split("\r\n");
    let mut request_line = lines
        .next()
        .ok_or_else(|| ApiRequestError::new(400, "INVALID_HTTP", None, "missing request line"))?
        .split_ascii_whitespace();
    let method = request_line.next().unwrap_or_default();
    let path = request_line.next().unwrap_or_default();
    let version = request_line.next().unwrap_or_default();
    if method.is_empty()
        || path.is_empty()
        || version != "HTTP/1.1"
        || request_line.next().is_some()
        || !path.starts_with('/')
    {
        return Err(ApiRequestError::new(
            400,
            "INVALID_HTTP",
            None,
            "invalid HTTP/1.1 request line",
        ));
    }
    let method = method.to_owned();
    let path = path.to_owned();
    let mut headers = BTreeMap::new();
    for line in lines {
        let (name, value) = line.split_once(':').ok_or_else(|| {
            ApiRequestError::new(400, "INVALID_HTTP", None, "malformed HTTP header")
        })?;
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim().to_owned();
        if name.is_empty() || headers.insert(name, value).is_some() {
            return Err(ApiRequestError::new(
                400,
                "INVALID_HTTP",
                None,
                "empty or duplicate HTTP header",
            ));
        }
    }
    if headers.contains_key("transfer-encoding") {
        return Err(ApiRequestError::new(
            400,
            "UNSUPPORTED_TRANSFER_ENCODING",
            None,
            "chunked request bodies are not supported",
        ));
    }
    let body_length = match headers.get("content-length") {
        Some(value) => value.parse::<usize>().map_err(|_| {
            ApiRequestError::new(
                400,
                "INVALID_CONTENT_LENGTH",
                None,
                "Content-Length must be an unsigned integer",
            )
        })?,
        None => 0,
    };
    if body_length > maximum_body_bytes {
        return Err(ApiRequestError::new(
            413,
            "REQUEST_TOO_LARGE",
            None,
            "request body exceeds the configured limit",
        ));
    }
    let required = header_end
        .checked_add(body_length)
        .ok_or_else(|| ApiRequestError::new(413, "REQUEST_TOO_LARGE", None, "request too large"))?;
    while bytes.len() < required {
        let read = stream.read(&mut buffer).map_err(bad_http_io)?;
        if read == 0 {
            return Err(ApiRequestError::new(
                400,
                "TRUNCATED_HTTP",
                None,
                "connection closed before the declared body completed",
            ));
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    Ok(HttpRequest {
        method,
        path,
        headers,
        body: bytes[header_end..required].to_vec(),
    })
}

fn authenticate(
    headers: &BTreeMap<String, String>,
    api_keys: &BTreeMap<String, u32>,
) -> Result<u32, ApiRequestError> {
    let token = headers
        .get("authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty())
        .ok_or_else(|| {
            ApiRequestError::new(
                401,
                "INVALID_API_KEY",
                None,
                "a valid bearer token is required",
            )
        })?;
    api_keys.get(token).copied().ok_or_else(|| {
        ApiRequestError::new(
            401,
            "INVALID_API_KEY",
            None,
            "a valid bearer token is required",
        )
    })
}

fn write_stream_headers(stream: &mut TcpStream) -> io::Result<()> {
    stream.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\nX-Accel-Buffering: no\r\n\r\n",
    )
}

fn write_sse(stream: &mut TcpStream, value: &Value) -> io::Result<()> {
    stream.write_all(b"data: ")?;
    serde_json::to_writer(&mut *stream, value)?;
    stream.write_all(b"\n\n")?;
    stream.flush()
}

fn write_json(stream: &mut TcpStream, status: u16, value: &impl Serialize) -> io::Result<()> {
    let body = serde_json::to_vec(value)?;
    write_response(stream, status, "application/json", &body)
}

fn write_error(stream: &mut TcpStream, error: &ApiRequestError) -> io::Result<()> {
    write_json(stream, error.status, &error.body)
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status} {}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        reason_phrase(status),
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Error",
    }
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn backend_request_error(error: ApiBackendError) -> ApiRequestError {
    ApiRequestError::new(500, error.code, None, error.message)
}

fn backend_unavailable() -> ApiRequestError {
    ApiRequestError::new(
        503,
        "ENGINE_NOT_HEALTHY",
        None,
        "the four-rank engine is not healthy",
    )
}

fn timeout_error() -> ApiRequestError {
    ApiRequestError::new(
        504,
        "REQUEST_TIMEOUT",
        None,
        "request exceeded its configured deadline",
    )
}

fn internal_io(error: io::Error) -> ApiRequestError {
    ApiRequestError::new(500, "HTTP_IO_ERROR", None, error.to_string())
}

fn bad_http_io(error: io::Error) -> ApiRequestError {
    let status = if matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    ) {
        408
    } else {
        400
    };
    ApiRequestError::new(status, "HTTP_READ_ERROR", None, error.to_string())
}

#[derive(Debug)]
pub enum ApiServerError {
    Config,
    BackendNotHealthy,
    Io(io::Error),
}

impl fmt::Display for ApiServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ApiServerError {}

impl From<io::Error> for ApiServerError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    };

    use super::*;

    struct MockBackend {
        health: Mutex<ApiHealthState>,
        next_id: AtomicU64,
        submitted: Mutex<Vec<(u32, ValidatedChatRequest)>>,
        cancelled: Mutex<Vec<(u32, u64)>>,
    }

    impl MockBackend {
        fn healthy() -> Arc<Self> {
            Arc::new(Self {
                health: Mutex::new(ApiHealthState::Healthy),
                next_id: AtomicU64::new(1),
                submitted: Mutex::new(Vec::new()),
                cancelled: Mutex::new(Vec::new()),
            })
        }
    }

    impl ApiBackend for MockBackend {
        fn health(&self) -> ApiHealth {
            let state = *self.health.lock().unwrap();
            ApiHealth {
                state,
                model: GLMAXX_MODEL_ID,
                model_revision: "b4734de4facf877f85769a911abafc5283eab3d9",
                backend: "test",
                tp: 4,
                sm: 120,
            }
        }

        fn metrics(&self) -> String {
            "glmaxx_test_requests_total 1\n".to_owned()
        }

        fn submit_chat(
            &self,
            tenant: u32,
            request: ValidatedChatRequest,
        ) -> Result<ApiCompletionHandle, ApiBackendError> {
            self.submitted.lock().unwrap().push((tenant, request));
            let request_id = self.next_id.fetch_add(1, Ordering::Relaxed);
            let (sender, events) = mpsc::channel();
            sender
                .send(ApiCompletionEvent::TextDelta("hello".to_owned()))
                .unwrap();
            sender
                .send(ApiCompletionEvent::Finished {
                    finish_reason: "stop".to_owned(),
                    usage: ApiUsage {
                        prompt_tokens: 2,
                        completion_tokens: 1,
                        total_tokens: 3,
                    },
                })
                .unwrap();
            Ok(ApiCompletionHandle { request_id, events })
        }

        fn cancel(&self, tenant: u32, request_id: u64) -> Result<(), ApiBackendError> {
            self.cancelled.lock().unwrap().push((tenant, request_id));
            Ok(())
        }
    }

    fn config() -> ApiServerConfig {
        ApiServerConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            api_keys: BTreeMap::from([("secret".to_owned(), 7)]),
            connection_workers: 2,
            connection_queue_capacity: 4,
            maximum_body_bytes: 64 * 1024,
            read_timeout: Duration::from_secs(2),
            write_timeout: Duration::from_secs(2),
            request_timeout: Duration::from_secs(2),
            maximum_buffered_response_bytes: 64 * 1024,
        }
    }

    fn request(addr: SocketAddr, method: &str, path: &str, auth: bool, body: &str) -> String {
        let mut stream = TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let authorization = if auth {
            "Authorization: Bearer secret\r\n"
        } else {
            ""
        };
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\n{authorization}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        stream.shutdown(std::net::Shutdown::Write).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }

    fn chat_json(stream: bool, extra: &str) -> String {
        format!(
            r#"{{"model":"glm-5.2","messages":[{{"role":"user","content":"hi"}}],"stream":{stream}{extra}}}"#
        )
    }

    fn bind_or_skip(backend: Arc<MockBackend>) -> Option<ApiHttpServer> {
        match ApiHttpServer::bind(config(), backend) {
            Ok(server) => Some(server),
            Err(ApiServerError::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied => {
                None
            }
            Err(error) => panic!("unexpected server bind failure: {error}"),
        }
    }

    #[test]
    fn request_validation_enforces_bounded_top_p_contract() {
        let request: ChatCompletionRequest =
            serde_json::from_str(&chat_json(false, r#","top_p":0.9"#)).unwrap();
        let error = request.validate().unwrap_err();
        assert_eq!(error.body.error.code, "UNBOUNDED_TOP_P_UNSUPPORTED");

        let request: ChatCompletionRequest =
            serde_json::from_str(&chat_json(false, r#","top_p":0.9,"top_k":256"#)).unwrap();
        let validated = request.validate().unwrap();
        assert_eq!(validated.sampling.top_k, Some(256));
        assert_eq!(validated.sampling.collective(), SamplingCollective::TopK);
        assert_eq!(validated.maximum_output_tokens, 1_024);

        let greedy: ChatCompletionRequest =
            serde_json::from_str(&chat_json(false, r#","temperature":0,"mtp_depth":6"#)).unwrap();
        let greedy = greedy.validate().unwrap();
        assert_eq!(greedy.sampling.collective(), SamplingCollective::Greedy);
        assert_eq!(greedy.mtp_depth, 6);

        let tool_request: ChatCompletionRequest = serde_json::from_str(
            r#"{
                "model":"glm-5.2",
                "messages":[
                    {"role":"user","content":"call it"},
                    {"role":"assistant","content":"","tool_calls":[
                        {"function":{"name":"ordered","arguments":"{\"z\":1,\"a\":2}"}}
                    ]}
                ],
                "tools":[
                    {"type":"function","function":{
                        "name":"ordered",
                        "parameters":{"z":{"type":"number"},"a":{"type":"number"}},
                        "strict":true
                    }}
                ],
                "reasoning_effort":"high"
            }"#,
        )
        .unwrap();
        let rendered = tool_request.validate().unwrap().render_prompt().unwrap();
        assert!(
            rendered
                .contains(r#""parameters": {"z": {"type": "number"}, "a": {"type": "number"}}"#)
        );
        assert!(rendered.contains(
            "<arg_key>z</arg_key><arg_value>1</arg_value><arg_key>a</arg_key><arg_value>2</arg_value>"
        ));
    }

    #[test]
    fn server_refuses_to_bind_before_full_engine_health() {
        let backend = MockBackend::healthy();
        *backend.health.lock().unwrap() = ApiHealthState::Fatal;
        let result = ApiHttpServer::bind(config(), backend);
        assert!(matches!(result, Err(ApiServerError::BackendNotHealthy)));
    }

    #[test]
    fn buffered_chat_and_auth_are_openai_compatible() {
        let backend = MockBackend::healthy();
        let Some(server) = bind_or_skip(backend.clone()) else {
            return;
        };
        let unauthorized = request(
            server.local_addr(),
            "POST",
            "/v1/chat/completions",
            false,
            &chat_json(false, ""),
        );
        assert!(unauthorized.starts_with("HTTP/1.1 401 Unauthorized"));
        assert!(unauthorized.contains("\"code\":\"INVALID_API_KEY\""));

        let response = request(
            server.local_addr(),
            "POST",
            "/v1/chat/completions",
            true,
            &chat_json(false, r#","seed":42"#),
        );
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("\"object\":\"chat.completion\""));
        assert!(response.contains("\"content\":\"hello\""));
        let submitted = backend.submitted.lock().unwrap();
        assert_eq!(submitted.len(), 1);
        assert_eq!(submitted[0].0, 7);
        assert_eq!(submitted[0].1.sampling.seed, Some(42));
    }

    #[test]
    fn streaming_chat_emits_sse_and_done_sentinel() {
        let backend = MockBackend::healthy();
        let Some(server) = bind_or_skip(backend) else {
            return;
        };
        let response = request(
            server.local_addr(),
            "POST",
            "/v1/chat/completions",
            true,
            &chat_json(true, ""),
        );
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("Content-Type: text/event-stream"));
        assert!(response.contains("\"delta\":{\"role\":\"assistant\"}"));
        assert!(response.contains("\"delta\":{\"content\":\"hello\"}"));
        assert!(response.ends_with("data: [DONE]\n\n"));
    }

    #[test]
    fn buffered_output_is_bounded_and_cancels_with_tenant_identity() {
        let backend = MockBackend::healthy();
        let mut bounded = config();
        bounded.maximum_buffered_response_bytes = 4;
        let server = match ApiHttpServer::bind(bounded, backend.clone()) {
            Ok(server) => server,
            Err(ApiServerError::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied => {
                return;
            }
            Err(error) => panic!("unexpected server bind failure: {error}"),
        };
        let response = request(
            server.local_addr(),
            "POST",
            "/v1/chat/completions",
            true,
            &chat_json(false, ""),
        );
        assert!(response.starts_with("HTTP/1.1 500 Internal Server Error"));
        assert!(response.contains("\"code\":\"BACKEND_OUTPUT_LIMIT\""));
        assert_eq!(*backend.cancelled.lock().unwrap(), [(7, 1)]);
    }

    #[test]
    fn health_metrics_and_cancellation_are_bounded_endpoints() {
        let backend = MockBackend::healthy();
        let Some(server) = bind_or_skip(backend.clone()) else {
            return;
        };
        let health = request(server.local_addr(), "GET", "/health", false, "");
        assert!(health.starts_with("HTTP/1.1 200 OK"));
        assert!(health.contains("\"state\":\"healthy\""));

        let metrics = request(server.local_addr(), "GET", "/metrics", false, "");
        assert!(metrics.starts_with("HTTP/1.1 200 OK"));
        assert!(metrics.contains("glmaxx_test_requests_total 1"));

        let cancelled = request(server.local_addr(), "DELETE", "/v1/requests/9", true, "");
        assert!(cancelled.starts_with("HTTP/1.1 200 OK"));
        assert_eq!(*backend.cancelled.lock().unwrap(), [(7, 9)]);
    }
}
