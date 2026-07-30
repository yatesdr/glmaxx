# Canonical page-table delta CPU proof v1

Date: 2026-07-29

Implementation and regression commit:
`271d1f40529c5f2bd1ac774c385676782f482424`

Status: CPU delta and page-granular mutation passed; independent review
pending

GPU claim: none

## Implemented boundary

`glm-cache` now defines `glmaxx.page-table-delta.v1`, a canonical
generation-to-generation representation of active sequence changes. A delta
requires an exact nonzero successor generation and contains:

- zero to 64 sorted sequence updates;
- zero to 64 sorted removals;
- request ID, MTP posture, committed and tentative counts;
- the complete page count after the mutation;
- the first changed ordinal and complete changed suffix;
- owner rank, target and optional draft local IDs, HBM state, and valid-token
  count for every changed page;
- one global SHA-256 digest; and
- one rank-local digest per owner, bound to the global digest and the exact
  rank-invariant fields.

A no-op generation advance, duplicate/unsorted identity, overlap between
update and removal, non-successor generation, wrong owner, zero/oversized
valid count, non-HBM active state, target/draft posture mismatch, malformed
suffix, or digest mismatch fails closed.

`PageTableMirror` is an independent CPU consumer. It knows the configured
target and draft arena bounds, applies a delta to a clone, validates every
complete reconstructed sequence, and adopts it only on success. Validation
checks contiguous ordinals, DCP4 ownership, exact target/draft posture,
committed plus tentative position count, page states, local-ID bounds,
per-sequence duplicate IDs, shared-physical consistency, and target/draft
collisions. A stale generation or late malformed update leaves the mirror
bit-identical.

The builder compares rank-visible page fields only. Reference counts and
prefix bookkeeping are intentionally absent from the rank delta; attaching a
second reference to an unchanged shared page does not create unrelated device
work.

## Page-granular append

Committed prefill mutation previously iterated once per token. It now:

1. fills an existing mutable tail with checked page arithmetic;
2. allocates and seals complete 64-token pages directly;
3. allocates at most one final partial mutable page; and
4. publishes the checked committed count after the page loop.

The existing clone-on-error wrapper still provides CPU rollback. The
regression compares bulk append with repeated one-token append for every
count from one through 257, crossing all DCP4 and page boundaries.

`every_tail_occupancy_and_mtp_depth_reserves_exactly_one_position_per_token`
exhausts all 64 tail occupancies and tentative depths one through seven. For
all 448 combinations it checks total valid positions, exact page count, the
64-token per-page ceiling, complete draft attachment, and bit-identical
rollback.

## Delta regressions

`delta_reconstructs_tentative_admission_and_removal_atomically` starts with
two active sequences, then performs an MTP7 tentative cross-page reservation,
one removal, and one two-page admission. It proves sorted update/removal
identity, exact changed suffixes, four distinct rank-local digests, and a
mirror reconstructed bit-equal to the authoritative after-table.

`unchanged_prefix_is_omitted_and_digest_tampering_fails_closed` proves that
two sealed pages are omitted when a third page is appended, and that both
verification and mirror application reject a changed global digest without
mutating the mirror.

`generation_shape_owner_and_noop_deltas_are_rejected` covers zero/skipped
generations, a no-op advance, a re-signed wrong-owner page, a re-signed
out-of-arena local ID, and a stale mirror generation.

## Gate result and exclusions

The full local gate passed 278 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
deterministic CPU proof regeneration, and all 66 then-present review handoff
provenance proofs.

Commands:

```text
cargo test --offline -p glm-cache delta::tests
cargo test --offline -p glm-cache
cargo test --workspace --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-cache/src/delta.rs
71ac2da15e869a6f2470c3551a7cd6ec4ff387850a23240e9a44ad96a538ff16

crates/glm-cache/src/sequence.rs
d48a93cbbbef67eaf2b1550cb1d20d6132bf10d0cf00c5e93d5b66d351981034

crates/glm-cache/src/lib.rs
a892febc0c979cfad3cc629aed005156639fa5aa1c27709207d9553d50575abc

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, model, or tier
transfer was used.

The delta is not yet wired into `ServingCoordinator`, `StepInput`, or
`RankExecutor`. It uses owned vectors/boxes and does not claim a
fixed-allocation hot path. The page table still uses clone-on-error outside
the page-granular inner append. There is no device upload acknowledgment,
rank receipt, removal quarantine, ABA prevention across acknowledged
generations, `CACHE_ONLY` cleanup, real HBM payload mapping, checkpoint
execution, quality, capacity under live tiers, or performance claim.
