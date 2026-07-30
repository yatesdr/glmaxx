# Fable handoff: direct-tier scheduler CPU v1 r2

Date: 2026-07-30

Status: corrective CPU scheduling-policy implementation review requested

Review candidate commit:
`e188fc7fcd31c7ca35a48750ff2933267dd40111`

Required result path:
`docs/reviews/fable-direct-tier-scheduler-cpu-v1-r2.md`

Requested acceptance token, only for an unqualified scoped pass:
`direct-tier-scheduler-cpu-v1-r2-accepted`

GPU or storage authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch GPU, container, storage-device,
or network work for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if needed. Hash every candidate input at review start and finish.
Any mismatch must withhold the token as stale.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `fable-direct-tier-io-v1.md` | `739313fd952b7000a6d9789699a36fa36e5ca35152ec28ae959c6f0ac0932882` |
| `docs/fable-direct-tier-io-v1-handoff.md` | `1c021b284ce4e27bfa0d5ffe1890b92b7515253341a0dccd90cdc4051b2775ac` |
| `docs/fable-direct-tier-scheduler-cpu-v1-handoff.md` | `be9994fe4d295497fb08678f7e772ba2bd56c7db042caa8f3becf5abaf127b98` |
| `docs/direct-tier-scheduler-cpu-proof-v1.md` | `514b40ccd75352c3b3a243ee63a8781a16c51f95aba87edb4e4f41252f25ed2f` |
| `docs/direct-tier-scheduler-cpu-proof-v1-r2.md` | `1b9626cc0c2656e3e710f18b102a982d1694665fd63da3200d1a41491388e54e` |
| `crates/glm-cache/src/direct_schedule.rs` | `be32b823bb2299f0288b68d571c0326f6359feb387c1971b1a0fcc69233f59d7` |
| `crates/glm-cache/src/direct_restore.rs` | `73578aa42bf944c37bfe431da21df5c27ad12ee58dfb81f64a24a180830b1c1f` |
| `crates/glm-cache/src/direct_state.rs` | `386c414a302a489c1e8c6fefa6d11b304e86ffa2040ad105e81cc8572999f814` |
| `crates/glm-cache/src/direct.rs` | `7514ad205b84f24c2a2f58647c2b50b0f6dab4398dfb04a2fc36186f09a82dd3` |
| `crates/glm-cache/src/lib.rs` | `6a7c4bae1ec942f6a304c702b4ac5a4b1dc4b86c37c52b15cd4c2ea8c8cf0603` |
| `docs/production-punchlist.md` | `75bb8e461cc90846b6f47e9ff45a1d5e5235e2d35917f31a28d1a1d00260520f` |
| `docs/results-index.md` | `0814def21be46459ef2ea0f1253ecc1a55cbbeb6e65062f5232c681cecc18b4c` |
| `scripts/local-checks.sh` | `56f728cdf3f047f9633509a57341d25a977efa802f0d5b371c9716830517db59` |
| `Cargo.toml` | `863c28560b339f1fd7fb6b80c1b812e9fa7bc3f8f8c782126d2a29ceeffc06ea` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-direct-tier-scheduler-cpu-v1-r2-handoff.md
cargo test --offline -p glm-cache direct_schedule::tests -- --nocapture
cargo test --offline -p glm-cache
cargo clippy --offline -p glm-cache --all-targets -- -D warnings
git diff --check e188fc7fcd31c7ca35a48750ff2933267dd40111^ \
  e188fc7fcd31c7ca35a48750ff2933267dd40111
