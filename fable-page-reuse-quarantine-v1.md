# Review: page reuse quarantine and in-place commit v1

Date: 2026-07-31

Reviewer: Fable (adversarial design-gate review, CPU only; no cn4 contact, no
CUDA launched)

Handoff: `docs/fable-page-reuse-quarantine-v1-handoff.md` (SHA-256
`b642d2b3c265150d1cd7c77c1c97c52a8bc8948c7ac74a5344bf65f941d2d8de`, per
`review-proof`)

Candidate commit:

832bf9784ae67b2db4891bb17dcb8fc2647cf53a

Result-path note: the handoff requests `fable-page-reuse-quarantine-v1.md` at
the repository root; the operator directed this review, like all reviews, into
`docs/reviews/` instead of the repo root. This artifact therefore lives at
`docs/reviews/fable-page-reuse-quarantine-v1.md`.

## Provenance

Reviewed in a detached worktree at the candidate commit (`git rev-parse HEAD`
matched the candidate at review start and finish; worktree clean apart from an
untracked copy of the handoff itself, copied in so `review-proof` could run).
Every pinned input was hashed with `shasum -a 256` at review START and again at
review FINISH; both hash sets matched the handoff exactly, with no drift
between start and finish.

| Input | Verified SHA-256 (start and finish) |
|---|---|
| `crates/glm-cache/src/lib.rs` | bc3f31265e26638afd40307262afa1947d5cc2e88cfea96a18399d9fcee1cf7d |
| `crates/glm-cache/src/sequence.rs` | 8c0491d4f2d3e50da12e15961c8ac65a2fe5449a3527d40a38cdaa5ef27d644e |
| `crates/glm-cache/src/delta.rs` | 71ac2da15e869a6f2470c3551a7cd6ec4ff387850a23240e9a44ad96a538ff16 |
| `crates/glm-engine/src/worker.rs` | 39a0c0b917921149869d2afc5d652815986bf776eca5ddc9b7abee41b4892652 |
| `crates/glm-serving/src/lib.rs` | 362312a48e1269f09f2f3f6e090dffcf896a8b6c688b65d6060e6b505aae0bae |
| `docs/serving-page-transaction-v1.md` | 31983cce95ee01a5968213d5daf12c7a855f75f8735314700f2b4a9e55625d1a |
| `docs/page-reuse-quarantine-proof-v1.md` | 94b6c39ee57fafa926d6bc375bf2841c00f8586c38fe99700d54e9b86065d84c |
| `docs/offline-serving-spine.md` | 500628e6da720a760a242034678e402ab7fb0e78bd479c901254e6603cd35c99 |
| `docs/production-punchlist.md` | 9a9f5c37f366f6beda67a68fa0ced3cf89e6fb9fd31b9c894b947108642048bb |
| `docs/results-index.md` | 7d31ba6c66f6d5362e717a9894ed706ad0ff92fe3062cf3fbfd15bbfa416f07c |
| `scripts/local-checks.sh` | 839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f |

`review-proof` on the handoff: verdict PASS, `repository_head` equal to the
candidate, every input `actual_sha256 == expected_sha256`.

## Gate results

All commands run from the worktree at the candidate commit:

- `review-proof docs/fable-page-reuse-quarantine-v1-handoff.md`: PASS.
- `cargo test --offline -p glm-cache sequence::tests`: 12 passed, 0 failed
  (58 filtered out of the 70-test glm-cache suite).
- `cargo test --offline -p glm-serving`: 41 passed, 0 failed.
- `cargo clippy --workspace --all-targets --offline -- -D warnings`: clean.
- `scripts/local-checks.sh`: exit 0; fmt, workspace tests, clippy, cuda-ffi
  type checks, cpu/matrix/engine/serving/cache-lifecycle proofs all passed
  against pinned fixtures; tokenizer proof skipped (`GLMAXX_TOKENIZER_DIR`
  unset); CUDA compile skipped (no `nvcc`); `review-proof-all` verified 70
  handoffs and 0/51 configured results.

Procedural disclosures: `local-checks.sh` was executed twice (the second run
solely to capture the exit status after the first run's output was garbled by
terminal control characters; both runs were consistent and exited 0), and one
additional `cargo test --workspace --offline` was run to verify the 286-test
claim independently. The independent workspace run summed to exactly 286
passed, 0 failed (70 glm-cache, 7 glmaxx, 11 glm-cuda, 46 glm-engine, 60
glm-format, 3 + 22 + 16 workspace/integration targets, 41 glm-serving, 10
remaining, plus empty doc-test targets).

