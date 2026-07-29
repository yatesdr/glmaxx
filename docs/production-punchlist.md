# Production punchlist

Date: 2026-07-29

Baseline audited: `59e11e5b14737020f72659b8a49d8c82982deba8`

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
| C01 | REVIEW | Corrected NVFP4 manifest ABI, EXL3 source projection, and EXL3 warp-decode gates accepted | Corrective handoffs at `0edfc8d`; no acceptance tokens are present | Fable re-review of the three r2 handoffs |
| C02 | REVIEW | Immutable row input carries prompt IDs/positions, sampling tuple, seed/RNG, MTP posture, limits, and page-delta binding to all ranks | `docs/step-execution-io-v1.md` and handoff `9f9a828`; transaction amendment at `e7bc477` | Resolve Fable review, then implement CPU ABI and four-rank hash consensus |
| C03 | REVIEW | Scheduler, prefix leases, active KV IDs, rank uploads, commit, rollback, and ID quarantine form one transaction | `docs/serving-page-transaction-v1.md`; handoff `61eb0b9` | Resolve active-page and transaction reviews, then implement bounded undo log |
| C04 | REVIEW | Active sequence page table and page-aligned C64/MTP6 arena arithmetic accepted | CPU implementations `3404e07` and `c33648a`; separate handoffs, tokens withheld | Fable page-table and cache-budget verdicts |
| C05 | OPEN | Persistent rank workers own real SM120 contexts, streams, graph instances, weight arenas, KV arenas, and collective handles | Rust worker boundary exists; `docs/sm120-rank-runtime.md` is binding-only | Implement qualified CUDA `RankExecutor` after C01/C02/C03 |
| C06 | OPEN | NVFP4 block-scaled MMA executes routed FC1, SwiGLU, FC2, compaction, activation quantization, epilogues, empty/skewed experts, and all required M buckets | CUDA-core and CUTLASS compile controls exist; no accepted device launch or complete fused operator | Authorized M2 correctness matrix, then replace controls with final MMA kernels |
| C07 | OPEN | Direct EXL3 gate/up/down execution consumes pinned source bytes with no unmeasured repack | CPU source projection and compile control exist; no accepted device result | C01 acceptance, then actual-shape device correctness and inclusive timing |
| C08 | OPEN | Complete GLM target layer implements attention/indexer, norms, residuals, routing, experts, and exact TP/DCP reductions | CPU sparse MoE references exist; no complete CUDA layer | One-layer TP4 replay after C06/C07 |
| C09 | REVIEW | Distributed greedy, bounded top-k/top-p, mass, residual, and bonus sampling execute without full-vocabulary gather | Contract candidate `7c71818` freezes vocabulary padding, filters, SplitMix counter draws, MTP proposal/accept/residual/bonus schedule, composite routes, and `StepOutput.v2`; implementation is intentionally absent | Fable review of `docs/fable-distributed-sampling-abi-v1-handoff.md`, then CPU proof |
| C10 | OPEN | Real recurrent GLM draft layer supports MTP0–6 proposal, verification, commit/rollback, accepted EOS, residual, and bonus semantics | CPU transition/output metadata exists; no real draft execution | C02/C03, real draft-layer runner, then matched MTP0 equivalence |
| C11 | OPEN | Strict four-rank checkpoint loader maps a fit-capable rank set into immutable device arenas and reaches healthy startup | Strict ingest/converter and startup consensus exist; no full device residency | Small-checkpoint smoke before any full conversion |
| C12 | OPEN | API backend serves checkpoint outputs rather than CPU worker tokens | Bounded adapter at `4cf3a62`; CPU-only and greedy-only | Connect only after C02/C03/C05/C09 |

## P0 — quality

