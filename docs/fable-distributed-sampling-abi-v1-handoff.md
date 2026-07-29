# Fable handoff: distributed sampling ABI v1

Date: 2026-07-29

Status: adversarial design review; implementation token withheld by Sol

GPU authorization conveyed by this handoff: none

Review candidate commit:
`7c718188b167615affabdb66f34939dcd6b22587`

Requested acceptance token, only if every blocker and major is resolved:
`distributed-sampling-abi-v1-accepted`

## Provenance

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/distributed-sampling-abi-v1.md` | `d717508e4d90f6ef378d486c0bd3e93e7dad522e6529b8504ccb687a0280fdce` |
| `docs/step-execution-io-v1.md` | `e8681e9278034b25fe6928c059ad58730818ce014fb3e0251549f678aa1621d5` |
| `docs/native-engine-plan.md` | `33552cd81e3d79b8b484856a99620420f3e2eddfdfa529a23b191353a702ed80` |
| `docs/benchmark-contract.md` | `cd51d22a8faf2baacfb4682ff5e1dcb5986edc27d8aa3af188105842bb49a507` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `crates/glm-reference/src/sampling.rs` | `6d90f43dbf0d2865ef63c001601c9084d6370c168056f883f49ffe7c732d9d13` |
| `crates/glm-serving/src/http.rs` | `0cd2f66e45a1e79e14035c44b34ecaa73d7da80fc9b3ba580771937a6c9b5c41` |
| `crates/glm-serving/src/backend.rs` | `34396a06b459e060af0c5f6b0cfb6451522af0f72536312da24804b25fe40c6c` |
| `crates/glm-scheduler/src/lib.rs` | `5651a507ad240f19755d50336f09eb3ca97e32f8be51f90e0fe49ef304350f38` |
| `crates/glm-scheduler/src/compile.rs` | `220cf549c0b5882d109ebce4ebd646e9b28ebbab80a83fa579ef5a2c591a070a` |
| `crates/glm-engine/src/output.rs` | `1a82e9990af4e2892831e950f5cd3db256032ab5e43642faeb08229cbf2f1c2c` |
| `crates/glm-engine/src/step.rs` | `4963a58da7c9c6bbed0fb57fb7ef56d90d1e0f09fe54da8cc02c35891f743359` |
| `crates/glm-engine/src/worker.rs` | `400c7c22f2c74d3f386fefe2d144da3437f27e6c99ce9f2d4bbf87ffe98fe437` |
| `docs/serving-observability-v1.md` | `4058d01d58c0d8f4d7222803e05577a9419cfa6f5d0f20a65c41e9e2779213e6` |

Hash every input at review start and finish. Review the exact candidate commit
in a separate worktree if `main` advances. The candidate deliberately
contains no promoted ABI or GPU implementation.

## Requested adversarial questions

1. Are the canonical GREEDY/TOP_K/MASS tuples complete, and must zero
   temperature with any nontrivial filter be rejected rather than ignored?
2. Is a CSPRNG-materialized omitted seed plus response header an acceptable
   compatibility/replay boundary? Identify security, privacy, or cross-process
   reproducibility concerns.
3. Re-derive the SplitMix64 word and `[0,1)` FP64 mapping expression by
   expression. Are CPU and CUDA conversion/rounding requirements sufficient?
4. Does sequential counter allocation remain deterministic under retry,
   batching, cancellation, early EOS, context clamp, and output clamp?
5. Is consuming one acceptance draw even at ratio zero or one the correct
   stable schedule?
6. For a rejection at position `i`, is consuming all already-generated draft
   proposal draws followed by `i+1` acceptance draws and one residual draw
   distributionally correct?
7. Are the `2R+1` maximum, accepted-draft-EOS `2R` case, and every
   no-bonus limit case exact? Enumerate counter totals for `R=1..6`.
8. Does early draft EOS require a smaller proposal count inside a fixed
   `K+1` graph, and are padded verify rows prevented from every state/RNG
   effect?
9. Is strict `uniform < min(1,p/q)` correct at zero, one, and
   representational boundaries? Should `p/q` use retained FP32 values,
   promoted FP64, or a different construction?
10. Is target-distribution fallback on a numerically zero residual mass safe,
    or should that event fail closed because a rejection should be impossible
    when `p=q`?
11. Does the temperature→top-k→top-p→normalization definition match the
    intended user semantics, especially whether top-p mass is normalized
    after the top-k mask?
12. Are padded rows and negative infinity handled safely by every top-k/mass
    edge case, including fewer than k finite logits and an all-masked row?
13. Can fixed token/rank accumulation order plus qualified `expf` provide
    repeatability on SM120? State what is bit-exact and what requires
    statistical/full-vocabulary tolerance.
14. Are the composite route sub-ordinals and wire records sufficient to avoid
    a hidden full-vocabulary gather? Re-derive the 8,192-byte top-k candidate
    bound and identify the exact exclusive PCIe-byte accounting still needed.
15. For speculative residual sampling after bounded top-k/top-p, can every
    rank reconstruct the exact sparse target/draft supports and normalization
    needed for `max(p-q,0)`?
16. May rows with different top-k/top-p/temperature/seed/counter safely share
    one graph and route class when buffers are sized to the maximum?
17. Are `StepOutput.v2` fields sufficient to prove exact counter
    transactionality, accepted EOS, residual versus bonus provenance, and
    four-rank consensus?
18. Does retrying a failed immutable step with the same counter create any
    externally visible duplicate draw or token?
19. Is including request ID only in the trace, not RNG entropy, the correct
    seeded-reproducibility choice?
20. Does the full-vocabulary quality record capture enough information to
    distinguish RNG disagreement, filtering error, numerical KLD, and
    tie-adjacent greedy divergence?
21. Which current API, scheduler, StepInput, StepOutput, worker, metrics, and
    quality contracts must version atomically?
22. Is the 16-item CPU proof matrix sufficient before a CUDA sampling route
    can launch?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Withhold
the token unless every blocker and major is resolved. State separately
whether:

- the design may amend `StepInput` and `StepOutput`;
- CPU implementation may begin;
- the backend must continue rejecting probabilistic requests;
- greedy-only SM120 kernel qualification may proceed independently;
- any finding changes MTP cache or page-transaction arithmetic; and
- no cn4 access or GPU launch is authorized by the verdict.
