# Rank-set load coordinator proof v1

Date: 2026-07-30

Status: implementation candidate; adversarial review pending

GPU claim: none

## Candidate

The proved source commit is
`c3314686bb371d11c6a563d8397cf1903d24a454`.

| Input | SHA-256 |
|---|---|
| `crates/glm-engine/src/checkpoint_load.rs` | `f674cad6d575fc4bed74730a9d1ffb0dd5d64e21afaa4cf92bf4fc04b54d375b` |
| `crates/glm-engine/src/lib.rs` | `06f668fa567057c895e29bd97515c58588347caac1ffa10c824bd6745ada867b` |
| `docs/native-rank-load-plan-proof-v1.md` | `19558f87ef912c8ead99c31cf0f1a1867dcc384ab79d8efbbee96f66abfe0e63` |
| `docs/checkpoint-load-cpu-core-proof-v1.md` | `a3cbd93be0b7f131d98d996601c75e653764ec429839f19e2c26835fa4bd20c1` |
| `docs/checkpoint-load-transaction-v1.md` | `79d9c376201f3540f247344c24c37dd7d819d629f10459899a73c15f8b27015f` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

## Implemented boundary

`RankSetLoadCoordinator` is the process-wide state machine for one four-rank
load attempt. Rank threads retain their own quarantined allocation lifecycle;
the coordinator retains only the reviewed plan, expected owner allocation
generations, prepared receipts, adoption acknowledgements, and terminal
decision.

The coordinator has exactly four states:

```text
Preparing -> Adopting -> Adopted
     \           \          \
      +-----------+-----------> Aborted
```

During `Preparing`, it accepts one plan-valid prepared receipt for each rank
zero through three. A receipt must carry the owner generation registered when
the attempt began, so a cryptographically valid receipt from an older arena
generation cannot be replayed. It emits the common `ADOPT` command only after
all four exact receipts construct a valid `PreparedRankSet`.

During `Adopting`, it accepts one acknowledgement per rank. Each
acknowledgement must bind the same plan, prepared-set digest, rank, and
registered owner generation. It emits the `AdoptedRankSetReceipt` only after
all four acknowledgements pass `PreparedRankSet::complete_adoption`.
Execution permits remain rank-local and still require this final receipt.

Any rank-reported reader, writer, receipt, or adoption failure transitions
the whole attempt to terminal `Aborted`. So do a duplicate receipt, duplicate
acknowledgement, stale owner generation, malformed rank, or phase violation.
The returned abort command is process-common and binds the plan plus a
nonzero load-attempt generation. Later reports return the same abort command;
no rank-local retry, profile substitution, or partial-adoption route exists.
The coordinator erases retained receipts and acknowledgements on abort and
can no longer return an adopted receipt.

## Tests

The focused CPU matrix proves:

- four exact prepared receipts are required before `ADOPT`;
- four exact adoption acknowledgements are required before completion;
- all four rank-local lifecycles can produce execution permits only after the
  coordinator returns the final adopted receipt;
- a preparation failure at each rank position zero through three emits the
  same process-wide abort route and every allocated/staging/prepared lifecycle
  becomes terminal;
- an adoption failure at each rank position zero through three aborts both
  still-prepared and already-adopted rank-local states, without a final
  receipt;
- stale owner allocation generations are rejected;
- duplicate prepared receipts and duplicate acknowledgements are terminal;
- zero attempt or owner generations cannot start a coordinator; and
- each lifecycle produces its physical cleanup obligation exactly once in
  the mock boundary.

At exact candidate `c3314686bb371d11c6a563d8397cf1903d24a454`,
`scripts/local-checks.sh` passed:

- 318 Rust unit/integration tests with zero failures;
- workspace formatting;
- workspace Clippy and CUDA-FFI Clippy with warnings denied;
- CUDA-FFI host type checking;
- deterministic CPU, 135-case matrix, manifest, native-rank, memory-budget,
  ABI, engine, serving, and cache-lifecycle proof regeneration and byte
  comparison; and
- all 80 then-current review handoffs, with 0/61 configured result artifacts.

The external tokenizer proof was skipped because `GLMAXX_TOKENIZER_DIR` was
unset. The local host has no `nvcc`; no CUDA source was compiled or launched.

## Explicit remaining work

This state machine does not spawn rank threads, impose a wall-clock timeout,
or physically synchronize and destroy an arena. Its abort command identifies
the attempt, but a later persistent-rank integration must return four
post-synchronization cleanup acknowledgements before the process can report
cleanup complete.

It also does not implement pinned host memory, CUDA events or streams, H2D
copies, device-content verification, complete rank-file streaming,
small-checkpoint smoke, production health, SM120 execution, capacity,
quality, or performance.

The next boundary is the rank-owned pinned staging/event ring and
quarantined device-arena writer. Its abort path must synchronize outstanding
DMA before freeing ring slots, events, streams, or allocations.
