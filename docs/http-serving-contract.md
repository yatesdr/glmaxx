# Minimal HTTP serving contract

Date: 2026-07-29

Status: CPU-tested protocol and bounded transport; no model-serving or
performance claim

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
seeds, temperature, top-p, bounded top-k, output limits, stop strings, tool
schema values, and user identity forwarding. Bearer keys map to fixed tenant
IDs before backend admission. Unknown request fields fail closed.

As required by `spec/engine-v0.md`, `top_p < 1` without an explicit
`top_k` in `1..=256` returns
`UNBOUNDED_TOP_P_UNSUPPORTED`. The server never silently substitutes a
candidate bound.

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
  deadline;
- one HTTP request per connection in the retained correctness transport.

The one-request-per-connection worker transport is a functional baseline, not
the final throughput route. Before production benchmarking it must be
replaced or qualified against a nonblocking transport while retaining this
parser, validation, authentication, deadline, and backpressure behavior.

## Remaining integration boundary

The production backend must render the pinned chat template, tokenize into
the fixed GLM vocabulary, admit through `ServingCoordinator`, detokenize
committed tokens, emit exact usage and finish reasons, propagate disconnect
cancellation at a collective-safe step boundary, and supply the required
observability registry. No current CLI command starts this server.
