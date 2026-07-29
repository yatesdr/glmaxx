# Fable handoff: scheduler batch-completion atomicity v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`2f7d0ce30392d1fe5c3256058e4d8604100791f2`

Required result path:
`fable-scheduler-batch-atomicity-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`scheduler-batch-atomicity-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-scheduler/src/lib.rs` | `5a820b2e5013f038f07b26f14ddc24d69d00e18d3d55837ab5ff3a68daee3074` |
| `crates/glm-scheduler/src/compile.rs` | `220cf549c0b5882d109ebce4ebd646e9b28ebbab80a83fa579ef5a2c591a070a` |
| `crates/glm-serving/src/lib.rs` | `9d011012cb103149aed5ff531f356746d50f0ed29398854f2d2516c42d82aeab` |
| `docs/offline-serving-foundation.md` | `9a722fdcc77522ac361493ca8fc02fea1e4692a28d1c22bd2e96607568cd4ce0` |
| `docs/offline-serving-spine.md` | `27b24d4cbafc8203937d3620e7bcd85d47fcb86cc4d8b89e237025e5d40a62f9` |
| `docs/scheduler-batch-atomicity-proof-v1.md` | `ea351b40c481aaab129eec263101a916de8d0b84fcb197e22b12bca73a8b2f71` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-scheduler-batch-atomicity-v1-handoff.md
cargo test --offline -p glm-scheduler
cargo test --offline -p glm-serving
cargo clippy --offline -p glm-scheduler --all-targets -- -D warnings
```

## Review boundary

This review covers only CPU scheduler completion validation, staging,
mutation ordering, bounded temporary storage, retry behavior, and the
distinguishing regression. It does not accept the pending serving-page
transaction design, a real rank executor, CUDA, collectives, checkpoint
execution, model output, or performance.

## Required adversarial questions

1. In the prior implementation, can a late per-tenant service overflow occur
   after earlier request rows and the inflight batch have already mutated?
2. Does every fallible lookup, shape check, arithmetic operation, duplicate
   check, completion check, request-state check, and cumulative tenant total
   now occur before `self.inflight.take()`?
3. Does the planner accumulate multiple rows for one tenant rather than
   repeatedly starting from the original service total?
4. On prefill failure, are prompt progress, request state, tenant totals,
   decode burst, and exact inflight identity unchanged?
5. On decode/verify failure, are generated counts, terminal states, tenant
   totals, decode burst, and exact inflight identity likewise unchanged?
6. Does the failed-backend path (`success == false`) stage every request as
   failed and commit the set only after all request identities are present?
7. Are duplicate inflight rows and duplicate/missing/extra completion rows
   rejected without mutation?
8. Is the fixed storage sufficient for every reachable batch because both
   scheduler configuration and engine ABI use the same
   `MAX_ACTIVE_SEQUENCES == 64` constant?
9. Can more than 64 distinct tenants appear in a legal batch, or can the
   fixed tenant-update array overflow on a valid schedule?
10. After preflight under exclusive `&mut Scheduler` access, can either
    `expect` in the apply phase fail without unsafe code or reentrant
    mutation? If so, the claimed infallible commit is false.
11. Is the internal production explicit-result path allocation-free at this
    boundary, and is the bounded linear search acceptable without hiding an
    unbounded input?
12. Does the regression force overflow on the second same-tenant row, prove
    all state unchanged, then successfully retry the exact batch? Does it
    fail on the old code for the claimed reason?
13. Are the 237-test claim and all GPU/model/performance non-claims accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- all completion errors precede mutation;
- cumulative request/tenant planning is complete and bounded;
- the apply phase is infallible under safe exclusive access;
- failure preserves an exactly retryable inflight step;
- the regression distinguishes the prior partial commit; and
- the CPU proof and its non-claims are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, remaining fallible apply
step, partial mutation, missing cumulative accounting, unbounded temporary
state, non-retryable failure, or a regression that cannot distinguish the
defect.

The token accepts only this CPU correction. It does not open cn4, authorize
the v2 prefill ABI implementation, or accept real model execution.
