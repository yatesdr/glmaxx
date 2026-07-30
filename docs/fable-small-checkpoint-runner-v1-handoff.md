# Fable handoff: deterministic small-checkpoint runner v1

Date: 2026-07-30

Status: superseded by `docs/small-checkpoint-runner-v1-r2.md`; do not review
this handoff and do not issue the v1 token

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`2b3318176d34eded55cc97e49998423ad4e902ce`

Required result path:
`docs/reviews/fable-small-checkpoint-runner-v1.md`

Requested acceptance token, only if every blocker and major is resolved:
`small-checkpoint-runner-v1-design-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `docs/nvfp4-physical-abi.md` | `d5a189ae06f8f39e828400e589fbd31f94f0245cb90488881369d6a806bd6d1e` |
| `docs/target-layer-execution-v1.md` | `7f16667d55a50441dbc81b7fe18285be5a6f0fb06c68ef008a40aad8f406f478` |
| `docs/checkpoint-load-transaction-v1.md` | `79d9c376201f3540f247344c24c37dd7d819d629f10459899a73c15f8b27015f` |
| `crates/glm-format/src/checkpoint.rs` | `08450f0cb33e592ec76dfbe655b06580ba1743e60f3109a65218d052dbea406c` |
| `crates/glm-engine/src/startup.rs` | `54d41acc810c90cc49fe4acc0623b6a13bb2c09b72b2f8e5fb6615250ead2ddd` |
| `docs/small-checkpoint-runner-v1.md` | `720e07e3791ab1c5174aedc9aa449cfe048e6bc1b9d483798c0d83d8319050f6` |
| `docs/production-punchlist.md` | `9d2fe5502020581adfe338f0c6231f9327e7eccccb352b6635b7bd8d237925d3` |
| `docs/results-index.md` | `13d9661a9b716cd1448293b8fc67d29ef511f35aa61c55788a0d6d68e7fbc6b5` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-small-checkpoint-runner-v1-handoff.md
```

## Review boundary

This review covers the deterministic M4 scope, exact layer-6 NVFP4
laboratory subset, source binding, captured input/reference boundary,
load/adoption/execution transaction, repetition/fault matrix, evidence, and
non-claims.

It does not accept any implementation, NVFP4 source checkpoint, packed
fixture, CUDA load, rank worker, graph, collective, layer replay, logits,
smoke result, quality result, capacity result, or performance claim.

## Required adversarial questions

1. Does M4 remain ordered after accepted M2 operator/collective evidence and
   the complete M3 layer-6 replay, with no way to use this design to skip
   either gate?
2. Is layer 6 the correct smallest real layer boundary that exercises full
   indexer, DCP attention, routed/shared experts, and TP reductions?
3. Is the absence of embedding and full token feedback explicit enough that
   the runner cannot be advertised as a full-model/API smoke?
4. Independently derive the 19 protected layer tensors, 512 combined-FC1/
   FC2 expert tensors, 2 final tensors, and exact total of 533 per rank.
5. Independently derive 147,487,232 protected layer bytes, 1,358,954,496
   expert-plane bytes, 475,803,648 final bytes, 1,982,245,376 total payload
   bytes, and 65,536 metadata bytes per rank.
6. Are the combined gate/up and down shapes, value bytes, scale bytes,
   block-16 grouping, and zero internal 256-byte alignment slack consistent
   with the reviewed NVFP4 physical ABI?
7. Does binding to the exact M2-accepted NVFP4 checkpoint/repack prevent
   silent EXL3 requantization, rank-local source choice, codec-policy drift,
   or a shape-compatible substitute?
8. Must implementation add a distinct laboratory rank-manifest schema or
   profile contract, and does the design correctly require the complete
   production validator to reject this subset?
9. Are captured real M3 hidden/cache/route inputs plus packed-weight
   reference logits sufficient for deterministic prefill/decode operator
   smoke without claiming an omitted-layer model recurrence?
10. Does the load sequence preserve quarantine through complete four-rank
    prepared receipt validation and process-wide adoption?
11. Can any partial adoption, worker, graph, collective, semantic, numerical,
    or cleanup failure publish output or leave an executable subset?
12. Is using the production `StepPlan`, graph, collective, DCP transport, and
    distributed-greedy paths compatible with a dedicated non-serving runner?
13. Are 100 warm repetitions and five cold load/run/destroy cycles enough to
    distinguish nondeterminism and monotonic resource leaks at this gate?
14. Does the fault matrix cover all rank-local and process-wide boundaries
    required before a full-checkpoint attempt?
15. Are sharded-logit comparison, padding/all-masked rejection, stable/tie
    classification, and no-full-vocabulary-gather behavior complete?
16. Are evidence fields sufficient to separate kernel, launch, collective,
    transfer, framework, and end-to-end time without making a speed claim?
17. Are all exclusions accurate, especially no EXL3, quality, capacity,
    MTP1–6, concurrency, prefix/tier, 1M-context, or performance result?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- M4 gate position and anti-skip rules are accepted;
- subset membership, tensor counts, and byte arithmetic are exact;
- source/checkpoint/codec identity is fail-closed;
- the captured M3 boundary is a valid small-runner scope;
- load, adoption, execution, and cleanup ownership are process-safe;
- the repetition and fault matrix is sufficient;
- evidence and numerical comparison are sufficient; and
- the runner cannot be mistaken for a full-model, serving, quality,
  capacity, or performance result.

Only if all eight answers are unqualified `YES`, end with the requested
token. Withhold it for a conditional pass, stale input, M2/M3 bypass,
incorrect membership/arithmetic, source fallback, production-manifest
masquerade, synthetic input, unpacked reference, pre-adoption visibility,
partial failure publication, incomplete fault matrix, full-logit-gather
dependency, insufficient evidence, or overstated model/serving/quality/
capacity/performance claim.

The token accepts only the M4 design. It does not open cn4, authorize CUDA
work, or accept a smoke result.
