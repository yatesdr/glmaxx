# Review: prefix-generation integrity v1

Date: 2026-07-31

Reviewer: Fable (adversarial design-gate review, CPU only)

Verdict: ACCEPTED

Candidate commit:

2e3aa222e0808c27793798dab6890dbdb7614ed3

Handoff reviewed: `docs/fable-prefix-generation-integrity-v1-handoff.md`
(untracked worktree copy, SHA-256
`6111c4dd8839c163fb627f119b2d7211d83f3cf64ea9da4a46a0c8da40b73e21` per
`review-proof`).

Result location note: the operator directed review artifacts into
`docs/reviews/` instead of the repository root named by the handoff; this
file at `docs/reviews/fable-prefix-generation-integrity-v1.md` is the
required result.

## Provenance

Reviewed in a detached worktree at the candidate commit;
`git rev-parse HEAD` returned the candidate exactly. Every pinned input was
hashed with `shasum -a 256` at review start and again at review finish; all
sixteen observations (eight files, twice) matched the handoff pins with no
drift. No provenance anomalies were observed on any pinned input.

Review-environment note: a concurrent agent sharing the session scratchpad
overwrote this review's `local-checks.log` capture file mid-run (two
distinct mktemp proof directories appear in one log), so that log file was
discarded as evidence; the gate result below rests on the script process's
own exit code (0) and on workspace test output captured synchronously by
this reviewer. The pinned worktree and inputs were unaffected.

| Input | Pinned and observed SHA-256 (start and finish) |
|---|---|
| `spec/engine-v0.md` | efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a |
| `crates/glm-cache/src/prefix.rs` | 459953bffe50061901dc10ee2a7593bc1cea5e4cd5eb448a8f349a2c261c6ef3 |
| `crates/glm-cache/src/tier.rs` | c31b07d7f9054f3d51bc5d24c2c414b6c9a134d88f042502bc0f82e29cad500f |
| `docs/online-prefix-publication-v1.md` | 67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873 |
| `docs/cache-lifecycle-proof-v1.md` | 11ad4936fea7cd0887e660911f50778d5b0918c21a6cebaca1a98a244b2e2de1 |
| `docs/durable-store-single-writer-proof-v1.md` | cc8e5182bad079c53504780c8ab1f6a7a7f410f094610965e5acd140837f4f47 |
| `docs/prefix-generation-integrity-proof-v1.md` | 4db63b0ddde70e2afe6371fd4b609bd57ad4965bb48cd45c6dfc5d06587473a0 |
| `scripts/local-checks.sh` | 839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f |

## Gate results (run once each from the worktree)

- `glmaxx review-proof docs/fable-prefix-generation-integrity-v1-handoff.md`:
  PASS, all eight pinned inputs matched, repository head equals the
  candidate, no bare acceptance lines in the handoff.
- `cargo test --offline -p glm-cache`: 52 passed, 0 failed.
- `cargo clippy --offline -p glm-cache --all-targets -- -D warnings`: clean.

Additional verification beyond the required commands:

- `cargo test --workspace --offline`: 249 passed, 0 failed (52 + 7 + 11 +
  38 + 60 + 3 + 21 + 15 + 32 + 10).
- `scripts/local-checks.sh`: full local proof gate completed with exit 0
  (fmt, workspace tests, workspace clippy `-D warnings`, cuda-ffi type
  checks, all deterministic CPU proof fixtures, `review-proof-all`).
  Tokenizer proof skipped (`GLMAXX_TOKENIZER_DIR` unset) and CUDA compile
  skipped (no nvcc), both as documented.
- `glmaxx review-proof-all`: verified 48 review handoffs (49 tracked plus
  the present untracked handoff, minus 2 skipped historical), PASS.
- `git show aecbcdf^:crates/glm-cache/src/prefix.rs` inspected to confirm
  the prior defective insertion path.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. `TierJournal::recover` (`crates/glm-cache/src/tier.rs` lines 328-337)
   still resolves duplicate published records for one `page_key` by keeping
   the highest generation with no logical byte comparison. This is the same
   replace-by-generation pattern this correction removes from the retained
   index, surviving at the durable replay layer. It is outside this review's
   boundary (the handoff explicitly excludes the pending durable
   `recover_namespace` work and this token does not accept it), but it must
   be closed with the same logical-identity collision check before durable
   recovery is submitted for acceptance.
