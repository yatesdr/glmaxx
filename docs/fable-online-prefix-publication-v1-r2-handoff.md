# Fable handoff: online prefix publication v1 r2

Date: 2026-07-30

Status: corrective adversarial design review requested

Review candidate commit:
`a9b40f1b1440797a05543d5e65e61927fd141b97`

Required result path:
`docs/reviews/fable-online-prefix-publication-v1-r2.md`

Requested acceptance token, only for an unqualified pass:
`online-prefix-publication-v1-accepted`

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch GPU, storage, container, or
filesystem work for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if needed. Hash every input at review start and finish. Any mismatch
with this table is a stale candidate and must withhold the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/online-prefix-publication-v1-r2.md` | `1a45faef321134c61ada9e7dda6ce6087c1ccb156bab6b11f3537c1d350cbfc6` |
| `docs/online-prefix-publication-v1.md` | `67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873` |
| `docs/direct-tier-durable-format-v1.md` | `19ca03edeab89b560d674689ca96ce497f2c5859a91d5fe5d4b50c78645e79e6` |
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `docs/serving-page-transaction-v1.md` | `31983cce95ee01a5968213d5daf12c7a855f75f8735314700f2b4a9e55625d1a` |
| `docs/hbm-dram-transfer-v1.md` | `ed701b2d96ceccb7257ae0ee2bb09988ad9f31309cc01ac28e220648f69f1464` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/prefix.rs` | `ad0bc0e498050d948807c9f1e27e5f98ea02c4fa334725428a8dab1dab068298` |
| `crates/glm-cache/src/sequence.rs` | `8c0491d4f2d3e50da12e15961c8ac65a2fe5449a3527d40a38cdaa5ef27d644e` |
| `crates/glm-cache/src/page.rs` | `d32d70b46f8e09c31923b6fb574db07ef6a8a7dfc7489392b39785dd563217ed` |
| `crates/glm-cache/src/tier.rs` | `0a1541f13462bcdec92284911f96531b06869b60c7fe85fc5e9669c80fabe693` |
| `crates/glm-cache/src/store.rs` | `0a2cd6f96bceb3ed352e5ade9fca302ed5f1498e0280de59a4b57286672dff0c` |
| `crates/glm-cache/src/residency.rs` | `2846361e521f66752cb4455c908b2f30fa2f2a27a59a8059866e43b2402a2d6d` |
| `crates/glm-serving/src/cache.rs` | `099bffde185307365f5932c84f14b15c1ccc4b4cfe29f00612265f69a46a9839` |
| `crates/glm-serving/src/lib.rs` | `bc7eff0297e14b73df7eec5ade3352ad0f75ceabeaca1862c4866a51efb948e3` |
| `docs/production-punchlist.md` | `a9d8c66c235e5616c52e80aacdf34cb377549252694941d9b2d8fa42e262110a` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-online-prefix-publication-v1-r2-handoff.md
git diff --check a9b40f1b1440797a05543d5e65e61927fd141b97^ \
  a9b40f1b1440797a05543d5e65e61927fd141b97
