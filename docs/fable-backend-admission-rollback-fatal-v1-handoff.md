# Fable handoff: backend admission rollback fatal drain v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`3ab31108f571c01ae4a83642c95e012d8b195123`

Required result path:
`fable-backend-admission-rollback-fatal-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`backend-admission-rollback-fatal-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-serving/src/backend.rs` | `8b4f34d771374c9f442d69ba98c6ca29c501cd6f3644fef417923b258084d30a` |
| `crates/glm-serving/src/lib.rs` | `3797647a8535b8a8ca80efd76b4d91407330e3147e5c8e3e0a728b5005043e11` |
| `crates/glm-serving/src/cache.rs` | `e7686a678537b1644608f655e0a8bd40133e0772e0cdf12865c02bbefb15b54b` |
| `docs/pending-admission-rollback-proof-v1.md` | `cfd008dacc26f7d82c3f524ad7347da9d492168ebd9e53bb255bdc6cbcbfddfd` |
| `docs/backend-admission-rollback-fatal-proof-v1.md` | `fd9c2e12a09af096afcf13dafc282e847c8984776a8ea70ef285d9d791866499` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-backend-admission-rollback-fatal-v1-handoff.md
cargo test --offline -p glm-serving
cargo clippy --offline -p glm-serving --all-targets -- -D warnings
```

## Review boundary

This review covers only CPU API-backend ownership and fatal propagation when
the serving coordinator retains a pending admission after poll or
cancellation rollback failure. It does not accept active page tables,
automatic cache repair, process supervision, CUDA, direct tier I/O,
checkpoint execution, model output, or performance.

## Required adversarial questions

1. Did the prior backend poll loop remove its pending ID and fail the active
   user on every coordinator error, even when coordinator rollback failed and
   retained that admission?
2. Did prior cancellation remove the backend pending ID before coordinator
   cancellation, then discard active/owner state on a retained rollback
   error?
3. In either old path, could the live coordinator retain prompt bytes, cache
   work, and a request ID that no backend registry could poll or cancel?
4. Does `has_pending_admission` expose only read-only request ownership,
   without weakening cache validation or allowing the backend to mutate
   coordinator internals?
5. Does a poll error with no remaining coordinator ownership still take the
   ordinary request-local failure path?
6. Does a poll error with retained ownership preserve the backend active map,
   pending set, external owner map, and coordinator state before returning
   `ADMISSION_ROLLBACK_FAILED`?
7. Does cancellation remove the backend pending ID only after coordinator
   cancellation succeeds?
8. Does cancellation with retained ownership preserve all registries before
   returning `CANCELLATION_ROLLBACK_FAILED`?
9. Do both command-consumption sites propagate that error, set the fatal flag
   before drain, fail all active and queued requests with the structured code,
   and return so the coordinator is dropped?
10. Is fail-stop the safe policy for an invariant-blocked rollback that the
    API backend cannot repair, rather than request-local failure or unbounded
    retry?
11. Can an event, owner removal, metric, or pending-set mutation occur before
    the retained-ownership decision in either corrected path?
12. Does the distinguishing regression exercise a real file-backed restore,
    wait for the corrupt worker result, and prove the active user, backend
    pending ID, external tenant owner, serving pending record, cache pending
    record, and exact prompt bytes all remain?
13. Would the old poll code fail those assertions for the claimed reason?
14. Does the same regression independently prove cancellation retains all
    ownership, then prove exact repair/cancel/event cleanup and one structured
    client cancellation?
15. Does existing fatal-step coverage prove `fail_all` drains active and
    queued users and clears the externally visible active count?
16. Are the 244-test claim, 42-handoff claim, and every GPU/model/performance
    non-claim accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- retained poll errors cannot become forgotten request-local failures;
- retained cancellation errors cannot lose backend ownership;
- ordinary completed-roll-back errors remain request-local;
- both runtime command paths fail-stop with a structured drain;
- the coordinator is dropped after fatal drain rather than kept live with
  unattributed work;
- the distinguishing regression fails the prior code for the claimed
  reasons; and
- the CPU proof and all scope exclusions are accurate.

Only if all seven answers are unqualified `YES`, end with the requested
token. Withhold it for a conditional pass, stale input, any lost registry,
request-local continuation with retained coordinator state, an unbounded
retry, failure to set fatal before drain, a live unattributed coordinator, a
nondistinguishing regression, or an overstated proof.

The token accepts only this CPU backend correction. It does not open cn4,
authorize CUDA work, or accept real model execution.