2. A byte-compatible MTP-capable candidate whose generation is equal to or
   lower than an existing target-only record does not upgrade capability
   (`crates/glm-cache/src/prefix.rs` lines 151-153 require strictly greater
   generation). This is conservative — a missed upgrade, never a downgrade —
   but it silently discards available draft capability; worth an explicit
   test or comment stating the intent.
3. The commit phase increments references with unchecked `+= 1`
   (`prefix.rs` line 156), relying on the preflight `checked_add` at lines
   139-142 plus the facts that pending keys are deduplicated and no removal
   API exists. Correct today, but fragile if a future decrement/eviction
   path interleaves between preflight and commit; a debug assertion or a
   second `checked_add` at the mutation site would make the invariant local.

### QUESTION

1. The reviewed matrix row "MTP existing + all-same-pieces MTP candidate:
   exact dedup; no write" (`docs/online-prefix-publication-v1.md` lines
   358-369) differs from the in-memory behavior, where a byte-compatible
   higher-generation MTP candidate replaces the stored record (test step 6,
   `prefix.rs` lines 456-459). The proof document frames this as a permitted
   physical tier-placement refresh; since logical bytes are proven identical
   it cannot alias, but confirm the durable layer will treat the refresh as
   index-only and not as a new durable write.

## Answers to the required adversarial questions

1. **Yes.** The prior insertion path (parent of `aecbcdf`,
   `crates/glm-cache/src/prefix.rs`, commit loop) replaced the stored
   record on `record.generation > existing.record.generation` alone, with
   no comparison of target KV, target indexer, or draft-sidecar bytes and
   no duplicate pending-key rejection. Confirmed by direct inspection of
   `git show aecbcdf^:crates/glm-cache/src/prefix.rs`.
2. **Yes.** The prior replacement predicate had no MTP guard, so a larger
   target-only generation replaced an MTP-capable record; the draft prefix
   then vanished from `longest_match_with_capability(_, true)`
   (`prefix.rs` line 192 breaks on `!page.record.mtp`).
3. **Yes.** The corrected preflight loop (`prefix.rs` lines 118-146) runs,
   for every page before any mutation: `TierRecord::validate` (line 122),
   namespace check (123), key derivation (126), `page_key` match (127-129),
   duplicate derived-key rejection via `pending_keys.insert` (130-132),
   parent-chain and logical-compatibility check against any existing page
   (133-138), and reference-count overflow via `checked_add` (139-142). The
   commit loop (148-169) runs only after the whole batch passes. Atomicity
   on late failure is regression-tested at lines 400-414 and in the
   collision assertions at 438-454.
4. **Yes.** `records_are_logically_compatible` (`prefix.rs` lines 231-239)
   compares `logical_piece_identity`, which is the pair
   `(byte_length, sha256)` (lines 241-247), for `TargetKv` and
   `TargetIndexer` unconditionally. Generation, tier, and
   `storage_offset` are deliberately excluded from the identity.
5. **Yes.** When `first.mtp && second.mtp`, the `DraftSidecar`
   `(byte_length, sha256)` identity must also match (lines 235-238); a
   mismatch fails the preflight compatibility check and returns
   `PrefixError::Collision` (lines 135-138), regression-tested at lines
   447-454.
6. **Yes.** `TierRecord::validate` (`tier.rs` lines 64-108) requires
   `pieces.len() == required.len()` (line 77) and each piece to be in the
   required set and unique via `seen.insert` (lines 87-88), so `TargetKv`
   and `TargetIndexer` exist exactly once, and `DraftSidecar` exists
   exactly once iff `mtp`. `validate` runs at `prefix.rs` line 122 before
   the identity helper, so `logical_piece_identity` can never compare two
   absent pieces (`None == None`) for a required piece.
7. **Yes.** With a byte-compatible MTP candidate over a target-only
   existing record, `(!existing.record.mtp || record.mtp)` holds and a
   strictly greater generation adopts the MTP record (`prefix.rs` lines
   151-154); tested at lines 426-430 (record becomes `mtp`, generation 2).
