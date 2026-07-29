# Fable handoff: prefill row-bucket and graph-profile ABI v2

Date: 2026-07-29

Status: adversarial design review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`9b0465284a1c0845772551902bb3e21c26025d51`

Required result path:
`fable-prefill-graph-profile-abi-v2.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`prefill-graph-profile-abi-v2-design-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/native-engine-plan.md` | `33552cd81e3d79b8b484856a99620420f3e2eddfdfa529a23b191353a702ed80` |
| `docs/offline-engine-contract.md` | `b5a51b15a0a600031fcddb7d840d4d499cf915d4c22f26099cdb1dc188d74fe1` |
| `docs/prefill-captured-shape-proof-v1.md` | `602574c997ccd356d6fef5b1d160d5051a533f85ee14736f4b53cd71db33847d` |
| `docs/prefill-graph-profile-abi-v2.md` | `37154c9e31109acdf35a382c6be87b3a865e2b7f6ae8f801969526789dd41f91` |
| `crates/glm-engine/src/step.rs` | `4963a58da7c9c6bbed0fb57fb7ef56d90d1e0f09fe54da8cc02c35891f743359` |
| `crates/glm-engine/src/graph.rs` | `c85ca1aa52ba42294fc6a43524e8f70357523977d343e8c2f212787e7754cd22` |
| `crates/glm-scheduler/src/lib.rs` | `98259570e137bad517e19e46ab68f604e1aeba35e1535ab82fc179a04fda5a0e` |
| `crates/glm-scheduler/src/compile.rs` | `220cf549c0b5882d109ebce4ebd646e9b28ebbab80a83fa579ef5a2c591a070a` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-prefill-graph-profile-abi-v2-handoff.md
cargo test --offline -p glm-engine
cargo test --offline -p glm-scheduler
```

The tests exercise the pinned v1 implementation only; they are a
provenance/sanity check and not v2 implementation evidence.

## Review boundary

This review accepts or rejects the proposed mode-neutral row-bucket
semantics, v2 identity bump, limits, graph-profile invariants,
scheduler/compiler binding, implementation sweep, and CPU/hardware gates.
It does not accept an implementation, CUDA graph, route table, checkpoint,
quality result, or performance result.

## Required adversarial questions

1. Is the v1 key collision real: do unique graph keys plus prefill
   `verifier_row_bucket == 0` prevent two prompt chunk sizes under the same
   sequence bucket and transport?
2. Is the global 448-row rejection real, and does it incorrectly reject the
   pinned 3,072-row prefill control while correctly bounding C64×MTP6?
3. Is a semantic v2 bump safer than silently reinterpreting the existing v1
   field and hash domain, given that compatibility is explicitly not a
   project requirement?
4. Does keeping offset 29 as a four-byte `row_bucket` preserve the exact
   85-byte hash input and 117-byte record without permitting v1/v2
   confusion?
5. Are new step-plan and graph-profile ABI strings, schemas, and independent
   hash-domain bumps sufficient to fail closed on v1 artifacts? Must any
   additional ABI or manifest identity be bumped?
6. Is 3,072 the correct initial prefill ceiling from the pinned source and
   kernel boundaries? Does any GLM-5.2 operation, collective payload, or
   workspace arithmetic impose a smaller limit that the design omits?
7. Are the mode invariants complete, especially
   `query_rows <= row_bucket`, prefill prompt/query equality, decode/verify
   ceilings, cache-only zeroing, and mixed-mode rejection?
8. Is requiring
   `maximum_query_rows == key.row_bucket` and, for prefill,
   `maximum_prompt_tokens == key.row_bucket` correct for a captured padded
   capacity? Would any legitimate graph-node update or masked-row use
   require inequality instead?
9. Can two prefill row buckets now coexist without falsely varying sequence
   bucket, transport, context band, topology, or weight policy?
10. Is it correct that context band and topology remain globally selected
    route-table inputs rather than graph keys when the captured DAG is
    structurally identical?
11. Does copying the bucket from the selected immutable graph entry, rather
    than deriving it from the live row count, prevent rank or replay drift?
12. Does the scheduler proof remain valid with v2 families, and is
    `(query_rows, active_rows)` clearly limited to a CPU progress policy
    pending measured SLO routing?
13. Does the implementation sweep name every source, fixture, manifest,
    documentation, and current-tree qualification class that must change?
14. Are the twelve CPU exit tests sufficient to distinguish the two v1
    defects and prevent a byte-compatible-but-semantically-ambiguous partial
    migration?
15. Are the GPU, model, quality, route, and performance non-claims accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- both v1 contradictions are reproduced and correctly scoped;
- the v2 identity, wire record, and fail-closed migration are accepted;
- mode limits and row-bucket invariants are complete;
- graph-key, scheduler, compiler, and route-table ownership are accepted;
- the implementation and CPU exit gates are complete; and
- the hardware boundary and non-claims are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, missing ABI bump, possible
v1/v2 confusion, wrong row ceiling, ambiguous bucket capacity, rank-local
choice, incomplete migration, or insufficient tests.

The token accepts only this design. It does not authorize implementation,
open cn4, qualify a CUDA graph, or accept performance.
