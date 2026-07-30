# Durable journal transaction sequence CPU proof v1

Date: 2026-07-29

Implementation and regression commit:
`a4bbfb0cd10b9e3edaa79abdeea9edc65b1d21c8`

Status: CPU journal-continuity correction passed; independent review pending

GPU claim: none

## Defect and retained sequence invariant

Every retained journal record has an independent CRC and strict schema, but
startup previously did not enforce the transaction sequence produced by its
single writer. Removing a complete, internally valid transaction group
therefore left only CRC-valid records. Replay accepted them and silently
removed the deleted published page from the recovered catalog.

The retained writer has one exact append order:

- the first transaction ID is one;
- every later `Begin` increments the preceding transaction ID by exactly one;
- all events for one transaction are contiguous; and
- after the stream advances to a later transaction, it never emits an event
  for an earlier transaction.

A crash may leave the current group incomplete. Reopen retains that orphan
and starts the next contiguous transaction, so the same monotonic rule still
holds across crashes.

The corrected file decoder tracks the current transaction while decoding
complete records. A changed ID is accepted only when:

1. it is exactly `current + 1` under checked arithmetic; and
2. the changing record is a `Begin`.

A skipped, decreasing, or first-non-`Begin` transaction returns
`StoreError::JournalSequence`. Existing `TierJournal` replay then retains
responsibility for duplicate begins, missing/duplicate pieces, premature or
duplicate publication, content identity, and generation rules.

Both `FileTierStore::open` and `FileTierReader::open` use this decoder before
catalog recovery or exposure. The next writer transaction is derived from
the final validated contiguous ID rather than the maximum ID in an
arbitrary event set.

## Distinguishing CPU proof

`missing_complete_transaction_group_fails_closed` covers two byte-level
deletions without modifying any retained record or CRC:

1. it publishes transactions one and two, removes the complete four-record
   transaction-one prefix, and proves both writer and reader startup return
   exactly `JournalSequence`; and
2. it publishes transactions one through three, removes the complete
   transaction-two group, and proves both constructors return the same
   error for the resulting one-to-three gap.

The prior decoder takes only the maximum transaction ID, so it accepts both
files. Prefix deletion exposes only page two; interior deletion exposes
pages one and three. All four assertions therefore distinguish the
correction from the previous silent catalog loss.

The complete store suite also proves the legal adjacent case. A transaction
may remain a begin-only, data-synced, or piece-attested crash orphan; after
reopen, the next contiguous transaction can publish successfully and later
replay still exposes only fully durable pages.

## Gate result and exclusions

The full local gate passed 268 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, and all 62 then-present review
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
0a2cd6f96bceb3ed352e5ade9fca302ed5f1498e0280de59a4b57286672dff0c

docs/durable-journal-data-presence-proof-v1.md
fc19414d706e317dd59491b2c284b9931c911161fc176e220fe121211c480b26

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

This correction detects complete transaction-group loss when a remaining
prefix or suffix exposes a sequence discontinuity. It is not an authenticated
hash chain, redundant commit ledger, general salvage mechanism, or proof
against malicious record replacement with recomputed CRCs. Deletion of the
final complete group remains indistinguishable from a legitimate crash before
that group became durable without an independently durable high-water mark.
The correction does not add direct I/O, `io_uring`, online publication,
segment cleaning, GPU transfers, or performance evidence.
