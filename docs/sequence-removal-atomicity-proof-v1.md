# Active sequence removal atomicity CPU proof v1

Date: 2026-07-29

Implementation commit:
`435f514f8d2a74005a0358de09f7ddb7b4c12fc2`

Status: CPU metadata-oracle correction passed; independent review pending

GPU claim: none

## Defect and invariant

`SequencePageTable` already snapshots and restores its CPU metadata for
admission, append, fork, tentative reservation, tentative commit, and
tentative rollback. `remove_sequence` was the exception.

It previously:

1. removed the sequence record;
2. released physical pages in reverse ordinal order; and
3. returned immediately on the first release error.

If a late physical page was missing or corrupt, earlier pages had already
lost references, entered the free sets, or removed prefix mappings. The
sequence record was also gone, so the caller could not repair the invariant
and retry removal.

Removal now runs under the same snapshot-on-error CPU oracle contract as the
other page-table mutations. Any error restores:

- the sequence and its ordered logical-page mapping;
- every physical page and reference count;
- target and draft free sets; and
- shared-prefix key mappings.

A sequence with a tentative transaction is also restored automatically when
removal returns `Transaction`.

## Distinguishing CPU proof

`failed_sequence_removal_restores_every_page_and_is_retryable` creates one
65-token target-only sequence spanning two DCP-owned physical pages. It saves
and removes the ordinal-zero physical record behind the table, so reverse
removal:

- successfully reaches and would free ordinal one first; then
- fails late when ordinal zero is missing.

The corrected code returns `Invariant` and proves:

- the sequence still contains both logical pages;
- the ordinal-one physical page still exists with its original reference;
- the ordinal-one physical ID did not enter its rank free set; and
- the pre-call corrupted state, but no additional partial mutation, is
  preserved.

The old implementation loses the sequence and frees ordinal one before
returning, so it fails these assertions for the claimed ordering.

The test then restores the missing ordinal-zero record and retries removal.
Both physical records disappear, both owner-local IDs return to their exact
free sets, and the sequence is removed.

## Gate result and exclusions

The full local gate passed 246 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, and all 44 then-present review
handoff provenance proofs.

Commands:

```text
cargo fmt --check
cargo test -p glm-cache
cargo clippy -p glm-cache --all-targets -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-cache/src/sequence.rs
c31f74eda75c9dfa93c03ce2d569175b3cda67c5fa8f0a56506c778b596a79c8

docs/serving-page-transaction-v1.md
e3a9a1d9f2eb26dc5312d7c42297fa3d832e444f7e3f269094746a85fb3deac2

docs/backend-event-cancellation-fatal-proof-v1.md
04794fb247b103e90d03a07e9827f13ce82d89e0a50dccb543c5e010f0f9bde5

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, or model execution
was used.

This is deliberately a clone-on-error CPU metadata oracle, as already scoped
by `docs/serving-page-transaction-v1.md`. It is not the fixed-capacity
production undo log, does not quarantine freed IDs until rank
acknowledgement, and is not integrated into `ServingCoordinator`. It does not
implement device page-table uploads, CUDA KV arenas, or performance evidence.
