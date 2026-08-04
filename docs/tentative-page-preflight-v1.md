# Tentative page transaction preflight v1

Date: 2026-08-04

Status: behavior-preserving CPU implementation candidate; adversarial review
required before treating the optimization as production-qualified

## Purpose

`SequencePageTable::begin_tentative` and `commit_tentative` previously cloned
the complete table before every request mutation. `ServingCoordinator` already
creates the exact before/reservation/commit tables needed for rank deltas, so
the nested per-request snapshots duplicated rollback state and scaled with all
active sequences and pages.

Commit `d225904d2992c047a8a3d7400f88a4c4cfc8f79c` replaces only those two
successful-path snapshots. Public inputs, errors, page identities, state
transitions, tentative depth, target/draft symmetry, rejected-page quarantine,
generation binding, and page-delta bytes are unchanged.

## Reservation transaction

The immutable reservation preflight runs under the same exclusive `&mut self`
call as publication and proves before mutation:

- the sequence exists, has no active tentative tail, and remains within the
  1,048,576-position per-sequence bound;
- the current tail exists in the physical map and is either a complete sealed
  page or a private incomplete mutable page;
- adding one through seven positions needs at most one new page;
- the exact owner rank has both the target ID and, for MTP, the draft ID;
- the selected physical ID is absent; and
- every required state transition and integer operation is valid.

The apply phase removes only the preselected free IDs, inserts at most one
tentative page, updates at most one old tail, appends that page identity, and
publishes the retained `TentativeTail`. There is no fallible caller-controlled
decision after the first mutation. Contradiction of a preflighted private
invariant is fail-stop, not a partial recoverable result.

## Commit transaction

Commit preflight independently derives the accepted position, desired page
count, retained page states and valid-byte counts, and rejected suffix. Before
mutation it verifies every retained physical entry and proves every retired
page has one reference, no prefix identity, and no target or draft ID already
present in either its free set or quarantine.

Apply then removes the tentative marker, retains accepted page IDs in place,
applies the precomputed state/valid-token updates, and moves only rejected
target/draft IDs into owner-rank quarantine. No rejected ID becomes free until
the existing exact-generation four-rank acknowledgement path runs.

Rollback, terminal removal, shared-prefix handling, and rank delta generation
are unchanged. Failure paths still return before publication; ordinary
rollback retains its conservative snapshot because it is not the successful
decode hot path.

## CPU proof and matched measurement

The exhaustive tail/depth test covers every tail occupancy and tentative
depth. Existing capacity, rollback, in-place identity, target/draft,
quarantine, ABA, and 1M tests remain. New mutations prove a corrupt retained
state and a retirement/quarantine collision both reject without changing a
single table field; the latter remains retryable after removing the injected
collision.

The complete local gate passes 124 `glm-cache` tests and all workspace tests,
formatting, Clippy, deterministic proofs, 571 profiler-plan cases, and 163
review handoffs.

The matched 40-cell cn4 result is recorded in
`docs/cn4-page-transaction-preflight-d225904-20260804.md`. All cells improved;
median speedup ranges from 1.55x to 21.29x. The evidence is CPU-only and makes
no CUDA, model, quality, physical KV capacity, or serving-throughput claim.

## Remaining cost

At C8 and 128k tokens per sequence, the optimized MTP0 median is 10.926 ms:
4.114 ms reservation-delta construction and 4.250 ms commit-delta
construction dominate. MTP3 is 15.056 ms, of which the two deltas consume
6.200 and 6.315 ms. Removing those full snapshot scans remains gated by the
separate fixed-page-transaction r2 contract; this implementation does not
preempt or simulate it.
