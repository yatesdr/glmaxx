# Fable handoff: prefix/residency generation coherence v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`72e60716cf58632dd9aba5ead41ba0d128f59395`

Required result path:
`fable-prefix-residency-coherence-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`prefix-residency-coherence-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/lib.rs` | `d7727125c2b022b2cd1da7e51b07b1e06365da3ed530b2735478b3ac40f67b06` |
| `crates/glm-cache/src/prefix.rs` | `7b4aff1407f83b2e12216d7a051049c1a5359f0bae7fb88724e8999077260f70` |
| `crates/glm-cache/src/residency.rs` | `04ffe885557b81ca91797b84f31bf6ae3f6f35bc4b7a5dae6bdc9ab08983e664` |
| `crates/glm-serving/src/cache.rs` | `709ab616feca96818f6fc6ce1331becd93de9f67324d2b278503f6f2ad3efe1f` |
| `crates/glm-cache/src/tier.rs` | `c31b07d7f9054f3d51bc5d24c2c414b6c9a134d88f042502bc0f82e29cad500f` |
| `docs/online-prefix-publication-v1.md` | `67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873` |
| `docs/prefix-generation-integrity-proof-v1.md` | `4db63b0ddde70e2afe6371fd4b609bd57ad4965bb48cd45c6dfc5d06587473a0` |
| `docs/prefix-residency-coherence-proof-v1.md` | `3f99eeb1f4f003f211922a906939ce9d6bbe03fb9b43ed13091fd38349bd194c` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-prefix-residency-coherence-v1-handoff.md
cargo test --offline -p glm-serving --lib \
  prefix_registration_uses_the_monotonic_index_record_atomically
cargo test --offline -p glm-cache
cargo clippy --offline -p glm-cache -p glm-serving --all-targets -- \
  -D warnings
```

## Review boundary

This review covers only retained in-process coordinator consistency between
`PrefixIndex` and four CPU `ResidencyManager`s during repeated prefix
registration. It does not accept durable recovery indexing, online
publication, direct I/O, a shared catalog, GPU/host movement, CUDA,
checkpoint execution, model output, or performance.

## Required adversarial questions

1. After the prefix-only correction, could the coordinator still retain an
   MTP record in `PrefixIndex` but install the newer target-only input record
   into residency under the same key?
2. Could draft-required lookup therefore advertise capability that exact
   restore-record validation could not satisfy?
3. Did exact/lower-generation dedup also increment the prefix reference and
   then fail residency registration as stale?
4. Does corrected registration construct the complete candidate index before
   changing live index or residency state?
5. For every key, does it fail closed unless the live index record exactly
   equals the owner rank's live residency record?
6. Is the post-insert candidate-index record, rather than the caller's input,
   the only record eligible for residency registration?
7. Does an unchanged authoritative record produce no residency write while
   preserving the prefix reference increment?
8. Are only actual changes grouped by deterministic `owner_rank(ordinal)`?
9. Does each rank plan reject invalid/stale/pinned/restoring records,
   duplicate page keys, or accounting underflow before any rank plan commits?
10. Are all four plans produced before the first infallible metadata commit,
    followed by candidate-index adoption?
11. Does planning copy only changed `TierRecord` metadata rather than
    resident/restored page payloads?
12. Does the regression prove exact dedup, target→MTP upgrade,
    MTP-preserving target-only dedup, target collision atomicity, and a real
    draft-required file restore?
13. Would the previous coordinator fail that regression either at exact
    dedup or later exact-record restore validation?
14. Does constructor rejection of a nonempty initial index prevent an
    immediately divergent state until durable parent/ordinal recovery exists?
15. Are the synchronous CPU boundary, 250-test, 48-handoff, and all
    GPU/publication/model/performance exclusions stated accurately?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- the index and owner-rank residency record remain exactly coherent;
- MTP capability cannot be lost through the coordinator after being retained
  by the index;
- exact dedup succeeds without rewriting residency;
- all rank registration failures are atomic across ranks and the index;
- the regression distinguishes the prior cross-component defect; and
- the CPU proof and all exclusions are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, any index/residency
divergence, MTP downgrade, partial rank mutation, unbounded payload clone,
nondistinguishing test, or overstated production claim.

The token accepts only this retained CPU coordinator correction. It does not
open cn4, authorize CUDA work, or accept durable online publication or model
execution.
