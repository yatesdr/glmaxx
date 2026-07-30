# Production punchlist

Date: 2026-07-29

Baseline audited: `4bf7bb5e817e01cc299058b56a488b35011fd79d`

Goal: complete GLM-5.2 serving on four RTX PRO 6000 Blackwell SM120 GPUs,
TP=4 over PCIe, with EXL3/NVFP4 hybrid weights, MTP0–6, 1M context, tiered
KV, concurrent serving, quality evidence, and matched benchmarks.

This is a blocking punchlist, not an optimization backlog. The engine is not
production-capable while any item below remains open.

State meanings:

- `PASS`: current hashed evidence proves the stated scope.
- `REVIEW`: implementation/design exists but its required adversarial token
  is withheld.
- `OPEN`: required implementation or evidence is missing.
- `AUTH`: work requires renewed cn4 operator authorization.

## P0 — correctness and execution

| ID | State | Required outcome | Current evidence / blocker | Next gate |
|---|---|---|---|---|
| C01 | REVIEW | Corrected NVFP4 manifest ABI, EXL3 source projection, and EXL3 warp-decode gates accepted and byte-bound to the source actually built | Corrective handoffs at `0edfc8d` have no acceptance tokens and the current profile has since changed. Prior manifest findings `m5` and `m7` have atomic-publication and bit-exact-scale CPU corrections awaiting review. Current-tree review-to-build binding is now specified at `60311cf` and awaits design review | Fable review of the v3 source-binding design and outstanding corrections, then implement and re-pin complete manifest/EXL3 gates |
| C02 | REVIEW | Immutable row input carries prompt IDs/positions, sampling tuple, seed/RNG, MTP posture, limits, and page-delta binding to all ranks | `docs/step-execution-io-v1.md` and handoff `9f9a828`; transaction amendment at `e7bc477` | Resolve Fable review, then implement CPU ABI and four-rank hash consensus |
| C03 | REVIEW | Scheduler, prefix leases, active KV IDs, rank uploads, commit, rollback, and ID quarantine form one transaction | `docs/serving-page-transaction-v1.md`; handoff `61eb0b9`. CPU corrections through `876e4ca` make scheduler completion, counted shared-prefix release, post-selection failure, successful event publication, multi-request terminal cleanup, cancellation cleanup, pending restore/admission rollback, backend admission/event ownership propagation, and active-sequence removal preflighted, fail-closed, retryable, or fail-stop as appropriate. Active table serving integration, private tails, rank uploads, and ID quarantine remain absent | Resolve the CPU transaction reviews, then integrate the active page table and bounded rank/page undo log |
| C04 | REVIEW | Active sequence page table and page-aligned C64/MTP6 arena arithmetic accepted | CPU implementations `3404e07` and `c33648a`; correction `876e4ca` makes late-failing sequence removal atomic and retryable in the clone-on-error CPU oracle. Separate handoffs await review | Fable page-table, cache-budget, and sequence-removal verdicts |
| C05 | REVIEW | Persistent rank workers own real SM120 contexts, streams, graph instances, weight arenas, KV arenas, and collective handles | Rust-owned executor candidate `b64cb6d` freezes normative startup, owner-thread RAII, deterministic arenas, two-phase load adoption, cooperative graph capture, stream/event/slab DAGs, route registry, receipts, and fatal drain; implementation remains the CPU mock/binding only. Correction `da46a30` makes the retained pool's outstanding bound track queued/running TP4 work through four-rank consensus after handle abandonment. Correction `1eb8e1c` prevents the retained constructor from publishing before exact four-rank thread readiness and cleans partial startup synchronously. Both reviews are pending | Fable review of `docs/fable-sm120-rank-executor-v1-handoff.md` and the retained worker corrections, then prerequisite ABI acceptance and CPU/mock lifecycle implementation |
| C06 | OPEN | NVFP4 block-scaled MMA executes routed FC1, SwiGLU, FC2, compaction, activation quantization, epilogues, empty/skewed experts, and all required M buckets | CUDA-core and CUTLASS compile controls exist; no accepted device launch or complete fused operator | Authorized M2 correctness matrix, then replace controls with final MMA kernels |
| C07 | OPEN | Direct EXL3 gate/up/down execution consumes pinned source bytes with no unmeasured repack | CPU source projection and compile control exist; no accepted device result | C01 acceptance, then actual-shape device correctness and inclusive timing |
| C08 | REVIEW | Complete GLM target layer implements embedding, absorbed MLA/indexer, norms, residuals, routing, experts, final head, pending logits, and exact TP/DCP reductions | Source-pinned execution candidate `83f5005` and adversarial handoff define the complete program; CPU ABI/reference and CUDA layer are intentionally absent | Fable target-layer design verdict, CPU proof/amendments, then one-layer TP4 replay after C06/C07 |
| C09 | REVIEW | Distributed greedy, bounded top-k/top-p, mass, residual, and bonus sampling execute without full-vocabulary gather | Contract candidate `7c71818` freezes vocabulary padding, filters, SplitMix counter draws, MTP proposal/accept/residual/bonus schedule, composite routes, and `StepOutput.v2`; implementation is intentionally absent | Fable review of `docs/fable-distributed-sampling-abi-v1-handoff.md`, then CPU proof |
| C10 | REVIEW | Real recurrent GLM draft layer supports MTP0–6 proposal, verification, commit/rollback, accepted EOS, residual, and bonus semantics | Source-derived candidate `fd80e16` specifies successor-aligned teacher sidecars, recurrent scratch, the one-token pipeline, q-state retention, and required ABI amendments; no CPU runner or model evidence exists | Fable review of `docs/fable-mtp-layer-execution-v1-handoff.md`, coordinated ABI amendments, CPU proof, then matched MTP0 equivalence |
| C11 | OPEN | Strict four-rank checkpoint loader maps a fit-capable rank set into immutable device arenas and reaches healthy startup | File-backed verification plus fixed complete tensor/source manifest authentication pass locally at `4bf7bb5`; corrected two-phase load candidate `4bb0708` and manifest-reader v2 both await review; no transaction implementation, CUDA sink, full-rank proof, or device residency | Fable load-transaction r2 and manifest-reader v2 reviews, CPU/mock transaction proof, then CUDA sink and small-checkpoint smoke |
| C12 | OPEN | API backend serves checkpoint outputs rather than CPU worker tokens | Bounded adapter at `4cf3a62`; CPU-only and greedy-only | Connect only after C02/C03/C05/C09 |

