# Retained HTTP startup cleanup CPU proof v1

Date: 2026-07-29

Implementation commits:

- HTTP partial-start cleanup:
  `2d99fe1e5863dc2f34a0dbcd5b8d7cc8ecf8adbc`
- physical TP4 saturation regression:
  `5066e4cc783e074482ef068022fec6264ed5fa82`

Status: retained CPU HTTP lifecycle correction passed; independent review
pending

GPU claim: none

## Defect and startup correction

`ApiHttpServer::bind` creates a fixed set of connection workers and then one
accept thread. Previously, each thread spawn used `?`. If worker `N` failed
to spawn, or the accept thread failed after all workers had started, stack
unwind dropped the stored `JoinHandle` values without joining them. Dropping
the last connection sender caused those detached workers to exit eventually,
but `bind` could return failure before their backend references and thread
resources were proven released.

The corrected startup path handles both failure boundaries explicitly:

1. on a connection-worker spawn error, it drops the only connection sender;
2. every already-started worker observes channel disconnect and exits;
3. startup joins every stored worker before returning the spawn error;
4. on accept-thread spawn error, closure destruction drops the moved sender
   and startup joins the complete worker set; and
5. if any partial worker panics during that cleanup, startup returns the
   distinct `ApiServerError::ThreadPanic` instead of hiding it behind the
   original spawn error.

No server object or listening endpoint is published on either failure path.
The retained implementation still treats successful OS thread creation as
startup; it does not add per-worker readiness receipts or a startup deadline.

## Distinguishing CPU proof

`startup_failure_joins_partial_http_workers_before_returning` uses the same
internal startup path with deterministic failures at:

- connection worker 2 of 4, after workers 0 and 1 have started; and
- the accept-thread boundary, after the full configured worker set starts.

After each failed bind, `Arc::strong_count` for the mock backend must be
exactly one: the caller's original reference. That proves all partial worker
backend clones, the function argument, and the failed-spawn locals were
destroyed before failure became observable. Both injections also require the
specific spawn error and prove no `ApiHttpServer` was returned.

The former `?` paths detached the already-started join handles and provided
no synchronous cleanup guarantee.

## Physical saturation evidence correction

The first full gate exposed an older serving rollback test that retained a
`StepHandle` to simulate a full TP4 queue. After the reviewed quota correction
moved ownership from the handle to physical work, that CPU step could finish
before the saturation assertion, making the test schedule-dependent.

`submit_failure_fails_selected_rows_without_stranding_inflight` now installs
four rank executors behind five-party entry and release barriers. It waits
until all four are inside the held operation before asking serving to submit
replacement work, then releases and receives the held result after proving
the selected request failed atomically. This is the actual condition the
test claims and remains deterministic under operation-owned quota.

## Gate result and exclusions

After correcting the obsolete saturation setup, the full local gate passed
264 Rust tests with zero failures, workspace formatting, workspace Clippy
with warnings denied, CUDA FFI type checks, every deterministic CPU proof
command, the unchanged serving/cache fixtures, and all 58 then-present
review-handoff provenance proofs.

Commands:

```text
cargo test --offline -p glm-serving http::tests -- --nocapture
cargo test --offline -p glm-serving \
  tests::submit_failure_fails_selected_rows_without_stranding_inflight \
  -- --nocapture
cargo clippy --offline -p glm-serving --all-targets -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-serving/src/http.rs
cc85c2084ac4cdcc3ba2926d1c93e3da39763db92102a0240269bb27764b098f

crates/glm-serving/src/lib.rs
c3ed1b80f5b0561626fc0e61426054a76f9e507b095e0f136accb29ac26a0c07

docs/http-serving-contract.md
036de32f5dd515a7a01aa33ff982d52c37fcee1b37565f35fce1e4a00d197adc

docs/nonblocking-http-transport-v1.md
e1ee381ad46b9f277640e884380aaab11a6a5b23e4f87c7cdea05334e3ebddc5

docs/retained-http-request-ownership-proof-v1.md
83616d0878a4177d0df2e268d36de02a58d1380361cff42bda414178ce8c0971

docs/tp4-step-operation-quota-proof-v1.md
ab5e025afe5f4c236738ea6658d1bbcd9d7a3eac73fd53d4bf0b1cc4600f2d88

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, or model execution
was used.

This correction does not implement the reviewed epoll/eventfd transport,
add a thread-readiness handshake or startup deadline, monitor worker health
after bind, cancel blocking socket syscalls, implement keep-alive or
pipelining, execute a checkpoint, or establish concurrency/performance. It
proves only synchronous cleanup of partial retained HTTP startup and restores
a truthful physical-work premise to the serving saturation regression.
