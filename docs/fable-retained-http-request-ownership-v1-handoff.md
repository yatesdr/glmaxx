# Fable handoff: retained HTTP request ownership v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`a7b1cc9a6cbae1d5abce75c672693759ac584794`

Required result path:
`fable-retained-http-request-ownership-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`retained-http-request-ownership-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-serving/src/http.rs` | `e05a4e828e94f2d1323fc80d0f86a46b5a5b7456450c54fb146635b091ba8941` |
| `crates/glm-serving/src/backend.rs` | `d4c1b2daaa6f6952d3c27158d33a0123abd891cef09ec894da006af8d7d7f8b0` |
| `docs/http-serving-contract.md` | `036de32f5dd515a7a01aa33ff982d52c37fcee1b37565f35fce1e4a00d197adc` |
| `docs/nonblocking-http-transport-v1.md` | `e1ee381ad46b9f277640e884380aaab11a6a5b23e4f87c7cdea05334e3ebddc5` |
| `docs/backend-event-cancellation-fatal-proof-v1.md` | `04794fb247b103e90d03a07e9827f13ce82d89e0a50dccb543c5e010f0f9bde5` |
| `docs/fable-coordinator-api-backend-v2-handoff.md` | `1443a9af63b908394ec087372bd995c9666a11f7ad30ff9859430de69452a9f2` |
| `docs/retained-http-request-ownership-proof-v1.md` | `83616d0878a4177d0df2e268d36de02a58d1380361cff42bda414178ce8c0971` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-retained-http-request-ownership-v1-handoff.md
cargo test --offline -p glm-serving http::tests -- --nocapture
cargo clippy --offline -p glm-serving --all-targets -- -D warnings
```

## Review boundary

This review covers only retained blocking HTTP parser bounds, accepted-socket
configuration, and cancellation dispatch after the initial streaming-header
write fails. It does not accept cancellation queue delivery, syscall
cancellation, keep-alive, pipelining, chunked requests, the planned
epoll/eventfd transport, real checkpoint execution, CUDA, concurrency
capacity, or performance.

## Required adversarial questions

1. Did the prior streaming path submit the backend request before writing SSE
   headers and then return `HTTP_IO` on initial header failure without
   dispatching cancellation?
2. Does the corrected path retain both authenticated tenant and assigned
   request ID, and dispatch exactly that pair before returning from the known
   disconnect boundary?
3. Does it avoid attempting a second HTTP response on the already-broken
   stream?
4. Is the proof explicit that `backend.cancel` can fail and that this change
   establishes dispatch, not lossless delivery or syscall cancellation?
5. Could the prior parser begin a fixed-size read below byte 32,768, receive
   a delimiter after the limit, and accept the oversized header?
6. Does every corrected header read cap its slice to the exact remaining
   allowance, independent of the underlying reader's chunk size?
7. Does the 4,000-byte regression distinguish the old implementation by
   placing a complete delimiter at byte 32,769 after the old path reached
   byte 32,000?
8. Are body reads bounded to the remaining declared `Content-Length`, and
   are already-buffered bytes beyond that body rejected?
9. Is the proof careful not to claim detection of extra bytes that arrive
   only after the exact body has been read on this one-shot connection?
10. Are sockets withheld from the worker queue if installing either timeout
    or `TCP_NODELAY` fails?
11. Does the TCP write-half shutdown regression deterministically force the
    initial header-write failure and prove exact `(tenant=7, request=91)`
    cancellation dispatch, while the prior implementation records none?
12. Do existing streaming, bounded-output, slow-consumer, authentication,
    cancellation, and fatal-drain tests remain green?
13. Are the 261-test, 55-handoff, CPU-only boundary, and all exclusions
    accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- the streaming-header ownership leak is closed by exact cancellation
  dispatch;
- the header limit is exact and independent of transport chunking;
- declared-body reads and already-buffered trailing bytes fail closed;
- accepted sockets cannot bypass the configured blocking-I/O bounds;
- both regressions distinguish the prior behavior; and
- the CPU proof and all exclusions are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, wrong tenant/request
identity, claimed lossless cancellation, chunk-dependent header acceptance,
overstated trailing-byte coverage, nondistinguishing tests, or any
overstated production claim.

The token accepts only this retained CPU HTTP correction. It does not open
cn4, authorize CUDA work, or accept the production nonblocking transport.