## P0 — quality

| ID | State | Required outcome | Current evidence / blocker | Next gate |
|---|---|---|---|---|
| Q01 | REVIEW | Target-only MTP0 logits match pinned reference through prefill/decode with full-vocabulary per-position retention | `docs/quality-acceptance-v1.md` freezes raw-logit identity, MPFR KLD, stable/tie classification, smoke and multi-window thresholds; handoff pins candidate `70222ab`; evaluator and model evidence are absent | Fable review of `docs/fable-quality-acceptance-v1-handoff.md`, evaluator CPU proof, then small-checkpoint target runner |
| Q02 | OPEN | Hybrid weight membership is selected from measured per-position quality and actual speed/capacity evidence | Immutable policy machinery exists; checked profile remains `conversion_allowed=false` | Calibrate EXL3/NVFP4/protected candidates after kernel evidence |
| Q03 | REVIEW | Downstream reasoning, coding, tool/JSON, repetition, and termination suite passes against pinned control | Quality candidate `70222ab` freezes the statistical gates; source candidate `83fb374` pins public tasks; generated-corpus candidate `27fa48e` now makes the 3,500 JSON/repetition/retrieval/termination cases byte- and token-implementable, but all three reviews, generator/materializer/evaluator code, gated multilingual hashes, tokenized windows, and model results remain absent | Fable quality, source, and `docs/fable-generated-quality-corpus-v1-handoff.md` reviews, then implement and prove the CPU materializer/evaluator before Q01 model execution |
| Q04 | OPEN | Frozen and randomized retrieval pass through the documented context limit | KV CPU arithmetic is not model retrieval | Execute model context-band matrix through 1,048,576 positions |
| Q05 | REVIEW | MTP1–6 output is equivalent to matched MTP0 under reviewed contract, with per-position acceptance/probability evidence | Quality candidate `70222ab` freezes per-depth stable/tie, full-vocabulary KLD, top-two error, task, and performance-enablement gates; no real draft execution exists | Fable quality review, C10, then matched greedy and stochastic evidence |