Count reconciliation for the proof doc's "69 handoffs / 0 of 50 configured
results": the observed 70/51 includes exactly one extra handoff — this
review's own handoff, copied into the worktree untracked — whose configured
result artifact does not yet exist. At the candidate tree the counts are
69/50 as claimed.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. Hot-path complexity is linear in table size per step, quadratic over a
   long decode. Every public mutator of `SequencePageTable` snapshots the
   entire table by `clone` for rollback (`crates/glm-cache/src/sequence.rs`
   lines 279, 381, 409, 472, 526, 537, 548), `commit_tentative_inner`
   revalidates every retained page `for ordinal in 0..desired_page_count`
   (sequence.rs:865-893 — 16,384 `BTreeMap` probes per MTP commit at the
   1M-token limit), and `PageTableDelta::between` renormalizes both full
   tables (`crates/glm-cache/src/delta.rs:110-136`), as does serving's
   per-tick `active_pages.clone()` (`crates/glm-serving/src/lib.rs:1199`,
   977, 998). Aggregate O(pages × steps). This matches the documented
   exclusion (proof v1 "Atomicity and capacity" and "Exclusions": the clone
   is explicitly not claimed as the production fixed-capacity hot path;
   performance is outside the review boundary), so it is not blocking, but
   the fixed-capacity journal remains a real prerequisite for production.

2. No serving-level regression directly asserts the retained bound
   quarantine after a worker-fatal cleanup.
   `rank_divergence_fails_every_row_in_the_selected_batch`
   (glm-serving/src/lib.rs:2292-2321) exercises the `WorkerRetired` path but
   asserts only removal and events, not
   `active_pages.reuse_quarantine_stats().bound_generation.is_some()`. The
   freeze itself is enforced structurally (lib.rs:1030, 1151, 1161-1170;
   sequence.rs:980-986) and mutation-while-bound is regression-covered at
   cache level (sequence.rs:1317-1321), so the property holds; the gap is
   test altitude only.

3. `bind_reuse_quarantine` does not require the bound generation to exceed
   previously acknowledged generations (sequence.rs:235-244 rejects only
   zero and rebinding). Cross-cycle monotonicity is imposed one layer up by
   the delta generation chain (`delta.rs:490-495`,
   `generation_after == generation_before + 1`) and the coordinator's single
   `sequence_table_generation` counter, so the integrated path cannot replay
   a stale generation; the table-level API alone would accept one.

### QUESTION

1. A worker-fatal cleanup whose removed sequences hold only shared pages
   with surviving references produces an empty quarantine; the
   `WorkerRetired` release then publishes the host removal with an advanced
   generation and no bound quarantine (lib.rs:1333-1337 returns Ok on empty)
   while rank mirrors are retired. No ID is retired, so no reuse hazard
   exists, and the closed pool fails every subsequent delta, but the
   host/mirror divergence in that terminal state is unchecked by
   construction. Confirm this is the intended fail-stop shape.

2. In the decode/verify tick, an error between `apply_page_delta` success
   (lib.rs:785) and publication (lib.rs:1172-1173) would leave rank mirrors
   one generation ahead of the host; the next transmitted delta would then
   fail the mirrors' `generation_before` check (delta.rs:311-313) and retire
   the worker generation. Confirm this reliance on generation-chain
   fail-stop rather than a host-side poison flag is intended.

## Answers to the required adversarial questions

1. In-place retention of accepted IDs: YES. `commit_tentative_inner`
   transitions every retained page in place from `HbmTentative` to its exact
   committed state (sequence.rs:883-892) and releases only pages popped
   beyond `desired_page_count` (sequence.rs:848-864); no accepted target or
   draft ID is ever freed and reacquired. Regression:
   `tentative_commit_keeps_accepted_ids_and_quarantines_only_rejected_pages`
   asserts physical and draft identity across commit with an empty
   quarantine (sequence.rs:1334-1390).

2. Exact retirement boundary, cross-page MTP at all tail occupancies: YES.
   `desired_page_count = ceil((original + accepted)/PAGE_TOKENS)`
   (sequence.rs:825-831), the pop loop releases exactly the pages beyond it,
   and the tail's `valid_tokens`/state are recomputed exactly for any
   occupancy (sequence.rs:865-892). The occupancy sweep
   `every_tail_occupancy_and_mtp_depth_reserves_exactly_one_position_per_token`
   (all 64 tails × depths 1-7) plus the cross-page rejection case
   (sequence.rs:1367-1389) and `mtp_zero_through_six_commit_and_rollback`
   cover the arithmetic.

3. Simultaneous membership in active map / free set / quarantine: NO (cannot
   occur). `release_page` removes the entry from `physical` before
   quarantining and fails with `Invariant` if the ID is already free or
   already quarantined (sequence.rs:956-976); `allocate_page` inserts into
   `physical` only from a free-set pop and rejects an existing entry
   (sequence.rs:901-941); `acknowledge_reuse_quarantine` moves by
   `BTreeSet::append`, preserving disjointness (sequence.rs:256-259). Any
   invariant failure restores the pre-mutation snapshot.

