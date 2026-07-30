# Active-prefix record binding CPU proof v1

Date: 2026-07-29

Implementation and regression commit:
`6a5c574bf7a3d4060cb28ef78bc0425bd61f305a`

Status: CPU metadata and restore-boundary correction passed; independent
review pending

GPU claim: none

## Defect and identity invariant

`SequencePageTable::admit_with_prefix` previously accepted each sealed page
as only:

```text
(PrefixPageKey, caller_supplied_has_draft)
```

The key remained bound to logical ordinal and DCP owner, but the active table
did not receive the retained tier record. A caller could claim draft
capability without binding the namespace, generation, target-KV hash,
target-indexer hash, or draft-sidecar hash. In particular, the table could
allocate an MTP draft slot for a stale same-generation “upgrade” or for a
record whose target or draft bytes conflicted with the already-shared page.

The corrected boundary uses `PrefixPageAttachment`, constructed only from a
strictly valid `TierRecord`. It retains:

- namespace and prefix key;
- durable generation;
- target-KV SHA-256;
- target-indexer SHA-256; and
- the optional draft-sidecar SHA-256.

Every shared physical prefix page stores that attachment together with its
logical ordinal. Reuse applies one fail-closed relation matrix:

| Current attachment | Candidate attachment | Result |
|---|---|---|
| target only | identical target only | exact reuse |
| MTP | identical target only | retain existing MTP attachment |
| target only | identical target plus draft at a newer generation | MTP upgrade |
| MTP | identical target and identical draft | exact reuse |
| any target identity mismatch | any | reject |
| target only | draft at the same or older generation | reject |
| MTP | different draft identity | reject |

Ordinal, owner, sealed state, full-page token count, and duplicate-key checks
remain mandatory. The existing clone-on-error CPU oracle restores the exact
table on any late draft-capacity or identity failure.

## Restore and serving boundary

`PrefixRestoreCoordinator` now derives the attachment from the authoritative
post-registration prefix-index record. A pending page retains that exact
attachment while its rank-owned restore runs. `RestoredPrefix` is published
only after every page is resident and pinned; it carries key and attachment
vectors whose shapes and identities must agree.

The `RestoredPrefix` vectors are private outside `glm-serving`; external code
can inspect them but cannot construct a forged ready result. The old public
`admit_prevalidated` bypass is crate-private. Production-facing token
admission must therefore derive and restore its prefix inside the
coordinator.

This does not make a standalone `PrefixPageAttachment` a payload-transfer
receipt. Direct users of the CPU metadata oracle can construct one from a
valid record without proving that CUDA-visible bytes were uploaded. The
future serving-page transaction must bind the rank transfer acknowledgment
and page-table delta before a device executor consumes the attachment.

## Distinguishing CPU proof

`prefix_attachment_binds_generation_and_every_logical_piece_hash`:

1. attaches one target-only generation-four page;
2. rejects a same-generation draft claim atomically;
3. rejects a newer attachment with a changed target identity atomically;
4. accepts the exact target identity plus a generation-five draft sidecar;
5. proves the shared target physical page is reused and one draft slot is
   allocated; and
6. rejects a later changed draft-sidecar identity without changing page
   counts.

The previous `(key, has_draft)` API cannot compile the strict regression
because it has no generation or piece identities to supply. Translating each
candidate to the only old representation available, the same key plus
`has_draft = true`, makes the old table accept the stale upgrade and both
content-conflict cases. The correction is therefore distinguished by the
missing identity boundary as well as the resulting acceptance behavior.

`prefix_registration_uses_the_monotonic_index_record_atomically` now also
proves that a real asynchronous draft-required restore returns an attachment
bit-equal to the retained MTP generation-two tier record after exact dedup,
MTP upgrade, and a later target-only record that must not downgrade it.

The deterministic cache-lifecycle proof now constructs active-page
attachments from the actual records recovered after torn-journal restart,
not from page keys plus hard-coded booleans.

## Gate result and exclusions

The full local gate passed 270 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, and all 64 then-present review
handoff provenance proofs.

Commands:

```text
cargo test --offline -p glm-cache \
  sequence::tests::prefix_attachment_binds_generation_and_every_logical_piece_hash
cargo test --offline -p glm-serving \
  cache::tests::prefix_registration_uses_the_monotonic_index_record_atomically
cargo test --workspace --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-cache/src/sequence.rs
e5902ffe36366916b728c54cd78f62331daf63136190d72cbc81d107e5150c36

crates/glm-cache/src/lib.rs
0d9d1fcdbb9c8350b1702d1c41263c24818861936d3ff37f4f4f73125cb6e269

crates/glm-serving/src/cache.rs
099bffde185307365f5932c84f14b15c1ccc4b4cfe29f00612265f69a46a9839

crates/glm-serving/src/lib.rs
8f4d33b6972bcee3a45f46416c3dfe2b4679a12b539704336c3f61f58fe73cb3

crates/glm-cli/src/cache_proof.rs
f88effadfae758e8afda8ed1ffed9fb2c50530d4476200644b5b6ef905d7f814

docs/serving-page-transaction-v1.md
e3a9a1d9f2eb26dc5312d7c42297fa3d832e444f7e3f269094746a85fb3deac2

docs/sequence-removal-atomicity-proof-v1.md
0baa3ff73b3fad73dd3471ee89fca9ab3278d5223fdae85c40f0e9066f11bc2b

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

This correction proves logical prefix identity and capability propagation
through the retained CPU restore and active-table APIs. It does not integrate
`SequencePageTable` into `ServingCoordinator`, reserve private step tails,
replace clone-on-error with a fixed undo log, upload a rank page-table delta,
quarantine physical IDs, prove CUDA-visible payload transfer, or establish
model quality, capacity under live concurrency, or performance.
