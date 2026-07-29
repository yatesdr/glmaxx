# Fable handoff: prefix-generation integrity v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`2e3aa222e0808c27793798dab6890dbdb7614ed3`

Required result path:
`fable-prefix-generation-integrity-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`prefix-generation-integrity-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/prefix.rs` | `459953bffe50061901dc10ee2a7593bc1cea5e4cd5eb448a8f349a2c261c6ef3` |
| `crates/glm-cache/src/tier.rs` | `c31b07d7f9054f3d51bc5d24c2c414b6c9a134d88f042502bc0f82e29cad500f` |
| `docs/online-prefix-publication-v1.md` | `67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873` |
| `docs/cache-lifecycle-proof-v1.md` | `11ad4936fea7cd0887e660911f50778d5b0918c21a6cebaca1a98a244b2e2de1` |
| `docs/durable-store-single-writer-proof-v1.md` | `cc8e5182bad079c53504780c8ab1f6a7a7f410f094610965e5acd140837f4f47` |
| `docs/prefix-generation-integrity-proof-v1.md` | `4db63b0ddde70e2afe6371fd4b609bd57ad4965bb48cd45c6dfc5d06587473a0` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-prefix-generation-integrity-v1-handoff.md
cargo test --offline -p glm-cache
cargo clippy --offline -p glm-cache --all-targets -- -D warnings
```

## Review boundary

This review covers only retained in-memory `PrefixIndex` handling of repeated
content keys, logical piece identity, atomic collision rejection, and
monotonic MTP capability. It does not accept the pending durable
`insert_child`/`recover_namespace` operations, online publication, a shared
catalog, direct I/O, DRAM/HBM transfers, GPU determinism, checkpoint
execution, model output, or performance.

## Required adversarial questions

1. Could the prior insertion path replace the record under a content-derived
   prefix key using only a larger generation, without comparing target KV,
   target indexer, or draft-sidecar bytes?
2. Could a larger target-only record therefore replace an existing
   MTP-capable record and make a formerly reusable draft prefix disappear?
3. Does corrected preflight validate every `TierRecord`, derive every key,
   reject duplicate derived keys, and complete all compatibility and reference
   overflow checks before mutating any page or reference count?
4. For every same-key pair, are target KV and target indexer compared by both
   logical byte length and SHA-256 regardless of generation, tier, or physical
   offset?
5. When both records are MTP-capable, is the draft sidecar compared by the
   same logical identity, with any mismatch rejected as a collision?
6. Does prior `TierRecord::validate` guarantee the required target/indexer
   pieces, and the draft piece for MTP records, exist exactly once before the
   optional identity helper is used?
7. Does target-only to byte-compatible MTP permit a monotonic upgrade?
8. Does existing MTP plus a byte-compatible newer target-only candidate count
   the shared reference while retaining the MTP record and its generation?
9. Can only a byte-compatible, MTP-capable higher generation replace an
   existing MTP record?
10. On target or draft hash collision, does the regression prove the exact
    prior record and reference count remain unchanged?
11. Would the old implementation fail the downgrade and both collision
    assertions, while the corrected implementation passes all 249 workspace
    tests and the complete local proof gate?
12. Are the retained-index-only boundary, 47-then-present-handoff count, and
    every GPU/durable/publication/model/performance exclusion stated
    accurately?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- same prefix keys can no longer alias different target/indexer bytes;
- two MTP records for one key can no longer alias different draft bytes;
- MTP capability is monotonic under compatible later insertions;
- all collision and overflow failures are preflighted before mutation;
- the regression distinguishes the previous replace-by-generation behavior;
  and
- the CPU proof and all exclusions are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, any logical-byte alias,
capability downgrade, partial mutation, nondistinguishing test, or overstated
production claim.

The token accepts only this retained CPU prefix-index correction. It does not
open cn4, authorize CUDA work, or accept online publication or model
execution.
