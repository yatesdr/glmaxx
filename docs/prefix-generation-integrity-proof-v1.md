# Prefix-generation integrity CPU proof v1

Date: 2026-07-29

Implementation and regression commit:
`aecbcdf`

Status: CPU prefix-index correction passed; independent review pending

GPU claim: none

## Defect and invariant

`PrefixPageKey` is a content identity derived from the namespace, parent key,
and exact 64 token IDs. The retained CPU `PrefixIndex::insert` previously
treated a larger durable generation as sufficient authority to replace the
record stored under that key. It did not compare the logical piece hashes and
allowed an MTP-capable record to be replaced by a target-only record.

That behavior violated the reviewed online-publication matrix in two ways:

- different target KV or target indexer bytes could silently acquire the same
  prefix content identity; and
- a later target-only generation could remove draft-sidecar capability from
  a prefix that had already been published as MTP-capable.

The corrected insertion path validates the complete candidate record, derives
every key, rejects duplicate derived keys, and preflights the complete
multi-page insertion before changing any record or reference count.

For an existing key:

- target KV and target indexer `(byte_length, sha256)` identities must always
  match;
- when both records are MTP-capable, the draft-sidecar identity must match;
- target-only to MTP is a permitted monotonic capability upgrade;
- MTP to target-only is a deduplication that retains the existing MTP record;
- a compatible higher MTP generation may refresh physical tier placement;
  and
- any logical identity conflict returns `PrefixError::Collision` with no
  mutation.

Generation, tier, and physical storage offsets are deliberately not logical
content identity. `TierRecord::validate` runs before comparison, so a missing
required piece cannot compare successfully as two absent values.

## Distinguishing CPU proof

`same_key_generations_require_identical_bytes_and_never_downgrade_mtp`:

1. inserts a target-only generation;
2. upgrades it to a byte-compatible MTP generation;
3. inserts a newer target-only generation and proves the stored MTP record and
   generation remain unchanged while its shared reference is counted;
4. changes the target KV digest and proves collision rejection preserves the
   exact prior record and reference count;
5. changes the draft-sidecar digest and proves the same atomic rejection; and
6. inserts a compatible newer MTP generation and proves it is adopted.

The prior implementation fails steps 3 through 5: it downgrades the MTP
record and accepts conflicting logical bytes solely because their generations
are larger.

The existing late-validation regression continues to prove that a multi-page
insert does not expose an earlier page when a later page is invalid. The new
preflight extends that atomicity to logical content collisions and reference
overflow.

## Gate result and exclusions

The full local gate passed 249 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, and all 47 then-present review
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
crates/glm-cache/src/prefix.rs
459953bffe50061901dc10ee2a7593bc1cea5e4cd5eb448a8f349a2c261c6ef3

crates/glm-cache/src/tier.rs
c31b07d7f9054f3d51bc5d24c2c414b6c9a134d88f042502bc0f82e29cad500f

docs/online-prefix-publication-v1.md
67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873

docs/cache-lifecycle-proof-v1.md
11ad4936fea7cd0887e660911f50778d5b0918c21a6cebaca1a98a244b2e2de1

docs/durable-store-single-writer-proof-v1.md
cc8e5182bad079c53504780c8ab1f6a7a7f410f094610965e5acd140837f4f47

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, or model execution
was used.

This correction does not implement the pending durable `insert_child` or
`recover_namespace` operations, asynchronous online publication, a live
shared catalog, direct I/O, registered buffers, DRAM/HBM transfer, or
cross-rank fatal propagation. It proves only retained in-memory prefix-index
collision handling and monotonic MTP capability. End-to-end duplicate writer
serialization, deterministic repeated-prefix GPU bytes, restart recovery,
and production performance remain open.
