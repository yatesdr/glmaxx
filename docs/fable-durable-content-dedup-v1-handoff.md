# Fable handoff: durable content deduplication v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`b097703b0a6def10d3732ae70835881c93a954dd`

Required result path:
`fable-durable-content-dedup-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`durable-content-dedup-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/lib.rs` | `0c287e5a542c242e18a3d20c25d8ef8e61bba69ce04c854e714915d915aadab0` |
| `crates/glm-cache/src/tier.rs` | `0a1541f13462bcdec92284911f96531b06869b60c7fe85fc5e9669c80fabe693` |
| `crates/glm-cache/src/store.rs` | `fd16e7e795ce742aff0b72125988b019b3f36cbfebd1f67dab2dd9ea8d72c5ad` |
| `crates/glm-cache/src/prefix.rs` | `ad0bc0e498050d948807c9f1e27e5f98ea02c4fa334725428a8dab1dab068298` |
| `crates/glm-cache/src/residency.rs` | `04ffe885557b81ca91797b84f31bf6ae3f6f35bc4b7a5dae6bdc9ab08983e664` |
| `crates/glm-serving/src/cache.rs` | `3026b4d3353839c0a644944e8a6103f2b168e741d25d272ea2d7d330e1610635` |
| `docs/online-prefix-publication-v1.md` | `67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873` |
| `docs/prefix-generation-integrity-proof-v1.md` | `4db63b0ddde70e2afe6371fd4b609bd57ad4965bb48cd45c6dfc5d06587473a0` |
| `docs/prefix-residency-coherence-proof-v1.md` | `3f99eeb1f4f003f211922a906939ce9d6bbe03fb9b43ed13091fd38349bd194c` |
| `docs/durable-content-dedup-proof-v1.md` | `75fd16886ef50e4509fb0a7b0701417a1469a0dad78809b39ce08e3e736a7514` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-durable-content-dedup-v1-handoff.md
cargo test --offline -p glm-cache
cargo test --offline -p glm-serving --lib cache::tests
cargo clippy --offline -p glm-cache -p glm-serving --all-targets -- \
  -D warnings
```

## Review boundary

This review covers the retained synchronous CPU logical-content relation,
prefix insertion, file-store preflight, journal replay, and coordinator
registration matrix. It does not accept the pending online publication
service, parent/ordinal recovery catalog, direct I/O, shared live catalog,
real DRAM/HBM movement, CUDA, checkpoint execution, model output, or
performance.

## Required adversarial questions

1. Could the prior file store append any larger generation for a page key
   without comparing target, indexer, or draft logical hashes?
2. Did the prior prefix index also replace exact same-content records with a
   larger generation despite the contract requiring revision retention?
3. Did prior journal replay select the largest generation without enforcing
   collision or MTP capability rules?
4. Does `TierRecord::relation_to` validate both records and compare exact
   namespace, page key, and `(byte_length, sha256)` for every applicable
   logical piece while ignoring only tier and physical offsets?
5. Does the relation implement all seven cells of the frozen
   deduplication/MTP-upgrade matrix without permitting MTP downgrade?
6. Is a target-only→MTP transition the only relation that can replace a
   record, and must its durable revision be strictly larger?
7. Do exact target, exact MTP, and MTP-retaining target-only candidates always
   retain the existing durable revision regardless of candidate generation?
8. Does file-store classification finish before `TierJournal::begin`, journal
   append, data write, transaction advance, or catalog mutation?
9. Do exact dedups leave journal/data lengths and the published record exactly
   unchanged?
10. Do target and draft collisions return `ContentCollision` with no file or
    catalog mutation and without poisoning later unrelated writes?
11. Does actual post-mutation failure still poison all later writes until
    close/reopen replay?
12. Does journal recovery independently retain exact dedup/MTP, adopt only a
    strictly newer MTP upgrade, and reject a fully durable collision?
13. Does the prefix test reject a non-newer MTP upgrade and retain an exact
    larger MTP revision?
14. Does the two-rank regression prove a late pinned-rank upgrade failure
    changes neither earlier-rank residency nor the prefix index, then prove a
    successful retry with real draft-required restore?
15. Is the prior prefix proof's exact-refresh claim explicitly superseded
    rather than silently represented as current?
16. Are the retained CPU boundary, 253-test, 49-handoff, and all
    GPU/publication/model/performance exclusions stated accurately?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- all three retained layers implement one exact logical-content relation;
- exact dedup performs no durable write and retains the revision;
- MTP upgrade is the only allowed same-key replacement;
- collisions fail before mutation and replay refuses preexisting collisions;
- the regressions distinguish the prior store, replay, prefix, and
  cross-rank behaviors; and
- the CPU proof and all exclusions are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, any content alias, exact
dedup write/revision advance, MTP downgrade, collision after mutation,
partial rank commit, nondistinguishing test, or overstated production claim.

The token accepts only this retained CPU content-matrix correction. It does
not open cn4, authorize CUDA work, or accept online publication or model
execution.
