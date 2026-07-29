# Scheduler batch-completion atomicity CPU proof v1

Date: 2026-07-29

Implementation commit:
`bc47da84c3fe43893b9a5ab7325d021c75400340`

Status: CPU/reference correction passed; independent review pending

GPU claim: none

## Defect and invariant

`Scheduler::complete_batch_internal` previously removed the inflight batch
and then updated each request and tenant in row order. A fallible operation
could occur after earlier rows had committed. In particular, if two rows
belonged to one tenant whose `service_units` was one below `u64::MAX`, the
first row changed its request and advanced the tenant to `u64::MAX`; the
second row changed its request and then returned `SchedulerError::Overflow`.
The batch was no longer inflight, so the caller could not retry or roll the
step back as one transaction.

A selected collective step now has one host commit boundary:

```text
validate batch/completion shape
    -> preflight every request update
    -> preflight cumulative per-tenant service totals
    -> retain inflight batch on every error
    -> apply the complete infallible plan
```

No request state, prompt progress, generated count, tenant service count,
decode-burst count, or inflight identity changes until every row succeeds.

## Preflight coverage

The planner rejects before mutation:

- more than the fixed C64 row ceiling;
- duplicate inflight request IDs;
- duplicate, missing, extra, or mode-incompatible completion records;
- a request missing from scheduler state;
- a tenant missing from scheduler state;
- prefill rows against a non-prefill request or with zero work;
- prompt progress overflow or progress beyond the request prompt;
- decode/verify rows against a non-decoding request or with prompt work;
- a generated count already beyond the request limit;
- zero or over-depth token commits;
- a commit beyond the remaining request limit;
- generated-count overflow; and
- cumulative tenant-service overflow, including overflow caused only by a
  later row for the same tenant.

The apply phase uses only identities proven under the same exclusive
`&mut Scheduler` access. It removes the inflight batch, copies staged
request fields, copies staged tenant totals, and updates the decode-burst
counter with no remaining fallible operation.

## Bounded scheduler overhead

The internal commit boundary uses fixed arrays sized from
`MAX_ACTIVE_SEQUENCES == 64` for:

- completion-to-row binding;
- request updates; and
- distinct tenant totals.

It does not allocate a map or vector while validating or committing the
batch. Duplicate and identity searches are bounded by C64. Public legacy
helpers may still materialize their input completion vector before entering
this boundary; the production explicit-result slice enters it directly.

## CPU proof

The distinguishing regression:

1. admits two one-token prefills for one tenant;
2. selects both in one inflight batch;
3. sets the tenant service total to `u64::MAX - 1`;
4. proves completion returns `SchedulerError::Overflow`;
5. proves both request progress records, the tenant total, decode-burst
   counter, and exact inflight batch remain unchanged;
6. resets only the synthetic overflow fixture;
7. retries the same inflight batch; and
8. proves both rows and the cumulative tenant total commit exactly once.

The old implementation fails this test after partially committing both
request rows and consuming the inflight batch.

The full local gate passed 237 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, and all 37 then-present handoff
provenance proofs.

Commands:

```text
cargo fmt --check
cargo test -p glm-scheduler
cargo test -p glm-serving
cargo clippy -p glm-scheduler --all-targets -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-scheduler/src/lib.rs
5a820b2e5013f038f07b26f14ddc24d69d00e18d3d55837ab5ff3a68daee3074

crates/glm-scheduler/src/compile.rs
220cf549c0b5882d109ebce4ebd646e9b28ebbab80a83fa579ef5a2c591a070a

crates/glm-serving/src/lib.rs
9d011012cb103149aed5ff531f356746d50f0ed29398854f2d2516c42d82aeab

docs/offline-serving-foundation.md
9a722fdcc77522ac361493ca8fc02fea1e4692a28d1c22bd2e96607568cd4ce0

docs/offline-serving-spine.md
27b24d4cbafc8203937d3620e7bcd85d47fcb86cc4d8b89e237025e5d40a62f9

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, or model execution
was used. This proof does not authorize cn4 or establish device execution,
model quality, serving throughput, or fault recovery beyond the CPU
scheduler boundary.
