# Minimal HTTP serving contract

Date: 2026-07-29

Status: CPU-tested protocol, bounded transport, and coordinator adapter; no
checkpoint-serving or performance claim

`glm-serving::ApiHttpServer` is the narrow HTTP/1.1 boundary for the fixed
GLM-5.2 engine. It is intentionally not a general OpenAI API implementation.
The server will bind only when its backend reports a healthy four-rank
SM120/TP4 engine. A CPU mock can exercise the transport in tests but cannot
produce a production health claim.

## Endpoints

- `POST /v1/chat/completions`
- `DELETE /v1/requests/{numeric-id}`
- `GET /health`
- `GET /metrics`

Chat completions support buffered JSON or server-sent events, deterministic
seeds, temperature, top-p, bounded top-k, output limits, stop strings,
order-preserving tool schema/call values, thinking controls, configurable
`mtp_depth` in `0..=6`, and user identity forwarding. Bearer keys map to
fixed tenant IDs before backend admission. Unknown request fields fail
closed.

As required by `spec/engine-v0.md`, `top_p < 1` without an explicit
`top_k` in `1..=256` returns
`UNBOUNDED_TOP_P_UNSUPPORTED`. The server never silently substitutes a
candidate bound.

Sampling is a per-request execution property. Decode scheduling cohorts
requests by MTP depth and collective route: greedy, bounded top-k, or
distributed mass. The compiler derives the collective directly from the
selected batch, so a process-wide setting cannot silently apply one user's
sampling route to another user and one TP rank cannot select a local
fallback.

## Bounds and failure behavior

- fixed connection worker count;
- bounded accepted-connection queue;
- configurable body limit capped at 16 MiB;
- 32-KiB header limit;
- read, write, and end-to-end completion deadlines;
- bounded buffered-response bytes;
- duplicate headers and request transfer encoding rejected;
- structured OpenAI-style error bodies;
- tenant-bound backend cancellation on explicit cancel, stream disconnect, or
  deadline, recorded independently of bounded submission-queue capacity and
  dispatched at the next collective-safe runtime poll;
- one HTTP request per connection in the retained correctness transport.

The one-request-per-connection worker transport is a functional baseline, not
the final throughput route. Before production benchmarking it must be
replaced or qualified against a nonblocking transport while retaining this
parser, validation, authentication, deadline, and backpressure behavior.

## Coordinator integration

`glm-tokenizer` now supplies the pinned template renderer, exact tokenizer
loader, padding-ID mask boundary, and incremental stop-safe detokenizer. The
CPU candidate `CoordinatorApiBackend` now connects those components to
`ServingCoordinator`, emits exact greedy usage and finish reasons, isolates
slow receivers, propagates tenant-bound cancellation at collective-safe step
boundaries through an owner-bound coalescing registry, and exposes bounded
request/step metrics. Host histograms cover
tokenization, queueing, prefix resolution, TTFT, ITL, graph selection,
scheduler padding, MTP acceptance, and collective bytes. Device and cache-tier
telemetry remain explicitly unqualified; see
[serving observability v1](serving-observability-v1.md). Exact greedy seed
state now enters the immutable four-rank `StepInput`. The adapter remains
fail-closed on probabilistic requests until rank output returns and the
coordinator atomically commits the reviewed final RNG counter.

Rank execution must still apply the padding mask before every distributed
sampling route. No current CLI command starts this server, and no current
health result represents a checkpoint-capable SM120 runtime.
