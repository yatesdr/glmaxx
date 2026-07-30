# Retained HTTP request ownership CPU proof v1

Date: 2026-07-29

Implementation commit:
`e2ab4d3f77575f46d6abfdf155772e764c3c115a`

Status: retained CPU HTTP correction passed; independent review pending

GPU claim: none

## Defects and corrections

The retained blocking HTTP adapter submits a streaming request before it
writes the initial SSE response headers. Previously, failure of that first
write returned an I/O error without dispatching cancellation for the active
backend request. No completion consumer remained, so request ownership was
left to later receiver-abandonment and fail-stop handling instead of being
released at the known disconnect boundary.

The corrected path extracts the assigned request ID before the first write.
If the streaming header write fails, it dispatches
`backend.cancel(tenant, request_id)` with the exact authenticated tenant and
assigned request identity, then returns without attempting a second HTTP
response on the broken stream. Cancellation dispatch itself can still fail;
the retained backend's receiver-abandonment and fatal-drain behavior remains
the final safety boundary.

The parser also previously checked the 32 KiB header limit only before each
fixed-size socket read. Its accepted byte count therefore depended on read
chunking: a read beginning below the limit could carry the header terminator
past byte 32,768 and still be accepted. Each header read is now capped to the
exact remaining allowance. Body reads are similarly capped to the remaining
declared `Content-Length`, and already-buffered bytes after the declared body
are rejected. The server is one-request-per-connection, so bytes arriving
later are closed with the connection; this proof claims rejection only for
trailing bytes already buffered by the header read.

Finally, accepted sockets are no longer queued unless the configured read
timeout, write timeout, and `TCP_NODELAY` setting all install successfully.
This keeps an unbounded or differently configured socket out of the bounded
worker pool.

## Distinguishing CPU proofs

`parser_enforces_exact_header_boundary_and_rejects_trailing_bytes` uses a
reader that returns at most 4,000 bytes. With the former implementation, a
32,769-byte header reached byte 32,000, the next read crossed the limit, and
the terminator was accepted. The corrected parser reads only the remaining
768 bytes and returns `HEADERS_TOO_LARGE`. The same test supplies a complete
one-byte body plus coalesced extra bytes and requires
`TRAILING_HTTP_BYTES`.

`streaming_header_failure_cancels_submitted_backend_request` uses a local TCP
pair and shuts down the server stream's write half before the initial SSE
headers. It proves the handler returns only after recording cancellation for
exactly tenant 7 and request 91. The former implementation returned
`HTTP_IO` with no cancellation dispatch and fails this regression.

The existing streaming, buffered-output, slow-consumer, cancellation,
bounded-endpoint, authentication, and fatal-drain tests remain green.

## Gate result and exclusions

The full local gate passed 261 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, the unchanged serving and cache
fixtures, and all 55 then-present review-handoff provenance proofs.

Commands:

```text
cargo test --offline -p glm-serving http::tests -- --nocapture
cargo clippy --offline -p glm-serving --all-targets -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-serving/src/http.rs
e05a4e828e94f2d1323fc80d0f86a46b5a5b7456450c54fb146635b091ba8941

crates/glm-serving/src/backend.rs
d4c1b2daaa6f6952d3c27158d33a0123abd891cef09ec894da006af8d7d7f8b0

docs/http-serving-contract.md
036de32f5dd515a7a01aa33ff982d52c37fcee1b37565f35fce1e4a00d197adc

docs/nonblocking-http-transport-v1.md
e1ee381ad46b9f277640e884380aaab11a6a5b23e4f87c7cdea05334e3ebddc5

docs/backend-event-cancellation-fatal-proof-v1.md
04794fb247b103e90d03a07e9827f13ce82d89e0a50dccb543c5e010f0f9bde5

docs/fable-coordinator-api-backend-v2-handoff.md
1443a9af63b908394ec087372bd995c9666a11f7ad30ff9859430de69452a9f2

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, or model execution
was used.

This correction does not implement the reviewed sharded epoll/eventfd
transport, keep-alive, HTTP pipelining, chunked request bodies, lossless
cancellation command delivery, syscall cancellation, checkpoint-backed
inference, or production concurrency/performance evidence. It proves only
exact retained-parser bounds, fail-closed accepted-socket configuration, and
correct cancellation dispatch when the initial streaming response cannot
take ownership of an already-submitted request.
