# Fable review: prefix-release atomicity v1

Date: 2026-07-31

Reviewer: Fable (adversarial CPU implementation review; no cn4, no CUDA)

Reviewed candidate commit:
`14b97a2de700973ef3132aeb446659e1c3d6edf6`
(`Record prefix release atomicity proof`; implementation commit
`96869116c32d7f32beb0f09926f551b2000670e0`, `Make prefix release
all-or-nothing`)

Handoff: `docs/fable-prefix-release-atomicity-v1-handoff.md`
(SHA-256 `5c2ab62404021473da0519b0b5a90429b949d5497a975a6bee549e36a3455de5`;
the handoff itself is not part of the candidate commit and was copied into
the detached worktree only for `review-proof`, then removed).

Review environment: exclusive detached git worktree pinned at the candidate
commit; `git status --porcelain` clean at start and finish.

Note: The operator directed review artifacts into docs/reviews/; the handoff
declares the repository root; this file may need moving on acceptance.

## Provenance hashes

All eight provenance-table files were hashed with `shasum -a 256` inside the
pinned worktree at review START and again at review FINISH. Both sets match
the handoff table exactly.

Start:

| Input | SHA-256 | Matches handoff |
|---|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` | yes |
| `crates/glm-cache/src/residency.rs` | `a83edc26e750573878888888cc58f1eb08846c39633ef1f9084db7cfeaba295c` | yes |
| `crates/glm-serving/src/cache.rs` | `3f3a4f1971036ecc6826746af828993ec57e5984e720e225c7e4f14f5b2671d6` | yes |
| `crates/glm-serving/src/lib.rs` | `683c247110ca806607d09111740e95ab77f14c35d0ab70cca337d53ae79a3de2` | yes |
| `docs/cache-lifecycle-proof-v1.md` | `11ad4936fea7cd0887e660911f50778d5b0918c21a6cebaca1a98a244b2e2de1` | yes |
| `docs/serving-page-transaction-v1.md` | `e3a9a1d9f2eb26dc5312d7c42297fa3d832e444f7e3f269094746a85fb3deac2` | yes |
| `docs/prefix-release-atomicity-proof-v1.md` | `7fbe0f4ced91d7ddc8da4f38b6c9c9a8bc73f524eb257ef1ca9a537f095bb9f4` | yes |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` | yes |

Finish: identical to start, byte for byte, for all eight files (re-hashed
after all probe files were removed and both temporary mutations were
reverted with `git checkout --`). No stale-candidate condition.

## Commands run in the worktree

| Command | Outcome |
|---|---|
| `cargo run --offline -p glm-cli --bin glmaxx -- review-proof docs/fable-prefix-release-atomicity-v1-handoff.md` | `"verdict": "PASS"`; every input `expected_sha256 == actual_sha256` |
| `cargo test --offline -p glm-cache` | ok. 48 passed; 0 failed |
| `cargo test --offline -p glm-serving` | ok. 24 passed; 0 failed |
| `cargo clippy --offline -p glm-cache -p glm-serving --all-targets -- -D warnings` | Finished with no warnings or errors |

