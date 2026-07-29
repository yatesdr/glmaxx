# Fable handoff: durable catalog extent integrity v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`de2d43a44474427d6f67fdb7fa300307d7b1caed`

Required result path:
`fable-durable-catalog-extent-integrity-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`durable-catalog-extent-integrity-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/store.rs` | `a68c672d51f59b79efeb514f8690aa2263730b2df5ed1ece688793ed6f897996` |
| `crates/glm-cache/src/tier.rs` | `0a1541f13462bcdec92284911f96531b06869b60c7fe85fc5e9669c80fabe693` |
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `docs/durable-store-single-writer-proof-v1.md` | `cc8e5182bad079c53504780c8ab1f6a7a7f410f094610965e5acd140837f4f47` |
| `docs/torn-journal-resume-proof-v1.md` | `2c0a5f131e72b41cf76f06e1127db7c9d492ba17332d9dec643392d301f85180` |
| `fixtures/cache-lifecycle-proof-v1.json` | `c1151c34a3a9bee4fd97dea11e807603a56c2af4d37deab813cc9b5631177d6a` |
| `docs/durable-catalog-extent-integrity-proof-v1.md` | `b8f38cf3ab3fde74d505ea7a118d063d3e235dd4049b6ce6e47c071099a2ea7d` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-durable-catalog-extent-integrity-v1-handoff.md
cargo test --offline -p glm-cache store::tests
cargo clippy --offline -p glm-cache --all-targets -- -D warnings
```

## Review boundary

This review covers only recovered-live-catalog interval validation and
physical-EOF append allocation in the retained buffered CPU store. It does
not accept startup payload authentication, obsolete/incomplete transaction
extents, direct I/O, catalog epochs, segment cleaning, asynchronous online
publication, CUDA, checkpoint execution, model output, or performance.

## Required adversarial questions

1. Could the prior reader and writer report healthy after replaying two live
   records whose individually valid piece intervals overlap each other?
2. Could they report healthy when a live interval ends beyond the physical
   length of `pages.dat`?
3. Does the correction derive every live half-open interval with checked
   addition, sort deterministically, and reject every adjacent overlap?
4. Is adjacent-pair testing after start/end sorting sufficient to detect any
   overlap in a set of nonempty half-open intervals?
5. Do both read-only and writable open validate catalog extents after strict
   journal replay and before returning success?
6. Does writable open finish catalog and data-bound validation before it may
   truncate a short journal tail?
7. Does a valid empty catalog/data pair remain accepted?
8. Are trailing unreferenced data bytes allowed but preserved, with the next
   allocation aligned at or beyond physical EOF?
9. Can overflow while deriving an extent end or next allocation offset fail
   closed without journal or data mutation?
10. Does the forged-overlap regression retain a valid per-record layout,
    valid record CRC, complete durability events, and therefore distinguish
    cross-record validation from prior per-record validation?
11. Do the one-byte truncation and 8,192-byte sentinel regressions
    distinguish the former lazy-bound-check and live-maximum allocator?
12. Do existing checksum, torn-tail, complete-corrupt-tail, and lifecycle
    proofs remain green and unchanged?
13. Are the 257-test, 52-handoff, CPU-only boundary, and all exclusions
    accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- reader and writer startup reject every overlapping live catalog;
- reader and writer startup reject every live out-of-bounds extent;
- invalid catalogs cause no short-tail repair or other mutation;
- resumed publication preserves every byte before prior physical EOF;
- the three regressions distinguish the prior implementation; and
- the CPU proof and all exclusions are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, incomplete interval
validation, mutation before validation, allocator reuse below prior physical
EOF, nondistinguishing tests, or overstated production claim.

The token accepts only this retained CPU catalog-extent correction. It does
not open cn4, authorize CUDA work, or accept the production direct-tier
implementation.
