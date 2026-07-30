# Fable handoff: deterministic small-checkpoint runner v1 r2

Date: 2026-07-30

Status: adversarial corrective M4 design review requested

Review candidate commit:
`16922e4c699b8145eb8d43455e5626b13679ea60`

Required result path:
`docs/reviews/fable-small-checkpoint-runner-v1-r2.md`

Requested acceptance token, only if every blocker and major is resolved:
`small-checkpoint-runner-v1-r2-design-accepted`

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
| `docs/small-checkpoint-runner-v1.md` | `f35c71debd3d74d22cda435b60d3a7485592f96e41889481cc479f2c283b75f7` |
| `docs/small-checkpoint-runner-v1-r2.md` | `3e7bb597981c86ed63190c67463911e3aeca192c6aa818b7e58132cac716993e` |
| `docs/nvfp4-laboratory-manifest-v1.md` | `8a0adb54dedfab1dba0afcf09579614ce567da92fda43134b0c404af5aafb0ee` |
| `docs/nvfp4-fused-routed-moe-v1-r3.md` | `f60d3adb777321ed715ae465abcf20895dbbf470c20970855636ed9b2b3a4db0` |
| `docs/target-layer-execution-v1.md` | `7f16667d55a50441dbc81b7fe18285be5a6f0fb06c68ef008a40aad8f406f478` |
| `docs/checkpoint-load-transaction-v1.md` | `bc3c938f488bdcbf002c788ce9c5ac493addfe81866f39875d583c4312842ccf` |
| `docs/sm120-rank-executor-v1-r2.md` | `4f40ea7652858b4cebbe4093dc81149cb30aa26bedc69edef72fa627c987df89` |
| `docs/prefill-graph-profile-abi-v2.md` | `37154c9e31109acdf35a382c6be87b3a865e2b7f6ae8f801969526789dd41f91` |
| `docs/step-execution-abi-v3.md` | `1cde3bcabba0a0d861691b06ddb140cb64dfbefaab1129c8a04bc302c0ce609e` |
| `docs/production-punchlist.md` | `747dbbd39a85637aca314a66731fa9afa7fded4ddbe10c8d68ee8f6716dd4bb4` |
| `docs/results-index.md` | `f4247c91f06ca700c0d6a37b501f6962fdb36533ec686c4e1956b9fdc93ab135` |
| `crates/glm-format/src/container.rs` | `2fbe22c55d481e40699a02be9929f9a964f50bc08199853772dd314c85ade47f` |
| `crates/glm-engine/src/checkpoint_load.rs` | `052198f4265ab2569eb19feb074c96b83daa34fa0566575e9517ec59f7ca5957` |
| `crates/glm-engine/src/worker.rs` | `3533f606400c8aa5c571caa360ba516abd69d39de0489b87be4658143a9bdc24` |
| `crates/glm-engine/src/startup.rs` | `1a5f1ac8aae94e6eb2aaf2cf4701dfc290604013103eacc1423046211609a5fc` |
| `crates/glm-engine/src/memory.rs` | `2131c999b6762a9b7e505cfe542c957877d95af4ee04056affa9d677156e9491` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-small-checkpoint-runner-v1-r2-handoff.md
git diff --check 16922e4c699b8145eb8d43455e5626b13679ea60^ \
  16922e4c699b8145eb8d43455e5626b13679ea60
