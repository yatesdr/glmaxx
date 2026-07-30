# Fable handoff: queue-independent backend cancellation v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`2ace56ccc9016c83422dbef1048371accd0430c8`

Required result path:
`fable-backend-lossless-cancellation-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`backend-lossless-cancellation-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-serving/src/backend.rs` | `8c9ec8fc9ce37f6d3261940c95d8efae802fa1d95c4095b8f305c3e91dc16078` |
| `crates/glm-serving/src/lib.rs` | `c3ed1b80f5b0561626fc0e61426054a76f9e507b095e0f136accb29ac26a0c07` |
| `docs/backend-lossless-cancellation-proof-v1.md` | `08fb526598d9fa2f170ced6432d579c5cc893826389f10b70a66937932b12d56` |
| `docs/coordinator-api-backend-v1.md` | `33b997380f0da1659355bca4e3d094ef21455d3dc25b1af66063239886e7a07f` |
| `docs/http-serving-contract.md` | `274630837cc79abddf7d68765f806fa3672fcdf2f6b3d44f32d7659b4a148f40` |
| `docs/retained-http-request-ownership-proof-v1.md` | `83616d0878a4177d0df2e268d36de02a58d1380361cff42bda414178ce8c0971` |
| `docs/backend-admission-rollback-fatal-proof-v1.md` | `fd9c2e12a09af096afcf13dafc282e847c8984776a8ea70ef285d9d791866499` |
| `docs/backend-event-cancellation-fatal-proof-v1.md` | `04794fb247b103e90d03a07e9827f13ce82d89e0a50dccb543c5e010f0f9bde5` |
| `docs/production-punchlist.md` | `7014d0da7f2a58a06d273552c4a33b3adde733dc3c326ead19ca097f521b6763` |
| `docs/results-index.md` | `27701bcccc85a45a22bb104b0e7113a5bd3ad98287e69168330436684272060f` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-backend-lossless-cancellation-v1-handoff.md
for run in 1 2 3 4 5 6 7 8 9 10; do
  cargo test --offline -p glm-serving \
    backend::tests::cancellation_survives_a_saturated_submission_queue
done
cargo test --offline -p glm-serving
cargo clippy --offline -p glm-serving --all-targets -- -D warnings
```

## Review boundary

This review covers only logical, in-process cancellation delivery while the
CPU backend runtime is healthy, including saturation of the independent
submission queue. It does not accept syscall cancellation, eventfd wakeup,
physical-collective interruption, the final nonblocking HTTP transport,
process supervision, CUDA, checkpoint execution, model output, or
performance.

## Required adversarial questions

1. Did the prior public cancellation path use
   `command_sender.try_send(Cancel)` on the same bounded channel as
   `Submit`, returning `ENGINE_OVERLOADED` when that channel was full?
2. Could an HTTP timeout, disconnect, or explicit cancel therefore abandon
   its completion receiver while an already-accepted request remained queued
   or active?
3. Does the corrected public path authenticate the request owner and recheck
   fatal/shutdown state before inserting a cancellation marker?
4. Do public and runtime paths consistently acquire `owners` before
   `cancellations`, with no reverse-order acquisition elsewhere?
5. Is there at most one `Requested` or `Dispatched` entry per accepted
   request, so duplicate public calls coalesce and registry cardinality
   cannot exceed owner cardinality?
6. Does a `Requested` marker remain retained when its submit command is still
   queued and therefore absent from the runtime active map?
7. Does each runtime turn consume the bounded command quota first, then
   dispatch cancellation before pending-admission polling and the next
   scheduler tick?
8. Is cancellation dispatch itself bounded by
   `maximum_commands_per_tick`, without adding an unbounded queue or an
   unbounded scan of active work?
9. Are active `Requested` entries selected before stale entries are pruned,
   preventing stale cleanup from delaying actionable cancellation within a
   bounded turn?
10. Does successful dispatch transition the retained marker to `Dispatched`
    so repeat public calls cannot cause repeated coordinator cancellation?
11. Is a marker pruned only after its owner is gone, including the queued
    submit case?
12. Do owner/active disagreement, registry poisoning, and coordinator
    cancellation failure still fail the runtime closed while preserving
    retained ownership for the existing fatal drain?
13. Does the distinguishing regression really saturate a one-slot submission
    queue with the target submit while four rank executors hold the runtime
    inside a physical step for a different request?
14. Are both cancellation calls made before that physical step is released,
    and do they prove one coalesced `Requested` marker?
15. After release, does the target receive exactly `REQUEST_CANCELLED`
    before it executes, while the peer completes normally and all marker and
    owner state is removed?
16. Would the prior implementation fail the first cancellation call for the
    claimed full-channel reason?
17. Is the proof careful that cancellation does not interrupt an in-flight
    collective, does not wake an idle runtime immediately, and may not
    deliver a terminal event to a disconnected client?
18. Are the ten-repeat focused result, 39 serving tests, 269-test full gate,
    63-handoff full-gate count, and every GPU/model/performance exclusion
    accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- the old bounded-channel cancellation loss is real and correctly
  distinguished;
- the corrected public path durably records one authenticated in-process
  marker independently of submission capacity;
- queued, active, repeated, completed, and fatal cancellation ownership are
  all handled fail-closed;
- runtime ordering dispatches cancellation before the target can execute;
- registry and per-turn work remain bounded;
- lock ordering and teardown are safe;
- the regression fails the prior code for the claimed reason; and
- the CPU proof and all latency, transport, GPU, model, and performance
  exclusions are accurate.

Only if all eight answers are unqualified `YES`, end with the requested
token. Withhold it for a conditional pass, stale input, cancellation loss
under submission saturation, unauthenticated insertion, unbounded state or
work, deadlock, repeat coordinator cancellation, premature pruning,
continued target execution, runtime continuation after state disagreement,
a nondistinguishing regression, or an overstated production claim.

The token accepts only this retained CPU backend cancellation correction. It
does not open cn4, authorize CUDA work, or accept checkpoint serving.