4. Shared-prefix quarantine only at last reference: YES. `release_page`
   decrements and returns early while `references != 0`
   (sequence.rs:949-955); only the zero-reference path removes the prefix
   binding and quarantines. Regressions:
   `shared_target_prefix_upgrades_to_mtp_without_duplication`
   (sequence.rs:1464-1489) and serving's shared-prefix release tests.

5. Consuming a quarantined ID early: NO. Allocation consults only
   `free_target`/`free_draft` (sequence.rs:906-919), never the quarantine
   sets, so an unbound quarantined ID is unreachable; while bound, every
   mutator is refused by `require_unbound_quarantine` (sequence.rs:980-986);
   a wrong-generation acknowledgement fails and leaves the binding intact
   (sequence.rs:252-254); a failed rank update leaves `rank_state`
   `WorkerRetired`, which skips acknowledgement forever (lib.rs:1030,
   1151-1170). Regression:
   `removed_target_and_draft_ids_cannot_aba_before_exact_generation_ack`
   proves `Capacity` before ack and `Transaction` while bound
   (sequence.rs:1393-1435).

6. Binding requires one nonzero generation, freezes mutators, rejects
   rebinding: YES. `bind_reuse_quarantine` rejects `generation == 0` and any
   existing binding (sequence.rs:235-244); all seven public mutators
   (`admit_with_prefix`, `append_committed`, `fork_sequence`,
   `begin_tentative`, `commit_tentative`, `rollback_tentative`,
   `remove_sequence`) call `require_unbound_quarantine` first
   (sequence.rs:272, 377, 397, 468, 517, 536, 547).

7. Exact acknowledgement is atomic and clears the binding: YES.
   `acknowledge_reuse_quarantine` verifies the exact generation, then moves
   every rank's target and draft quarantine into the free sets with
   infallible `append` and clears `quarantine_generation`
   (sequence.rs:248-262); no partial state is reachable.

8. Bind before transmitting the successor delta: YES. In the decode/verify
   tick the coordinator binds the complete rejected+removal set at
   lib.rs:759-768, builds the commit delta from that bound table at
   lib.rs:769-784, and only then transmits (lib.rs:785). In
   `commit_request_releases` the order is bind (lib.rs:1150), transmit
   (lib.rs:1152-1158), acknowledge (lib.rs:1161-1170).

9. Ordinary cleanup acknowledges only after four validated receipts: YES.
   `acknowledge_reuse_quarantine` is gated on
   `rank_state == Acknowledged` (lib.rs:1161-1170), which is set only after
   `Tp4WorkerPool::apply_page_delta` returns Ok (lib.rs:785-792,
   1158-1159); that method validates, for all four distinct ranks, the exact
   successor generation, global digest, and recomputed rank-local digest
   (`crates/glm-engine/src/worker.rs:586-618`), with each rank's ack
   constructed only after its persistent mirror applied the delta
   (worker.rs:620-633, 737-743).

10. No host reuse published before mirror removal: YES. Terminal removal,
    cancellation, accepted EOS, length completion, and late rollback all
    route through `plan_request_releases` + `commit_request_releases`
    (lib.rs:957-986, 988-1037, 1145-1189) or through the in-tick commit path
    (lib.rs:749-793); in every path `apply_page_delta` (with receipt
    validation) precedes acknowledgement, and a failed transmit leaves the
    plan unpublished (`self.active_pages` unchanged) or `WorkerRetired`.
    Late output rollback additionally applies a rollback delta to ranks
    before releasing (lib.rs:708-723, 1039-1056).

11. Fatal worker generation cannot forge a receipt; quarantine stays
    unusable: YES. `PageDeltaAck` values exist only after a successful
    mirror apply (worker.rs:620-633); any rank/mirror/consensus failure
    terminates the dispatch loop (worker.rs:531-538), after which every pool
    command returns `Closed`. Host cleanup after a fatal sets
    `RankReleaseState::WorkerRetired` (lib.rs:1030), which skips both
    transmit and acknowledgement (lib.rs:1151, 1161), publishing a table
    whose bound quarantine freezes every subsequent mutator
    (sequence.rs:980-986). See MINOR 2 and QUESTION 1 for coverage notes.