Additional verification runs (beyond the handoff's required set):

- `cargo test --workspace --offline`: 237 passed, 0 failed across all test
  binaries (sums exactly to the proof's 237-test claim).
- `glmaxx review-proof-all`: verified 39 handoffs with my temporary copy of
  the new handoff present, i.e. 38 then-present handoffs at the candidate
  commit (40 tracked `docs/*handoff*` files minus 2 skipped historical:
  `fable-phase-a-engine-handoff.md`, `fable-review-handoff.md`). The
  38-handoff claim is exact.
- 7 throwaway adversarial probe tests (temporary uncommitted
  `crates/glm-serving/tests/adversarial_probe.rs`, removed before finish):
  all passed against the candidate.
- Two mutation runs (temporary reverts of the fix, restored afterward with
  hash re-verification) proving both regressions distinguish the prior
  defects; details under questions 12 and 13.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. `crates/glm-serving/src/lib.rs:305-311`, `313-323`, `329-335`
   (`finish_token_admission`): on the validation-failure, draft-capability
   fallback, and admission-failure paths, the cache `release(...)?` is
   sequenced before `release_prompt_reservation(...)`. If that release ever
   returned an error, the function would propagate it with the prompt-byte
   reservation still held and no `pending_admissions` entry left to retry
   through — a permanent reservation leak. This is unreachable today: the
   released pages were pinned under the same `&mut self` in the same
   admission flow, so the counted preflight cannot fail (same invariant as
   the apply-phase `expect`). It is a robustness/ordering fragility, not a
   live defect. Suggest releasing the reservation before (or independent of)
   the cache release on these terminal-error paths, or documenting the
   invariant at the call sites.
2. `crates/glm-serving/src/lib.rs:1320-1341`: the serving regression proves
   lease retention and exact retry for a fully-matched prompt
   (`matched_tokens == prompt_tokens`), so `request_tokens` is empty by the
   time the failure is injected. Token-buffer/byte-counter retention on a
   cache-release failure for a partially-matched prompt is guaranteed by
   code order (`lib.rs:621-634`: precompute at 622, fallible release at
   623-628, removals only at 629-633) but is not pinned by a regression.
3. `crates/glm-serving/src/cache.rs:419-594` and
   `crates/glm-serving/src/lib.rs:1212-1389`: both shipped regressions live
   inside large multi-purpose lifecycle tests
   (`multi_page_restore_is_submitted_without_blocking_admission`,
   `prefix_admission_restores_real_durable_bytes_before_skipping_prefill`).
   They do distinguish the defects (verified by mutation), but an unrelated
   earlier assertion change could silently retire the regression coverage.
   Extracting each into a named regression test would protect intent.
4. `crates/glm-serving/src/cache.rs:339-354` (`release_plan`): rank
   ownership is derived from the position of each key in the caller-supplied
   slice, not from a stored lease. A permutation that keeps every key within
   its owner rank (e.g. swapping ordinals 0 and 4) is accepted — confirmed
   by probe — with a correct net pin effect, and any rank-crossing
   permutation fails closed (Missing). Correct, but the API contract
   "callers must pass the lease's key vector in original order" is
   convention, not type-enforced.

### QUESTION

1. `crates/glm-serving/src/cache.rs:310` uses
   `.expect("prefix release was preflighted under exclusive coordinator access")`
   — a panic as the fail-closed invariant assertion. In a long-lived serving
   process a panic here takes down the coordinator thread. Given the
   preflight proof this is unreachable without an internal invariant
   violation, and fail-closed-by-panic is a defensible choice; confirming it
   is the intended posture rather than returning an internal-error variant.

## Answers to the 14 required adversarial questions

1. **Prior reverse-order partial unpin?** YES, confirmed against the real
   prior code. `git show 9686911` shows the old
   `PrefixRestoreCoordinator::release` iterated
   `page_keys.iter().copied().enumerate().rev()` calling per-page
   `unpin(key)?`. For `[invalid, valid]` the reverse loop unpinned the valid
   later page (ordinal 1) first, then returned `Missing` for the invalid
   earlier page — an error after a partial release. Reproduced by mutation
   (see Q12).
2. **Same owner rank as the restore path?** Yes. Restore derives
   `rank = owner_rank(ordinal)` from the position in the matched key list
   (`cache.rs:167-169`); the release plan derives `rank =
   owner_rank(ordinal)` from the position in the passed slice
   (`cache.rs:344-346`); `owner_rank` is `page_ordinal % 4`
   (`crates/glm-cache/src/page.rs:52-55`). The serving lease stores
   `restored.page_keys` verbatim in restore order (`lib.rs:337`) and passes
   it unmodified (`lib.rs:627`), so both paths compute identical ranks per
   key. See MINOR 4 for the convention caveat.
3. **Counting by `(rank, page_key)` with repeats?** Yes. Each occurrence is
   folded into `BTreeMap<(u8, [u8; 32]), u32>` with `checked_add`
   (`cache.rs:343-349`). A repeated key in the same rank produces one entry
   with the cumulative count, validated against that rank's single pin count
   (probe: cumulative 2 vs pin 1 → `State`, zero unpins). A repeated key
   routed to a different rank produces a distinct entry validated against
   that rank's manager, where it is `Missing` unless genuinely registered
   and pinned there (probe: `Missing`, zero unpins). Per-rank pin state is
   held in separate `ResidencyManager`s, so cross-rank entries can never
   share or double-spend one pin count.
4. **All checks before any unpin?** Yes. Ordinal conversion
   (`u64::try_from`, `cache.rs:345`) and count overflow (`cache.rs:348`)
   are checked while building the plan; then `validate_unpin_count` runs
   for every unique entry (`cache.rs:350-352`), which checks nonzero count,
   page existence, HBM residency, and `pin_count >= count`
   (`residency.rs:432-445`). `release_plan` takes `&self` — it cannot
   mutate. The first `unpin_count` executes only after the whole plan
   validates (`cache.rs:306-312`).
5. **Can any safe/reentrant mutation invalidate a preflighted entry?** No.
   Validation and apply are in one synchronous function body under
   `&mut self`; there are no callbacks, no interior mutability touching pin
   state (`ResidencyManager` has none; `RestoreService` atomics track only
   outstanding-slot counts; NVMe worker results reach residency state only
   through explicit `poll_restore` calls under `&mut`). Plan keys are unique
   `(rank, key)` pairs and `unpin_count` only decrements `pin_count`
   (`residency.rs:447-458`), so no apply step can change another entry's
   existence, residency, or pin count. The invariant assertion and
   atomicity claim hold.
6. **Failure leaves pin counts byte-for-byte unchanged?** Yes. On any
   validation error, no mutation has occurred at all (`&self` planning
   only). Probes confirm: after a failed release with a later-invalid
   entry, the exact original release subsequently succeeds and each page is
   then pinned exactly zero times (exact-count probing via
   `validate_unpin_count` semantics through the release API), proving every
   pin was exactly 1 — unchanged — after the failure.
7. **Prior serving path removed the lease before the release error?** YES —
   `git show 9686911` shows the old `release_request_prefix` executed
   `self.prefix_leases.remove(&request_id)` before the fallible
   `release(...)?`, so a release error orphaned the pins with no
   request-owned handle. The old `release_request_tokens` also removed the
   token buffer before the underflow check. Reproduced by mutation (Q13).
8. **New path retains lease and token reservation until release succeeds?**
   Yes. `lib.rs:621-634`: the post-release byte value is precomputed
   (line 622), the fallible cache release runs with the lease still in
   `prefix_leases` and tokens still in `request_tokens` (623-628); the `?`
   on failure leaves both intact; removal and counter publication happen
   only after success (629-633).
9. **Underflow checked before removing the token buffer in both paths?**
   Yes. Both `release_request_prefix` (line 622) and
   `release_request_tokens` (lines 637-645) call
   `retained_prompt_bytes_after_token_release` (647-658), which performs
   the `checked_sub` on `&self` before any `request_tokens.remove` or
   counter write.
10. **Post-release updates infallible?** Yes. After the cache release
    succeeds, the remaining operations are `BTreeMap::remove` twice and one
    `u64` assignment of a precomputed value (`lib.rs:629-633`) — no
    fallible operation, under the same exclusive access.
11. **Bounded to 16,384 entries; allocation outside the decode loop?** Yes.
    1,048,576 / `PAGE_TOKENS` (= 64, `crates/glm-cache/src/lib.rs:37`) =
    16,384 ordinals; the plan is keyed by unique `(rank, key)` so it never
    exceeds the input length. The `BTreeMap` is allocated inside
    `release()` (`cache.rs:306`), which is called only from terminal or
    admission-error paths (`lib.rs:309,317,333,627` via request
    finish/cancel/fail at `lib.rs:506,581,595`), never inside the per-token
    decode path of `tick_observed`. Worst-case transient cost (~16,384
    entries × ~37 bytes payload plus node overhead, low single-digit MB) on
    a once-per-request terminal operation is acceptable.
12. **Cache regression distinguishes the old partial unpin?** Yes, verified
    by mutation. Reverting `release` to the old reverse loop makes
    `multi_page_restore_is_submitted_without_blocking_admission` fail
    exactly at the distinguishing assertion
    (`validate_nvme_registration(&newer_second)` returned `Ok(())` instead
    of `Err(Pinned)` — the valid page had lost its pin), while the error
    return itself still occurred. The candidate passes for the right
    reason: it proves the good page retained its original pin after the
    failed release (`cache.rs:564-575`).
13. **Serving regression distinguishes remove-before-release and proves
    exact retry?** Yes, verified by mutation. Reverting
    `release_request_prefix` to the old ordering makes
    `prefix_admission_restores_real_durable_bytes_before_skipping_prefill`
    fail exactly at `assert!(serving.prefix_leases.contains_key(&77))`
    (lease lost). The candidate passes: release fails with
    `Residency(State)` behind a deliberately damaged fixture, the lease is
    retained, the fixture is repaired by re-pinning, and the exact same
    `release_request_prefix(77)` retry succeeds and removes the lease once
    (`lib.rs:1320-1341`). Coverage caveat in MINOR 2.
14. **237 tests, 38 handoffs, scope and non-claims accurate?** Yes.
    `cargo test --workspace --offline` at the candidate: exactly 237
    passed, 0 failed. `review-proof-all`: 38 then-present handoffs verified
    (40 tracked minus 2 skipped historical). The proof's non-claims (no
    GPU/CUDA/model/performance claim, no whole-tick transaction, no
    tier-I/O or crash-recovery transactionality,
    `docs/prefix-release-atomicity-proof-v1.md:133-142`) match the code
    reality and the handoff's review boundary; nothing in the proof
    overstates what the CPU tests demonstrate. The recorded hashes in the
    proof match the candidate files.

## The seven separate statements

1. **All cache-release errors precede every pin mutation** — YES.
   `release_plan` is `&self`-only; every error (`Overflow`, `Missing`,
   `State`, zero-count `Request`) is raised before the first `unpin_count`
   (`cache.rs:305-354`, `residency.rs:432-445`); probes confirmed zero
   mutation on failure.
2. **Repeated page keys cannot bypass cumulative pin validation** — YES.
   Occurrences are counted into unique `(rank, key)` entries with
   overflow-checked cumulative counts validated against per-rank pin
   counts; same-rank repeats exceeding the pin count and cross-rank repeats
   both fail closed with zero unpins (probes passed).
3. **The counted apply phase is infallible under safe exclusive access** —
   YES. Single synchronous body under `&mut self`, no reentrancy, no
   interior mutability over pin state, unique plan keys, `unpin_count`
   touches only `pin_count`; nothing can invalidate a preflighted entry
   between validation and apply.
4. **A serving release error preserves its retryable lease and token
   ownership** — YES. Lease and token buffer are removed only after the
   cache release succeeds, with the byte counter precomputed before the
   fallible call; verified by the shipped regression (lease + exact retry)
   and by code order for the token buffer (`lib.rs:621-634`).
5. **Both regressions distinguish the prior partial-mutation defects** —
   YES. Verified by reverting each defect in place: each regression fails
   at exactly its distinguishing assertion under the old code and passes
   for the right reason under the candidate.
6. **The boundedness claim is appropriate for the legal 1M context limit**
   — YES. 1,048,576 positions (spec limit, `spec/engine-v0.md`) at 64
   tokens/page gives at most 16,384 plan entries; allocation is in the
   terminal release path, not the decode loop.
7. **The CPU proof and its non-claims are accurate** — YES. The 237-test
   and 38-handoff counts are exact; the defect description matches the real
   prior code; the invariant argument matches the implementation; the
   non-claims correctly scope out GPU, whole-tick transactionality, tier
   I/O, crash recovery, model quality, and performance.

## Architecture & maintainability

- **Layering is clean and improved by this change.** Counted preflight
  (`validate_unpin_count`/`unpin_count`) lives in `glm-cache`'s
  `ResidencyManager` where pin state lives; plan construction and rank
  routing live in the serving-side coordinator; request-ownership
  (lease/token/byte) sequencing lives in `ServingCoordinator`. Each layer's
  commit boundary is local and testable.
- **Plan construction complexity** is O(n log n) in the number of released
  keys (BTreeMap insertions plus one validation pass) with O(u) space for u
  unique `(rank, key)` pairs — no quadratic behavior. `unpin_count`
  re-validates internally after `validate_unpin_count` already ran
  (`residency.rs:447-458`); harmless double work, and the redundancy is
  what makes the `expect` honest, but a comment noting the intentional
  re-validation would help future readers.
- **Duplication:** the three `finish_token_admission` error arms repeat the
  `prefix_cache.as_mut().ok_or(CacheUnavailable)?.release(...)` +
  `release_prompt_reservation` pair with subtly different ordering
  obligations (MINOR 1); a small
  `fn abort_admission(&mut self, keys, token_count)` helper would make the
  ordering rule single-sourced. Likewise `rollback_pending` and `release`
  are two different unwind mechanisms (per-page best-effort vs planned
  all-or-nothing); the asymmetry is justified (mid-flight restore state vs
  settled pins) but deserves a comment.
- **API surface:** `release(&[PrefixPageKey])` encodes ownership by slice
  position (MINOR 4). A future tightening could accept the `RestoredPrefix`
  lease (or an opaque lease token) instead of a raw key slice, making the
  "original order" contract structural. `coordinator.ranks` being
  crate-private forces external probes to infer pin counts through release
  behavior; a `#[cfg(test)]` pin-count accessor would make future
  regressions crisper.
- **Test placement:** the two regressions should graduate into their own
  named tests (MINOR 3). The probe suite written for this review (double
  release, same-rank and cross-rank repeated keys, later-invalid entry,
  same-rank permutation, empty release) would be cheap to adopt as
  permanent tests.

## Token decision

Provenance verified at start and finish with zero drift; all four required
commands pass; all 14 questions answered with evidence; both regressions
mutation-verified; findings are MINOR/QUESTION only, none rising to a
conditional pass; all seven statements are unqualified YES.

prefix-release-atomicity-v1-accepted
