# Fable adversarial review: captured-shape prefill scheduling v1

Date: 2026-07-31

Reviewer: Fable (adversarial CPU implementation review; no GPU/cn4 work)

Handoff: `docs/fable-prefill-captured-shape-v1-handoff.md`

Reviewed candidate commit (detached worktree; no modification, no commit):

9bdb2084619f0ede4425da3c626993b96fc3e6f8

Result-path note: the handoff requested `fable-prefill-captured-shape-v1.md`
at the repository root. The operator directed reviews into `docs/reviews/`;
this artifact is written there under that directive.

## Provenance

All pinned inputs were hashed with `shasum -a 256` in the detached worktree
at review start and re-hashed at review finish; both sets matched the
handoff table exactly. No stale candidate. The handoff file was absent from
the tree at the pinned commit; a byte-identical copy (SHA-256
`7b56ea375d0f9c528a279ca25bcdd011a84b40f0cfa487a7dc8e0be9ffaa6343`, verified
against the main tree) was placed in the worktree to run `review-proof`,
which returned `"verdict": "PASS"`.

| Input at candidate commit | Pinned = Start = Finish SHA-256 |
|---|---|
| `spec/engine-v0.md` | efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a |
| `crates/glm-scheduler/src/lib.rs` | 98259570e137bad517e19e46ab68f604e1aeba35e1535ab82fc179a04fda5a0e |
| `crates/glm-engine/src/graph.rs` | c85ca1aa52ba42294fc6a43524e8f70357523977d343e8c2f212787e7754cd22 |
| `docs/native-engine-plan.md` | 33552cd81e3d79b8b484856a99620420f3e2eddfdfa529a23b191353a702ed80 |
| `docs/offline-serving-foundation.md` | 9a722fdcc77522ac361493ca8fc02fea1e4692a28d1c22bd2e96607568cd4ce0 |
| `docs/offline-serving-spine.md` | 27b24d4cbafc8203937d3620e7bcd85d47fcb86cc4d8b89e237025e5d40a62f9 |
| `docs/prefill-captured-shape-proof-v1.md` | 602574c997ccd356d6fef5b1d160d5051a533f85ee14736f4b53cd71db33847d |
| `scripts/local-checks.sh` | 839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f |

Evidence commands executed in the worktree: `review-proof` (PASS);
`cargo test --offline -p glm-scheduler` (14 passed);
`cargo test --offline -p glm-serving` (24 passed);
`cargo test --offline --workspace` (236 passed — matching the proof's full
local gate count exactly);
`cargo clippy --offline -p glm-scheduler --all-targets -- -D warnings`
(clean). The pre-correction implementation was additionally inspected at the
parent of the proof's implementation commit
(`git show 4c421615120de9e73edfdf36b3ebedf60c28a693^`) to confirm the
defect-reproduction claims.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

None.

### QUESTION

**Q-1** — `best_graph` mirrors every *size* constraint of
`GraphProfile::admit` but not the route-compatibility constraints
(`compatible_tp_routes`/`compatible_dcp_routes`/`compatible_sampling_routes`),
which `admit` also checks against the eventual `StepPlan`. Within this
candidate the scheduler never fabricates route IDs (the step compiler draws
them from the selected entry), so no late rejection is constructible here,
but the invariant "the scheduler's chosen entry always admits the compiled
plan" rests on the compiler using only that entry's route lists. Worth one
sentence in the step-compiler contract so the invariant is owned somewhere.

## Answers to the twelve required adversarial questions

1. **Old progress defect real?** Yes, reproduced from source. The prior
   `build_prefill_batch` filled one batch bounded only by
   `config.maximum_prefill_tokens`/`maximum_batch_sequences` and then asked
   for a graph. With a 65-token prompt, a 64-token config limit, and only a
   32-row captured prefill entry, it constructed a 64-row batch, found no
   fitting entry, and returned `UncapturedShape` although the 32-row entry
   could make progress. The new per-entry construction fixes exactly this.
2. **Constructed rows satisfy each entry's limits?** Yes. Per candidate
   entry, sequences are capped at
   `min(config.maximum_batch_sequences, entry.maximum_active_sequences)`
   via `take(...)`, and total prompt tokens at
   `min(config.maximum_prefill_tokens, entry.maximum_prompt_tokens,
   entry.maximum_query_rows)` via the `available` budget
   (lib.rs:458-483); for prefill, query rows equal summed prompt tokens, so
   all three entry limits hold by construction.
3. **`min(...)` covers all `admit` prefill size constraints?** Yes.
   `admit` checks `active_sequences <= maximum_active_sequences`,
   `scheduled_prompt_tokens <= maximum_prompt_tokens`, and
   `query_rows <= maximum_query_rows` (graph.rs:94-97); construction bounds
   all three, and `best_graph` re-applies the same three plus the
   prefill-specific `maximum_prompt_tokens >= query_rows` (lib.rs:581-586).
   Route compatibility is outside the size correction (Q-1).
4. **Selected candidate failing `finalize_batch`?** No. The constructing
   entry is a fitting witness when `finalize_batch` runs: profile entries
   are immutable and validated (`Scheduler::new` calls `profile.verify()`),
   `validate_entries` forces prefill keys to `mtp_depth == 0` and
   `verifier_row_bucket == 0` (graph.rs:135-140) so the witness always
   passes `best_graph`'s prefill filter, and no scheduler state mutates
   between candidate construction and finalization inside the same
   `&mut self` call.
