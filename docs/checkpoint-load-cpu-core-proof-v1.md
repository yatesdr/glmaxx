# Checkpoint load CPU core proof v1

Date: 2026-07-30

Status: implementation candidate; adversarial review pending

GPU claim: none

## Candidate

The proved source commit is
`65614928c86cc4213ba5c3c492a75b7f0f27a0ab`.

| Input | SHA-256 |
|---|---|
| `crates/glm-engine/src/checkpoint_load.rs` | `eb120bd7365d16f137a44e9c2cd230600d99a06cfdec4e5757c1c67c1171e3c8` |
| `crates/glm-engine/src/startup.rs` | `1a5f1ac8aae94e6eb2aaf2cf4701dfc290604013103eacc1423046211609a5fc` |
| `crates/glm-engine/src/lib.rs` | `6d6f710fd6aed79ecc42a085be68b307fc7366fdbe11f330f2bac2453ae4648e` |
| `docs/checkpoint-load-transaction-v1.md` | `79d9c376201f3540f247344c24c37dd7d819d629f10459899a73c15f8b27015f` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |

## Implemented boundary

The Rust engine now owns deterministic CPU definitions for:

- the exact 416-byte `RankSetLoadPlanHeader.v1`;
- four exact 248-byte rank entries;
- exact 64-byte tensor arena entries;
- the canonical plan preimage and domain-separated plan hash;
- process-common rank order, unique device and file identities, nonzero
  contract identities, fixed reader chunk size, and fixed staging-ring
  minima;
- checked primary, auxiliary, and codec-metadata destination intervals,
  including alignment, overflow, bounds, overlap, exact arena extent, and
  rank-local arena-layout hashing;
- exact 256-byte prepared-rank receipts and domain-separated receipt hashes;
- the four-rank prepared-set digest, one identical adoption command, four
  rank/generation-bound acknowledgements, and the final adopted-set digest;
- the rank-local `Allocated -> Staging -> Prepared -> Adopted` lifecycle with
  terminal, exactly-once abort accounting; and
- a non-cloneable execution permit that can be produced only after the
  coordinator validates all four adoption acknowledgements.

The planned streaming sink implements the existing `RankTensorSink`
boundary. It checks each streamed descriptor against the precomputed arena
entry, writes metadata exactly once, routes primary and auxiliary chunks to
their fixed offsets, rejects early auxiliary data, overruns, duplicate or
out-of-order tensor callbacks, descriptor drift, writer failure, and early
seal. Successful chunk callbacks allocate no sink-side memory.

`StartupCoordinator` now rejects the ordinary
`MemoryPlanned -> WeightsLoaded` transition. The production transition
requires an `AdoptedRankSetReceipt`; its digest must be zero before weight
adoption, identical on all ranks at adoption, and immutable through
`Healthy`. The existing CPU-only startup proof uses an internal mock-only
route and cannot be called by an external production coordinator.

The v1 validator rejects `FS_VERITY` for laboratory and serving profiles.
The reviewed contract leaves that restart route closed until a separate
implementation and provenance gate exists.

## Tests

The checkpoint-load module has twelve focused tests covering:

- byte-stable plan and receipt encodings;
- overlap, tail, capacity, alignment, rank, device, and semantic drift;
- exact uploaded byte totals;
- four-rank adoption and divergent acknowledgement rejection;
- lifecycle skip rejection and exactly-once abort;
- absence of an execution permit before global adoption;
- streamed plane routing, split chunks, and complete sealing;
- early auxiliary data, overrun, descriptor mismatch, arena mismatch, and
  injected writer failure; and
- fail-closed FS-verity posture.

The startup module additionally proves that:

- `WeightsLoaded` cannot be reached through ordinary stage consensus;
- a completed adoption receipt opens only that exact transition;
- an adoption digest cannot appear early; and
- the digest cannot diverge or change after adoption.

At the exact candidate commit, `scripts/local-checks.sh` passed:

- 309 Rust unit/integration tests with zero failures;
- workspace formatting;
- workspace Clippy and CUDA-FFI Clippy with warnings denied;
- CUDA-FFI host type checking;
- deterministic CPU, 135-case matrix, manifest, native-rank, memory-budget,
  ABI, engine, serving, and cache-lifecycle proof regeneration and byte
  comparison; and
- all 78 then-current review handoffs, with 0/59 configured result artifacts.

The external tokenizer proof was skipped because `GLMAXX_TOKENIZER_DIR` was
unset. The local host has no `nvcc`; no CUDA source was compiled or launched.

## Explicit remaining work

This result is the CPU core of the load transaction, not the complete C11
gate. It does not yet prove:

- construction of a production or laboratory load plan directly from four
  actual native-rank readers and their full semantic catalogs;
- the complete per-rank injected-fault matrix over real reader callbacks;
- pinned host allocation or a fixed CUDA event ring;
- H2D copy completion, asynchronous CUDA error observation, or device-memory
  content verification;
- owner-thread CUDA arena allocation/free or persistent rank-worker
  integration;
- a complete external four-rank native checkpoint proof;
- graph capture, KV initialization, collectives, or production `Healthy`;
- a small-checkpoint or fit-capable checkpoint smoke; or
- any SM120 correctness, capacity, quality, or performance result.

The next implementation gate is the native-reader-to-plan builder plus a
mock four-rank fault coordinator. After adversarial acceptance, the device
successor is a fixed pinned-host/event-ring writer owned by each persistent
SM120 rank thread.