12. Failed mutations atomic across all state: YES. Every fallible mutator
    clones the whole table and restores it on any error, covering active
    pages, references, prefixes, free sets, quarantine sets, and tentative
    state together (sequence.rs:279-369, 381-388, 409-461, 472-509,
    526-533, 537-544, 548-567); `PageTableMirror::apply` mutates a candidate
    clone and installs it only after validation (delta.rs:309-356); serving
    mutates clones and publishes only on success (lib.rs:337-351,
    1145-1189). Regressions: `failed_cross_page_reservation_is_atomic`,
    `failed_sequence_removal_restores_every_page_and_is_retryable`,
    `page_capacity_failure_is_atomic_and_never_reaches_rank_workers`,
    `late_terminal_cleanup_failure_does_not_partially_publish_the_batch`.

13. Regressions distinguishing, not tautological: YES. The ABA test uses a
    one-page arena so pre-ack reuse must surface as `Capacity`
    (sequence.rs:1412-1416) — the pre-milestone free-on-release behavior
    would have admitted; wrong-generation and bound-mutation asserts return
    `Transaction` (sequence.rs:1420-1427, 1318-1321); in-place identity is
    paired with an empty-quarantine assert (sequence.rs:1350-1355), which
    free-and-reacquire would violate; the rejected-suffix test counts exactly
    one quarantined target and draft ID on the rejected page's owner rank
    (sequence.rs:1374-1382); shared-prefix, MTP0-6
    (`mtp_zero_through_six_commit_and_rollback`), and the 1M tests
    (`one_million_positions_fill_exactly_balanced_dcp4_slots`,
    `exact_one_million_context_is_admitted_accounted_executed_and_released`)
    assert exact counts and exact physical identity, not merely success.

14. Gate counts reproducible: YES. Independently measured 286 workspace
    tests, 0 failures, including exactly 70 glm-cache and 41 glm-serving;
    formatting, Clippy with `-D warnings`, cuda-ffi type checks, and all
    deterministic proof fixtures passed via `scripts/local-checks.sh` (exit
    0); tokenizer and nvcc skips reproduced with the stated causes; the
    69-handoff / 0-of-50 configured-results claim reconciles exactly with
    the observed 70/51 once this review's own untracked handoff is
    subtracted.

15. Exclusions accurate: YES. The clone-backed rollback and owned
    `PageTableDelta`/mirror storage are real in the code (sequence.rs
    snapshots; delta.rs `Box<[_]>`/`BTreeMap`) and admitted
    (proof v1 "Exclusions"); no fixed-capacity undo journal, no CUDA-visible
    tables, no upload events or stream receipts, no `CACHE_ONLY` plan ABI,
    no device zeroization or teardown exist in the pinned files, and none is
    claimed; model output, quality, capacity under live payloads, and
    performance are neither tested nor claimed. The handoff conveys no GPU
    authorization and none was used.

## Eight summary statements

- Accepted pages preserve exact target/draft identity: YES.
- Rejected and removed pages cannot ABA before exact receipt: YES.
- Generation binding and mutation freeze are fail-closed: YES.
- All four rank receipts precede ordinary allocator reuse: YES.
- Fatal worker retirement cannot become a false acknowledgement: YES.
- Rollback and shared-prefix reference behavior remain atomic: YES.
- All regressions and gate counts are accurate: YES.
- Every device/model/performance exclusion is accurate: YES.

## Architecture & maintainability

The design is a clean two-phase ownership protocol layered on an existing
CPU oracle: a single disjoint quarantine per rank pair (target/draft), one
optional bound generation, and a freeze that is enforced at the only eight
entry points that can mutate the table. The strongest property is that reuse
safety does not depend on the coordinator behaving well — the table itself
makes early reuse unrepresentable (quarantined IDs are simply absent from
every allocatable set) and makes acknowledgement unforgeable at the serving
layer by construction (`RankReleaseState` is a three-state latch whose
`Acknowledged` arm is reachable only through the receipt-validating pool
call). Generation chaining (`before + 1 == after`) at the delta layer gives
end-to-end monotonicity without the table needing its own counter, at the
cost of the table-level API being slightly weaker than the system property
(MINOR 3). The snapshot-clone rollback keeps every mutator trivially atomic
and reviewable, which is the right trade for an oracle, but it and the
O(pages) commit revalidation must not survive into the fixed-capacity
production allocator; the punchlist already tracks that. Test altitude is
generally excellent (exhaustive occupancy sweeps, exact-count asserts); the
one soft spot is serving-level assertion of the post-fatal frozen quarantine
(MINOR 2). Naming and layering (cache table / delta / worker pool /
coordinator) remain consistent with the serving-page-transaction-v1 design
note.

## Token decision

Provenance verified at start and finish with zero drift; all gates pass; no
BLOCKER or MAJOR findings; all fifteen answers and all eight summary
statements are unqualified YES. The acceptance below covers only this CPU
quarantine and receipt-ordering milestone; it does not open cn4, authorize
CUDA work, or accept production serving.

page-reuse-quarantine-v1-accepted