5. **Deterministic maximization and canonical ties?** Yes. Entries are
   sorted by `graph_id` at profile construction (graph.rs:49) and iterated
   in that order; replacement requires strictly greater
   `(query_rows, rows.len())` (lib.rs:488-494), so exact ties keep the
   lowest graph ID; `best_graph` breaks its own ties with an explicit
   `(sequence_bucket, verifier_row_bucket, maximum_query_rows, graph_id)`
   key. All containers are `BTreeMap`/`BTreeSet`/sorted `Vec`; no hash-map
   iteration order or rank-local state is involved.
6. **Fairness preserved; starvation possible?** Preserved, and no. Every
   candidate's rows are a prefix of the same deterministic weighted-fair
   order (`ordered_requests`, integer cross-multiplied service/weight
   scores with `(score, tenant, id)` tie-break), so the fair-order head
   appears in every nonempty candidate, including a high-row/low-sequence
   one; the head therefore progresses in every selected batch and
   weighted-fair rotation bounds every tenant's wait. Bounded decode-burst
   arbitration (`maximum_decode_burst`) is untouched.
7. **Step ID exactly once?** Yes. `next_step_id` is read and incremented
   only inside `finalize_batch` after the graph lookup succeeds
   (lib.rs:557-568), and `build_prefill_batch` calls `finalize_batch`
   exactly once, for the chosen candidate only. (The decode path's
   retry loop also cannot double-increment: failed attempts return before
   the increment.)
8. **Empty/malformed profiles fail closed?** Yes. `GraphProfile::new`/
   `verify` reject empty entry lists, zero/duplicate graph IDs, duplicate
   keys, zero limits, and malformed prefill keys; `Scheduler::new` refuses
   an unverifiable profile; with no prefill entry, `build_prefill_batch`
   yields `UncapturedShape` — an error, never an uncaptured batch.
9. **65-token regression?** Yes.
   `prefill_chunks_to_a_captured_shape_when_config_is_wider` drives
   65 prompt tokens through a profile whose only prefill entry is 32-row and
   asserts three successive batches of exactly `32, 32, 1` query rows with
   per-row token counts, then decode eligibility. Under the prior
   implementation the first batch would be 64 rows and fail
   `UncapturedShape`; the test distinguishes.
10. **Tradeoff regression?** Yes.
    `prefill_selects_the_highest_work_fitting_graph_across_shape_tradeoffs`
    offers a 1-sequence/64-row entry and a 4-sequence/32-row entry with a
    1-token and a 65-token request; only a per-entry search can discover
    that the 32-row entry admits `1 + 31 = 32` rows across two requests
    versus 1 row on the wide entry. The test asserts graph ID 2 and the
    exact `[1, 31]` row construction. The prior implementation would build
    64 rows and fail; the test distinguishes.
11. **Adjacent ABI limitation accurate?** Yes. `validate_entries` requires
    prefill `verifier_row_bucket == 0` and `GraphKey` uniqueness
    (graph.rs:115-140), so two prefill entries sharing
    `(mode, sequence_bucket, attention_transport)` necessarily collide
    (remaining key fields are pinned to zero) and cannot coexist. The proof
    correctly frames this as an adjacent H05 blocker requiring its own
    design (the pending row-bucket ABI v2), and it does not invalidate this
    correction: the correction operates entirely within profiles the
    current ABI can express, and the second regression legitimately uses
    distinct sequence buckets.
12. **236-test claim and non-claims accurate?** Yes. The workspace suite at
    the pinned commit passes exactly 236 tests (reproduced); scheduler and
    serving suites and scheduler Clippy are clean; the proof's exclusions
    (no CUDA/graph/route/quality/performance claims, tokenizer proof
    skipped for an unset `GLMAXX_TOKENIZER_DIR`) match this host's
    observations and claim nothing beyond the CPU correction.

## Six acceptance statements

- The old progress defect is reproduced: **YES**.
- Every constructed and selected batch is graph-bounded: **YES**.
- Selection, fairness, and step-ID behavior are deterministic: **YES**.
- Both regressions distinguish the prior behavior: **YES**.
- The adjacent graph-key limitation and scope boundary are accurate:
  **YES**.
- The CPU proof and its non-claims are accurate: **YES**.

## Architecture & maintainability

The correction keeps the right separation: profile entries remain the sole
authority on captured shapes, the scheduler searches only validated
immutable entries, and `finalize_batch` remains the single admission/step-ID
choke point. Constructing rows per candidate entry costs O(entries x
sequences) per prefill batch with small bounded factors — appropriate for a
CPU reference and trivially replaceable by a smarter index later without
contract change. The explicit "(query_rows, active_rows) is a progress
policy, not a measured SLO policy" framing in the proof is the honest
boundary; the SM120 sweep owns the real routing decision. The one
cross-contract seam worth writing down is Q-1 (compiler must draw route IDs
from the selected entry).

## Token decision

Findings: 0 BLOCKER, 0 MAJOR, 0 MINOR, 1 QUESTION. Provenance verified at
start and finish; `review-proof` PASS; all six acceptance statements are
unqualified YES. The requested token follows.

prefill-captured-shape-v1-accepted
