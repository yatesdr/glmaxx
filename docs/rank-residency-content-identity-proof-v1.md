# Rank residency content identity CPU proof v1

Date: 2026-07-29

Implementation commit:
`e04a474937be8f4ed660618eb852549eeed6e7b6`

Status: CPU rank-residency correction passed; independent review pending

GPU claim: none

## Defect and corrected boundary

The prefix coordinator already derives an authoritative post-insert record
with `TierRecord::relation_to`, but the public `ResidencyManager`
registration boundary still accepted any same-key record with a larger
generation. A direct caller could therefore:

- replace a page with different target, indexer, or draft content under the
  same logical key;
- replace an MTP-capable page with a newer target-only record; or
- demote and unaccount a pinned HBM/DRAM page for a byte-identical
  higher-generation record.

Correctness depended on every caller reproducing coordinator filtering.

Residency registration now applies the shared logical-content matrix itself:

| Existing | Candidate | Residency action |
|---|---|---|
| target-only | identical target-only | retain exact existing record |
| target-only | identical target plus draft | replace only at a strictly larger generation |
| MTP | identical target-only | retain MTP record |
| MTP | identical target plus identical draft | retain exact MTP record |
| either | different target/indexer/draft identity | reject as `Record` |

Retain actions do not enter the commit plan and therefore cannot change
record revision, payload, residency, pins, clocks, HBM bytes, or DRAM bytes.
Only an MTP upgrade may replace an existing record, and it additionally
requires the page to be unpinned and not restoring. A non-newer upgrade is
`Stale`.

Multi-record planning classifies every distinct page key and computes all
accounting before commit. A later collision, stale upgrade, pin, restore
state, or overflow leaves every rank-local entry unchanged.

## Distinguishing CPU proofs

`registration_deduplicates_and_only_mtp_upgrade_releases_resident_accounting`
restores and pins a target-only page in HBM, submits an identical
generation-9 candidate, and proves the exact generation-1 record, pin, HBM
residency, and byte accounting are retained. A real durable generation-2
MTP upgrade is rejected while pinned, then succeeds after unpin and releases
the old HBM accounting. A later generation-99 target-only candidate cannot
downgrade the MTP record.

`direct_registration_rejects_same_key_content_collision_and_stale_upgrade`
calls `ResidencyManager` directly, without the prefix coordinator. It proves
a different target digest is rejected with no record mutation and a
same-generation target-to-MTP upgrade is rejected as stale.

The existing coordinator regressions still prove:

- all four rank plans complete before any plan commits;
- a late pinned-rank MTP upgrade leaves every earlier rank unchanged;
- the retry upgrades all required ranks atomically; and
- real draft-required restore returns the exact MTP durable record.

The multi-page restore regression now checks its retained pin through the
explicit unpin preflight instead of treating rejection of an exact
registration as a pin probe. Exact registration is intentionally a no-op.

## Gate result and exclusions

The full local gate passed 258 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, the unchanged cache-lifecycle
fixture, and all 53 existing review-handoff provenance proofs.

Commands:

```text
cargo test --offline -p glm-cache residency::tests
cargo test --offline -p glm-serving cache::tests
cargo clippy --offline -p glm-cache -p glm-serving --all-targets -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-cache/src/residency.rs
ea8337b22a043436147bb461a618f13fb993de1cd6750dcfefb205d64fdea5fd

crates/glm-cache/src/tier.rs
0a1541f13462bcdec92284911f96531b06869b60c7fe85fc5e9669c80fabe693

crates/glm-serving/src/cache.rs
3ce2f435d2538c736c1b10b3fd6f27c1fb08a8221c92eb62e1db7d49a832283c

docs/durable-content-dedup-proof-v1.md
75fd16886ef50e4509fb0a7b0701417a1469a0dad78809b39ce08e3e736a7514

docs/prefix-residency-coherence-proof-v1.md
3f99eeb1f4f003f211922a906939ce9d6bbe03fb9b43ed13091fd38349bd194c

docs/online-prefix-publication-v1.md
67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873

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

This correction covers synchronous CPU residency metadata only. It does not
move real HBM/DRAM bytes, implement online durable publication or
parent/ordinal restart metadata, share a live catalog, provide direct I/O,
propagate a collision through a four-rank fatal drain, execute CUDA, or
establish model correctness or performance.
