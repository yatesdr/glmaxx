# Backend runtime readiness CPU proof v1

Date: 2026-07-29

Implementation commit:
`b22781c3bc548a4cf807cc05fab4b51f7c53d3d1`

Status: retained CPU backend-startup correction passed; independent review
pending

GPU claim: none

## Defect and correction

`CoordinatorApiBackend::spawn_with_tokenizer` previously returned a
production-healthy backend as soon as the operating system accepted creation
of `glmaxx-serving-runtime`. The new thread had not yet proved that it owned
the `ServingCoordinator`, command receiver, request maps, and lifecycle
controls or that it could reach the runtime loop. A failure in that interval
could therefore make healthy publication race a terminal runtime failure.

The corrected constructor uses a capacity-one startup channel:

1. the constructor creates the runtime thread but retains backend publication;
2. the runtime takes ownership of the coordinator and command receiver;
3. it creates the runtime-owned active-request and pending-admission maps;
4. only then does it send one readiness receipt;
5. the constructor publishes `ApiHealth::production_healthy` only after
   receiving that receipt; and
6. channel closure before the receipt makes construction join the runtime
   thread and return `CoordinatorBackendError::RuntimeStartup`.

The existing unwind boundary surrounds runtime initialization and execution.
A pre-ready panic therefore drops the coordinator inside the runtime thread.
That synchronously drops its prefix restore services and TP4 worker pool; the
pool closes its dispatcher channel and joins the dispatcher and all four rank
threads. The constructor's subsequent runtime-thread join proves this entire
ownership tree is gone before the startup error becomes observable.

If the constructor side disappears before receipt, the readiness send fails
and the runtime returns through the same ownership cleanup. No backend object,
healthy state, command sender, or detached runtime is published on a
pre-ready failure.

## Distinguishing CPU proof

`backend_waits_for_runtime_readiness_and_cleans_failed_startup` uses the real
private construction path and injects a panic immediately before the runtime
receipt. Its coordinator owns a real retained `Tp4WorkerPool` whose four rank
executors share an atomic drop counter.

The test requires:

- construction returns exactly `RuntimeStartup`, not a backend;
- the caught panic closes the startup channel;
- construction joins the runtime before returning; and
- the rank-executor drop count is exactly four at the return boundary.

Without the receipt, the former constructor returned success independently of
runtime scheduling and could not make this synchronous cleanup claim.
Existing backend tests exercise the success route, proving a received receipt
still permits bounded multi-user request execution, cancellation, slow-client
isolation, and fatal draining.

## Gate result and exclusions

The focused gate passed all 38 `glm-serving` tests and Clippy with warnings
denied. The full local gate passed 265 Rust tests with zero failures,
workspace formatting, workspace Clippy, CUDA FFI type checks, every
deterministic CPU proof command, and all 59 then-present review-handoff
provenance proofs.

Commands:

```text
cargo test -p glm-serving \
  backend::tests::backend_waits_for_runtime_readiness_and_cleans_failed_startup \
  -- --exact --nocapture
cargo test -p glm-serving
cargo clippy -p glm-serving --all-targets -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-serving/src/backend.rs
175d20a7adde12f6c4f0b64a7e93b8b3004c581f7269eec735b9d7936db5fb63

docs/coordinator-api-backend-v1.md
c3e0617c72523ac05221f67fb016cf1be8f391e1aebceeffed4c5940345225f6

docs/backend-admission-rollback-fatal-proof-v1.md
fd9c2e12a09af096afcf13dafc282e847c8984776a8ea70ef285d9d791866499

docs/backend-event-cancellation-fatal-proof-v1.md
04794fb247b103e90d03a07e9827f13ce82d89e0a50dccb543c5e010f0f9bde5

docs/tp4-rank-startup-handshake-proof-v1.md
4bbdf757d288d065ddbb75aa090e5f5a757fa1f55abbbe71dd2edd5880984b0d

docs/http-serving-contract.md
036de32f5dd515a7a01aa33ff982d52c37fcee1b37565f35fce1e4a00d197adc

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, model, or network
server was used.

This correction does not add a startup deadline, post-start liveness
watchdog, non-forgeable device-health capability, production SM120 rank
factory, checkpoint-backed executor, nonblocking HTTP transport, CUDA
execution, or serving performance evidence. It proves only that the retained
backend cannot publish health before its runtime owns initialized host state
and that a pre-ready failure is synchronously and completely cleaned up.