```

The handoff is coordination metadata added after the candidate and is not a
candidate input. The v1 handoff is a pinned input because its superseded
status must be verified. Do not produce the v1 result or token.

## Review purpose

The v1 scheduler candidate implemented the accepted direct-tier CPU policy,
but self-adversarial review found an untested decision-order defect: new W0
admission was checked before useful service, so multiple eligible candidates
and ample shared resources could generate consecutive admission receipts
ahead of R0/R1 or already-admitted W0 work.

This r2 candidate claims a narrow correction:

- due accepted-W0 service is evaluated before new-W0 admission;
- one boolean latch permits at most one admission between service decisions;
- every R0, R1, W0, and W1 service clears the latch;
- the latch is observable in scheduler statistics; and
- a regression requires
  `admit A -> read -> service A -> admit B -> service B`.

Every v1 invariant remains part of this review: deterministic ordering,
bounded/fallible construction, atomic rejection, exact new-W0 and accepted-W0
byte bounds, variable-read projection, read reserves, two W0 CQ slots,
cleaner watermarks, and final accounting.

## Review boundary

Acceptance covers only the corrected deterministic bounded CPU scheduler and
the named tests.

Acceptance does not cover:

- an io_uring authority or authoritative production resource ledger;
- terminal W0 resource replenishment or scheduled-command cancellation;
- an SQE, CQE, registered file/buffer, direct I/O, fsync, async cancel,
  eventfd, filesystem, or device;
- the durable codec, journal, catalog, checkpoint, recovery, publication, or
  cleaner;
- CUDA, HBM transfer, KV reconstruction, attention, or model execution;
- capacity, latency, throughput, decode isolation, or production health; or
- cn4 access.

## Required adversarial questions

1. Do all seventeen candidate-input hashes match at review start and finish
   in a detached worktree?
2. Does the v1 handoff visibly forbid review/token issuance and point to this
   corrective proof?
3. Reproduce or independently simulate the v1 bug: with multiple eligible W0
   candidates and excess shared resources, can consecutive admissions precede
   useful service?
4. In r2, is due accepted-W0 service evaluated before any new admission,
   including when both service and admission byte thresholds are due?
5. Does `publication_admitted_since_service` become true only after successful
   resource reservation and admission, never after a refused attempt?
6. Can a second admission occur while that latch is true?
7. Does every actual R0, R1, W0, and W1 service clear the latch, while a
   no-decision or rejected call leaves ownership state unchanged?
8. With one R0 and two W0 candidates below the high watermark, is the exact
   order `admit A -> R0 -> service A -> admit B -> service B`?
9. With no reads, must `service A` follow `admit A` before candidate B can be
   admitted?
10. Under continuous reads and continuous eligible W0 offers, do admission
    receipts, read work, and accepted-W0 service all retain their specified
    progress bounds?
11. Are R0/R1 service byte counters updated only by service, not by admission
    receipts, and do projected variable-size reads still prevent overshoot?
12. Does W0 still reserve one buffer, one descriptor, and two CQ entries
    without consuming any read reserve?
13. Are all five class heaps and two membership sets fallibly preallocated,
    hard-capped, deterministic in output, and stable through configured
    capacity?
14. Do duplicate, overflow, invalid-class, invalid-resource, and
    unconstructible-configuration paths remain atomic?
15. Does W1 remain below both low watermarks and unable to interfere with
    queued publication work?
16. Does removing the latch, failing to clear it, or restoring
    admission-before-service make a committed regression fail?
17. Do all 17 focused tests, 114 `glm-cache` tests, 395 workspace tests, and
    109-handoff/8-of-91 provenance figures reproduce?
18. Does the proof accurately leave authoritative resource refresh,
    terminal replenishment, cancellation, syscall/storage, CUDA, model,
    capacity, and performance work outside acceptance?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately whether:

1. the v1 admission-decision starvation defect is reproduced and closed;
2. useful service always separates successive admission receipts;
3. due accepted-W0 service has priority over new admission;
4. all original ordering, bound, reserve, and cleaner invariants remain exact;
5. failure paths and construction remain bounded and atomic;
6. tests distinguish removal or misordering of the correction;
7. v1 is correctly superseded with no token; and
8. no production authority, syscall, storage, CUDA, model, capacity, or
   performance evidence is implied.

Only if all eighteen questions and all eight statements are unqualified
`YES`, end with:

```text
direct-tier-scheduler-cpu-v1-r2-accepted
```

Withhold for stale provenance, consecutive admissions, service starvation, a
latch that can stick or clear early, a byte-bound or reserve regression,
nondeterminism, a nondistinguishing test, any v1 token, acceptance of the
missing production authority, or any storage/GPU/model/performance
overstatement.
