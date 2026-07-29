# Fable handoff: captured-shape prefill scheduling v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`9bdb2084619f0ede4425da3c626993b96fc3e6f8`

Required result path:
`fable-prefill-captured-shape-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`prefill-captured-shape-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-scheduler/src/lib.rs` | `98259570e137bad517e19e46ab68f604e1aeba35e1535ab82fc179a04fda5a0e` |
| `crates/glm-engine/src/graph.rs` | `c85ca1aa52ba42294fc6a43524e8f70357523977d343e8c2f212787e7754cd22` |
| `docs/native-engine-plan.md` | `33552cd81e3d79b8b484856a99620420f3e2eddfdfa529a23b191353a702ed80` |
| `docs/offline-serving-foundation.md` | `9a722fdcc77522ac361493ca8fc02fea1e4692a28d1c22bd2e96607568cd4ce0` |
| `docs/offline-serving-spine.md` | `27b24d4cbafc8203937d3620e7bcd85d47fcb86cc4d8b89e237025e5d40a62f9` |
| `docs/prefill-captured-shape-proof-v1.md` | `602574c997ccd356d6fef5b1d160d5051a533f85ee14736f4b53cd71db33847d` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-prefill-captured-shape-v1-handoff.md
cargo test --offline -p glm-scheduler
cargo test --offline -p glm-serving
cargo clippy --offline -p glm-scheduler --all-targets -- -D warnings
```

## Review boundary

This review accepts or rejects only the CPU scheduler correction that fits
prefill work to an existing legal captured graph. It does not accept CUDA
graph capture, the SM120 prefill graph family, PCIe route choice, the
adjacent `GraphKey` ABI extension, checkpoint execution, or performance.

Treat the documented inability to encode multiple prompt-row buckets under
one sequence/transport key as an adjacent H05 blocker that requires its own
design and review, not as an omitted claim of this narrow correction. If it
invalidates the correction itself, explain the concrete dependency and
withhold the token.

## Required adversarial questions

1. Can the prior implementation build a batch wider than every legal
   prefill graph and return `UncapturedShape` even though a smaller entry can
   make progress?
2. For every profile entry considered, do the constructed rows satisfy its
   active-sequence, scheduled-prompt-token, and query-row limits?
3. Does considering
   `min(config tokens, maximum_prompt_tokens, maximum_query_rows)` cover all
   prefill size constraints used by `GraphProfile::admit`?
4. Can a selected candidate ever fail `finalize_batch` because the concrete
   entry used to build it is no longer a fitting witness?
5. Is `(query_rows, active_rows)` maximization deterministic, and do exact
   ties retain canonical graph-ID order without depending on a hash-map or
   rank-local state?
6. Does the selection preserve the existing weighted-fair request order and
   bounded decode-burst behavior? Could choosing a high-row/low-sequence
   entry permanently starve another tenant?
7. Is `next_step_id` incremented exactly once for the chosen batch, rather
   than once per candidate considered?
8. Do empty or malformed profile situations still fail closed rather than
   creating an uncaptured batch?
9. Does the 65-token regression prove repeated `32, 32, 1` progress and fail
   on the old implementation?
10. Does the tradeoff regression genuinely require searching legal profile
    entries and prove exact `1 + 31` row construction?
11. Is the stated adjacent ABI limitation accurate: with prefill
    `verifier_row_bucket == 0` and unique `GraphKey`, two chunk sizes cannot
    coexist for the same sequence bucket and attention transport?
12. Are the 236-test claim and all GPU/graph/performance non-claims accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- the old progress defect is reproduced;
- every constructed and selected batch is graph-bounded;
- selection, fairness, and step-ID behavior are deterministic;
- both regressions distinguish the prior behavior;
- the adjacent graph-key limitation and scope boundary are accurate; and
- the CPU proof and its non-claims are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, possible uncaptured batch,
constraint omission, rank-local/nondeterministic choice, starvation,
step-ID drift, or a regression that cannot distinguish the defect.

The token accepts only this CPU correction. It does not open cn4, qualify a
CUDA graph, or accept the adjacent graph-key ABI.
