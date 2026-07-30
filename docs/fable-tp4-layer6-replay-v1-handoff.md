# Fable handoff: TP4 layer-6 replay and layer-7 reuse gate v1

Date: 2026-07-30

Status: adversarial M3 design review requested

Review candidate commit:
`4a1e3766d15440f63873b3c50203080121be0d7e`

Required result path:
`docs/reviews/fable-tp4-layer6-replay-v1.md`

Requested acceptance token, only if every blocker and major is resolved:
`tp4-layer6-replay-v1-design-accepted`

GPU, host, process, container, network, model, checkpoint, or storage
authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4, query its state, start or stop a process,
build, test, create a CUDA context, access a checkpoint, or launch work for
this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if needed. Hash every candidate input at review start and finish.
Any mismatch must withhold the token as stale.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `docs/manifest-source-audit-20260729.md` | `480bc583315b071f6af6aba2372400db6007e96c17ee1f49767b650a51290095` |
| `docs/tp4-layer6-replay-v1.md` | `70157b10753c7e043e48566e219cca0ca1e29f596c198ff7a153f576192bee66` |
| `docs/target-layer-execution-v1.md` | `7f16667d55a50441dbc81b7fe18285be5a6f0fb06c68ef008a40aad8f406f478` |
| `docs/nvfp4-fused-routed-moe-v1-r3.md` | `f60d3adb777321ed715ae465abcf20895dbbf470c20970855636ed9b2b3a4db0` |
| `docs/prefill-graph-profile-abi-v2.md` | `37154c9e31109acdf35a382c6be87b3a865e2b7f6ae8f801969526789dd41f91` |
| `docs/step-execution-abi-v3.md` | `1cde3bcabba0a0d861691b06ddb140cb64dfbefaab1129c8a04bc302c0ce609e` |
| `docs/sm120-rank-executor-v1-r2.md` | `4f40ea7652858b4cebbe4093dc81149cb30aa26bedc69edef72fa627c987df89` |
| `docs/kv-reconstruction-rounding-v2.md` | `1bf0e69b0920e0ed2e13e92a9a51648f9587ea9c756930d97ed7963542f0a2cd` |
| `docs/quality-acceptance-v1.md` | `3f87cd128b633d6812dce31fb6f3bfbd700debae587a32350e0cb46e24a6e1e9` |
| `docs/small-checkpoint-runner-v1-r2.md` | `3e7bb597981c86ed63190c67463911e3aeca192c6aa818b7e58132cac716993e` |
| `docs/production-punchlist.md` | `42f179f4e5111e4d540ac8efb54aa5b734497dc5845989bf3586f9fc6b82b142` |
| `docs/results-index.md` | `f06cdbe9a349e3d2d7ae4872bbbd07cbdb897e17889d51892e82fa14b257f8df` |
| `crates/glm-cache/src/attention.rs` | `662965eb0c7e9e22768ee7c95c849b403a0a0004a1c061fb98c996fdd9c4e89f` |
| `crates/glm-reference/src/routed_fc1.rs` | `55709be01710d08f5b13de22afdb24884d233cdfab1c5de132ad22edd304f40b` |
| `crates/glm-reference/src/routed_fc2.rs` | `4f34f5b89cd542f096269a7442da5289900d1e831e32fcf83462560dc410a40d` |
| `crates/glm-engine/src/step.rs` | `4963a58da7c9c6bbed0fb57fb7ef56d90d1e0f09fe54da8cc02c35891f743359` |
| `crates/glm-engine/src/graph.rs` | `c85ca1aa52ba42294fc6a43524e8f70357523977d343e8c2f212787e7754cd22` |
| `crates/glm-cuda/src/abi.rs` | `28905e69300a3a8c8105752ee9aaeb4d718cbe4387cab548139c257242ec68a4` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-tp4-layer6-replay-v1-handoff.md
git diff --check 4a1e3766d15440f63873b3c50203080121be0d7e^ \
  4a1e3766d15440f63873b3c50203080121be0d7e
