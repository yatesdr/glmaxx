# Fable handoff: active-prefix record binding v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`92568f6045bf70a1d607435de318cebd6b4ef249`

Required result path:
`fable-active-prefix-record-binding-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`active-prefix-record-binding-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/sequence.rs` | `e5902ffe36366916b728c54cd78f62331daf63136190d72cbc81d107e5150c36` |
| `crates/glm-cache/src/lib.rs` | `0d9d1fcdbb9c8350b1702d1c41263c24818861936d3ff37f4f4f73125cb6e269` |
| `crates/glm-cache/src/tier.rs` | `0a1541f13462bcdec92284911f96531b06869b60c7fe85fc5e9669c80fabe693` |
| `crates/glm-cache/src/prefix.rs` | `ad0bc0e498050d948807c9f1e27e5f98ea02c4fa334725428a8dab1dab068298` |
| `crates/glm-cache/src/residency.rs` | `2846361e521f66752cb4455c908b2f30fa2f2a27a59a8059866e43b2402a2d6d` |
| `crates/glm-serving/src/cache.rs` | `099bffde185307365f5932c84f14b15c1ccc4b4cfe29f00612265f69a46a9839` |
| `crates/glm-serving/src/lib.rs` | `8f4d33b6972bcee3a45f46416c3dfe2b4679a12b539704336c3f61f58fe73cb3` |
| `crates/glm-cli/src/cache_proof.rs` | `f88effadfae758e8afda8ed1ffed9fb2c50530d4476200644b5b6ef905d7f814` |
| `docs/active-prefix-record-binding-proof-v1.md` | `9bb87c359d78c340d740ef9723ac78ef23510af5fabf4b29b1630211499b4c12` |
| `docs/serving-page-transaction-v1.md` | `05466da477fd9de88e9d8849cca67952b1f8999563743aea0599e741dc8e4c26` |
| `docs/offline-serving-spine.md` | `24230e2503b386391bd01274ae6586808c751202c619aa47aa81f9d8c277e8c7` |
| `docs/sequence-removal-atomicity-proof-v1.md` | `0baa3ff73b3fad73dd3471ee89fca9ab3278d5223fdae85c40f0e9066f11bc2b` |
| `docs/prefix-residency-coherence-proof-v1.md` | `3f99eeb1f4f003f211922a906939ce9d6bbe03fb9b43ed13091fd38349bd194c` |
| `docs/production-punchlist.md` | `2b38129a5b5179dfc1917975f691618b77c0720e16719b1289d80d476f525487` |
| `docs/results-index.md` | `9e66ebe429005252893761bc10f82d323ef55c6c29ea08b7a384d14a8ab46bf1` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-active-prefix-record-binding-v1-handoff.md
cargo test --offline -p glm-cache \
  sequence::tests::prefix_attachment_binds_generation_and_every_logical_piece_hash
cargo test --offline -p glm-serving \
  cache::tests::prefix_registration_uses_the_monotonic_index_record_atomically
cargo test --offline -p glm-cli \
  cache_proof::tests::cache_lifecycle_is_bounded_recoverable_and_fail_closed
cargo clippy --workspace --all-targets --offline -- -D warnings
```

## Review boundary

This review covers logical prefix-record identity at the retained CPU restore
and active-page-table APIs. It does not accept a CUDA-visible payload-transfer
receipt, active-table integration into `ServingCoordinator`, fixed-capacity
undo logs, rank page-table deltas, physical-ID quarantine, CUDA, checkpoint
execution, model output, live-concurrency capacity, or performance.

## Required adversarial questions

1. Did the previous active-table API receive only
   `(PrefixPageKey, has_draft)`, with no namespace, generation, target hash,
   indexer hash, or draft hash?
2. Could the old API therefore allocate an MTP draft slot for any same-key
   `has_draft = true` claim, including a stale generation or logically
   conflicting record?
3. Are all `PrefixPageAttachment` fields private and is its public constructor
   derived only after `TierRecord::validate` succeeds?
4. Does that constructor retain namespace, exact key, generation, both target
   logical piece hashes, and the optional draft-sidecar hash?
5. Does active-table reuse still require the exact ordinal, deterministic
   owner, sealed state, full valid-token count, and unique key?
6. Does the relation reject any namespace, key, target-KV, or target-indexer
   mismatch?
7. Does target-only to MTP require an unchanged target identity and a strictly
   newer generation?
8. Does an existing MTP attachment retain its draft identity when a compatible
   target-only record is presented, rather than downgrading?
9. Do two MTP attachments reuse only when the draft-sidecar hashes agree?
10. Does a valid upgrade update the stored attachment before use and allocate
    a draft slot only for an MTP sequence that lacks one?
11. If a later identity or draft-capacity check fails, does snapshot rollback
    restore attachment, references, physical records, and target/draft free
    sets exactly?
12. Does `PrefixRestoreCoordinator` construct each attachment from the
    authoritative candidate-index record rather than the caller's discarded
    or downgraded input record?
13. Is a pending attachment returned only after the rank residency operation
    has completed exact-record validation and the page has been pinned?
14. For an already-resident page, do registration coherence and pinning still
    bind the returned attachment to the same authoritative record?
15. Do private `RestoredPrefix` vectors prevent external construction of a
    forged ready result while retaining read-only inspection?
16. Is `admit_prevalidated` now crate-private so production-facing external
    callers cannot claim cached prompt progress without a restore lease?
17. Does the active-table regression reject the stale upgrade, changed target,
    and changed draft atomically, while accepting the one valid newer MTP
    upgrade on the same physical target page?
18. Is it accurate that the prior source cannot compile that strict regression
    because its ABI lacks the identities, and that translating the cases to
    its only representation `(same_key, true)` accepts them?
19. Does the real restore regression compare the returned attachment exactly
    with the retained MTP generation-two record after dedup, upgrade, and
    attempted downgrade?
20. Does the cache-lifecycle proof now use the actual torn-journal-recovered
    records instead of hard-coded capability booleans?
21. Is the proof explicit that a standalone attachment validates logical
    metadata but does not prove payload bytes were uploaded to a CUDA-visible
    slot?
22. Are the 270-test claim, 64-handoff full-gate claim, and every
    serving-integration/GPU/model/performance exclusion accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- the old capability-forging boundary is real;
- the attachment binds every required logical record identity;
- exact reuse, retain-MTP, newer MTP upgrade, and collision rejection are
  correct and atomic;
- restore and active-table metadata consume the same authoritative record;
- external serving code cannot forge a restored-prefix result or use the
  prevalidated bypass;
- the regression distinguishes the missing old ABI and resulting old
  acceptance behavior;
- payload-transfer and serving-integration scope remain accurately excluded;
  and
- the CPU proof and all gate counts are accurate.

Only if all eight answers are unqualified `YES`, end with the requested
token. Withhold it for a conditional pass, stale input, forgeable capability,
identity omission, stale upgrade, downgrade, content collision acceptance,
partial mutation, index/residency/attachment disagreement, public restored
result construction, public cache-progress bypass, a nondistinguishing
regression, or an overstated device/serving claim.

The token accepts only this retained CPU prefix-record binding correction. It
does not open cn4, authorize CUDA work, or accept checkpoint serving.
