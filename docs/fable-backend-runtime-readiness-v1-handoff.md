# Fable handoff: backend runtime readiness v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`5ff3d48eef1a504bbbb0c65cfc9a0737dfcceac4`

Required result path:
`fable-backend-runtime-readiness-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`backend-runtime-readiness-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-serving/src/backend.rs` | `175d20a7adde12f6c4f0b64a7e93b8b3004c581f7269eec735b9d7936db5fb63` |
| `docs/coordinator-api-backend-v1.md` | `c3e0617c72523ac05221f67fb016cf1be8f391e1aebceeffed4c5940345225f6` |
| `docs/backend-runtime-readiness-proof-v1.md` | `cadd6f5805caf9f62ccada34c2a68b4eb9e1ee244c47c549830d41df07db2846` |
| `docs/backend-admission-rollback-fatal-proof-v1.md` | `fd9c2e12a09af096afcf13dafc282e847c8984776a8ea70ef285d9d791866499` |
| `docs/backend-event-cancellation-fatal-proof-v1.md` | `04794fb247b103e90d03a07e9827f13ce82d89e0a50dccb543c5e010f0f9bde5` |
| `docs/tp4-rank-startup-handshake-proof-v1.md` | `4bbdf757d288d065ddbb75aa090e5f5a757fa1f55abbbe71dd2edd5880984b0d` |
| `docs/http-serving-contract.md` | `036de32f5dd515a7a01aa33ff982d52c37fcee1b37565f35fce1e4a00d197adc` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-backend-runtime-readiness-v1-handoff.md
cargo test --offline -p glm-serving \
  backend::tests::backend_waits_for_runtime_readiness_and_cleans_failed_startup \
  -- --exact --nocapture
cargo test --offline -p glm-serving
cargo clippy --offline -p glm-serving --all-targets -- -D warnings
```

## Review boundary

This review covers only the retained CPU backend's readiness receipt,
pre-ready failure classification, synchronous runtime join, and destruction
of the coordinator's four-rank ownership tree before startup failure returns.
It does not accept a production SM120 executor, device-health capability,
startup deadline, liveness watchdog, nonblocking HTTP transport, checkpoint
execution, CUDA, serving capacity, or performance.

## Required adversarial questions

1. Did the prior constructor publish a production-healthy backend immediately
   after OS thread creation, before any receipt from inside that thread?
2. Is the new readiness channel bounded and created before the runtime thread?
3. Is the one receipt sent only after the runtime owns the coordinator and
   command receiver and initializes its active-request and
   pending-admission maps?
4. Can the constructor construct or return `CoordinatorApiBackend` before
   receiving that receipt?
5. Does a pre-ready panic unwind through `runtime_loop`, dropping the
   coordinator, prefix restore services, TP4 pool, dispatcher, and rank
   workers?
6. Does the outer unwind boundary close the startup sender and mark fatal
   without allowing a panic to escape the runtime thread?
7. Does startup channel closure make the constructor join the runtime before
   returning exactly `RuntimeStartup`?
8. Does OS thread-spawn failure remain separately reported as `Thread`?
9. If the constructor-side receipt disappears, does readiness-send failure
   make the runtime return and clean its ownership instead of running
   detached?
10. Does the injected pre-ready panic use the same construction and unwind
    path as production, differing only at the deterministic fault point?
11. Does an exact rank-executor drop count of four at the error-return
    boundary distinguish synchronous cleanup from eventual or detached
    cleanup?
12. Do existing successful backend tests still prove that the normal receipt
    route admits and completes bounded concurrent requests?
13. Is the proof explicit that there is no startup deadline, post-start
    liveness monitor, device execution, or production-health qualification?
14. Are the 265-test, 59-handoff, CPU-only boundary, and all exclusions
    accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- backend health cannot publish before the runtime readiness receipt;
- pre-ready failure synchronously joins the runtime and complete coordinator
  ownership tree;
- no backend, command sender, runtime thread, or rank executor survives the
  failed constructor;
- the fault injection distinguishes the former behavior;
- success behavior and strict local gates remain intact; and
- the CPU proof and all exclusions are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, pre-receipt publication,
detached runtime/rank ownership, swallowed startup failure,
nondistinguishing test, or overstated production claim.

The token accepts only this retained CPU backend-startup correction. It does
not open cn4, authorize CUDA work, or accept the production executor or
transport.
