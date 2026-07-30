# Fable handoff: restore operation quota ownership v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`95683d8d5ea1c31f1f9299ac5956dea99ef3ca63`

Required result path:
`fable-restore-operation-quota-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`restore-operation-quota-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/residency.rs` | `2846361e521f66752cb4455c908b2f30fa2f2a27a59a8059866e43b2402a2d6d` |
| `crates/glm-serving/src/cache.rs` | `46962a84ce6c3edec0217a4d4edaac0f7a7e4e283f555bccea8492772881b229` |
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `docs/pending-admission-rollback-proof-v1.md` | `cfd008dacc26f7d82c3f524ad7347da9d492168ebd9e53bb255bdc6cbcbfddfd` |
| `docs/cache-lifecycle-proof-v1.md` | `11ad4936fea7cd0887e660911f50778d5b0918c21a6cebaca1a98a244b2e2de1` |
| `fixtures/cache-lifecycle-proof-v1.json` | `c1151c34a3a9bee4fd97dea11e807603a56c2af4d37deab813cc9b5631177d6a` |
| `docs/restore-operation-quota-proof-v1.md` | `6f7fc39db0a7cdc97c3ee9dd51d37b2adaeeb8dd3e087cb4c3fe85ff102a0128` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-restore-operation-quota-v1-handoff.md
cargo test --offline -p glm-cache residency::tests
cargo test --offline -p glm-serving cache::tests
cargo clippy --offline -p glm-cache -p glm-serving --all-targets -- \
  -D warnings
```

## Review boundary

This review covers only bounded outstanding-operation ownership and abandoned
response handling in the retained blocking CPU restore service. It does not
accept syscall cancellation, waiter deduplication, io_uring, fixed buffers,
direct I/O, catalog epochs, decode isolation, real HBM/DRAM movement, CUDA,
checkpoint execution, model output, or performance.

## Required adversarial questions

1. Did the prior handle-owned counter decrement on result receive, timeout,
   disconnect, or handle drop even though none of those events cancels a
   queued/running blocking read?
2. Could cancellation/rollback therefore admit replacement work against a
   slot whose original read and hash were still physically active?
3. Does `try_submit` now move exactly one uncloneable permit into every
   successfully queued `RestoreCommand`?
4. Does every pre-queue failure or failed `try_send` drop that permit exactly
   once without leaking or underflowing the counter?
5. Does the worker retain the permit through complete read and SHA-256 work,
   then release it before making the response observable?
6. If the response receiver was dropped, are the result and payload safely
   destroyed while logical residency rollback prevents late adoption?
7. If the worker unwinds or its receiver closes with queued commands, do
   command drops release all owned permits?
8. Can any handle method or `Drop` implementation still decrement the
   operation counter?
9. Does checked atomic decrement avoid release-build wraparound on an
   impossible double release?
10. Does the timeout regression deterministically retain count one until the
    command permit drops, and would the prior implementation report zero?
11. Does saturation rollback separate immediate logical cleanup from bounded
    physical drain and use a hard deadline before asserting zero operations?
12. Does normal receive still expose a result only after the operation count
    reaches zero?
13. Are the 259-test, 54-handoff, CPU-only boundary, and all exclusions
    accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- outstanding count now measures queued/running physical operations;
- handle timeout/drop cannot release an active operation slot;
- every send/worker shutdown path releases exactly one permit;
- abandoned results cannot mutate residency or leak payloads;
- the timeout and saturation regressions distinguish prior behavior; and
- the CPU proof and all exclusions are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, handle-owned decrement,
permit leak/double release, response-before-release ordering, unsafe late
adoption, nondistinguishing tests, or overstated production claim.

The token accepts only this retained CPU restore-accounting correction. It
does not open cn4, authorize CUDA work, or accept the production direct-tier
service.