8. **Yes.** With an existing MTP record and a byte-compatible newer
   target-only candidate, the replacement guard is false (candidate not
   `mtp`), so the MTP record and its generation are retained while
   `references` increments (line 156); tested at lines 432-436
   (references 3, `mtp` true, generation still 2).
9. **Yes.** Replacement requires all three of: preflighted byte
   compatibility (target, indexer, and draft when both MTP), strictly
   greater generation, and `record.mtp` when the existing record is MTP
   (lines 133-138 and 151-154). Only a byte-compatible, MTP-capable higher
   generation can replace an existing MTP record; tested at lines 456-459.
10. **Yes.** After both the target-hash and draft-hash collision attempts,
    the regression asserts `references(key) == Some(3)` and
    `record(key) == &upgrade` — the exact prior record object and the exact
    prior reference count (lines 443-445 and 452-454).
11. **Yes.** The old implementation replaces on generation alone, so it
    fails step 3 (assertion `mtp` at line 435 and generation at 436) and
    accepts both conflicting inserts, failing the `Err(Collision)`
    assertions at lines 440-443 and 449-451 — the test is distinguishing,
    not tautological. The corrected implementation passed all 249 workspace
    tests and the complete `scripts/local-checks.sh` proof gate in this
    review environment (exit 0).
12. **Yes.** The retained-index-only boundary is accurate: `prefix.rs`
    contains no durable, publication, or I/O operations (grep for
    `insert_child`/`recover_namespace`/`durable` is empty). The
    47-then-present handoff count is accurate: 49 tracked `*-handoff.md`
    files existed at the proof commit `aecbcdf`, of which 2 are skipped as
    historical, giving the claimed 47 verified proofs then; with the
    present handoff, `review-proof-all` now verifies 48. The proof
    document's exclusions (no CUDA compiler, GPU, collective, checkpoint,
    or model execution; tokenizer proof skipped with unchanged fixture; no
    durable `insert_child`/`recover_namespace`, online publication, shared
    catalog, direct I/O, registered buffers, DRAM/HBM transfer, cross-rank
    fatal propagation, or performance claims) match both the code and this
    review environment.

## Six summary statements

1. Same prefix keys can no longer alias different target/indexer bytes in
   the retained index: **YES** (preflight identity check, lines 133-138 and
   231-247).
2. Two MTP records for one key can no longer alias different draft bytes:
   **YES** (lines 235-238, tested 447-454).
3. MTP capability is monotonic under compatible later insertions: **YES**
   (replacement guard line 152, tested 432-436).
4. All collision and overflow failures are preflighted before mutation:
   **YES** (lines 118-146 complete before commit loop 148-169; tested
   400-414 and 438-454).
5. The regression distinguishes the previous replace-by-generation
   behavior: **YES** (prior implementation verified to fail steps 3-5 by
   inspection of `aecbcdf^`).
6. The CPU proof and all exclusions are accurate: **YES** (249 tests, 47
   then-present handoff proofs, and every exclusion independently
   reverified in this environment).

## Architecture & maintainability

The two-phase insert (pure preflight over an owned `pending` vector, then
an infallible commit loop) is the right shape for atomicity under `&mut
self` and reads clearly. Logical identity as `(byte_length, sha256)` per
piece, kept deliberately separate from generation/tier/offset, matches the
publication doc's revision-vs-placement split and is centralized in two
small helpers, which keeps the collision policy auditable. Two seams to
watch: the same logical-identity check should be lifted into a shared
helper reachable from durable replay before `recover_namespace` lands
(MINOR 1), and the preflight/commit invariant coupling on reference counts
(MINOR 3) should gain a local guard once an eviction or release path
exists. Test coverage is strong and behavior-driven; the single large
same-key regression could be split per matrix row as the matrix grows.

## Token decision

All twelve questions and all six summary statements are unqualified YES.
No blockers or majors; the three minors are hardening notes outside or
beyond the accepted boundary and do not qualify the pass. Hashes verified
at both start and finish with no drift. This acceptance covers only the
retained CPU prefix-index correction; it does not open cn4, authorize CUDA
work, or accept durable recovery, online publication, or model execution.

prefix-generation-integrity-v1-accepted
