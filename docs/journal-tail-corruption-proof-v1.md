# Complete journal-tail corruption CPU proof v1

Date: 2026-07-29

Implementation and regression commit:
`2da57248f703702f2fddbf189a65f294cabbc649`

Status: CPU journal replay correction passed; independent review pending

GPU claim: none

## Defect and boundary

The retained journal decoder divided the file into complete 512-byte records
and a short remainder. It correctly ignored the short remainder as a torn
trailing write, but it also ignored any decoding error in the final complete
record.

That exception covered far more than a torn fragment. A full-size final
record with a bad CRC, bad magic, unsupported version, invalid event type,
invalid tier, impossible piece table, or malformed `TierRecord` was silently
dropped.

The most damaging case is corruption of the final `Publish` record. Replay
would keep its preceding `Begin` and `PieceDurable` events as an invisible
orphan and reopen successfully with the previously published page missing.
That is silent catalog loss, not fail-closed recovery.

The corrected boundary is exact:

- bytes after the last complete 512-byte record remain an ignored crash tail;
- every complete record, including the last, must pass CRC, header, schema,
  event, piece, and `TierRecord` validation; and
- any complete invalid record prevents both writable store and read-only
  snapshot construction.

No truncation or repair is performed automatically.

## Distinguishing CPU proof

`complete_corrupt_trailing_journal_record_is_never_ignored`:

1. publishes a real target page;
2. flips one byte in the complete final publish record;
3. proves both `FileTierStore::open` and `FileTierReader::open` return exactly
   `JournalChecksum`;
4. creates a second valid store and appends one complete 512-byte garbage
   record; and
5. proves writable reopen again returns `JournalChecksum`.

The prior decoder drops both invalid final records and returns success. In
the first case it silently loses the page from the recovered catalog.

`torn_trailing_journal_record_is_ignored` remains unchanged and appends only
113 bytes. Reopen still succeeds and restores the page, distinguishing a
genuine incomplete crash tail from a complete corrupt record.

## Gate result and exclusions

The full local gate passed 254 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, and all 50 then-present review
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
crates/glm-cache/src/store.rs
30a281fbd79bccd58ebecfdb906029985ace6df50907b2b84476f044586b8fc0

crates/glm-cache/src/tier.rs
0a1541f13462bcdec92284911f96531b06869b60c7fe85fc5e9669c80fabe693

docs/durable-content-dedup-proof-v1.md
75fd16886ef50e4509fb0a7b0701417a1469a0dad78809b39ce08e3e736a7514

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

This correction does not add journal truncation, salvage, compaction,
redundant superblocks, direct I/O, a live catalog, or online publication.
Recovery remains the synchronous retained CPU path. Operator-directed repair
and segment-level durability belong to the pending direct-tier service.