| ID | State | Required outcome | Current evidence / blocker | Next gate |
|---|---|---|---|---|
| Q01 | REVIEW | Target-only MTP0 logits match pinned reference through prefill/decode with full-vocabulary per-position retention | `docs/quality-acceptance-v1.md` freezes raw-logit identity, MPFR KLD, stable/tie classification, smoke and multi-window thresholds; handoff pins candidate `70222ab`; evaluator and model evidence are absent | Fable review of `docs/fable-quality-acceptance-v1-handoff.md`, evaluator CPU proof, then small-checkpoint target runner |
| Q02 | OPEN | Hybrid weight membership is selected from measured per-position quality and actual speed/capacity evidence | Immutable policy machinery exists; checked profile remains `conversion_allowed=false` | Calibrate EXL3/NVFP4/protected candidates after kernel evidence |
| Q03 | REVIEW | Downstream reasoning, coding, tool/JSON, repetition, and termination suite passes against pinned control | Quality candidate `70222ab` freezes minimum item counts, paired noninferiority, behavior failures, retrieval bands, and multiple-comparison correction; dataset/evaluator manifests remain absent | Fable quality review, then pin datasets/evaluator and run after Q01 |
| Q04 | OPEN | Frozen and randomized retrieval pass through the documented context limit | KV CPU arithmetic is not model retrieval | Execute model context-band matrix through 1,048,576 positions |
| Q05 | REVIEW | MTP1–6 output is equivalent to matched MTP0 under reviewed contract, with per-position acceptance/probability evidence | Quality candidate `70222ab` freezes per-depth stable/tie, full-vocabulary KLD, top-two error, task, and performance-enablement gates; no real draft execution exists | Fable quality review, C10, then matched greedy and stochastic evidence |

## P0 — capacity, cache, and recovery

| ID | State | Required outcome | Current evidence / blocker | Next gate |
|---|---|---|---|---|
| K01 | PASS | CPU KV/page arithmetic represents one 1,048,576-token MTP-capable sequence with balanced DCP4 ownership and explicit slack | `glm-cache` tests plus budget/profile at `c33648a` | Preserve while integrating C03 |
| K02 | OPEN | Real target/indexer/draft KV uses preallocated HBM arenas and qualified asynchronous HBM↔DRAM transfers | CPU byte-owning residency only | C03/C05, pinned host memory and CUDA transfer streams |
| K03 | REVIEW | Qualified DRAM↔NVMe restore/eviction works under pressure without blocking decode | Contract candidate `69895e0` defines aligned 493/501-block direct extents, registered-buffer io_uring ownership, restore dedup/cancel, shared catalog epochs, tier scheduling, segment cleaning, and matched decode-isolation evidence; implementation is intentionally absent | Fable review of `docs/fable-direct-tier-io-v1-handoff.md`, then direct-format/CPU state proof |
| K04 | REVIEW | Newly sealed runtime pages publish target plus draft sidecar durably and become reusable without restart | Contract candidate `d0a09d7` separates HBM/durable generations, publication leases, live catalog, incremental/restart index, and bounded writes; implementation is intentionally absent | Fable review of `docs/fable-online-prefix-publication-v1-handoff.md`, then CPU proof matrix |
| K05 | OPEN | Prefix cache survives restart, corruption, torn journal, cache thrash, pinned pressure, and DCP posture changes end to end | Store unit tests cover several faults; no model/GPU lifecycle | Integrated fault matrix with cold/warm evidence |
| K06 | OPEN | One live 1M request is admitted and executed without false active-capacity accounting or tier thrash | CPU table/budget proof only | Full fit-capable checkpoint plus C02/C03/C05 |

## P1 — concurrency and service

| ID | State | Required outcome | Current evidence / blocker | Next gate |
|---|---|---|---|---|
| S01 | PASS | CPU scheduler supports bounded continuous batching, weighted tenant fairness, prefix-pending admission, and collective-safe cancellation | Scheduler/serving tests through `9607aa0` | Re-prove with real executor |
| S02 | REVIEW | Bounded HTTP-to-coordinator adapter, tenant ownership, stop-safe streaming, slow-client isolation, and structured fatal drain accepted | Candidate `8aaef8e`; combined v2 handoff pending | Adversarial verdict and fixes |
| S03 | REVIEW | Host request/step observability has correct clocks, counts, MTP ordinals, graph routes, no metric-recording allocation, and consistent concurrent lifecycle totals | Metrics candidate `9607aa0`; backend concurrency delta `8aaef8e`; combined v2 handoff pending | Adversarial verdict and fixes |
| S04 | REVIEW | Exact probabilistic parameters and deterministic seed/RNG state reach rank execution and responses | Sampling/RNG candidate `7c71818` plus pending `StepInput.v1`; backend correctly rejects non-greedy | Accept both contracts, implement CPU ABI/consensus, then remove fail-closed rejection |
| S05 | REVIEW | Nonblocking network transport sustains target concurrency with bounded memory | Contract candidate `3608a03` defines sharded Linux epoll/eventfd reactors, connection generations, off-reactor admission, lossless completion wakeups, bounded output, and shutdown/fault evidence; implementation is intentionally absent | Fable review of `docs/fable-nonblocking-http-transport-v1-handoff.md`, then CPU parser/reactor proof |
| S06 | REVIEW | Admission enforces per-tenant queued-token, resident-KV, and context-band limits | Contract candidate `7e810c4` defines a single permit ledger, authenticated ingress, global-physical/tenant-logical charges, restore/step/offload transactions, exact 1M/page-slack arithmetic, and fatal cleanup; implementation is intentionally absent | Fable review of `docs/fable-tenant-resource-quotas-v1-handoff.md`, then CPU proof |
| S07 | OPEN | Sustained multi-user, cache-thrash, cancellation, rank-fault, and slow-client tests pass | CPU unit schedules only | Load/fault harness against real service |