## P0 — capacity, cache, and recovery

| ID | State | Required outcome | Current evidence / blocker | Next gate |
|---|---|---|---|---|
| K01 | PASS | CPU KV/page arithmetic represents one 1,048,576-token MTP-capable sequence with balanced DCP4 ownership and explicit slack | `glm-cache` tests plus budget/profile at `c33648a` | Preserve while integrating C03 |
| K02 | OPEN | Real target/indexer/draft KV uses preallocated HBM arenas and qualified asynchronous HBM↔DRAM transfers | CPU byte-owning residency only | C03/C05, pinned host memory and CUDA transfer streams |
| K03 | REVIEW | Qualified DRAM↔NVMe restore/eviction works under pressure without blocking decode | Contract candidate `69895e0` defines aligned 493/501-block direct extents, registered-buffer io_uring ownership, restore dedup/cancel, shared catalog epochs, tier scheduling, segment cleaning, and matched decode-isolation evidence; implementation is intentionally absent. The retained workers use read-only snapshots, reject invalid live extents, append after physical EOF, and at `95683d8` retain quota through abandoned physical reads, but still block and retain private catalogs | Fable reviews of the direct-tier and retained-store correction handoffs, then direct-format/CPU state proof |
| K04 | REVIEW | Newly sealed runtime pages publish target plus draft sidecar durably and become reusable without restart | Contract candidate `d0a09d7` separates HBM/durable generations, publication leases, live catalog, incremental/restart index, and bounded writes; implementation is intentionally absent. The retained CPU path fail-stops uncertain writes, enforces one writer, and corrections through `eceee04` apply one no-write dedup/MTP-upgrade/collision matrix across prefix, direct rank residency, file append, and journal replay; dedicated reviews are pending | Fable reviews of the online-publication and retained-store correction handoffs, then the online CPU proof matrix |
| K05 | OPEN | Prefix cache survives restart, corruption, torn journal, cache thrash, pinned pressure, and DCP posture changes end to end | Candidate `d0ac1d3` adds a deterministic integrated CPU lifecycle proof. Corrections through `95683d8` bind restore identity, make admission/release transactions atomic, retain physical-operation quota after cancellation, fail-stop uncertain writes, reject multiple writers and same-key conflicts, retain exact revisions/MTP capability, reject journal/catalog corruption, and preserve append-only allocation. All await review, and no model/GPU/direct-I/O lifecycle exists | Preserve the CPU fixture and correction proofs while implementing accepted K03/K04 and C05 paths, then run the integrated cold/warm fault matrix |
| K06 | OPEN | One live 1M request is admitted and executed without false active-capacity accounting or tier thrash | CPU table/budget proof only | Full fit-capable checkpoint plus C02/C03/C05 |

## P1 — concurrency and service

