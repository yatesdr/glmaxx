# Durable content deduplication CPU proof v1

Date: 2026-07-29

Implementation and regression commit:
`85d950ee45294f2551d674736b35781986dda874`

Status: CPU durable-content correction passed; independent review pending

GPU claim: none

## Defects and one shared relation

The frozen online-publication contract requires the single writer to compare
logical piece hashes before appending. The retained file store instead
accepted any larger generation for the same page key. Conflicting target KV,
target indexer, or draft-sidecar bytes could therefore be made durable before
the later prefix-registration layer rejected them.

The retained layers also disagreed about exact deduplication:

- the file store appended a same-content larger generation;
- the prefix index replaced its record with a same-content larger generation;
- residency rejected an identical generation as stale; and
- journal replay simply selected the largest generation.

That behavior contradicted the contract's durable-revision rule: exact
deduplication performs no write and retains the existing revision. Only a
target-only to MTP-capable transition creates a new durable revision.

The correction defines one validated `TierRecord::relation_to` operation and
uses it in the prefix index, file writer, and journal replay:

| Existing | Candidate | Result |
|---|---|---|
| none | valid target-only or MTP | append |
| target-only | same target/indexer, target-only | exact dedup; retain |
| target-only | same target/indexer plus draft | MTP upgrade |
| target-only | different target/indexer | content collision |
| MTP | same target/indexer, target-only | dedup; retain MTP |
| MTP | all same logical pieces, MTP | exact dedup; retain |
| MTP | different target/indexer or draft | content collision |

Logical identity is `(byte_length, sha256)` for each required piece plus exact
namespace and page key. Tier and storage offsets are physical placement, not
content identity. Both records must pass complete `TierRecord::validate`
before classification.

An MTP upgrade must have a strictly larger durable revision. Exact dedup and
MTP-retaining downgrade candidates do not advance the revision even when the
candidate carries a larger number.

This rule supersedes the earlier prefix-generation candidate's claim that a
same-content larger MTP record may refresh physical placement. That behavior
was inconsistent with `docs/online-prefix-publication-v1.md`; the historical
proof remains pinned to its exact candidate but is not the current contract
implementation.

## Writer and recovery behavior

`FileTierStore::publish` now hashes and validates all candidate pieces before
classification. Exact dedup or an MTP-retaining target-only candidate returns
the existing record without changing:

- transaction allocation;
- journal length;
- data length;
- next data offset; or
- the published catalog.

A logical mismatch returns `StoreError::ContentCollision` before the begin
journal record or any data write. This deterministic preflight failure does
not poison the writer; a later unrelated valid publication still succeeds.
An actual error after the mutation boundary retains the existing fail-stop
write-poison behavior.

`TierJournal::recover` applies the same relation in transaction order. It
retains exact duplicates and MTP capability, adopts only a strictly newer MTP
upgrade, and fails closed on any logical collision instead of selecting the
largest generation.

The prefix index now records only MTP upgrades as replacements. The
coordinator consequently skips residency updates for all exact dedup cases,
including while a page is pinned. A real two-page/two-rank upgrade regression
also proves that a later pinned rank rejects all four plans before an earlier
rank or the candidate index mutates.

## Distinguishing CPU proofs

`durable_content_dedup_upgrade_and_collision_are_preflighted` proves:

1. target-only generation 1 becomes durable;
2. same-content target-only generation 9 returns generation 1 with byte-exact
   journal and data lengths;
3. a non-newer MTP upgrade is rejected without mutation;
4. MTP generation 2 appends one complete three-piece revision;
5. newer target-only and same-MTP candidates both retain generation 2 without
   writes;
6. conflicting target and draft bytes are rejected without writes or catalog
   mutation;
7. the writer remains usable for a different page; and
8. close/reopen replay retains the MTP revision.

`recovery_applies_dedup_upgrade_and_collision_matrix` independently builds a
fully durable journal containing an exact duplicate, MTP upgrade, and later
target-only record. Recovery selects only the MTP upgrade, then rejects a
fully durable conflicting draft record.

`same_key_generations_require_identical_bytes_and_never_downgrade_mtp` now
proves a same-revision MTP upgrade is rejected atomically and a same-content
larger MTP candidate retains the prior durable revision.

`multi_rank_mtp_upgrade_is_atomic_on_a_late_pinned_rank` proves two owner
ranks remain on their exact target-only records when rank 1 is pinned, then
the retry upgrades both ranks and completes a real draft-required asynchronous
file restore.

The prior file store fails the no-write, retain-revision, and both collision
assertions. The prior journal replay accepts the conflicting revision. The
prior prefix index adopts the exact larger MTP generation.

## Gate result and exclusions

The full local gate passed 253 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, and all 49 then-present review
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
crates/glm-cache/src/lib.rs
0c287e5a542c242e18a3d20c25d8ef8e61bba69ce04c854e714915d915aadab0

crates/glm-cache/src/tier.rs
0a1541f13462bcdec92284911f96531b06869b60c7fe85fc5e9669c80fabe693

crates/glm-cache/src/store.rs
fd16e7e795ce742aff0b72125988b019b3f36cbfebd1f67dab2dd9ea8d72c5ad

crates/glm-cache/src/prefix.rs
ad0bc0e498050d948807c9f1e27e5f98ea02c4fa334725428a8dab1dab068298

crates/glm-cache/src/residency.rs
04ffe885557b81ca91797b84f31bf6ae3f6f35bc4b7a5dae6bdc9ab08983e664

crates/glm-serving/src/cache.rs
3026b4d3353839c0a644944e8a6103f2b168e741d25d272ea2d7d330e1610635

docs/online-prefix-publication-v1.md
67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873

docs/prefix-generation-integrity-proof-v1.md
4db63b0ddde70e2afe6371fd4b609bd57ad4965bb48cd45c6dfc5d06587473a0

docs/prefix-residency-coherence-proof-v1.md
3f99eeb1f4f003f211922a906939ce9d6bbe03fb9b43ed13091fd38349bd194c

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, or model execution
was used.

This correction remains the synchronous retained CPU store. It does not
implement durable parent/ordinal metadata, online `insert_child`, a live
shared catalog, direct I/O, registered buffers, segment cleaning, real
DRAM/HBM movement, cross-rank fatal propagation, or performance evidence.
Content collision is returned as a typed error; the pending production
coordinator must classify it as engine-fatal and drain all ranks identically.
