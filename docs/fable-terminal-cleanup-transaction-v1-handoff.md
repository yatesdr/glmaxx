# Fable handoff: terminal cleanup transaction v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`6535248bb217b20d56ec0d6670c8fb6f33791205`

Required result path:
`fable-terminal-cleanup-transaction-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`terminal-cleanup-transaction-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-serving/src/cache.rs` | `f265314cc36e5453219b96b351a2a6adad04dbf12b6647b9682b5a5cb7f80ea0` |
| `crates/glm-serving/src/lib.rs` | `5f67b28a7a2169564687822c49b3f6c26710352f8edd1361f4daf834f21346b0` |
| `crates/glm-cache/src/residency.rs` | `a83edc26e750573878888888cc58f1eb08846c39633ef1f9084db7cfeaba295c` |
| `crates/glm-scheduler/src/lib.rs` | `5a820b2e5013f038f07b26f14ddc24d69d00e18d3d55837ab5ff3a68daee3074` |
| `docs/prefix-release-atomicity-proof-v1.md` | `7fbe0f4ced91d7ddc8da4f38b6c9c9a8bc73f524eb257ef1ca9a537f095bb9f4` |
| `docs/selected-step-failure-finalization-proof-v1.md` | `36be571d84cff086ad3058f3426fc0fee6bdd4d33b1c4317473128e4d861512e` |
| `docs/serving-page-transaction-v1.md` | `e3a9a1d9f2eb26dc5312d7c42297fa3d832e444f7e3f269094746a85fb3deac2` |
| `docs/terminal-cleanup-transaction-proof-v1.md` | `5998b9abb4e1587ef5a4a83ebbc1c2e6bee551122fdd075af0ea1cac01172862` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-terminal-cleanup-transaction-v1-handoff.md
cargo test --offline -p glm-cache
cargo test --offline -p glm-serving
cargo clippy --offline -p glm-cache -p glm-serving --all-targets -- -D warnings
```

## Review boundary

This review covers only CPU publication/cleanup for successful selected
steps, selected-step failure, and idle cancellation: fixed-capacity event
staging, cumulative prompt accounting, multi-request prefix release,
shared-page pin counts, lease/token removal, and retry behavior. It does not
accept active page tables, private KV tails, rank deltas, physical-ID
quarantine, admission rollback, CUDA, direct tier I/O, checkpoint execution,
model output, or performance.

## Required adversarial questions

1. Did the prior successful-step loop commit the entire scheduler batch
   before releasing and publishing rows one at a time, allowing an error on a
   later row to coexist with earlier terminal events and released leases?
2. Does the new successful publication plan complete every fallible
   arithmetic, row/output lookup, event-capacity check, prompt-byte check,
   cache lookup, owner derivation, pin-count check, and residency check before
   `complete_batch_with_results(true, ...)`?
3. Are the planner's prefill, decode, verifier, EOS, accepted-draft ordinal,
   output-limit, and terminal calculations expression-equivalent to scheduler
   completion and `StepOutput` validation?
4. Does each page-key slice restart its logical ordinal at zero before
   `owner_rank`, rather than flattening requests into one false global
   ordinal sequence?
5. Are repeated `(rank, page_key)` values counted across all requests before
   validation, including two users sharing one prefix page?
6. Can one pin satisfy two planned releases, or does
   `validate_unpin_count` require the complete cumulative count?
7. Under exclusive coordinator access, can residency change between
   `plan_release_many` and `commit_release` through safe or reentrant code?
8. After successful scheduler completion, is every cache, lease, token,
   prompt-counter, and event operation truly infallible? Identify any hidden
   lookup, allocation, arithmetic, or validation that can still return an
   error after scheduler mutation.
9. Does request deduplication subtract each token buffer once and make
   full-prefix release dominate token-only release?
10. Is fixed event capacity exactly
    `C64 * (MTP6 seven tokens + terminal) == 512`, and does the boundary test
    exercise all 512 entries rather than a smaller shape?
11. Does ordinary nonterminal decode avoid new event/release heap allocation?
    Are the remaining older progress/completion vectors explicitly excluded
    rather than hidden by the claim?
12. If cleanup preflight fails in `fail_selected_step`, does scheduler
    failure still consume the inflight batch before the cleanup error returns,
    with no partial lease/pin/event mutation?
13. Does idle cancellation preflight the full cancelled set before releasing
    any request or inserting any terminal marker, and is a failed attempt
    exactly retryable?
14. Does the distinguishing regression contain two requests sharing one page
    plus a later corrupt request, prove both shared pins and all leases survive,
    prove no events publish, and prove no inflight batch remains?
15. Would the prior implementation fail that regression for the claimed
    partial-success reason?
16. Does the cancellation phase independently prove no partial release/event
    and exact retry after repair?
17. Are the HBM-bounded release-map claim, 241-test claim, 40-handoff claim,
    and every GPU/model/performance non-claim accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- all successful-step publication errors precede scheduler mutation;
- shared prefixes use cumulative multi-request pin counts;
- the post-scheduler release/event commit is infallible under safe exclusive
  access;
- selected-step failure preserves the no-stranded-inflight invariant even
  when cleanup preflight fails;
- cancellation cleanup is all-or-nothing and retryable;
- fixed staging covers the exact maximum verifier event shape;
- the distinguishing regression fails the prior code for the claimed reason;
  and
- the CPU proof and all scope exclusions are accurate.

Only if all eight answers are unqualified `YES`, end with the requested
token. Withhold it for a conditional pass, stale input, flattened ownership,
undercounted shared pins, any post-scheduler fallible operation, partial
event/resource publication, stranded inflight state, non-retryable
cancellation, an unbounded hot-path allocation, a nondistinguishing
regression, or an overstated proof.

The token accepts only this CPU correction. It does not open cn4, authorize
the pending prefill ABI implementation, or accept real model execution.
