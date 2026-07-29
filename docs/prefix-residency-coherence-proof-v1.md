# Prefix/residency generation coherence CPU proof v1

Date: 2026-07-29

Implementation and regression commit:
`a3f5957b6e8d526cedb2ab58fa2204bb34d9f8b7`

Status: CPU coordinator correction passed; independent review pending

GPU claim: none

## Defect and cross-component invariant

The earlier prefix-generation correction made `PrefixIndex` retain an
existing MTP-capable record when a byte-compatible, newer target-only record
was inserted. `PrefixRestoreCoordinator::register_prefix` nevertheless sent
the caller's input records directly to each `ResidencyManager`.

The two structures could therefore disagree under the same page key:

- the prefix index advertised MTP capability and retained the three-piece
  record; while
- residency stored the newer target-only record without a draft sidecar.

A draft-required lookup could then be admitted from the index but fail
restore identity validation, or falsely report draft capability for a
target-only residency record. Exact deduplication was also unusable because
residency rejected an identical generation as stale even though the prefix
index had correctly counted the shared reference.

The corrected coordinator treats the post-insert `candidate_index` record as
the sole registration authority:

1. it builds the complete candidate index without mutating live state;
2. for every derived key, it proves the live index record and rank-owned
   residency record are identical before continuing;
3. it skips residency writes when insertion retained the prior record,
   including exact/lower-generation dedup and MTP-preserving target-only
   candidates;
4. it groups only actual record changes into four rank-owned plans;
5. all four plans validate record shape, generation, pin/restore state,
   duplicate keys, and resulting HBM/DRAM accounting before any plan commits;
   and
6. it commits the four infallible metadata plans, then adopts the candidate
   index.

Registration planning copies only the changed `TierRecord` metadata. It does
not clone restored page payloads or the resident map, which would be
unbounded at the 1M-context target.

The retained coordinator cannot reconstruct rank residency from a populated
index because that index does not expose a durable recovery snapshot with
verified page ordinals. Construction therefore rejects a nonempty initial
index instead of creating an immediately inconsistent coordinator.

## Distinguishing CPU proof

`prefix_registration_uses_the_monotonic_index_record_atomically`:

1. publishes byte-compatible target-only generation 1 and MTP generation 2
   for one real durable page;
2. registers generation 1 twice and proves exact dedup succeeds;
3. registers the MTP upgrade;
4. registers target-only generation 3 and proves both index and residency
   retain the exact MTP generation-2 record while the shared reference is
   counted;
5. rejects a conflicting target digest and proves exact index, residency, and
   reference preservation;
6. performs a draft-required asynchronous restore from the real file store
   and proves the returned page is MTP-capable; and
7. proves a prepopulated-index constructor fails closed.

The previous coordinator fails step 2 with `ResidencyError::Stale`. If exact
dedup is omitted from the regression, it stores the target-only input at step
4 while the index retains MTP, and the real generation-2 restore then fails
exact-record validation. The prior prefix-only test cannot distinguish that
cross-component divergence.

The existing multi-page registration regression continues to exercise a late
rank pin failure. Because all per-rank plans are produced before any commit,
that failure leaves earlier ranks and the candidate index unchanged.

## Gate result and exclusions

The full local gate passed 250 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, and all 48 then-present review
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
d7727125c2b022b2cd1da7e51b07b1e06365da3ed530b2735478b3ac40f67b06

crates/glm-cache/src/prefix.rs
7b4aff1407f83b2e12216d7a051049c1a5359f0bae7fb88724e8999077260f70

crates/glm-cache/src/residency.rs
04ffe885557b81ca91797b84f31bf6ae3f6f35bc4b7a5dae6bdc9ab08983e664

crates/glm-serving/src/cache.rs
709ab616feca96818f6fc6ce1331becd93de9f67324d2b278503f6f2ad3efe1f

crates/glm-cache/src/tier.rs
c31b07d7f9054f3d51bc5d24c2c414b6c9a134d88f042502bc0f82e29cad500f

docs/online-prefix-publication-v1.md
67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873

docs/prefix-generation-integrity-proof-v1.md
4db63b0ddde70e2afe6371fd4b609bd57ad4965bb48cd45c6dfc5d06587473a0

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, or model execution
was used.

This correction does not implement durable `insert_child` or
`recover_namespace`, online publication, a live shared catalog, direct I/O,
registered buffers, real DRAM/HBM movement, cross-rank fatal propagation, or
production cache performance. The registration plans are synchronous
in-process CPU metadata transactions. Restart reconstruction remains
intentionally unavailable until the reviewed durable parent/ordinal catalog
exists.
