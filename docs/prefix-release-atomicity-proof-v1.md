# Prefix-release atomicity CPU proof v1

Date: 2026-07-29

Implementation commit:
`96869116c32d7f32beb0f09926f551b2000670e0`

Status: CPU/reference correction passed; independent review pending

GPU claim: none

## Defects and invariant

`PrefixRestoreCoordinator::release` previously unpinned pages in reverse
ordinal order. If a later release encountered a missing page, invalid
residency, or pin-count mismatch, every page visited before that error had
already lost its pin. The operation returned an error after a partial
release.

`ServingCoordinator::release_request_prefix` also removed the request's
prefix lease before calling the fallible cache release. A release error
therefore discarded the only request-owned handle to pins which remained
live.

Release now has one CPU commit boundary:

```text
count every (owner rank, page key)
    -> validate every counted unpin
    -> compute the post-release prompt reservation
    -> apply the preflighted cache release
    -> remove the prefix lease and token reservation
```

Every observable error occurs before the cache, lease map, token map, or
retained-prompt byte counter changes.

## Counted cache preflight

The release planner derives ownership from each logical page ordinal and
builds a `BTreeMap<(rank, page_key), count>`. Counting is required because a
malformed input with repeated keys must prove that the resident pin count
covers the complete release, not merely one occurrence.

For every unique plan entry, `ResidencyManager::validate_unpin_count`
requires:

- a nonzero count;
- a known page;
- HBM residency; and
- `pin_count >= count`.

Rank selection, ordinal conversion, count overflow, page existence,
residency, and cumulative pin availability are therefore checked before the
first unpin. The apply loop runs under the same exclusive
`&mut PrefixRestoreCoordinator` access. Its keys are unique and no code
between validation and application can mutate residency, so a counted
unpin cannot fail without unsafe code or an internal invariant violation.
That condition is fail-closed with an explicit invariant assertion.

At the 1,048,576-token limit and 64-token pages, the plan has at most 16,384
entries. Release is a terminal control-plane operation; its bounded
`BTreeMap` cost is not in the token decode loop.

## Serving ownership preflight

The serving coordinator now computes the exact post-release
`retained_prompt_bytes` value before cache release. It retains the prefix
lease while the cache operation can fail. Only after the cache release
succeeds does it:

1. remove the lease;
2. remove the retained request-token buffer; and
3. publish the already-checked byte counter.

Token-only release follows the same precompute-then-remove rule, eliminating
the prior remove-before-underflow-check ordering.

## CPU proof

Two distinguishing regressions exercise the prior partial-mutation paths:

1. A two-page coordinator release supplies a bogus first page and a valid
   second page. The old reverse loop unpinned the valid second page before
   reporting the missing first page. The new code returns `Missing` and a
   newer registration for the second page remains blocked by its original
   pin.
2. A serving request's cache pin is deliberately removed behind its retained
   lease. Request release returns the expected cache-state error and proves
   the lease is still present. After restoring the missing pin, retrying the
   same request release succeeds and removes the lease exactly once.

The full local gate passed 237 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, and all 38 then-present review
handoff provenance proofs.

Commands:

```text
cargo fmt --check
cargo test -p glm-cache
cargo test -p glm-serving
cargo clippy -p glm-cache -p glm-serving --all-targets -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-cache/src/residency.rs
a83edc26e750573878888888cc58f1eb08846c39633ef1f9084db7cfeaba295c

crates/glm-serving/src/cache.rs
3f3a4f1971036ecc6826746af828993ec57e5984e720e225c7e4f14f5b2671d6

crates/glm-serving/src/lib.rs
683c247110ca806607d09111740e95ab77f14c35d0ab70cca337d53ae79a3de2

docs/cache-lifecycle-proof-v1.md
11ad4936fea7cd0887e660911f50778d5b0918c21a6cebaca1a98a244b2e2de1

docs/serving-page-transaction-v1.md
e3a9a1d9f2eb26dc5312d7c42297fa3d832e444f7e3f269094746a85fb3deac2

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, or model execution
was used.

This correction covers prefix-cache unpin and request-owned lease/token
publication only. It does not make an entire multi-request serving tick,
active page-table mutation, rank submission, CUDA execution, direct tier
I/O, or process-crash recovery transactional. It does not establish model
quality, serving throughput, or cn4 device correctness.
