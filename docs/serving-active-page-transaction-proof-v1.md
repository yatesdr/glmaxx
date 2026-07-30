# Serving active-page transaction CPU proof v1

Date: 2026-07-29

Implementation and regression commit:
`f480ef179ec7088005b1dbcdc04be113c289974d`

Status: CPU serving integration passed; independent review pending

GPU claim: none

## Defect and corrected ownership boundary

`ServingCoordinator` previously advanced a sequence-table generation without
owning a `SequencePageTable`. Scheduler progress, prefix leases, and rank
worker submission could therefore proceed without reserving any bounded
target or draft page IDs. Capacity failure, tentative MTP rollback, and
terminal removal were proven only in a disconnected cache oracle.

The coordinator now requires `PageTableConfig` and owns exactly one active
table. Admission uses a cloned candidate table to attach the authoritative
restored `PrefixPageAttachment` records and any private cached positions
before scheduler admission. The candidate becomes visible only after both
preflights succeed. The prevalidated cached-token bypass is compiled only for
tests; production admission must obtain its prefix attachments through the
restore coordinator.

Serving rejects a request when `prompt_tokens + maximum_new_tokens` exceeds
1,048,576. This is a request-budget bound, not a claim that the configured
arena can fit every request; smaller arenas still fail closed at their exact
physical capacity.

## Step transaction

Before a selected batch reaches any rank worker, the coordinator:

1. clones the current active table;
2. checks every row's committed position against
   `scheduler.prompt_done + scheduler.generated`;
3. appends the exact scheduled prefill count, reserves one MTP0 position, or
   reserves `K + 1` MTP positions on the candidate; and
4. discards the whole candidate and fails every selected row if any late row
   or owner lacks capacity.

Rank execution and four-rank output consensus occur only after this
reservation succeeds. Prefill requires an empty output. Decode and verify
commit the exact consensus output count on the candidate. Event and prefix
release planning then preflight against that candidate before scheduler
completion. The final publication installs the active table before releasing
any prefix pin.

Worker, output, compile, or page-capacity failure never publishes the
candidate. The failed selected rows are removed from the last committed table
as part of terminal cleanup. Cancellation is applied at a collective-safe
boundary before another runnable batch is selected, so a continuously
runnable peer cannot indefinitely retain the cancelled sequence's active
mapping or prefix pin.

The host-visible generation advances once for each published admission,
successful step, cancellation cleanup, failure cleanup, or terminal removal.
It does not yet represent the separate reserve/commit generations required
by the future rank-visible delta protocol.

## MTP tail bound

The scheduler previously selected the configured MTP depth even when fewer
than `K + 1` generation positions remained. That could waste verifier work
and made a 1M-boundary reservation fail before rank execution.

For each decode class it now selects the deepest captured verify depth no
greater than both the configured depth and `remaining_new_tokens - 1`. If no
such verify shape is captured, it uses the captured MTP0 decode graph. Rows
are batched only when this effective depth and sampling route agree.
Admission still requires the configured full-depth graph and a decode graph,
so a request cannot become stranded at its tail.

`mtp_depth_clamps_to_captured_tail_shape_and_falls_back_to_decode` proves a
depth-six request transitions through captured depth six, depth five, and
MTP0 as its remaining output budget contracts.

## Distinguishing regressions

`page_capacity_failure_is_atomic_and_never_reaches_rank_workers` uses one
256-token page per rank. Four 64-token prefill steps commit exactly 256
positions. The next one-token reservation fails on capacity, leaves the rank
call counter unchanged at 16, marks the request failed, and releases every
active page. The prior serving source would submit the fifth TP4 step because
it had no active capacity object to consult.

`exact_one_million_context_is_admitted_accounted_executed_and_released`
constructs the full CPU active-set boundary with 4,096 pages per rank:

- an MTP0 sequence starts at 1,048,575 committed positions with exactly
  4,096 target pages on every rank;
- its final decode executes on all four workers and terminal cleanup returns
  every page;
- an MTP6-capable sequence at the same position owns exactly 4,096 target and
  4,096 draft pages on every rank;
- its one-token tail uses MTP0 decode, executes, and releases both arenas; and
- a request budget of 1,048,577 positions is rejected before admission.

The test-only cached-position path avoids materializing a million-token prompt
or claiming durable payload transfer. It proves coordinator admission,
scheduler/page-position agreement, balanced capacity accounting, one final
TP4 CPU step, and terminal cleanup only.

Existing regressions additionally prove:

- real restored-prefix admission installs the exact authoritative attachment;
- a rank-output disagreement and worker-queue saturation release active
  mappings;
- MTP multi-user completion leaves both target and draft page counts at zero;
- accepted draft EOS commits without a fake target token; and
- cancellation removes one request before a runnable peer executes its next
  step.

## Gate result and exclusions

The local gate passed 273 Rust tests with zero failures, workspace formatting,
workspace Clippy with warnings denied, CUDA FFI type checks, deterministic
CPU proof regeneration, and all 65 then-present review handoff provenance
proofs.

Commands:

```text
cargo test --offline -p glm-scheduler
cargo test --offline -p glm-serving
cargo test --workspace --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-serving/src/lib.rs
d63508beaee3fdc5baed8d47f3435460c4f3143298c406d6e084babd02bf3da7

crates/glm-serving/src/backend.rs
a1dca883453d03e0e69a7896370f9d0b95cc1e7271443b6b91686a8d0d6e44e9

crates/glm-scheduler/src/lib.rs
5fd0c4506002c4da5679f1ca3bf96a880ca7b0b348d5f55ada26a2e06ae7ff4d

crates/glm-cli/src/main.rs
2af7739f311520b60601b18b2d14d3617320df535de24ecd310596add7ac3ff4

fixtures/cpu-serving-proof-v1.json
c95e1049bc52f8a8aaacd5a2d704008df9e8cfe72c8f3486982568adbaa7b47e

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, model, DRAM/NVMe
transfer, or model-quality execution was used.

This remains a clone-on-step, token-looping CPU oracle. It does not implement
the fixed-capacity undo log, page-granular hot path, rank page-table delta or
digest, device upload acknowledgment, removal-generation acknowledgment,
physical-ID quarantine, cache-only delta step, preallocated CUDA arenas, or
real HBM payload mapping. It does not prove live tier thrash, a real 1M model
request, checkpoint serving, quality, or performance.
