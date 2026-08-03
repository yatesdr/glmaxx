# Fable handoff: resident-weight runtime generation v1

Date: 2026-08-03

Status: adversarial design review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4, inspect external evidence, run CUDA, or modify any
runtime resource for this review. This is a CPU/design gate only.

Review candidate commit:
`9710c0db7245592a17084b65efe041010612bcfa`

Required result path:
`fable-resident-weight-runtime-generation-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`resident-weight-runtime-generation-v1-design-accepted`

## Provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Withhold the token for any mismatch or incomplete input.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/resident-weight-runtime-generation-v1.md` | `ec76be8698ab53480ede07044bdfa73c8ccd9bbf391771bc728569c3023ef8b1` |
| `docs/cn4-experiment-isolation-v1.md` | `aab1dc4860fd2dde21e19b067b211f842387436d3d92a48b2fb31037a945d735` |
| `docs/checkpoint-load-transaction-v1.md` | `bc3c938f488bdcbf002c788ce9c5ac493addfe81866f39875d583c4312842ccf` |
| `docs/sm120-rank-executor-v1.md` | `e97c54b865ed50c40ff8b15f6580d0edc18dbd0783135bc1c17d11cc19986fd4` |
| `docs/sm120-rank-executor-v1-r2.md` | `4f40ea7652858b4cebbe4093dc81149cb30aa26bedc69edef72fa627c987df89` |
| `docs/sm120-rank-runtime.md` | `908b8adf0e1fc230145c009db01c71e69437ab359c76a545031fd9157c1ceea9` |
| `docs/prefill-graph-profile-abi-v2.md` | `37154c9e31109acdf35a382c6be87b3a865e2b7f6ae8f801969526789dd41f91` |
| `docs/matched-runtime-control-v1.md` | `446e25396e7eabd2fce85aa848c70318f964b1a9a7cf02a4945acc9917c02bf8` |
| `crates/glm-engine/src/checkpoint_cuda.rs` | `0b5e411d68a61fa1a39ccb7cc6b36702b85b3d385098764fa2d33b18227efdbe` |
| `crates/glm-engine/src/checkpoint_load.rs` | `052198f4265ab2569eb19feb074c96b83daa34fa0566575e9517ec59f7ca5957` |
| `crates/glm-engine/src/native_worker.rs` | `6173e7575a5a994c9090476154621e742063282e3c17e3c87a77eaa2a30da4db` |
| `crates/glm-engine/src/worker.rs` | `3533f606400c8aa5c571caa360ba516abd69d39de0489b87be4658143a9bdc24` |
| `crates/glm-engine/src/startup.rs` | `1a5f1ac8aae94e6eb2aaf2cf4701dfc290604013103eacc1423046211609a5fc` |
| `crates/glm-engine/src/step.rs` | `4963a58da7c9c6bbed0fb57fb7ef56d90d1e0f09fe54da8cc02c35891f743359` |
| `crates/glm-engine/src/graph.rs` | `c85ca1aa52ba42294fc6a43524e8f70357523977d343e8c2f212787e7754cd22` |
| `crates/glm-engine/src/memory.rs` | `2131c999b6762a9b7e505cfe542c957877d95af4ee04056affa9d677156e9491` |
| `crates/glm-scheduler/src/lib.rs` | `5fd0c4506002c4da5679f1ca3bf96a880ca7b0b348d5f55ada26a2e06ae7ff4d` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

Run the complete CPU-only local gate and record its exit status:

```text
./scripts/local-checks.sh
```

## Required independent work

Do not accept the state machine by prose inspection alone. Independently:

1. construct the full rank-local transition table, including every invalid
   state/command pair, duplicate, stale epoch, skipped ordinal, timeout, and
   single-rank disagreement;
2. construct an interleaving model for four ranks, selected physical steps,
   queued requests, prepare, quiesce, commit, grace, rollback, retirement, and
   process poison, then search it for a mixed-generation step or publication;
3. rederive the maximum simultaneously live generation resources and prove the
   two-slot active/secondary scheme cannot overlap a third candidate;
4. separate user-owned allocations from CUDA-driver-owned module/graph bytes
   and determine whether the proposed before/after measurement and escrow can
   fail closed without claiming allocator control GLMAXX does not have;
5. trace current loader and owner capabilities to decide whether opens, reads,
   staging, and H2D can be made unreachable during reload and whether monotonic
   counters can prove zero weight traffic without rereading the arenas;
6. trace current step, MTP, scheduler, graph, and rank-worker identities to
   identify every field needed to bind one physical step to a common runtime
   epoch; and
7. compare the cold-start measurement boundary with the matched-control and
   cn4 isolation contracts, including page-cache posture and immutable vLLM
   controls.

## Required decisions

Answer every decision with an unqualified `YES` or `NO`:

1. Is the resident identity complete, deterministic, rank-ordered, and
   sufficient to bind immutable weight and metadata arenas without making
   process-local pointers part of its portable digest?
2. Is the canonical runtime manifest independently serializable with no float,
   key-order, unknown-field, path, or mutable-artifact ambiguity?
3. Is the allowed hot-compatible surface narrow enough that precision, model
   semantics, KV, sampling, batching, MTP, and collective posture cannot drift?
4. Are rank ownership and capability boundaries sufficient to prevent another
   thread or reload path from touching weights or a rank's CUDA state?
5. Is the two-slot active/secondary resource model complete, KV-preserving,
   and honest about user-owned allocations versus CUDA-driver-owned HBM?
6. Can declared ceilings, live HBM deltas, and fixed escrow reject every
   unbudgeted module/graph allocation without a third hidden slot?
7. Is the rank state machine total and fail-closed for every legal and illegal
   transition, duplicate, timeout, stale epoch, and rank disagreement?
8. Does prepare remain concurrent with old work while quiesce reaches an exact
   safe boundary with no live graph, stream, collective, or selected step?
9. Is four-rank commit atomic from the scheduler and request-publication point
   of view even though the owner threads acknowledge separately?
10. Is every target and MTP draft/verify operation in one physical step bound
    to exactly one common generation epoch on all ranks?
11. Is rollback permitted only before any candidate output or KV successor is
    externally visible, with all ambiguous or post-publication cases fatal?
12. Does the sixteen-step grace rule and ban on another prepare make retirement
    and slot reuse deterministic and bounded?
13. Can capability separation plus monotonic counters and immutable arena
    receipts prove zero model opens, reads, staging, and H2D across reloads?
14. Are cold start and hot reload measured as distinct paths with complete
    phase accounting, explicit cache posture, and matched immutable controls?
15. Does the cn4 path and evidence design remain isolated from ongoing vLLM
    work and require unique, non-overwriting, hash-complete records?
16. Are the CPU proof obligations exhaustive enough to precede implementation
    review and SM120 evidence without prematurely accepting a runtime claim?
17. Does the gate sequence obey the repository contracts and are all nonclaims
    accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, followed
by the independent models/derivations and all seventeen decisions. Only if
every decision is `YES`, attest the candidate commit and all eighteen exact
input hashes, then end with the requested token as the only bare acceptance
line.

Acceptance opens only the CPU state machine, manifest, resource ledger,
capability-counter, and exhaustive failure-proof implementation. It does not
accept CUDA reload, cn4 execution, real resident weights, quality, KV capacity,
cold-start, latency, throughput, or any performance claim.