## P1 — hardware evidence and performance

| ID | State | Required outcome | Current evidence / blocker | Next gate |
|---|---|---|---|---|
| H01 | AUTH | Re-inventory cn4 and reproduce the current source/toolchain/test state without disturbing other workloads | cn4 was explicitly released after prior compile-only work; no current GPU authorization | Obtain new authorization, inventory before any launch |
| H02 | AUTH | Actual-shape NVFP4 and EXL3 correctness/performance matrix on all required row buckets | Prior records prove `sm_120f` cubins and ABI only | H01 plus C01 |
| H03 | AUTH | PCIe TP/DCP routes measured pairwise and collectively on the actual topology | Route compiler is CPU-only | H01, then NCCL controls and deterministic alternatives |
| H04 | OPEN | One complete sparse-layer TP4 replay matches reference activations/logits | No replay artifact | C06–C08 and H03 |
| H05 | OPEN | Graph-captured prefill/decode/MTP buckets pass repeated correctness and timing | Compile-only graph controls | C05/H02, then capture/eager comparison |
| H06 | OPEN | Matched end-to-end matrix reports TTFT, ITL p50/p95/p99, throughput, useful-token throughput, capacity, failures, cold/warm prefix, context, concurrency, and MTP depth | Host telemetry candidate only | Complete correctness/quality gates first |
| H07 | OPEN | Pinned vLLM, SGLang, llama.cpp/ExLlama, BF16/FP8, and existing-runtime controls are run under matched precision/cache/batch/context | No matched result bundle | H06 harness and pinned control revisions |

## P2 — reproducibility and delivery

| ID | State | Required outcome | Current evidence / blocker | Next gate |
|---|---|---|---|---|
| D01 | PASS | Local format, tests, Clippy, FFI checks, deterministic fixtures, review provenance, and pinned tokenizer proof pass | Full gate at `39af260`: 216 Rust tests and 20 candidate-based handoffs verified; latest implementation is `59e11e5` | Keep green at every milestone |
| D02 | OPEN | Current cn4 environment record pins source, container, driver, firmware, topology, clocks, occupancy, toolchains, and commands | Historical compile-only records exist; current state unavailable after release | H01 |
| D03 | OPEN | Exact build, conversion, deployment, serving, recovery, and benchmark commands reproduce production | Preparation scripts exist; no production server/deployment | Complete relevant implementation, then freeze commands |
| D04 | OPEN | Immutable results index covers every accepted CPU, GPU, quality, capacity, and benchmark artifact | Fail-closed verifier at `59e11e5` proves candidate hashes for all 20 pinned handoffs and exact token state for supplied reviews; most required result artifacts still do not exist | Append only provenance-complete records and supply each review artifact explicitly |
| D05 | OPEN | Full-checkpoint conversion is allowed only after policy fit and quality gates pass | Profile correctly blocks conversion today | Q01/Q02 and measured HBM budget |

## External state

- cn4 GPU work is not currently authorized. Do not connect or launch until a
  new operator authorization is given.
- The user has stated that NVFP4 and EXL3 checkpoints already exist on the
  servers. No checkpoint download is required. Their current paths and hashes
  must be re-inventoried under H01; model bytes remain outside Git.
- Quality datasets/evaluator revisions are not yet pinned in this repository.
- Fable responses are required for every row marked `REVIEW`; their absence
  does not prevent independent CPU design or test work on unrelated rows.
