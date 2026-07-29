# Fable handoff: torn journal resume v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`8fb3adf9535683b0de9b54fe2743cb5651b9bdc2`

Required result path:
`fable-torn-journal-resume-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`torn-journal-resume-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/store.rs` | `48fb1db40c25109d80b3c2a7ee0fc09346ac52a5758ade1e752a1e4fa577e6e0` |
| `crates/glm-cli/src/cache_proof.rs` | `3371395bb723d2ec092c16cfd28bcb25b54ca1e38fc2096dff471941b2ac9358` |
| `fixtures/cache-lifecycle-proof-v1.json` | `c1151c34a3a9bee4fd97dea11e807603a56c2af4d37deab813cc9b5631177d6a` |
| `docs/journal-tail-corruption-proof-v1.md` | `d8bb0b738caba8afe5a5084ffbc969dacd9d01fa6eae298b9adb289cfa950a7d` |
| `docs/cache-lifecycle-proof-v1.md` | `11ad4936fea7cd0887e660911f50778d5b0918c21a6cebaca1a98a244b2e2de1` |
| `docs/torn-journal-resume-proof-v1.md` | `2c0a5f131e72b41cf76f06e1127db7c9d492ba17332d9dec643392d301f85180` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-torn-journal-resume-v1-handoff.md
cargo test --offline -p glm-cache store::tests
cargo test --offline -p glm-cli cache_lifecycle
cargo clippy --offline -p glm-cache -p glm-cli --all-targets -- \
  -D warnings
```

## Review boundary

This review covers only retained CPU handling of a validated incomplete
journal tail before writable append, plus read-only nonmutation and the
refreshed deterministic lifecycle fixture. It does not accept general
salvage, complete-record repair, compaction, direct I/O, online publication,
CUDA, checkpoint execution, model output, or performance.

## Required adversarial questions

1. Could the prior writer ignore a short tail, seek to its physical end, and
   append a new complete record at a permanently misaligned offset?
2. Would the next reopen then decode the tail plus the front of that record as
   one corrupt complete record?
3. Does corrected writable open acquire the exclusive lock and finish strict
   complete-record decode, journal replay, catalog recovery, and offset
   derivation before truncating anything?
4. Is truncation limited exactly to
   `journal_len % JOURNAL_RECORD_BYTES`, with no complete record removed?
5. Is the repaired length synced before seeking to end and accepting a new
   publication?
6. Does any complete invalid final record still fail before the truncation
   branch?
7. Does `FileTierReader` restore across a short tail without changing the
   journal length?
8. Does the writer regression prove exactly 113 bytes are removed, then
   publish page B and restore both pages after another close/reopen?
9. Would the previous implementation fail that final reopen?
10. Is the lifecycle fixture delta limited to the intended journal digest,
    with all other artifact fields unchanged?
11. Does the refreshed fixture reproduce byte-for-byte under
    `scripts/local-checks.sh`?
12. Are the retained CPU boundary, 254-test, 51-handoff, and all
    GPU/direct-I/O/model/performance exclusions stated accurately?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- writable recovery resumes only from a validated record boundary;
- no complete invalid record is truncated or repaired;
- read-only snapshots never mutate the short tail;
- post-repair publication remains recoverable on the next restart;
- the regression and refreshed fixture distinguish the prior behavior; and
- the CPU proof and all exclusions are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, truncation before complete
validation, any full-record deletion, reader mutation, misaligned resumed
append, nondistinguishing fixture/test, or overstated production claim.

The token accepts only this retained CPU torn-tail resume correction. It does
not open cn4, authorize CUDA work, or accept online publication or model
execution.
