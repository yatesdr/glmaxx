# Torn journal resume CPU proof v1

Date: 2026-07-29

Implementation commit:
`da22ab4254592b3e21033ff1330212954f27b5ec`

Final regression-fixture commit:
`c768220cb6f5d4f595a0e4800b4b8aaf398ac25a`

Status: CPU torn-tail resume correction passed; independent review pending

GPU claim: none

## Defect and repair transaction

The strict decoder correctly ignores a sub-512-byte trailing fragment as an
incomplete journal record. A writable reopen previously left that fragment
on disk and sought to the physical end of file. The next valid 512-byte
record was appended after the fragment instead of at a record boundary.

On the following restart, fixed-size decoding consumed the fragment plus the
front of the new record as one corrupt record. A store that had appeared to
recover successfully from a torn tail therefore became unrecoverable as soon
as it published again.

The corrected writable-open sequence is:

1. acquire the exclusive writer lock;
2. read the complete journal bytes;
3. strictly decode every complete 512-byte record;
4. replay and validate the complete journal, including logical-content
   collision and publication durability rules;
5. derive the published catalog, next transaction, and next data offset;
6. only after all validation succeeds, truncate a sub-record trailing
   fragment to the last complete boundary;
7. `sync_data` the repaired file length; and
8. seek to the repaired end before allowing publication.

The repair never truncates a complete invalid record. That case remains a
hard error under `journal-tail-corruption-proof-v1`.

`FileTierReader` remains strictly read-only. It may ignore the same short
fragment for a private snapshot, but it neither truncates nor syncs the
journal.

## Distinguishing CPU proof

The extended `torn_trailing_journal_record_is_ignored`:

1. publishes page A and appends a 113-byte crash fragment;
2. opens a read-only snapshot, restores A, and proves the journal length is
   unchanged;
3. opens the exclusive writer, restores A, and proves exactly 113 bytes were
   truncated;
4. publishes page B after the repaired boundary;
5. closes and reopens the writer; and
6. restores both A and B.

The prior implementation passes steps 1 and 2, does not truncate at step 3,
and fails the second reopen because B begins 113 bytes off the record
boundary.

The complete-corrupt-tail regression still proves that a full 512-byte
invalid record prevents both reader and writer open. The repair is therefore
limited to bytes that cannot constitute a complete record.

The deterministic cache-lifecycle proof also exercises torn-tail recovery.
Its only output change is the expected journal SHA-256 after removing the
orphan fragment:

```text
old 4e408ba928de61dcd9f8b5a21aeeb5f7f7282ffeb81839b088529e0f7bfe15d7
new daf77c508fd2288de635249bb5ea9b42ae3e75d3bc36fea8e34b51a3e46ee98a
```

All other lifecycle fields and the 951-byte artifact shape remain unchanged.

## Gate result and exclusions

The full local gate passed 254 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, the refreshed cache-lifecycle fixture,
and all 51 then-present review handoff provenance proofs.

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
48fb1db40c25109d80b3c2a7ee0fc09346ac52a5758ade1e752a1e4fa577e6e0

crates/glm-cli/src/cache_proof.rs
3371395bb723d2ec092c16cfd28bcb25b54ca1e38fc2096dff471941b2ac9358

fixtures/cache-lifecycle-proof-v1.json
c1151c34a3a9bee4fd97dea11e807603a56c2af4d37deab813cc9b5631177d6a

docs/journal-tail-corruption-proof-v1.md
d8bb0b738caba8afe5a5084ffbc969dacd9d01fa6eae298b9adb289cfa950a7d

docs/cache-lifecycle-proof-v1.md
11ad4936fea7cd0887e660911f50778d5b0918c21a6cebaca1a98a244b2e2de1

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, or model execution
was used.

This correction is not general journal salvage or compaction. It does not
repair complete corrupt records, validate filesystem atomicity beyond the
retained sync protocol, implement redundant metadata, direct I/O, online
publication, or performance evidence. The pending production tier service
still needs its reviewed segment and catalog recovery design.
