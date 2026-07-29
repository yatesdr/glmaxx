# Durable tier-store single-writer CPU proof v1

Date: 2026-07-29

Implementation and final regression commit:
`37268646cff31d8d6a637389508defd3d6e272f9`

Status: CPU ownership correction passed; independent review pending

GPU claim: none

## Defect and ownership invariant

The durable tier contracts require one process-wide journal/data append
authority. The retained blocking implementation did not enforce that rule.
Every `FileTierStore::open` independently replayed the same journal and
derived the same next transaction and data offsets. Two live instances could
therefore:

- allocate the same transaction ID;
- write different payloads at the same physical offsets;
- append interleaved journal records; and
- publish private catalogs that disagree about durable visibility.

The restore path made the problem broader: each of four rank restore services
opened a `FileTierStore`, so read workers held the public write-capable type
even though they called only `restore`.

The corrected blocking store enforces two roles:

- `FileTierStore` acquires a nonblocking exclusive `flock` on `journal.log`
  before replay. Its journal descriptor retains that lock for the complete
  store lifetime. A second writer fails with `WriterLocked` before replay or
  mutation.
- `FileTierReader` has no publication method, opens both files read-only, and
  takes a transient shared lock while reading and validating its journal
  snapshot. A live writer prevents snapshot creation. After the snapshot is
  built, the journal descriptor and shared lock are released; the reader
  retains only a read-only data handle and immutable private catalog.

`RestoreService` now owns `FileTierReader`, not `FileTierStore`. Four rank
workers can therefore restore concurrently without acquiring transaction IDs,
choosing append offsets, or exposing a write method.

Existing snapshot readers may coexist with a writer opened later. Published
extents are append-only in this blocking implementation, so those readers can
continue reading their validated snapshot but cannot observe later records.

## Distinguishing CPU proof

`journal_lock_enforces_one_live_store_writer`:

1. opens a writer and publishes one page;
2. proves a second writer returns exactly `WriterLocked`;
3. proves a reader snapshot also returns `WriterLocked` while the writer owns
   the journal;
4. proves the live writer can still restore its prior page;
5. drops the writer and opens four simultaneous read-only snapshots;
6. proves all four restore the same checksummed page;
7. opens a new writer while those snapshot readers remain live;
8. publishes and restores a second page through the writer; and
9. proves every existing reader remains snapshot-isolated and reports the
   second page absent.

The prior implementation fails the second-writer rejection and gives every
restore worker a write-capable store. A variant that retains a shared lock
for each reader's whole lifetime would fail step 7 and prevent the required
four-reader/one-later-writer ownership shape.

The full serving suite also distinguishes the type split: all 32 tests pass
with four `RestoreService` workers backed by `FileTierReader`. Giving the
exclusive writer lock to each restore worker makes coordinator construction
fail on the second worker.

## Gate result and exclusions

The full local gate passed 248 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, and all 46 then-present review
handoff provenance proofs.

Commands:

```text
cargo test --workspace --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo fmt --all -- --check
scripts/local-checks.sh
```

Relevant hashes:

```text
Cargo.lock
392c03e631b234e57cf9950078f2add73d06ae427e1616438725f001fe414bec

crates/glm-cache/Cargo.toml
176be2353dcee1c479714247fedf380cd36de29a8390069406e4853250d89e67

crates/glm-cache/src/lib.rs
1ade53e58b2f9f9f122185ad4a6c986dd4b8fa815e7533cf7ccf7ea8bb07b00e

crates/glm-cache/src/store.rs
5a4229e2c82c158ed6172a574c912ee2145438959bf676491282cd61d9d49247

crates/glm-cache/src/residency.rs
30ad6f64069b5766c71d9c8c78e90ad4e25a8cbf2db66a70d36a36c1eeda3c3f

docs/online-prefix-publication-v1.md
67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873

docs/direct-tier-io-v1.md
7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2

docs/durable-store-write-fail-stop-proof-v1.md
067c2b1f93a72ca0a5e661be02bad665da658154e3bfdf9ab33744137873dd09

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, or model execution
was used.

`flock` is an advisory retained-blocking-store guard, not the final
`TierIoService` ownership mechanism. This correction does not implement a
live shared catalog, online prefix publication, direct I/O, `io_uring`,
registered buffers, segment cleaning, asynchronous publication, HBM/DRAM
movement, cross-process hostile-writer defense, or performance evidence.
Snapshot readers deliberately do not see records published after they open.
The final process-wide service must replace their private catalogs with the
reviewed immutable catalog-epoch design.
