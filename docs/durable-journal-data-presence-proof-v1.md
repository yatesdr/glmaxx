# Durable journal/data presence CPU proof v1

Date: 2026-07-29

Implementation and regression commit:
`d3ca693e4c0e5de16e03518e19b7dadc8d8323c3`

Status: CPU startup-integrity correction passed; independent review pending

GPU claim: none

## Defect and retained write-order invariant

`FileTierStore::open` and `FileTierReader::open` previously accepted a
nonempty `pages.dat` when `journal.log` contained no complete 512-byte
record. Replay treated that state as an ordinary empty cache, so deleting,
replacing, or truncating the entire journal could silently discard the
catalog while retaining apparently valid page data.

That state cannot be produced by the retained writer protocol. Publication
performs these durable operations in order:

1. construct and validate the complete page record;
2. append its complete `Begin` journal record;
3. synchronize `journal.log`;
4. only after that synchronization succeeds, write page payload bytes; and
5. synchronize `pages.dat` before journaled piece attestations and
   publication.

An error while appending or synchronizing the first `Begin` returns before
the first payload write. Therefore a pristine empty journal may accompany
only an empty data file. A journal containing only a sub-record torn tail is
also zero complete records and cannot legitimately accompany payload bytes
under this protocol.

The corrected writer and read-only snapshot share one startup invariant:
if the count of complete journal bytes is zero while the physical data length
is nonzero, construction returns `StoreError::UnjournaledData`. The check
uses the complete-record boundary, not the raw journal length.

This does not broaden torn-tail repair. When one or more complete records
exist, a short trailing fragment remains an ignorable crash tail. Every
complete record still passes strict decoding and checksum validation, and
catalog extents still pass bounds and overlap validation before startup
succeeds.

## Distinguishing CPU proof

`nonempty_data_without_a_complete_journal_fails_closed` first creates the
store files, writes 4,096 data bytes while leaving the journal empty, and
proves that both:

- `FileTierStore::open`; and
- `FileTierReader::open`

return exactly `UnjournaledData`.

It then writes a 113-byte journal fragment and proves that both constructors
still return `UnjournaledData`. The prior implementation returns successful
empty catalogs for both the empty-journal and torn-only-journal cases, so all
four assertions distinguish the correction.

Existing regressions retain both adjacent boundaries:

- `torn_trailing_journal_record_is_ignored` proves a valid prior journal plus
  a 113-byte tail remains readable, is repaired by the exclusive writer, and
  supports later publication and another reopen; and
- `complete_corrupt_trailing_journal_record_is_never_ignored` proves a full
  corrupt record still prevents both reader and writer startup.

## Gate result and exclusions

The full local gate passed 267 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, and all 61 then-present review
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
7408e65a42e4e15598a761587dec31b63736c316f4f49a0d42c47cfd44884dff

docs/durable-store-write-fail-stop-proof-v1.md
067c2b1f93a72ca0a5e661be02bad665da658154e3bfdf9ab33744137873dd09

docs/torn-journal-resume-proof-v1.md
2c0a5f131e72b41cf76f06e1127db7c9d492ba17332d9dec643392d301f85180

docs/journal-tail-corruption-proof-v1.md
d8bb0b738caba8afe5a5084ffbc969dacd9d01fa6eae298b9adb289cfa950a7d

docs/durable-catalog-extent-integrity-proof-v1.md
b8f38cf3ab3fde74d505ea7a118d063d3e235dd4049b6ce6e47c071099a2ea7d

docs/direct-tier-io-v1.md
7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, or model execution
was used.

This correction detects total loss of complete journal history when physical
page data remains. It does not add redundant metadata, journal mirroring,
general salvage, operator repair, direct I/O, `io_uring`, online publication,
segment cleaning, GPU transfers, or performance evidence. It does not claim
that nonempty data following at least one valid journal record is corrupt:
such bytes can be legitimate unpublished crash orphans and remain protected
by the existing append-only and catalog-integrity rules.
