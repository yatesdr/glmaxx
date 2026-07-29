# Fable handoff: durable tier-store write fail-stop v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`a5019aafa7400f82928d944b0fb9a31ddae0605d`

Required result path:
`fable-durable-store-write-fail-stop-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`durable-store-write-fail-stop-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `crates/glm-cache/src/store.rs` | `8658f495486cfe35e9b7bc9581520201cad30a5704d9be4a2f44b36c09df07ed` |
| `crates/glm-cache/src/tier.rs` | `c31b07d7f9054f3d51bc5d24c2c414b6c9a134d88f042502bc0f82e29cad500f` |
| `docs/online-prefix-publication-v1.md` | `67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873` |
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `docs/cache-lifecycle-proof-v1.md` | `11ad4936fea7cd0887e660911f50778d5b0918c21a6cebaca1a98a244b2e2de1` |
| `docs/durable-store-write-fail-stop-proof-v1.md` | `067c2b1f93a72ca0a5e661be02bad665da658154e3bfdf9ab33744137873dd09` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-durable-store-write-fail-stop-v1-handoff.md
cargo test --offline -p glm-cache store::tests::
cargo clippy --offline -p glm-cache --all-targets -- -D warnings
```

## Review boundary

This review covers only fail-stop behavior after uncertain writes in the
current synchronous CPU `FileTierStore`. It does not accept the pending
online-publication or direct-I/O designs, registered memory, `io_uring`,
segment cleaning, HBM/DRAM transfers, rank uploads, CUDA, checkpoint
execution, model output, or performance.

## Required adversarial questions

1. Could a public `publish` error previously occur after in-memory journal
   mutation, a durable begin, payload writes/sync, piece-durable records, or
   in-memory publication?
2. Did the same live store previously accept a second publication after every
   such returned error?
3. Can a partial journal `write_all` followed by another append turn a torn
   trailing record into an interior corruption that the replay rule cannot
   safely ignore?
4. Are stale generation, piece shape/count, checked extent, final tail
   alignment, and complete `TierRecord` validation resolved before the store
   enters `publish_prevalidated`?
5. Does every error returned by `publish_prevalidated`, including every
   append, write, sync, journal transition, sequence, and injected crash
   error, set `write_poisoned` before control returns?
6. Does a poisoned publication return `WritePoisoned` before changing either
   file, either offset/sequence, the in-memory journal, or the published map?
7. Do preflight errors leave the writer usable without weakening validation?
8. Can already published pages still be read while writes are poisoned
   without exposing the failed transaction?
9. Does close/reopen derive visibility only from fully durable published
   transactions and create the sole supported unpoisoned recovery path?
10. Does the distinguishing regression verify both file lengths, prior-page
    readability, failed-page invisibility, and successful post-replay reuse?
11. Do the three phase failpoints independently prove that begin-journaled,
    data-synced, and first-piece-journaled failures all poison later writes?
12. Would the prior implementation fail the poisoned-write assertions for
    each phase?
13. Are the 247-test claim, 45-handoff claim, and every GPU/model/performance
    exclusion accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- the preflight/commit boundary is complete and correctly placed;
- every uncertain publication failure fail-stops later writes;
- ordinary validation failures remain safely retryable;
- poisoned reads and reopen/replay preserve only durable visibility;
- the regressions distinguish the prior behavior at every injected phase;
  and
- the CPU proof and all exclusions are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, a post-mutation error that
does not poison, any file mutation after poison, an incorrectly poisoned
preflight error, failed-page visibility, unsupported in-process recovery, a
nondistinguishing regression, or an overstated production claim.

The token accepts only this synchronous CPU durability correction. It does
not open cn4, authorize CUDA work, or accept real model execution.