| ID | State | Required outcome | Current evidence / blocker | Next gate |
|---|---|---|---|---|
| S01 | PASS | CPU scheduler supports bounded continuous batching, weighted tenant fairness, prefix-pending admission, and collective-safe cancellation | Scheduler/serving tests include `2f7d0ce` for retryable C64 completion, `14b97a2` for counted release, `2ff0ac1` for consuming every failed selection, `6535248` for fixed-capacity success publication plus all-or-nothing multi-user terminal/cancellation cleanup, `bfbe7f4` for retryable pending restore/admission rollback and pending-ID exclusion, `3ab3110` for retained-admission ownership, `0f0dd21` for event-cancellation ownership plus fail-stop drain, and `da46a30` for physical TP4-step quota ownership after response abandonment. All dedicated Fable handoffs await review | Resolve the CPU transaction reviews, then re-prove with the real executor |
| S02 | REVIEW | Bounded HTTP-to-coordinator adapter, tenant ownership, stop-safe streaming, slow-client isolation, and structured fatal drain accepted | Candidate `8aaef8e`; combined v2 handoff pending. Corrections `3ab3110` and `0f0dd21` prevent retained admission rollback and event cancellation rejection from being ignored or misclassified as request-local success/failure. Correction `a7b1cc9` dispatches exact cancellation when initial SSE headers fail, enforces an exact chunk-independent header cap, rejects already-buffered trailing bytes, and fails closed if socket bounds cannot be installed. Correction `20c773c` joins all partial connection workers before worker/accept spawn failure returns and exposes cleanup panics. All dedicated handoffs await review | Adversarial verdicts and fixes |
| S03 | REVIEW | Host request/step observability has correct clocks, counts, MTP ordinals, graph routes, no metric-recording allocation, and consistent concurrent lifecycle totals | Metrics candidate `9607aa0`; backend concurrency delta `8aaef8e`; combined v2 handoff pending | Adversarial verdict and fixes |
| S04 | REVIEW | Exact probabilistic parameters and deterministic seed/RNG state reach rank execution and responses | Sampling/RNG candidate `7c71818` plus pending `StepInput.v1`; backend correctly rejects non-greedy | Accept both contracts, implement CPU ABI/consensus, then remove fail-closed rejection |
| S05 | REVIEW | Nonblocking network transport sustains target concurrency with bounded memory | Contract candidate `3608a03` defines sharded Linux epoll/eventfd reactors, connection generations, off-reactor admission, lossless completion wakeups, bounded output, and shutdown/fault evidence; implementation is intentionally absent | Fable review of `docs/fable-nonblocking-http-transport-v1-handoff.md`, then CPU parser/reactor proof |
| S06 | REVIEW | Admission enforces per-tenant queued-token, resident-KV, and context-band limits | Contract candidate `7e810c4` defines a single permit ledger, authenticated ingress, global-physical/tenant-logical charges, restore/step/offload transactions, exact 1M/page-slack arithmetic, and fatal cleanup; implementation is intentionally absent | Fable review of `docs/fable-tenant-resource-quotas-v1-handoff.md`, then CPU proof |
| S07 | OPEN | Sustained multi-user, cache-thrash, cancellation, rank-fault, and slow-client tests pass | CPU unit schedules only | Load/fault harness against real service |

## P1 — hardware evidence and performance

| ID | State | Required outcome | Current evidence / blocker | Next gate |
|---|---|---|---|---|
| H01 | OPEN | Re-inventory cn4 and reproduce the current source/toolchain/test state without disturbing other workloads | Final read-only inventory found 4×SM120 and an existing four-rank vLLM allocation; no GLMAXX process or launch occurred, `/home/derek/glmaxx` was not a Git worktree, and cn4 was released | After that workload and renewed authorization, pin source/toolchain/container state before any launch |
| H02 | AUTH | Actual-shape NVFP4 and EXL3 correctness/performance matrix on all required row buckets | Prior records prove `sm_120f` cubins and ABI only | H01 plus C01 |
| H03 | AUTH | PCIe TP/DCP routes measured pairwise and collectively on the actual topology | Route compiler is CPU-only | H01, then NCCL controls and deterministic alternatives |
| H04 | OPEN | One complete sparse-layer TP4 replay matches reference activations/logits | No replay artifact | C06–C08 and H03 |
| H05 | OPEN | Graph-captured prefill/decode/MTP buckets pass repeated correctness and timing | CPU candidate `9bdb208` now chunks configured prefill work to the highest-work legal captured entry instead of stalling on a wider uncaptured shape. Design candidate `9b04652` addresses both discovered ABI blockers: the unique-key collision for multiple prefill chunks and the incorrect global 448-row cap that rejects the planned 3,072-row control. No v2 implementation or CUDA graph exists | Fable reviews of `docs/fable-prefill-captured-shape-v1-handoff.md` and `docs/fable-prefill-graph-profile-abi-v2-handoff.md`, then implement/prove v2 before C05/H02 capture/eager comparison |
| H06 | OPEN | Matched end-to-end matrix reports TTFT, ITL p50/p95/p99, throughput, useful-token throughput, capacity, failures, cold/warm prefix, context, concurrency, and MTP depth | Host telemetry candidate only | Complete correctness/quality gates first |
| H07 | OPEN | Pinned vLLM, SGLang, llama.cpp/ExLlama, BF16/FP8, and existing-runtime controls are run under matched precision/cache/batch/context | No matched result bundle | H06 harness and pinned control revisions |

