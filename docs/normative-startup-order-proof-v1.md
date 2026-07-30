# Normative startup order proof v1

Date: 2026-07-30

Implementation commit:
`4fc17f2204cac987f8f27eed61af279e3b446bcd`

Status: CPU prerequisite passed; independent review pending

GPU claim: none

## Corrected state machine

The retained `StartupCoordinator` previously used an obsolete seven-stage
mock:

```text
Cold -> ContextReady -> InventoryVerified -> WeightsLoaded
     -> MemoryProved -> GraphsReady -> CollectivesReady -> Healthy
```

That ordering contradicted the normative engine contract by loading weights
before the per-rank memory plan was proved. It also omitted distinct host
validation, topology, module, KV, and collective-vote gates.

The Rust state machine now exactly represents:

```text
Created
  -> HostValidated
  -> CudaContextsReady
  -> TopologyValidated
  -> ModulesReady
  -> MemoryPlanned
  -> WeightsLoaded
  -> GraphsCaptured
  -> KvReady
  -> CollectivesVoted
  -> Healthy
```

`Failed` remains terminal and is not part of the successful sequence.
`NORMATIVE_ORDER` is a public fixed array so later rank-executor and
checkpoint-load code can bind evidence to the same stage identity rather
than recreating a local ordering.

## Fail-closed behavior

The coordinator still advances only when exactly ranks 0–3 report the exact
next stage with identical nonzero process-immutable digests. Any rank count,
stage, digest, or worker error poisons the coordinator.

A distinguishing regression advances through `ModulesReady`, then submits
the obsolete `WeightsLoaded` transition. The coordinator rejects it with
`RankAgreement` and enters terminal `Failed`; it cannot silently skip
`MemoryPlanned` or recover.

The existing four-thread mock now traverses all ten successful transitions
before returning `Healthy`. Rank rejection and rank-local collective-route
divergence remain fail-closed.

## Verification

```text
cargo test -p glm-engine -p glm-serving --offline
cargo clippy -p glm-engine -p glm-serving --all-targets --offline -- -D warnings
./scripts/local-checks.sh
```

Results:

- 48 `glm-engine` tests passed;
- 41 `glm-serving` tests passed against the renamed health state;
- the complete workspace gate passed 295 Rust tests;
- workspace formatting and Clippy with warnings denied passed;
- CUDA-FFI host checks and deterministic proof regeneration passed; and
- 76 review handoffs were provenance-verified with 0 of 57 configured review
  results present.

The local host did not have `GLMAXX_TOKENIZER_DIR` set, so the pinned
tokenizer bundle proof was skipped. It also had no `nvcc`, so this run did
not compile or launch CUDA.

Implementation hashes:

```text
spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a

docs/checkpoint-load-transaction-v1.md
79d9c376201f3540f247344c24c37dd7d819d629f10459899a73c15f8b27015f

crates/glm-engine/src/startup.rs
54d41acc810c90cc49fe4acc0623b6a13bb2c09b72b2f8e5fb6615250ead2ddd
```

## Exclusions

This proves only the CPU coordinator's exact normative stage order and
fail-closed transition behavior. It does not implement the checkpoint load
transaction, allocate or upload a device arena, make a quarantined weight
handle executable, construct real CUDA contexts, capture graphs, initialize
KV, vote collectives, launch a kernel, or establish that a real engine has
reached `Healthy`.
