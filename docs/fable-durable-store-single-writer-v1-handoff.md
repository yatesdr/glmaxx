# Fable handoff: durable tier-store single-writer ownership v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`535a8d6764ff968a21cb5d668e1d895ef0e940fb`

Required result path:
`fable-durable-store-single-writer-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`durable-store-single-writer-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `Cargo.lock` | `392c03e631b234e57cf9950078f2add73d06ae427e1616438725f001fe414bec` |
| `crates/glm-cache/Cargo.toml` | `176be2353dcee1c479714247fedf380cd36de29a8390069406e4853250d89e67` |
| `crates/glm-cache/src/lib.rs` | `1ade53e58b2f9f9f122185ad4a6c986dd4b8fa815e7533cf7ccf7ea8bb07b00e` |
| `crates/glm-cache/src/store.rs` | `5a4229e2c82c158ed6172a574c912ee2145438959bf676491282cd61d9d49247` |
| `crates/glm-cache/src/residency.rs` | `30ad6f64069b5766c71d9c8c78e90ad4e25a8cbf2db66a70d36a36c1eeda3c3f` |
| `docs/online-prefix-publication-v1.md` | `67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873` |
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `docs/durable-store-write-fail-stop-proof-v1.md` | `067c2b1f93a72ca0a5e661be02bad665da658154e3bfdf9ab33744137873dd09` |
| `docs/durable-store-single-writer-proof-v1.md` | `cc8e5182bad079c53504780c8ab1f6a7a7f410f094610965e5acd140837f4f47` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-durable-store-single-writer-v1-handoff.md
cargo test --offline -p glm-cache
cargo test --offline -p glm-serving --lib
cargo clippy --offline -p glm-cache --all-targets -- -D warnings
```

## Review boundary

This review covers only exclusive writer ownership and read-only snapshot
restore handles in the retained synchronous CPU tier store. It does not
accept the pending online-publication or direct-I/O designs, a live shared
catalog, registered memory, `io_uring`, segment cleaning, HBM/DRAM
transfers, CUDA, checkpoint execution, model output, or performance.

## Required adversarial questions

1. Did two prior `FileTierStore::open` instances derive independent copies of
   the same next transaction, next data offset, journal, and published map?
2. Could those instances therefore collide on transaction IDs, payload
   extents, journal appends, and durable visibility?
3. Did each of four `RestoreService` workers previously own the public
   write-capable store type even though it used only `restore`?
4. Does corrected `FileTierStore::open` acquire a nonblocking exclusive
   journal lock before reading replay bytes or returning a writable handle?
5. Does the journal descriptor retain that exclusive lock for the complete
   writer lifetime, including publication and poisoned-read operation?
6. Does a second writer fail with `WriterLocked` before journal replay or any
   mutation?
7. Does `FileTierReader` expose no publication method and open both journal
   and data read-only?
8. Does its transient shared journal lock prevent snapshot creation while a
   writer is live and protect the complete journal read/replay interval?
9. After snapshot construction, can four readers coexist and restore the
   same checksummed page without holding write authority?
10. Can a later writer coexist with those readers without overwriting any
    extent present in their published snapshots?
11. Do existing readers deliberately remain snapshot-isolated from the later
    publication?
12. Does `RestoreService` now own only `FileTierReader`, and do all 32 serving
    tests prove four-worker construction and restoration still succeed?
13. Would the old implementation fail the second-writer rejection, and would
    a lifetime shared reader lock fail the later-writer assertion?
14. Are advisory-lock, private-catalog, 248-test, 46-handoff, and all
    GPU/model/performance limitations stated accurately?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- the retained store now has exactly one live writable authority;
- restore workers have no journal/data mutation capability;
- snapshot creation is serialized safely against the writer;
- four read workers and a later writer preserve immutable snapshot extents;
- the regressions distinguish both the old multi-writer and overlocked-reader
  alternatives; and
- the CPU proof and all exclusions are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, any second live writer,
reader mutation authority, snapshot replay concurrent with a writer, a
published extent that can be overwritten, a reader that falsely observes a
later record, a nondistinguishing regression, or an overstated production
claim.

The token accepts only this retained synchronous CPU ownership correction. It
does not open cn4, authorize CUDA work, or accept real model execution.
