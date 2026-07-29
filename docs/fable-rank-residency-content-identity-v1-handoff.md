# Fable handoff: rank residency content identity v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`eceee043cedab38b30f4f64cd5871eede0a254e5`

Required result path:
`fable-rank-residency-content-identity-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`rank-residency-content-identity-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/residency.rs` | `ea8337b22a043436147bb461a618f13fb993de1cd6750dcfefb205d64fdea5fd` |
| `crates/glm-cache/src/tier.rs` | `0a1541f13462bcdec92284911f96531b06869b60c7fe85fc5e9669c80fabe693` |
| `crates/glm-serving/src/cache.rs` | `3ce2f435d2538c736c1b10b3fd6f27c1fb08a8221c92eb62e1db7d49a832283c` |
| `docs/durable-content-dedup-proof-v1.md` | `75fd16886ef50e4509fb0a7b0701417a1469a0dad78809b39ce08e3e736a7514` |
| `docs/prefix-residency-coherence-proof-v1.md` | `3f99eeb1f4f003f211922a906939ce9d6bbe03fb9b43ed13091fd38349bd194c` |
| `docs/online-prefix-publication-v1.md` | `67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873` |
| `fixtures/cache-lifecycle-proof-v1.json` | `c1151c34a3a9bee4fd97dea11e807603a56c2af4d37deab813cc9b5631177d6a` |
| `docs/rank-residency-content-identity-proof-v1.md` | `fc50dacf554be5b5af5288b07c2db4514b3ec639c988acb83ff92c553821376c` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-rank-residency-content-identity-v1-handoff.md
cargo test --offline -p glm-cache residency::tests
cargo test --offline -p glm-serving cache::tests
cargo clippy --offline -p glm-cache -p glm-serving --all-targets -- \
  -D warnings
```

## Review boundary

This review covers only synchronous CPU rank-residency registration identity,
dedup, MTP-retention/upgrade, pin/restore-state, and accounting behavior. It
does not accept real HBM/DRAM movement, online publication, durable prefix
metadata, shared catalog epochs, direct I/O, four-rank fatal propagation,
CUDA, checkpoint execution, model output, or performance.

## Required adversarial questions

1. Could the prior public residency API replace any same-key record solely
   because the candidate generation was larger, without comparing logical
   target/indexer/draft identity?
2. Could that accept conflicting bytes or downgrade an MTP record to
   target-only outside the prefix coordinator?
3. Could an identical higher-generation candidate demote a pinned HBM page
   and subtract its accounting even though no content changed?
4. Does the correction validate every candidate record and NVMe tier before
   applying the shared `TierRecord::relation_to` matrix?
5. Are exact target/MTP dedup and existing-MTP/candidate-target relations
   omitted from the commit plan so every residency field and counter remains
   unchanged?
6. Is target→MTP the only replacement relation, with strictly larger
   generation, zero pins, and non-restoring state required?
7. Does any logical content mismatch fail as `Record` before planning or
   mutation?
8. Does a multi-record plan reject duplicate page keys and finish all
   relation, state, and accounting validation before any commit?
9. Can a retained record accidentally be subtracted from HBM/DRAM accounting
   or reinserted as NVMe during commit?
10. Do the direct-manager collision and stale-upgrade assertions bypass the
    coordinator and therefore distinguish the prior unsafe boundary?
11. Does the pinned exact-dedup regression prove record generation,
    residency, pin usability, and byte accounting are all retained?
12. Does the real durable MTP upgrade remain rejected while pinned, succeed
    after unpin, and resist a later target-only downgrade?
13. Was the old serving pin probe correctly replaced with explicit unpin
    preflight because exact registration is now intentionally successful?
14. Are the 258-test, 53-handoff, CPU-only boundary, and all exclusions
    accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- direct rank registration rejects every same-key content collision;
- exact dedup and MTP retention cause no residency/accounting mutation;
- only a strictly newer, unpinned, non-restoring MTP upgrade may replace;
- multi-record planning remains all-or-nothing;
- the direct and coordinator regressions distinguish the prior behavior; and
- the CPU proof and all exclusions are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, content replacement outside
the matrix, exact-dedup mutation, pin/restore bypass, partial multi-record
commit, nondistinguishing tests, or overstated production claim.

The token accepts only this CPU rank-residency correction. It does not open
cn4, authorize CUDA work, or accept production cache movement.