```

The handoff and queue metadata are added after the candidate and are not
candidate inputs. This is a source-only design review. Do not connect to a
host, inspect a checkpoint, read model bytes, or reproduce a proposed run.

## Review purpose

Determine whether r2 makes the M4 small-checkpoint runner implementable
without:

- conflating source and native output;
- weakening the separate laboratory manifest/type state;
- creating CUDA resources in an impossible order;
- accepting stale warm-run output;
- forging cleanup after an unsafe owner/DMA failure; or
- turning a truncated layer-6-to-head control into a full-model claim.

## Review boundary

Acceptance covers only the r2 corrective design, including its amended
dependency, identity, transaction, execution, repetition, failure, evidence,
and CPU/mock-gate boundaries.

Acceptance does not implement or accept a format correction, manifest,
converter, budget, loader, checkpoint, fixture, kernel, graph, collective,
M2/M3/M4 result, target program, model output, quality result, capacity
result, performance result, or cn4 run. It does not pass H04.

The base v1 handoff is superseded and must not issue its token.

## Required adversarial questions

1. Do all twenty-one candidate-input hashes match at review start and finish
   in a detached worktree?
2. Are all six diagnosed integration defects real when the base is compared
   with the laboratory manifest, executor r2, and checkpoint transaction?
3. Does the dependency list prevent M4 from bootstrapping, replacing, or
   bypassing M2 actual-shape operator evidence or the complete M3 replay?
4. Are complete source, selected source, conversion, and four-rank native
   output identities distinct and substitution-resistant?
5. Are the five conversion routes closed, and is 2D-to-1D unavailable
   without its separately accepted conversion and per-position quality
   evidence?
6. Does the reference consume the exact native output planes rather than
   unconverted source values?
7. Re-derive 533 tensors, 1,982,245,376 file payload bytes, 65,536 file
   metadata bytes, 1,982,245,376 device weight bytes, 130,944 device metadata
   bytes, and 1,982,310,912 uploaded bytes.
8. Re-derive the 137,856-byte load-plan preimage and confirm the
   laboratory-specific plan hash domain is required.
9. Are header flags exactly 11, with no dual spelling or path around the
   protected-header correction?
10. Does every production/laboratory schema, budget, catalog, profile, type,
    permission, and plan-domain substitution fail before execution?
11. Is the corrected owner-thread order compatible with executor r2:
    persistent owners and contexts, deterministic arenas, collectives and
    reconciliation, weight streaming/adoption, graphs, fixture, then
    execution permit?
12. Can any quarantined arena, prepared receipt, partial adoption, laboratory
    weight handle, or pre-graph/cache state launch a model operation?
13. Is `LaboratoryWeightHandle` incapable of reaching a production weight
    handle, `HEALTHY`, HTTP, prefix namespace, production scheduler, or full
    checkpoint?
14. Does the exact truncated program cover layer-6 MLA/indexer/attention,
    target-KV/indexer writes, residuals, router/shared/routed experts, TP/DCP
    collectives, final norm/head, and distributed greedy without a
    full-vocabulary gather?
15. Are the pre-step and post-step page-table/KV/indexer identities sufficient
    to detect missing, stale, misowned, or mispublished cache writes?
16. Is directly applying final norm/head to layer-6 output clearly limited to
    a matched truncated reference rather than full-model logits?
17. Do monotonically increasing input/output/graph/collective/completion
    generations plus poison/overwrite checks prevent stale warm outputs or
    receipts from passing?
18. Are cold-load, cold-graph, cold-fixture, warm, eager, and captured controls
    mutually clear and matched?
19. Are five fresh child cycles and 100 warm captured repetitions per shape
    sufficient for the M4 determinism/resource-lifecycle scope without being
    mislabeled as a sustained-serving result?
20. Is the recoverable fault set correctly limited to failures for which
    owner-thread synchronization and exact cleanup remain provable?
21. Is the process-fatal set complete for owner loss, failed synchronization,
    ambiguous DMA, communicator abort, supervisor timeout, cleanup mismatch,
    and ownership loss?
22. Does the fatal path correctly forbid freeing possibly referenced
    resources, forging cleanup receipts, retrying one rank, or continuing the
    process?
23. Is the engine-owned resource ledger plus isolated child termination a
    sounder gate than demanding global free-HBM byte equality?
24. Does the evidence separate physical identities, state generations, cache
    successors, fault classes, cleanup/fatal receipts, and all timing
    boundaries without making a speed claim?
25. Does the required CPU/mock proof cover every identity, arithmetic,
    type-state, ordering, stale-generation, cache-output, recoverable-fault,
    fatal-child, and bounded-resource boundary before CUDA?
26. Are every no-implementation, no-checkpoint, no-cn4, no-full-model,
    no-quality, no-capacity, no-serving, no-MTP1–6, and no-performance
    nonclaim accurate?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately whether:

1. the six corrective findings are accurate and completely resolved;
2. gate prerequisites prevent M2/M3 bypass;
3. source, conversion, and native-output identities are complete;
4. laboratory file/device/plan arithmetic and domains are exact;
5. owner-thread resource creation, adoption, graph, fixture, and execution
   ordering is implementable;
6. type state prevents partial or laboratory state from reaching production;
7. the truncated layer-6-to-head and cache-write comparison boundary is
   numerically meaningful and truthfully limited;
8. cold/warm generation isolation rejects stale output;
9. recoverable cleanup and process-fatal safety are correctly separated;
10. the repetition/fault matrix is sufficient for M4;
11. the evidence schema is complete and makes no performance claim;
12. CPU/mock implementation remains correctly ordered behind review and
    before any CUDA M4 run; and
13. no implementation, checkpoint, device, full-model, quality, capacity,
    serving, or performance evidence is implied.

Only if all twenty-six questions and all thirteen statements are
unqualified `YES`, end with exactly one bare line containing the requested
acceptance token shown above.

Withhold for stale provenance, M2/M3 bypass, source/output conflation, a
conversion fallback, byte or domain drift, impossible owner ordering,
pre-adoption execution, a production type escape, stale warm output,
unverified cache writes, full-logit gather, unsafe cleanup, forged fatal
receipts, incomplete CPU gates, or any model/serving/quality/capacity/
performance overstatement.