```

The handoff itself is coordination metadata added after the candidate and is
not a candidate input.

## Prior verdict and corrective scope

The first review at candidate `d0a09d7` withheld this token on four MAJOR
findings:

1. pre-lease generation mismatch could mean either benign legal retirement
   or an ABA defect;
2. committed-token cursor behavior on sequence fork was absent;
3. immutable catalog publication had no complexity bound; and
4. the claimed indexer ABI namespace input did not exist in
   `NamespaceInputs`.

The r2 amendment closes those findings and incorporates the prior ten MINOR
findings and four questions. Review the complete correction, not only the
four-row summary.

The correction also finds a connected scalability defect in the pending
durable-format base: its ordinary per-shard logical SHA requires an
estate-dependent scan to compute the planned post-root before every
`PublishBegin`. R2 amends only the catalog-root algorithm to a 504-level
binary sparse-Merkle tree. Every durable field width and offset remains
unchanged, but the catalog schema digest and root domain change. Acceptance
of this handoff accepts that narrow amendment; it does not independently
accept the rest of the pending durable-format candidate.

## Review boundary

An unqualified token permits one subsequent coordinated CPU implementation
candidate only after the base durable-format token also exists.

Acceptance does not accept or implement:

- the namespace-v2 Rust types;
- allocation-generation ledgers, committed-token cursors, leases, publisher,
  persistent tree, catalog, journal, checkpoint, cleaner, or recovery code;
- the pending HBM-to-DRAM transfer design or native ABI;
- io_uring, registered memory, NUMA placement, CUDA, SM120, NVMe-device, or
  checkpoint evidence;
- prefix publication, cold restore, warm reuse, 1M execution, K03, K04, or
  K05 as passing;
- cn4 access; or
- a production serving, quality, capacity, or performance claim.

## Required adversarial questions

1. Do all sixteen candidate hashes match at review start and finish in a
   detached worktree?
2. Does namespace v2 bind the target KV record, mandatory target-indexer
   record, and combined draft-KV/indexer sidecar without ambiguity?
3. Are namespace/page-key domain changes, fail-closed v1 handling, and the
   durable format descriptor change explicit and sufficient?
4. Do cursor append rules handle every tail count, prefill batches, MTP
   batches up to seven, multiple boundaries, zero padding, and overflow?
5. Does fork copy the exact parent/ordinal/tail cursor transactionally, with
   identical and divergent branch behavior correct?
6. Is the allocation ledger complete for all four allocation kinds, and can
   generation zero/reuse/overflow or executor restart alias a live source?
7. Does the atomic lease matrix classify every legally retired or
   quarantined pre-lease ticket as a skip while preserving all live,
   contradictory, and post-lease mismatches as fatal?
8. Can all-or-none lease acquisition ever leave a partial target/indexer/
   draft reference on skip, error, retry, or duplicate observation?
9. Is the mandatory owner equality exact for target KV, target indexer,
   draft KV, and draft indexer?
10. Does owner terminal outcome plus rank-common `CACHE_ONLY` digest plus
    four successor receipts prevent early physical ID reuse?
11. Does graceful shutdown cover every state, including a successful,
    failed, or permanently pending submitted copy, without reporting false
    closure?
12. Is re-offer bounded by identity, epoch/backoff, and one-ticket rules while
    allowing transient pressure to heal?
13. Re-derive the sparse-Merkle geometry: 256 shards, one excluded shard
    byte, 63 remaining key bytes, 504 branch decisions, one leaf, at most 505
    replacement tree objects, and 256 fixed top descriptors.
14. Are empty, leaf, node, and top-root domains unambiguous, including depth
    encoding, bit order, shard count width, epoch, and entry bytes?
15. Can insert, update, visibility change, relocation, and delete calculate
    exact pre/post roots without scanning any untouched catalog entry?
16. Does checkpoint decoding still validate ordinary padded payload hashes,
    sorting, entry digests, reconstructed sparse-Merkle roots, and the
    catalog root without weakening file integrity?
17. Are old readers safe under one atomic snapshot swap and epoch-held
    `Arc` subtrees, and are node allocation failures pre-publication?
18. Does the public lookup API hide parent-pending entries while avoiding a
    second full-estate prefix map?
19. Can chained parent healing proceed from the bounded adjacency index
    without catalog scans, unbounded journal records, or deadlock?
20. Are pending-parent entries, healing continuations, and re-offers bounded
    under missing-parent and branched-prefix cases?
21. Is the residency set restricted to fixed HBM/DRAM/operation capacity and
    its deterministic indexed eviction O(log R)?
22. Do explicit delete/relocation/retirement rules prevent an MTP upgrade or
    catalog deletion from falsely reclaiming physical bytes?
23. Are sync failure, write poison, capacity pressure, post-Commit retry,
    checksum collision, and allocation ABA classified at the correct
    request/publication/engine boundary?
24. Are publisher and SHA worker counts, queues, buffers, and CPU cost fixed
    so publication cannot block rank owners or hide unbounded work?
25. Does retaining the base durable sync barriers avoid weakening crash
    ordering while honestly leaving group commit to a later amendment?
26. Does the 22-row CPU gate cover the four prior MAJORs, all ten prior
    MINORs, concurrency, complexity, restart, corruption, and shutdown?
27. Is the dependency on both design tokens explicit, with no implementation
    or evidence leakage?
28. Does any clause conflict with rank-common routing, the fixed page
    transaction, direct-tier extent geometry, or target/draft atomicity?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer each statement separately:

1. namespace and page-key v2 are complete and fail closed;
2. cursor and fork semantics produce exact rank-common full-page tickets;
3. the allocation ledger and lease matrix distinguish retirement from ABA;
4. lease acquisition/release/shutdown never permit early reuse;
5. sparse-Merkle root math and persistent snapshots are bounded and
   estate-size independent per mutation;
6. visible lookup, pending healing, and residency avoid duplicate
   full-estate maps or scans;
7. capacity, deletion, cleaning, failure, and worker-budget rules are
   fail-closed;
8. the revised CPU gate is implementable and sufficient;
9. the durable-root amendment changes no serialized field width or offset;
   and
10. acceptance opens only the declared CPU proof after both tokens.

Only if all twenty-eight questions and all ten statements are unqualified
`YES`, end with:

```text
online-prefix-publication-v1-accepted
```

Withhold for stale provenance, missing ABI identity, ambiguous retirement,
partial lease acquisition, fork/cursor loss, early allocation reuse,
uncontained shutdown, an estate-dependent mutation, ambiguous Merkle
serialization, hidden pending entries, duplicate catalogs, unbounded
healing/residency/workers, false physical reclamation, weakened durability,
an unimplementable proof, dependency leakage, or evidence overstatement.
