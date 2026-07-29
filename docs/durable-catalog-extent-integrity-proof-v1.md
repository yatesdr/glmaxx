# Durable catalog extent integrity CPU proof v1

Date: 2026-07-29

Implementation commit:
`a44a69156e3a16ff71d609158c54598332745303`

Status: CPU startup-integrity correction passed; independent review pending

GPU claim: none

## Defect and correction

The retained blocking store validated each `TierRecord` independently, but
did not validate the recovered live catalog against `pages.dat` as a whole.
A CRC-valid, fully published journal could therefore:

- alias one live page piece with another live page piece; or
- name a live piece whose end lies beyond the physical data file.

Reader and writer startup both reported success. The defect became visible
only when a later restore returned a checksum or short-read error.

The corrected startup path derives every live half-open physical interval
`[storage_offset, storage_offset + byte_length)`, with checked arithmetic,
and rejects startup if:

1. any live interval ends beyond the current `pages.dat` length; or
2. any two live intervals overlap after deterministic start/end sorting.

Both `FileTierStore::open` and `FileTierReader::open` run this validation
after strict journal decoding and recovery. The writable path runs it before
repairing a short journal tail, so invalid catalog or data state causes no
journal mutation.

Writable reopen now derives its next aligned allocation offset from physical
data-file length, not only from the maximum live catalog end. This preserves
all bytes belonging to crash orphans or older immutable snapshots. The
blocking store remains append-only; reclamation is reserved for the future
reviewed segment cleaner.

## Distinguishing CPU proofs

`startup_rejects_cross_page_extent_overlap` publishes two pages, rewrites the
second page's `Begin` record so its target-KV interval aliases the first
page's target-KV interval, and regenerates the journal-record CRC. The
modified record remains internally valid and its durability events remain
complete. Both reader and writer must reject it specifically as
`CatalogOverlap`.

`startup_rejects_catalog_extent_beyond_data_file` publishes a page, truncates
`pages.dat` by one byte, and proves both reader and writer reject startup as
`CatalogOutOfBounds`.

`resumed_publication_preserves_every_byte_before_physical_eof` publishes a
page, appends 8,192 sentinel bytes after the last live extent, reopens the
writer, and publishes a second page. It proves the second allocation begins
at or beyond aligned prior physical EOF and that all sentinel bytes remain
unchanged. The previous live-maximum allocator overwrites those bytes.

Existing restore-time SHA-256 validation remains unchanged and continues to
reject payload corruption. Existing short-tail and complete-corrupt-journal
tests also remain green.

## Gate result and exclusions

The full local gate passed 257 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, the unchanged cache-lifecycle
fixture, and all 52 existing review-handoff provenance proofs.

Commands:

```text
cargo test --offline -p glm-cache store::tests
cargo clippy --offline -p glm-cache --all-targets -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-cache/src/store.rs
a68c672d51f59b79efeb514f8690aa2263730b2df5ed1ece688793ed6f897996

crates/glm-cache/src/tier.rs
0a1541f13462bcdec92284911f96531b06869b60c7fe85fc5e9669c80fabe693

docs/direct-tier-io-v1.md
7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2

docs/durable-store-single-writer-proof-v1.md
cc8e5182bad079c53504780c8ab1f6a7a7f410f094610965e5acd140837f4f47

docs/torn-journal-resume-proof-v1.md
2c0a5f131e72b41cf76f06e1127db7c9d492ba17332d9dec643392d301f85180

fixtures/cache-lifecycle-proof-v1.json
c1151c34a3a9bee4fd97dea11e807603a56c2af4d37deab813cc9b5631177d6a

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, or model execution
was used.

This correction validates only the recovered live catalog of the retained
buffered CPU store. It does not authenticate every payload at startup,
validate obsolete or incomplete transaction extents, implement direct I/O,
catalog epochs, segment cleaning, online asynchronous publication,
redundant metadata, CUDA transfer, model execution, or performance evidence.
