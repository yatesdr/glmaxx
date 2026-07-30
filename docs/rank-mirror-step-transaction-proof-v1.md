# Four-rank mirror/step transaction CPU proof v1

Date: 2026-07-29

Implementation commit:
`e1d51ce57da7db163b82697a568ab2751602d832`

Status: integrated CPU transaction passed; independent review pending

GPU claim: none

## Implemented transaction

`Tp4WorkerPool` now owns one persistent `PageTableMirror` on each of its four
rank threads. `ServingCoordinator` initializes all four from the same empty
bounded table and nonzero generation before it can publish a usable
coordinator.

Every active-table mutation is ordered through the same dispatcher as compute
steps:

1. admission constructs one canonical successor delta, preflights scheduler
   admission on a clone, receives four exact mirror acknowledgments, and only
   then publishes scheduler/table/sampling state;
2. prefill/decode/verify constructs one reservation successor and immutable
   `StepInput`, then shares the same `Arc` instances with all four rank
   threads;
3. each rank independently verifies the plan, schedule, input, and delta,
   atomically applies the delta to its persistent mirror, enters its explicit
   `execute_bound` implementation, and returns input/global/local/output
   digests;
4. the dispatcher checks the exact rank set, common input/global/output
   identity, and every rank's independently derived local digest;
5. decode/verify preflights scheduler publication, then applies a second
   acknowledged commit/rollback/removal delta; prefill's reservation is
   already its final page state; and
6. only after those receipts does the coordinator publish scheduler progress,
   page state, prefix release, prompt release, events, and generation.

The plan-only submit API is test-only. Non-test callers must use the bound
input/delta path.

## Failure behavior

A post-execution host preflight error applies an explicit successor delta from
the reserved mirror state back to the authoritative pre-step page state. The
host adopts that rollback generation before retrying terminal cleanup. The
late prefix-release regression proves this alignment indirectly by repairing
the injected prefix failure and then successfully removing all three active
sequences through further acknowledged deltas.

A rank execution, malformed output, missing rank, or consensus error closes
the entire worker generation. Host fail-stop cleanup does not submit a fake
second receipt to a dead rank set and returns the original worker error.

Initialization and standalone delta application refuse overlap with an
outstanding physical step. An uninitialized bound step, duplicate
initialization, stale generation, malformed delta, wrong local digest, or
rank-set mismatch is fatal.

## Exact input retention

Serving retains one canonical `StepSampling` per active request and the
canonical prompt token vector through final prefill. Batch construction
derives exact prompt slices, context/generation counts, configured/effective
MTP depths, limits, seed, and pre-step RNG counter.

The production API backend now materializes an omitted greedy seed as the
request ID and preserves an explicitly supplied greedy seed through
admission. Probabilistic admission remains disabled because `StepOutput`
does not yet return the reviewed final RNG counter.

The CPU mock's bound output hashes the input hash and request ID, ensuring the
bound path cannot accidentally regress to its legacy plan-only token
function. A dedicated custom executor independently checks the exact seed and
context delivered to all four rank threads.

## Gate result

`scripts/local-checks.sh` passed:

- 284 Rust tests with zero failures;
- workspace formatting;
- workspace Clippy with warnings denied;
- CUDA FFI type checks;
- deterministic CPU proof regeneration; and
- all 68 then-present review handoffs with 0/49 configured result artifacts.

The external tokenizer proof was skipped because `GLMAXX_TOKENIZER_DIR` was
unset. CUDA compilation was skipped because `nvcc` is not installed on this
CPU host.

Implementation hashes:

```text
crates/glm-engine/src/input.rs
c3d090429015030416f6c03ddb6fef2dfd569859ff6e0fcc05bcb2d6a163ffa2

crates/glm-engine/src/worker.rs
39a0c0b917921149869d2afc5d652815986bf776eca5ddc9b7abee41b4892652

crates/glm-engine/src/lib.rs
b3ca0da8e0e61f05a92a3b15bc9dc7822395545733ebbdc270c9ff1fb21d6a54

crates/glm-serving/src/lib.rs
b70cb901a8ef86545342771c09f285e44f9df8eb226cf728809e0aa4d7040a5b

crates/glm-serving/src/backend.rs
c1f9e9d06b44674a1d1d0ef3c24553a9ebe63e913805946bff7c2780233fe94b

crates/glm-cache/src/delta.rs
71ac2da15e869a6f2470c3551a7cd6ec4ff387850a23240e9a44ad96a538ff16
```

## Exclusions

The four mirrors contain validated CPU metadata, not CUDA-visible sequence
tables or KV payloads. No upload event, stream dependency, graph launch,
physical-ID reuse quarantine, fixed-allocation undo log, direct tier
transfer, checkpoint tensor, model output, quality result, live-tier capacity
result, or performance result is claimed.

The authoritative host page table and mirror application still clone for
rollback and allocate owned vectors/maps. `CACHE_ONLY` remains unresolved
because its reviewed `StepPlan` requires generation zero. Probabilistic
sampling remains fail-closed until final RNG counters and the full reviewed
sampling trace enter output consensus and atomic coordinator commit.
