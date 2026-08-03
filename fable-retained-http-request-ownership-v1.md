# Fable review: retained HTTP request ownership v1

Date: 2026-07-31
Reviewer: Fable (adversarial design-gate review)
Handoff: `docs/fable-retained-http-request-ownership-v1-handoff.md`

Note: the handoff requests this result at the repository root; the operator
directed all review results into `docs/reviews/`, so it is written here.

## Reviewed candidate

Reviewed candidate commit (detached worktree, never moving `main`):

a7b1cc9a6cbae1d5abce75c672693759ac584794

Implementation commit under review within the candidate:
`e2ab4d3f77575f46d6abfdf155772e764c3c115a` ("Harden retained HTTP request
ownership"); the candidate commit adds only the proof document.

## Verified input hash table

Every pinned input was hashed with `shasum -a 256` in the detached worktree
at the candidate commit at review start and again at review finish; all
hashes matched the handoff at both points, and `glmaxx review-proof`
independently returned verdict PASS for the same table.

| Input | SHA-256 (verified start and finish) |
|---|---|
| `spec/engine-v0.md` | efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a |
| `crates/glm-serving/src/http.rs` | e05a4e828e94f2d1323fc80d0f86a46b5a5b7456450c54fb146635b091ba8941 |
| `crates/glm-serving/src/backend.rs` | d4c1b2daaa6f6952d3c27158d33a0123abd891cef09ec894da006af8d7d7f8b0 |
| `docs/http-serving-contract.md` | 036de32f5dd515a7a01aa33ff982d52c37fcee1b37565f35fce1e4a00d197adc |
| `docs/nonblocking-http-transport-v1.md` | e1ee381ad46b9f277640e884380aaab11a6a5b23e4f87c7cdea05334e3ebddc5 |
| `docs/backend-event-cancellation-fatal-proof-v1.md` | 04794fb247b103e90d03a07e9827f13ce82d89e0a50dccb543c5e010f0f9bde5 |
| `docs/fable-coordinator-api-backend-v2-handoff.md` | 1443a9af63b908394ec087372bd995c9666a11f7ad30ff9859430de69452a9f2 |
| `docs/retained-http-request-ownership-proof-v1.md` | 83616d0878a4177d0df2e268d36de02a58d1380361cff42bda414178ce8c0971 |
| `scripts/local-checks.sh` | 839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f |

Gates run in the worktree: `review-proof` (PASS), `cargo test --offline -p
glm-serving http::tests` (8 passed, 0 failed), `cargo clippy --offline -p
glm-serving --all-targets -- -D warnings` (clean), and a full workspace
`cargo test --offline` (261 passed, 0 failed).

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. `accept_loop` treats every `listener.accept()` error as retryable
   (`let Ok(..) else { continue; }`, `crates/glm-serving/src/http.rs:512`).
   Under persistent accept failure (for example file-descriptor exhaustion,
   EMFILE) this busy-spins the accept thread at 100% CPU with no backoff.
   Pre-existing behavior, untouched by this diff and outside the declared
   boundary; flagged for the planned epoll/eventfd transport work.

### QUESTION

1. The proof's defect description says the prior failure returned `HTTP_IO`;
   the actual code path (`internal_io`) produced code `HTTP_IO_ERROR`.
   Cosmetic naming drift only — the behavioral claim (error return without
   cancellation dispatch) is exactly right.

## Answers to the handoff's required questions

1. Yes. The pre-change code ran `write_stream_headers(stream)
   .map_err(internal_io)?` after `backend.submit_chat`, so an initial
   header-write failure returned the I/O error with no cancellation
   dispatch and an abandoned completion receiver.
2. Yes. The corrected `write_streaming_completion` binds
   `request_id = handle.request_id` before the first write and on header
   failure calls `backend.cancel(tenant, request_id)` with exactly the
   authenticated tenant and assigned ID before returning.
3. Yes. It returns `Ok(())`, so `worker_loop` does not attempt
   `write_error` on the broken stream; the same applies to the first-SSE
   chunk failure path.
4. Yes. The proof states "Cancellation dispatch itself can still fail; the
   retained backend's receiver-abandonment and fatal-drain behavior remains
   the final safety boundary" — dispatch, not lossless delivery or syscall
   cancellation, is claimed.
5. Yes. The old loop read into a full fixed 4,096-byte buffer with the
   limit checked only before each read, so a read starting below 32,768
   could carry a terminator past the limit and be accepted (there was no
   post-hoc `header_end` check).
6. Yes. Every corrected header read caps its slice to
   `remaining_header_bytes.min(buffer.len())`, so the accepted byte count
   is independent of the underlying reader's chunk size, and a defensive
   `header_end > MAXIMUM_HEADER_BYTES` check backstops the loop.
7. Yes. `parser_enforces_exact_header_boundary_and_rejects_trailing_bytes`
   uses a `ChunkedReader` capped at 4,000 bytes with a 32,769-byte header
   whose complete `\r\n\r\n` terminator ends at byte 32,769: eight chunks
   reach byte 32,000, the old code's ninth uncapped read crossed the limit
   and accepted the terminator, while the corrected parser reads only the
   remaining 768 bytes, finds no terminator, and returns
   `HEADERS_TOO_LARGE`.
8. Yes. Body reads are capped to `remaining_body_bytes`, and
   `bytes.len() > required` after the body loop rejects already-buffered
   trailing bytes with `TRAILING_HTTP_BYTES` (regression: body "x" plus
   coalesced "extra").
9. Yes. The proof states the server is one-request-per-connection and
   claims rejection "only for trailing bytes already buffered by the
   header read"; bytes arriving after the exact body has been read are
   explicitly not claimed detected.
10. Yes. `accept_loop` now queues a socket only if `set_read_timeout`,
    `set_write_timeout`, and `set_nodelay(true)` all succeed; on any
    failure it `continue`s and the socket is dropped, never entering the
    worker queue.
11. Yes. `streaming_header_failure_cancels_submitted_backend_request`
    shuts down the server-side write half before the SSE headers, forcing
    a deterministic first-write failure, and asserts the mock backend
    recorded exactly `[(7, 91)]`. The prior implementation returned the
    error before any cancel call and records nothing — distinguishing.
12. Yes. All 8 `http::tests` (streaming, bounded buffered output,
    slow-consumer, authentication, cancellation endpoint, fatal/health
    gating, header/trailing bounds, startup) pass at the candidate.
13. Yes. The full workspace run at the candidate passed 261 tests;
    `git ls-tree` counts 57 tracked handoffs, i.e. 55 excluding the two
    umbrella handoffs; the proof claims CPU-only evidence and excludes the
    nonblocking transport, keep-alive, pipelining, chunked bodies,
    lossless cancellation delivery, syscall cancellation, checkpoint
    inference, and performance.

## Handoff's six separate statements

- The streaming-header ownership leak is closed by exact cancellation
  dispatch: YES.
- The header limit is exact and independent of transport chunking: YES.
- Declared-body reads and already-buffered trailing bytes fail closed: YES.
- Accepted sockets cannot bypass the configured blocking-I/O bounds: YES.
- Both regressions distinguish the prior behavior: YES.
- The CPU proof and all exclusions are accurate: YES (the `HTTP_IO` vs
  `HTTP_IO_ERROR` naming drift in the proof's prose is cosmetic and does
  not overstate any claim).

## Architecture & maintainability

Genericizing `read_http_request` over `impl Read` is what makes the
chunk-boundary regression possible without sockets — a good testability
move. The remaining-allowance arithmetic is written the safe way
(`remaining.min(buffer.len())` on the slice, never on the request size),
and the two defensive backstops (`header_end` check, trailing-bytes check)
mean a future refactor of the read loop cannot silently reopen either
hole. The streaming-failure path's "cancel exactly once, answer nothing"
shape is consistent with the buffered path's existing cancel-on-limit and
cancel-on-timeout behavior. The accept-loop error spin (MINOR-1) is the
one place this file still trusts the environment.

## Token decision

All six required statements are an unqualified YES; no blockers or majors.
Input hashes were re-verified at review finish and matched. The acceptance
token follows.

retained-http-request-ownership-v1-accepted
