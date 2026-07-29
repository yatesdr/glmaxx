# Fable handoff: prefix-release atomicity v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`14b97a2de700973ef3132aeb446659e1c3d6edf6`

Required result path:
`fable-prefix-release-atomicity-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`prefix-release-atomicity-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/residency.rs` | `a83edc26e750573878888888cc58f1eb08846c39633ef1f9084db7cfeaba295c` |
| `crates/glm-serving/src/cache.rs` | `3f3a4f1971036ecc6826746af828993ec57e5984e720e225c7e4f14f5b2671d6` |
| `crates/glm-serving/src/lib.rs` | `683c247110ca806607d09111740e95ab77f14c35d0ab70cca337d53ae79a3de2` |
| `docs/cache-lifecycle-proof-v1.md` | `11ad4936fea7cd0887e660911f50778d5b0918c21a6cebaca1a98a244b2e2de1` |
| `docs/serving-page-transaction-v1.md` | `e3a9a1d9f2eb26dc5312d7c42297fa3d832e444f7e3f269094746a85fb3deac2` |
| `docs/prefix-release-atomicity-proof-v1.md` | `7fbe0f4ced91d7ddc8da4f38b6c9c9a8bc73f524eb257ef1ca9a537f095bb9f4` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-prefix-release-atomicity-v1-handoff.md
cargo test --offline -p glm-cache
cargo test --offline -p glm-serving
cargo clippy --offline -p glm-cache -p glm-serving --all-targets -- -D warnings
```

## Review boundary

This review covers only CPU prefix-page release planning, counted residency
preflight, request prefix-lease ownership, prompt-token reservation release,
retry behavior, bounded temporary storage, and the two distinguishing
regressions. It does not accept the wider serving-page transaction, a whole
multi-request tick transaction, rank execution, CUDA, direct tier I/O,
checkpoint execution, model output, or performance.

## Required adversarial questions

1. In the prior cache implementation, can a reverse-order release unpin a
   valid later page before returning an error for an invalid earlier page?
2. Does the new plan derive the same owner rank from each ordinal that the
   restore path uses when it pins the page?
3. Does counting by `(rank, page_key)` correctly handle repeated keys,
   including repeated keys owned by different ranks?
4. Are ordinal conversion, count overflow, page existence, HBM residency,
   and the cumulative pin count for every unique entry all checked before
   any unpin occurs?
5. Under exclusive `&mut PrefixRestoreCoordinator` access, can any safe or
   reentrant mutation invalidate a preflighted entry between plan validation
   and the corresponding `unpin_count`? If so, the invariant assertion and
   atomicity claim are false.
6. Does a cache-release failure leave every page pin count byte-for-byte
   unchanged?
7. Did the prior serving path remove the request lease before discovering a
   cache-release error, making remaining pins unreachable through that
   request?
8. Does the new serving path retain both the lease and token reservation
   until cache release succeeds?
9. Is prompt-reservation underflow checked before removing the token buffer
   in both prefix and token-only release?
10. After cache release succeeds, are the remaining lease/token/counter
    updates infallible under the same exclusive coordinator access?
11. Is the release plan bounded to at most 16,384 entries for one legal
    1,048,576-token, 64-token-page request? Is its allocation outside the
    token decode loop, and is that overhead acceptable for a terminal path?
12. Does the cache regression force the old partial-unpin ordering and prove
    the good page retained its original pin after failure?
13. Does the serving regression prove that the old remove-before-release
    ordering loses the lease, while the new path preserves it and supports
    an exact retry after the fixture is repaired?
14. Are the 237-test claim, 38-handoff claim, scope boundary, and every
    GPU/model/performance non-claim accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- all cache-release errors precede every pin mutation;
- repeated page keys cannot bypass cumulative pin validation;
- the counted apply phase is infallible under safe exclusive access;
- a serving release error preserves its retryable lease and token ownership;
- both regressions distinguish the prior partial-mutation defects;
- the boundedness claim is appropriate for the legal 1M context limit; and
- the CPU proof and its non-claims are accurate.

Only if all seven answers are unqualified `YES`, end with the requested
token. Withhold it for a conditional pass, stale input, partial unpin, lost
lease, remove-before-check ordering, unbounded temporary state, a regression
that cannot distinguish the defect, or an overstated proof.

The token accepts only this CPU correction. It does not open cn4, authorize
the pending prefill ABI implementation, or accept real model execution.
