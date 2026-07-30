# Fable handoff: restore operation quota ownership v1 r2

Date: 2026-07-30

Status: corrective adversarial CPU implementation rereview requested

Review candidate commit:
`12c0c49c0ab966101eaf2797a3a23555ec069b2f`

Required result path:
`docs/reviews/fable-restore-operation-quota-v1-r2.md`

Requested acceptance token, only for an unqualified pass:
`restore-operation-quota-v1-accepted`

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch GPU, container, storage-device,
or network work for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if needed. Hash every input at review start and finish. Any mismatch
must withhold the token as a stale candidate.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/residency.rs` | `145bff4c2bc016ef65d77c99a5bd555e6ec273ef5b62c8ad57b7416282d0e025` |
| `crates/glm-serving/src/cache.rs` | `a44a1fbabb4ea01c225c7062ac470945f49d70fca4a5ca2874a073f82c96f5e8` |
| `docs/restore-operation-quota-proof-v1-r2.md` | `5a79079464bad5dbe0d0aba86b0ebc22a5b326a03f30c50e8a4286aa6479c08e` |
| `docs/restore-operation-quota-proof-v1.md` | `6f7fc39db0a7cdc97c3ee9dd51d37b2adaeeb8dd3e087cb4c3fe85ff102a0128` |
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `docs/direct-tier-durable-format-v1.md` | `19ca03edeab89b560d674689ca96ce497f2c5859a91d5fe5d4b50c78645e79e6` |
| `docs/pending-admission-rollback-proof-v1.md` | `cfd008dacc26f7d82c3f524ad7347da9d492168ebd9e53bb255bdc6cbcbfddfd` |
| `docs/production-punchlist.md` | `2b4949054c4b8b11f1ae58e38fe2448355f3cb6845cfeb7b1cdc8264c4c8e8ad` |
| `scripts/local-checks.sh` | `56f728cdf3f047f9633509a57341d25a977efa802f0d5b371c9716830517db59` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-restore-operation-quota-v1-r2-handoff.md
git diff --check 12c0c49c0ab966101eaf2797a3a23555ec069b2f^ \
  12c0c49c0ab966101eaf2797a3a23555ec069b2f
cargo fmt --all -- --check
cargo test --offline -p glm-cache residency::tests
cargo test --offline -p glm-serving cache::tests
cargo clippy --offline -p glm-cache -p glm-serving --all-targets -- \
  -D warnings
```

Then reproduce both repeated filters:

```text
for run_index in {1..50}; do
  cargo test --offline --quiet -p glm-serving cache::tests >/dev/null ||
    exit 1
done
for run_index in {1..50}; do
  cargo test --offline --quiet -p glm-cache residency::tests >/dev/null ||
    exit 1
done
```

The handoff itself is coordination metadata added after the candidate and is
not a candidate input.

## Prior review and correction

The first review found no blocker or major. It independently accepted the
operation-owned permit and reported three robustness findings:

1. impossible underflow was non-wrapping but silent in optimized builds;
2. the saturation drain assertion was not itself distinguishing, although
   the direct handle-abandonment regression was; and
3. unexpected submit-failure rollback-preflight failure could return before
   retaining the just-begun restore in the pending map.

R2 makes underflow observable by setting a permanent poison bit that blocks
admission, retains the original direct distinguishing regression, and adds a
new release-build poison regression. It folds the just-begun page into an
explicit `RestoreReserved` pending state before fallible rollback can escape;
failed preflight retains the complete request, and polling either finishes
rollback or reinserts the request.

The first review also asked whether response-buffer memory is separately
bounded. The production direct-tier contract preallocates and
generation-binds fixed maximum-sized buffers, but that contract remains
unimplemented and outside this correction. Verify that the proof says no
more.

## Review boundary

Acceptance covers only the retained blocking CPU restore service:

- queued/running physical-operation quota ownership;
- release-visible fail-closed underflow;
- handle abandonment and result ordering;
- identity-bound logical rollback; and
- recoverable failed-submission rollback state.

Acceptance does not accept:

- production response-buffer accounting, io_uring, fixed buffers, direct I/O,
  or a storage device;
- waiter deduplication, syscall cancellation, catalog/publication/cleaner
  implementation, or endurance behavior;
- actual HBM/DRAM bytes or CUDA event ordering;
- native ranks, checkpoint/model execution, 1M execution, quality, capacity,
  or performance;
- K03, K04, or K05 as passing; or
- cn4 access.

## Required adversarial questions

1. Do all ten input hashes match at review start and finish in a detached
   worktree?
2. Is the highest bit disjoint from every valid configured quota count?
3. Can any valid maximum, count, or increment overflow into the poison bit?
4. Does admission atomically reject both poison and saturation without
   changing the word?
5. Does every positive-count permit release decrement once and preserve any
   pre-existing poison?
6. Does zero-count release leave count zero and set poison in optimized as
   well as debug builds?
7. Once poisoned, can any public or private service path clear poison or
   admit another operation?
8. Does the poison regression fail against the prior silent optimized
   behavior for the intended reason?
9. Are handle timeout/drop, failed send, worker unwind/shutdown, complete
   read/hash, response ordering, and abandoned-payload behavior preserved?
10. On submit failure, is the just-begun restore represented in the same
    complete pending set before any fallible rollback preflight can escape?
11. Does successful preflight still roll back every prior pin/restore plus
    the just-begun reservation atomically?
12. Does failed preflight insert the complete pending set under its exact
    request ID before returning the rollback error?
13. Can a `RestoreReserved` page ever be polled as completed I/O, or does
    poll only retry identity-bound rollback?
14. If that retry still fails, does the ordinary fail-polled path reinsert
    the complete pending request?
15. Does the new recovery regression deliberately force the former orphan
    interval, prove retained ownership, repair the exact identity, and prove
    final NVMe/no-pending state?
16. Would the former special-case `?` path fail that regression for the
    intended reason?
17. Does the production direct-tier buffer statement accurately describe
    only a pending contract, not an implementation or accepted result?
18. Do the exact tests report ten cache-residency and six serving-cache
    passes?
19. Do both fifty-run loops, formatting, and warnings-denied Clippy
    reproduce?
20. Are the CPU-only scope and all exclusions accurate?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately:

1. quota underflow is release-visible, non-wrapping, and fail-closed;
2. every valid operation still owns exactly one permit through physical
   completion;
3. handle abandonment and result delivery cannot release work early or
   permit late adoption;
4. failed-submission rollback cannot lose coordinator ownership;
5. both new regressions distinguish the prior robustness gaps; and
6. the response-buffer boundary, CPU proof, and exclusions are accurate.

Only if all twenty questions and all six statements are unqualified `YES`,
end with:

```text
restore-operation-quota-v1-accepted
```

Withhold for stale provenance, counter/poison overlap, wrap, poison-clearing
admission, permit leak/double release, response-before-release, orphaned
restore state, partial rollback, a nondistinguishing regression, or any
production/GPU overstatement.
