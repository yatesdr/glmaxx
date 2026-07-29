# Fable handoff: backend event cancellation fatal propagation v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`0f0dd21204827f5893143ba93b7c71e9cc99d3c9`

Required result path:
`fable-backend-event-cancellation-fatal-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`backend-event-cancellation-fatal-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-serving/src/backend.rs` | `d4c1b2daaa6f6952d3c27158d33a0123abd891cef09ec894da006af8d7d7f8b0` |
| `crates/glm-serving/src/lib.rs` | `3797647a8535b8a8ca80efd76b4d91407330e3147e5c8e3e0a728b5005043e11` |
| `docs/backend-admission-rollback-fatal-proof-v1.md` | `fd9c2e12a09af096afcf13dafc282e847c8984776a8ea70ef285d9d791866499` |
| `docs/backend-event-cancellation-fatal-proof-v1.md` | `04794fb247b103e90d03a07e9827f13ce82d89e0a50dccb543c5e010f0f9bde5` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-backend-event-cancellation-fatal-v1-handoff.md
cargo test --offline -p glm-serving
cargo clippy --offline -p glm-serving --all-targets -- -D warnings
```

## Review boundary

This review covers only CPU backend event-dispatch ownership and immediate
coordinator cancellation-error propagation. It does not accept atomic
multi-user event delivery, active page tables, process supervision, CUDA,
checkpoint execution, model output, or performance.

## Required adversarial questions

1. Did admission/prefill/output mismatch, slow-client, decoder-stop, and
   decoder-error branches previously ignore coordinator cancellation errors?
2. Could those old branches fail or successfully finish a user and remove its
   external owner while the scheduler continued owning the request?
3. Does every corrected branch request cancellation before publishing its
   backend terminal action or removing the owner?
4. On cancellation error, does `cancel_dispatch_request` reinsert the exact
   removed `ActiveRequest` before returning
   `EVENT_CANCELLATION_FAILED`?
5. Can reinsertion silently overwrite a reappearing request, or does that
   impossible exclusive-access condition fail closed?
6. Do both ordinary runtime dispatch sites set fatal, drain all active and
   queued users with the structured error, and return?
7. If step execution and event cancellation both fail, is the more specific
   event-cancellation error used without suppressing fatal drain?
8. Does accepted collective-safe cancellation still rely on the next tick,
   with later cleanup failure covered by the existing tick fatal path rather
   than overstated as synchronous cleanup?
9. Does the distinguishing regression admit and dispatch a real backend
   request before forcing deterministic generation overflow?
10. Does the invalid token event prove the active request, tenant owner, and
    scheduler request remain and that no user completion was emitted?
11. Would the old ignored-error path fail those assertions for the claimed
    reason?
12. Does repair followed by cancellation prove exact active/owner removal and
    one structured client cancellation?
13. Do the existing slow-client, stop-string, decoder-error, fatal-step, and
    concurrent tests traverse the corrected common boundary without
    regression?
14. Are the 245-test claim, 43-handoff claim, and every GPU/model/performance
    non-claim accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- no event branch ignores an immediate coordinator cancellation error;
- active and owner state survive until cancellation is accepted;
- dispatch failure becomes a structured runtime fatal drain;
- step-error dispatch preserves the most specific fatal cause;
- asynchronous collective-safe cleanup is accurately scoped;
- the distinguishing regression fails the prior code for the claimed
  reason; and
- the CPU proof and all scope exclusions are accurate.

Only if all seven answers are unqualified `YES`, end with the requested
token. Withhold it for a conditional pass, stale input, ignored cancellation
error, active/owner loss before acceptance, user success before rejected
cancellation, runtime continuation, a nondistinguishing regression, or an
overstated proof.

The token accepts only this CPU backend correction. It does not open cn4,
authorize CUDA work, or accept real model execution.