## P2 — reproducibility and delivery

| ID | State | Required outcome | Current evidence / blocker | Next gate |
|---|---|---|---|---|
| D01 | PASS | Local format, tests, Clippy, FFI checks, deterministic fixtures, review provenance, and pinned tokenizer proof pass | Latest full local run: 264 Rust tests, including synchronous retained-HTTP partial-start cleanup and physical saturation evidence, synchronous exact four-rank startup/partial cleanup, physical TP4-step quota retention after response abandonment, exact retained-HTTP header bounds and streaming-header cancellation, physical restore-quota retention after timeout/abandonment and bounded drain, direct rank-residency collision rejection/no-op retention, live catalog integrity, journal-tail recovery/corruption, durable dedup/replay, atomic multi-rank registration, serving rollback/fatal cleanup, C64/MTP6 staging, captured-shape prefill, restore identity, HBM admission, and all prior CPU fixtures, passed; all 59 handoffs verified in the same full run; accepted-review classification still requires the exact candidate, every pinned input hash, any required result path, and a distinct review artifact; the external tokenizer was not rerun because its directory variable was unset, and its pinned fixture is unchanged | Keep green at every milestone |
| D02 | OPEN | Current cn4 environment record pins source, container, driver, firmware, topology, clocks, occupancy, toolchains, and commands | `docs/cn4-release-20260729.md` pins the final GPU/driver/topology/occupancy observation, but source, container, firmware, clocks, and toolchains were not established because another workload already occupied the host | H01 |
| D03 | OPEN | Exact build, conversion, deployment, serving, recovery, and benchmark commands reproduce production | Preparation scripts exist; no production server/deployment | Complete relevant implementation, then freeze commands |
| D04 | OPEN | Immutable results index covers every accepted CPU, GPU, quality, capacity, and benchmark artifact | Fail-closed v2 verifier proves candidate hashes for all 59 pinned handoffs, automatically ingests declared result paths when they appear, and rejects self-review or token-only acceptance artifacts that omit candidate/input attestations. The v3 current-tree binding design is pending review; most required result artifacts still do not exist | Accept and implement v3, then append only provenance-complete records and supply each review artifact explicitly |
| D05 | OPEN | Full-checkpoint conversion is allowed only after policy fit and quality gates pass | Profile correctly blocks conversion today | Q01/Q02 and measured HBM budget |

## External state

- cn4 GPU work is not currently authorized. Do not connect or launch until a
  new operator authorization is given. The final authorized inventory found
  a four-rank vLLM job already occupying all four GPUs.
- The user has stated that NVFP4 and EXL3 checkpoints already exist on the
  servers. No checkpoint download is required. Their current paths and hashes
  must be re-inventoried under H01; model bytes remain outside Git.
- Public quality-source revisions and the deterministic reasoning/coding/tool
  selections are pinned in the non-runnable
  `manifests/quality-corpus-sources-v1.json` design candidate. Generated
  corpora, gated FLORES+ content hashes, tokenized windows, the materialized
  corpus manifest, and the evaluator are still absent.
- Fable responses are required for every row marked `REVIEW`; their absence
  does not prevent independent CPU design or test work on unrelated rows.
