# Fable handoff: rank residency content identity v1 r2

Date: 2026-07-30

Status: corrective adversarial CPU implementation rereview requested

Review candidate commit:
`386ea9a61bae10836a97efec24176118ee8e7632`

Required result path:
`docs/reviews/fable-rank-residency-content-identity-v1-r2.md`

Requested acceptance token, only for an unqualified pass:
`rank-residency-content-identity-v1-accepted`

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
| `crates/glm-cache/src/residency.rs` | `7b95464718006e89fca2fb78c82f385d69bc95bd806d79a5a63f75be0946e5c0` |
| `crates/glm-cache/src/tier.rs` | `0a1541f13462bcdec92284911f96531b06869b60c7fe85fc5e9669c80fabe693` |
| `crates/glm-serving/src/cache.rs` | `607dcc935807ad7c664a15ffc74f54a41907d8337e9a8fe5e3387b7291dd5077` |
| `crates/glm-cache/src/store.rs` | `0a2cd6f96bceb3ed352e5ade9fca302ed5f1498e0280de59a4b57286672dff0c` |
| `docs/rank-residency-content-identity-proof-v1-r2.md` | `5b8e2ecb60f2ec274e0e16123faf0dcc24ec2767eaee53ca49f8dc8810d097c4` |
| `docs/rank-residency-content-identity-proof-v1.md` | `fc50dacf554be5b5af5288b07c2db4514b3ec639c988acb83ff92c553821376c` |
| `docs/durable-content-dedup-proof-v1.md` | `75fd16886ef50e4509fb0a7b0701417a1469a0dad78809b39ce08e3e736a7514` |
| `docs/prefix-residency-coherence-proof-v1.md` | `3f99eeb1f4f003f211922a906939ce9d6bbe03fb9b43ed13091fd38349bd194c` |
| `docs/production-punchlist.md` | `48889ff451db12b3ec5f2d31e562921c118c0b8a2c5927343531663feed4cf40` |
| `scripts/local-checks.sh` | `56f728cdf3f047f9633509a57341d25a977efa802f0d5b371c9716830517db59` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof \
  docs/fable-rank-residency-content-identity-v1-r2-handoff.md
git diff --check 386ea9a61bae10836a97efec24176118ee8e7632^ \
  386ea9a61bae10836a97efec24176118ee8e7632
cargo fmt --all -- --check
cargo test --offline -p glm-cache residency::tests
cargo test --offline -p glm-serving cache::tests
cargo clippy --offline -p glm-cache -p glm-serving --all-targets -- \
  -D warnings
```

Then reproduce the former flaky gate:

```text
for run_index in {1..50}; do
  cargo test --offline --quiet -p glm-serving cache::tests >/dev/null ||
    exit 1
done
```

The handoff itself is coordination metadata added after the candidate and is
not a candidate input.

## Prior verdict and correction

The first review at `eceee043` found the content relation and accounting
logic sound but withheld the token on one MAJOR: the required parallel cache
test could select the same `(PID, quantized wall clock)` temporary directory
twice and fail nondeterministically with the production store's correct
`WriterLocked` response.

R2 adds process-local atomic uniqueness and explicit serving-test labels.
It also closes the prior MINOR stale-plan hazard by binding public
`NvmeRegistrationPlan` values to a monotonic manager state generation and
making commit fail before mutation when stale.

Do not reopen previously accepted content-matrix behavior without evidence,
but verify that r2 preserves it.

## Review boundary

Acceptance covers only synchronous CPU rank-residency:

- same-key content collision rejection;
- exact dedup and MTP retention;
- strictly newer target-to-MTP replacement;
- pin/restore-state and byte-accounting preservation;
- all-or-nothing multi-record planning;
- stale plan rejection; and
- deterministic temporary-store test identity.

Acceptance does not accept:

- the pending online-publication r2 or durable-format design;
- a production prefix index or catalog;
- real HBM, pinned DRAM, NVMe, io_uring, registered buffers, or CUDA events;
- native rank workers, checkpoint/model execution, cold/warm reuse, 1M model
  execution, quality, capacity, or performance;
- K03, K04, or K05 as passing; or
- cn4 access.

## Required adversarial questions

1. Do all eleven input hashes match at review start and finish in a detached
   worktree?
2. Can any two temporary-store calls in one process still return the same
   path when realtime is constant or quantized?
3. Do explicit serving-test labels, PID, wall clock, and atomic sequence
   avoid both the reproduced collision and accidental cross-test ambiguity?
4. Is `Relaxed` ordering sufficient because only uniqueness is required?
5. Is the production store writer lock unchanged and still fail-closed?
6. Does `state_generation` advance after every successful mutation that can
   affect registration action, pin/restore state, or HBM/DRAM accounting?
7. Does any successful relevant mutation fail to advance it, or any failed
   mutation advance it?
8. Do state-generation overflow and stale-plan checks occur before the first
   record or counter mutation?
9. Does a retain-only plan remain a true no-op without an unnecessary
   generation change?
10. Can the plan's absolute HBM/DRAM counters ever overwrite newer manager
    state after an intervening direct API mutation?
11. Does the new regression actually produce nonzero HBM accounting after
    the plan and prove the old plan neither resets it nor inserts its page?
12. Does that regression fail against the former infallible commit for the
    intended reason?
13. Does the prefix coordinator construct and validate all four plans before
    committing the first?
14. Does exclusive `&mut self` ownership make an intervening per-rank
    mutation impossible between coordinator preflight and commit?
15. Could one rank commit and a later rank return `Stale` through any valid
    execution path, or is that limited to an internal ownership bug that
    correctly fails rather than silently rewinds?
16. Are the previously accepted collision, exact-dedup, MTP-retention,
    strict-upgrade, pin, and multi-record atomicity properties preserved?
17. Do the targeted tests, fifty-run stress loop, formatting, and Clippy
    commands reproduce with zero failures?
18. Are the CPU-only scope and all exclusions accurate?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately:

1. temporary-store identity is deterministic and collision-free within one
   test process;
2. stale registration plans cannot mutate records or rewind accounting;
3. retain-only and valid current-generation commits preserve the accepted
   relation matrix;
4. four-rank coordinator planning remains all-or-nothing under its ownership
   model;
5. the new and retained regressions distinguish the prior unsafe behavior;
   and
6. the CPU proof and exclusions are accurate.

Only if all eighteen questions and all six statements are unqualified `YES`,
end with:

```text
rank-residency-content-identity-v1-accepted
```

Withhold for stale provenance, a remaining test-root collision, writer-lock
weakening, an untracked plan-relevant mutation, generation advance on
failure, a post-mutation stale check, stale accounting overwrite, partial
coordinator commit, content-matrix regression, nondistinguishing tests, or
evidence overstatement.
