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
rank execution. Silently accepting the API values now would change output
quality.

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

## Backpressure and cancellation

- command capacity is fixed at construction and capped at 65,536;
- completion event capacity is per request, fixed, and capped at 4,096;
- command work per scheduler turn is bounded so admission cannot starve
  execution;
- an event receiver that fills or disconnects is isolated by cancelling only
  that request rather than blocking the global runtime;
- cancellation verifies the authenticated tenant before it is enqueued;
- prefix-restore and scheduled cancellations pass through
  `ServingCoordinator::cancel`;
- runtime shutdown or fatal step failure attempts one terminal error for every
  active receiver and drops all request ownership.

Stop strings may occur across tokens. When the decoder observes one, text
before the stop is emitted, the stop itself and all later tokens from the same
already-committed verifier result are hidden, usage ends at the token that
completed the stop, and the coordinator request is cancelled. This is a
terminal response, not an engine failure.

## CPU proof

The in-crate tests cover:

- a prompt-to-length completion through prefix admission, scheduler, four CPU
  ranks, position validation, decoding, and exact usage;
- decoder-reported stop termination without leaking stop text or continuing
  generation (cross-token matching remains covered in `glm-tokenizer`);
- tenant mismatch rejection and delivery of explicit cancellation to the
  waiting completion receiver;
- fail-closed rejection of probabilistic sampling; and
- HTTP refusal when a backend reports healthy state with a non-TP4 topology.

The workspace proof remains `scripts/local-checks.sh`.

## Not closed by this milestone

- `StepInput`, page-table delta, and transactional KV reservation are pending
  adversarial review and implementation.
- No padding-logit mask or distributed sampling kernel is connected.
- The retained HTTP/1.1 worker transport is functional, not the final
  nonblocking throughput route.
- No CUDA kernel, checkpoint tensor, quality gate, or serving benchmark is
  exercised by these tests.
