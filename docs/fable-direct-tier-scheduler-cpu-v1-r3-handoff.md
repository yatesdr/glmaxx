# Fable handoff: direct-tier scheduler CPU v1 r3

Date: 2026-07-30

Status: consolidated corrective CPU scheduling-policy review requested

Review candidate commit:
`b602a9c26f1821f70b2872b158a2201155f71ef1`

Required result path:
`docs/reviews/fable-direct-tier-scheduler-cpu-v1-r3.md`

Requested acceptance token, only for an unqualified scoped pass:
`direct-tier-scheduler-cpu-v1-r3-accepted`

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
| `docs/fable-direct-tier-scheduler-cpu-v1-r2-handoff.md` | `59b2c2c3d599117b8f310f64eda869221f9bfbac095b348982e797149993550f` |
| `docs/direct-tier-scheduler-cpu-proof-v1.md` | `514b40ccd75352c3b3a243ee63a8781a16c51f95aba87edb4e4f41252f25ed2f` |
| `docs/direct-tier-scheduler-cpu-proof-v1-r2.md` | `1b9626cc0c2656e3e710f18b102a982d1694665fd63da3200d1a41491388e54e` |
| `docs/direct-tier-scheduler-cpu-proof-v1-r3.md` | `3ba91bddf8ac24fecf2a8ee880a97031edac07d46f527e840b4d6e39e33eed64` |
| `crates/glm-cache/src/direct_schedule.rs` | `9dd9feacaa04e927ffa0d1153a797a6dd1ce34ed8c8e07634c846fd03b7bcb04` |
| `crates/glm-cache/src/direct_restore.rs` | `73578aa42bf944c37bfe431da21df5c27ad12ee58dfb81f64a24a180830b1c1f` |
| `crates/glm-cache/src/direct_state.rs` | `386c414a302a489c1e8c6fefa6d11b304e86ffa2040ad105e81cc8572999f814` |
| `crates/glm-cache/src/direct.rs` | `7514ad205b84f24c2a2f58647c2b50b0f6dab4398dfb04a2fc36186f09a82dd3` |
| `crates/glm-cache/src/lib.rs` | `6a7c4bae1ec942f6a304c702b4ac5a4b1dc4b86c37c52b15cd4c2ea8c8cf0603` |
| `docs/production-punchlist.md` | `9ee613c21c5f94adf761611af4e6980fc5d90ffb6cd333ca33a7e9a9e8a010ad` |
| `docs/results-index.md` | `ee27fd6945cf55fb42489a32263284e2d81abe8d32e612e4171d4e3cae695d39` |
| `scripts/local-checks.sh` | `56f728cdf3f047f9633509a57341d25a977efa802f0d5b371c9716830517db59` |
| `Cargo.toml` | `863c28560b339f1fd7fb6b80c1b812e9fa7bc3f8f8c782126d2a29ceeffc06ea` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-direct-tier-scheduler-cpu-v1-r3-handoff.md
cargo test --offline -p glm-cache direct_schedule::tests -- --nocapture
cargo test --offline -p glm-cache
cargo clippy --offline -p glm-cache --all-targets -- -D warnings
git diff --check b602a9c26f1821f70b2872b158a2201155f71ef1^ \
  b602a9c26f1821f70b2872b158a2201155f71ef1
```

The handoff is coordination metadata added after the candidate and is not a
candidate input. Both prior scheduler handoffs are pinned inputs because
their superseded/token-forbidden state must be verified. Do not produce v1
or r2 results or tokens.

## Review purpose

Review the complete scheduler, not only the r3 delta. R3 consolidates:

- deterministic R0/R1/W0/W1 class and in-class ordering;
- bounded fallible preallocation and atomic failures;
- read-reserved buffer, descriptor, and CQ protection;
- independent new-W0 admission and accepted-W0 service byte bounds;
- service-before-admission and at most one admission between services;
- maximum read-command size no larger than any fairness bound;
- bounded projected-byte R0 preference so R1 cannot starve; and
- W1 suppression above read/publication low watermarks.

The two new r3 claims are:

1. no accepted R0/R1 command can be too large for the configured R1 or W0
   progress bounds; and
2. continuous R0 service cannot indefinitely defer R1.

## Review boundary

Acceptance covers only the consolidated deterministic CPU scheduler policy
and named tests.

Acceptance does not cover production binding of the maximum physical extent,
the authoritative resource ledger, terminal replenishment, command
cancellation, io_uring/syscalls/storage, durable codec/recovery/publication/
cleaning, CUDA/HBM/KV/attention/model execution, capacity, latency,
throughput, health, or cn4 access.

## Required adversarial questions

1. Do all nineteen candidate-input hashes match at review start and finish in
   a detached worktree?
2. Are both v1 and r2 visibly superseded and token-forbidden?
3. Can any successful configuration set the maximum R0/R1 command above the
   R1-progress, W0-admission, or W0-service byte bound?
4. Is an oversized R0/R1 command rejected before any ID, order, byte, queue,
   or fairness state mutates?
5. Can W0/W1 accounting values be accidentally rejected by the R0/R1-only
   size rule?
6. Does the first queued R1 begin with zero prior R0 debt?
7. While R1 waits, does only serviced R0 work accrue its byte counter?
8. Does R1 service reset the counter, and does the counter remain zero with
   no R1 waiting?
9. At exact equality, does R1 run after the bound is consumed; before a
   variable-size R0 would cross it, does R1 run first?
10. Under continuous replenished R0 arrivals, can R1 ever starve?
11. Does W0 projection use the read selected after applying R1 fairness,
    rather than an R0 that will not actually run next?
12. Does due accepted-W0 service still precede admission, and can at most one
    admission occur between useful service decisions?
13. Are new-W0 admission and accepted-W0 service still bounded under mixed
    R0/R1 arrivals without consuming read reserves?
14. Does every admitted W0 still consume exactly one shared buffer, one
    descriptor, and two CQ entries from the caller-owned snapshot?
15. Do deterministic class/order behavior, queue caps, duplicate rejection,
    overflow handling, and capacity-stability tests remain exact?
16. Does W1 remain suppressed above either low watermark?
17. Do mutations removing any new size inequality, R1 counter update/reset,
    or projected-R1 selection fail a committed test?
18. Do 18 focused tests, 115 `glm-cache` tests, 396 workspace tests, and
    110-handoff/8-of-92 provenance figures reproduce?
19. Does every unimplemented production, storage, CUDA, model, capacity, and
    performance boundary remain accurately excluded?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately whether:

1. both self-discovered scheduler defects are reproduced and closed;
2. R0 preference is bounded without sacrificing deterministic ordering;
3. every accepted read fits all progress bounds;
4. W0 admission/service and read reserves remain exact;
5. prior admission-starvation, cleaner, allocation, and atomic-failure
   corrections remain intact;
6. tests distinguish every r3 correction;
7. v1 and r2 are correctly superseded with no tokens; and
8. no production authority, storage, CUDA, model, capacity, or performance
   evidence is implied.

Only if all nineteen questions and all eight statements are unqualified
`YES`, end with:

```text
direct-tier-scheduler-cpu-v1-r3-accepted
```

Withhold for stale provenance, an oversized-read escape, R1 starvation,
incorrect projected selection, a W0/read-reserve regression, nondeterminism,
a nondistinguishing test, any v1/r2 token, acceptance of missing production
ownership, or any storage/GPU/model/performance overstatement.
