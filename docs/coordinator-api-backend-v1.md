# Coordinator API backend v1

Date: 2026-07-29

Status: CPU-tested implementation candidate; adversarial review required

GPU evidence: none

## Scope

`glm-serving::CoordinatorApiBackend` is the bounded adapter between
`ApiHttpServer`, the pinned GLM-5.2 tokenizer, and the single-owner
`ServingCoordinator`. It closes the previously mocked lifecycle path without
claiming that rank execution is checkpoint-capable.

One runtime thread owns all mutable scheduler, prefix-restore, worker-pool,
and request-decoder state. HTTP workers perform immutable chat rendering and
tokenization concurrently, then submit commands through a bounded channel.
The runtime:

1. admits the exact token vector through prefix-aware coordinator admission;
2. polls asynchronous cache restores without blocking other submissions;
3. advances continuous batches one collective-safe step at a time;
4. verifies contiguous output positions;
5. incrementally detokenizes committed IDs with stop-prefix withholding;
6. emits bounded per-request completion events; and
7. converts disconnect, cancellation, output, or engine failures into
   collective-safe request termination.

## Fail-closed boundaries

The adapter currently admits only the canonical greedy tuple:

```text
temperature = 0
top_p       = 1
top_k       = omitted
```

Every probabilistic request returns `SAMPLING_ABI_NOT_PROMOTED` before it
enters the scheduler. `StepInput.v1` defines the exact sampling and RNG fields,
but that candidate has not passed adversarial review or been implemented by
rank execution. Negative-zero temperature is rejected rather than treated as
canonical greedy zero. Silently accepting unsupported API values now would
change output quality.

Prompt tokens plus requested output must fit 1,048,576 positions. Request IDs,
token counts, positions, and usage use checked arithmetic. A missing or
malformed tokenizer bundle prevents backend construction through
`PinnedTokenizer`; unmapped output IDs fail detokenization.

The public adapter constructor additionally requires a `StartupCoordinator`
that has reached `Healthy` through four-rank stage and immutable-digest
consensus. `ApiHttpServer` then validates the complete healthy identity before
binding: model `glm-5.2`, pinned model revision, TP=4, and SM=120. A healthy
state bit with the wrong topology is insufficient. The CPU worker and private
fake constructor used by unit tests are not production health evidence.

Backend construction now also waits for a bounded readiness receipt sent from
inside the runtime thread after it owns the coordinator, command receiver,
request maps, and lifecycle controls. A panic before that receipt closes the
startup channel; construction joins the failed runtime synchronously and
returns `RuntimeStartup` instead of publishing a production-healthy backend.
This retained receipt has no startup deadline.

## Backpressure and cancellation

- command capacity is fixed at construction and capped at 65,536;
- completion event capacity is per request, fixed, and capped at 4,096;
- command work per scheduler turn is bounded so admission cannot starve
  execution;
- an event receiver that fills or disconnects is isolated by cancelling only
  that request rather than blocking the global runtime;
- cancellation verifies the authenticated tenant and records one coalescing
  marker in an owner-bound registry instead of competing with the bounded
  submission channel;
- a marker for a request whose submit command is still queued remains
  retained until that request becomes active, then is dispatched before
  pending-admission polling or the next scheduler step;
- cancellation work per scheduler turn is bounded by the command-turn quota,
  while registry cardinality cannot exceed already-accepted owners;
- prefix-restore and scheduled cancellations pass through
  `ServingCoordinator::cancel`;
- submission holds the bounded request-registry gate through its nonblocking
  command enqueue; cancellation holds the same gate through its
  fatal/shutdown recheck and owner-bound marker insertion;
- a fatal step marks the backend unhealthy before terminal draining, attempts
  one terminal error for every active receiver, drains every already-accepted
  queued submission with a terminal error, and only then clears ownership;
- orderly runtime shutdown applies the same active-plus-queued drain.

This ordering closes the interval in which an API thread could pass the first
health check while the runtime was becoming fatal, enqueue after the terminal
drain, and receive only a disconnected channel. The gate never covers a
blocking send: command insertion remains `try_send`.

Request and step telemetry is defined in
[serving observability v1](serving-observability-v1.md). Recording uses fixed
histograms and graph counters and performs no allocation in the runtime hot
path.

Stop strings may occur across tokens. When the decoder observes one, text
before the stop is emitted, the stop itself and all later tokens from the same
already-committed verifier result are hidden, usage ends at the token that
completed the stop, and the coordinator request is cancelled. This is a
terminal response, not an engine failure.

## CPU proof

The in-crate tests cover:

- a prompt-to-length completion through prefix admission, scheduler, four CPU
  ranks, position validation, decoding, and exact usage;
- an injected runtime panic before readiness, proving construction fails and
  all four rank executors are destroyed before the error is returned;
- decoder-reported stop termination without leaking stop text or continuing
  generation (cross-token matching remains covered in `glm-tokenizer`);
- tenant mismatch rejection and delivery of explicit cancellation to the
  waiting completion receiver;
- queue-independent cancellation while a peer is held in a physical TP4 step
  and a queued request saturates the one-slot submission channel, including
  duplicate-call coalescing, pre-execution cancellation, peer completion,
  and marker pruning;
- fail-closed rejection of probabilistic sampling; and
- exact lifecycle totals for four concurrent requests across two tenants;
- isolation of a full completion channel while a concurrent peer reaches its
  normal terminal response;
- an injected rank-execution failure with one active and three queued
  submissions with connected, non-backpressured receivers, proving four
  structured terminal errors, fatal health, zero successful-step
  observations, and no leaked owners; and
- HTTP refusal when a backend reports healthy state with a non-TP4 topology.

The three backend concurrency/fault schedules pass ten consecutive targeted
runs in addition to the workspace gate. They use CPU rank executors and do not
constitute throughput or GPU evidence.

The workspace proof remains `scripts/local-checks.sh`.

## Not closed by this milestone

- `StepInput`, page-table reservation/commit deltas, persistent rank mirrors,
  and four-rank host receipts are implemented as CPU candidates and await
  adversarial review. Device upload receipts, fixed hot-path storage, and
  physical-ID quarantine remain pending.
- No padding-logit mask or distributed sampling kernel is connected.
- The retained HTTP/1.1 worker transport is functional, not the final
  nonblocking throughput route.
- Runtime readiness has no startup deadline or post-start liveness watchdog.
- Cancellation does not interrupt a collective already in flight or wake the
  runtime through an eventfd; dispatch waits for the next collective-safe
  runtime poll.
- No CUDA kernel, checkpoint tensor, quality gate, or serving benchmark is
  exercised by these tests.