```

The handoff and queue metadata are added after the candidate and are not
candidate inputs. This is a source-only design review. Do not connect to a
host, inspect a checkpoint, fetch weights, read model bytes, or reproduce a
proposed run.

## Review purpose

Determine whether the candidate turns M3 into an implementable, honest,
complete TP4 layer gate instead of letting isolated kernels, fixture-fed
routes, a truncated production plan, stale graph output, or an offline
full-logit gather masquerade as target-layer execution.

## Review boundary

Acceptance covers only the M3 fixture, replay-weight/program identities,
reference ladder, exact layer-6 and layer-7 boundaries, cache transaction,
offline continuation, repetition/fault matrix, timing controls, evidence,
and CPU-gate design.

Acceptance does not implement or accept a fixture, extractor, reference
runner, replay weights, CUDA ABI, kernel, graph, collective, device run,
M2/M3/M4 result, checkpoint, model logits, quality result, performance
result, serving path, or cn4 use. It does not pass H04.

## Required adversarial questions

1. Do all twenty-three candidate-input hashes match at review start and
   finish in a detached worktree?
2. Does the gate remain strictly after current-tree CPU proofs, actual-shape
   M2 operator gates, and measured TP4/DCP4 route qualification?
3. Does the operation manifest prove that layer 6 is the first layer both
   sparse-MoE and `FULL` for index group 3, with layer 7 a `SHARED` consumer?
4. Can M3 result bytes bootstrap, replace, or bypass any M2 result,
   checkpoint-load gate, laboratory manifest, or hardware authorization?
5. Are source tensors, conversion routes, native planes, protected tensors,
   codec/projection/layout IDs, and four rank-local spans bound separately
   and completely?
6. Is a dedicated non-serving `ReplayWeightSet` necessary, complete, and
   unable to masquerade as a `.g5n`, laboratory, or production checkpoint?
7. Recompute the 320-byte `ReplayProgram.v1` offsets and confirm every field
   is necessary, canonical, and nonoverlapping.
8. Does the M3-specific layer-record domain bind the exact target-program-v2
   16-byte records for layers 6–7 without becoming a truncated production
   `StepPlan`?
9. Can mode, row count/bucket, sequence bucket, transport, eager/captured
   posture, weights, graph, schedule, fixture, numerical policy, route, or
   capability be substituted without changing the replay-program digest?
10. Can `ReplayProgram` or `ReplayWeightHandle` reach production scheduling,
    `HEALTHY`, HTTP, prefix publication, M4, or any full target program?
11. Are the decode and prefill fixtures real, bounded, canonical, complete,
    and sufficient to exercise all four DCP owners and tentative current-row
    writes?
12. Does prefill select exactly one previously qualified route globally,
    compare both only when both are qualified, and avoid rank-local or
    unmeasured fallback?
13. Are `native_cpu` and `source_control` genuinely separate comparisons that
    distinguish kernel error from quantization error?
14. Can reference router IDs, expert routes, candidates, winners, partials,
    compaction, or cache successors leak into device inputs?
15. Does the primary program execute every layer-6 norm, MLA/indexer,
    DCP-attention, KV/indexer write, residual, router, routed/shared expert,
    compaction, SwiGLU, FC2, and TP-reduction phase in source order?
16. Are CKV and query-route candidate/winner semantics accurately
    distinguished, including no candidate exchange on CKV?
17. Does every rank consume one target/replay program, graph, route, and
    `CollectiveOp.v2` schedule with no local fallback or empty-owner choice?
18. Does layer 7 consume the actual graph-resident layer-6 winner generation,
    execute no indexer projection/key write/candidate exchange, and still run
    its complete attention/MLP/residual program?
19. Are layer-6/7 target-KV and layer-6-only indexer records compared both
    physically and numerically with exact owner/generation/page successors?
20. Can any replay cache state escape into serving, prefix lookup, eviction,
    another request, or another repetition?
21. Are the two layer-boundary injection continuations meaningful sensitivity
    tests, and are they clearly not full-model device logits or a quality
    pass?
22. Is the offline full-vocabulary evidence gather safely separated from the
    production no-full-logit-gather rule?
23. Are per-position logit/KLD/tie values retained rather than hidden behind
    aggregate hashes or means?
24. Do fresh logical generations and poison/overwrite checks reject stale
    eager, captured, cache, winner, collective, and completion state?
25. Are cold-process, cold-graph, cold-fixture, and warm labels exact and
    matched across both paths?
26. Does the fault matrix cover empty/skewed experts, record corruption,
    ownership/generation/causality, schedule divergence, pre/post-collective
    failure, asynchronous CUDA failure, timeout, and cleanup disagreement?
27. Is recoverable cleanup limited to provably synchronized resources, while
    collective/DMA/owner uncertainty remains process-fatal with no unsafe
    free or forged cleanup?
28. Do timing boundaries separate transfer, capture, kernels, collectives,
    launch/runtime, reference continuation, and whole-transaction time?
29. Are the matched controls truly matched, and does
    `CORRECT_BUT_REDESIGN_REQUIRED` prevent an unexplained layer regression
    from opening M4?
30. Does the required CPU proof cover the program encoding, real-fixture
    parser, two-layer native oracle, cache/winner lifetime, reference
    continuation, mutation/fault matrix, and bounded resources before CUDA?
31. Are every no-checkpoint, no-full-device-model, no-EXL3-without-repeat,
    no-serving, no-MTP, no-prefix/tier, no-1M, no-quality, and no-end-to-end
    performance nonclaim accurate?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately whether:

1. M3's gate position and anti-bypass rules are complete;
2. replay source/native weights and non-serving type state are exact;
3. `ReplayProgram.v1` and both hash domains are implementable and
   substitution-resistant;
4. both real fixtures are sufficient and fail closed;
5. the two-branch reference ladder separates implementation and quantization
   error;
6. layer 6 executes the complete target program and computes its own routes;
7. layer 7 proves generated-winner reuse without indexer work;
8. cache physical/numerical/successor comparison is complete;
9. offline full-vocabulary sensitivity is useful without weakening
   production distributed sampling;
10. cold/warm and eager/captured generation isolation is complete;
11. recoverable and process-fatal failure handling is safe;
12. timing and matched performance disposition are honest;
13. the CPU/mock gate is complete and correctly precedes CUDA; and
14. no implementation, device, checkpoint, full-model, quality, serving,
    capacity, or performance evidence is implied.

Only if all thirty-one questions and all fourteen statements are unqualified
`YES`, end with exactly one bare line containing the requested acceptance
token shown above.

Withhold for stale provenance, M2 bypass, source/native conflation, a fixture
route leak, partial program masquerade, a type escape, CKV/query confusion,
rank-local fallback, stale winner/cache/output acceptance, missing cache
comparison, production full-logit gather, aggregate-only quality evidence,
unsafe cleanup, an unfrozen performance envelope, incomplete CPU gates, or
any checkpoint/model/quality/serving/capacity/performance overstatement.
