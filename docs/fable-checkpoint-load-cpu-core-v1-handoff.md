# Fable handoff: checkpoint load CPU core v1

Date: 2026-07-30

Status: adversarial implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`d29ce96a0d6037e045c359fd1116187ca0722c42`

Required result path:
`docs/reviews/fable-checkpoint-load-cpu-core-v1.md`

Requested acceptance token, only if every blocker and major is resolved:
`checkpoint-load-cpu-core-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `crates/glm-engine/src/checkpoint_load.rs` | `eb120bd7365d16f137a44e9c2cd230600d99a06cfdec4e5757c1c67c1171e3c8` |
| `crates/glm-engine/src/startup.rs` | `1a5f1ac8aae94e6eb2aaf2cf4701dfc290604013103eacc1423046211609a5fc` |
| `crates/glm-engine/src/lib.rs` | `6d6f710fd6aed79ecc42a085be68b307fc7366fdbe11f330f2bac2453ae4648e` |
| `docs/checkpoint-load-cpu-core-proof-v1.md` | `a3cbd93be0b7f131d98d996601c75e653764ec429839f19e2c26835fa4bd20c1` |
| `docs/checkpoint-load-transaction-v1.md` | `79d9c376201f3540f247344c24c37dd7d819d629f10459899a73c15f8b27015f` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/production-punchlist.md` | `d21c244444a91f4e70b58ad22d4463d32ab5ad1b1079644b9830bd47a81bb427` |
| `docs/results-index.md` | `41cc79b58a1129abfef341fb9a350278b0d98fb69f9c46efa4738ed7d6fa7f95` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-checkpoint-load-cpu-core-v1-handoff.md
cargo test --offline -p glm-engine checkpoint_load
cargo test --offline -p glm-engine startup
cargo clippy --offline -p glm-engine --all-targets -- -D warnings
```

## Review boundary

This review covers only the deterministic load-plan/receipt ABI, planned
streaming sink, quarantined ownership state, four-rank adoption protocol,
execution-permit boundary, and adoption-bound startup transition implemented
at the candidate.

It does not accept the earlier r2 design by implication, a native-reader plan
builder, a production or laboratory manifest, a CUDA sink, a device arena, a
checkpoint smoke, SM120 execution, production health, capacity, quality, or
performance.

## Required adversarial questions

1. Are the 416-byte header, four 248-byte rank entries, 64-byte tensor
   entries, field offsets, little-endian encodings, reserved zeros, ordering,
   and complete preimage length exact against the r2 design?
2. Does plan construction reject zero or duplicate identities, missing or
   reordered ranks/tensors, arithmetic overflow, invalid alignment, out-of-
   bounds intervals, interval overlap, semantic drift, and unaccounted arena
   tails?
3. Is `arena_layout_sha256` sufficiently bound to rank, both arena sizes,
   tensor count, and every physical entry without an ambiguous encoding?
4. Are the plan, prepared-rank, prepared-rank-set, and adopted-rank-set hash
   domains and concatenation order exact and noninterchangeable?
5. Does the implementation correctly leave `FS_VERITY` unusable for every
   v1 profile rather than accidentally opening the unimplemented restart
   route?
6. Does each 256-byte prepared receipt bind the exact plan, rank, device,
   file, payload, arena layout/sizes, verified bytes, uploaded bytes,
   generation, and nonzero evidence digest?
7. Can receipt reordering, a duplicate rank, one changed acknowledgement,
   one changed generation, or partial rank-local adoption ever produce an
   adopted-set receipt?
8. Are `RankArenaLifecycle` and `WeightArenaExecutionPermit` non-cloneable,
   are state skips rejected, and is the one physical abort/free obligation
   issued exactly once?
9. Does `PlannedRankTensorSink` compare every relevant descriptor field,
   route metadata/primary/auxiliary bytes only to planned intervals, reject
   early auxiliary data and short/long planes, and allocate nothing on
   successful chunk callbacks?
10. Does any sink/writer/order/bounds failure poison sealing, with no path to
    reuse a partially written sink as prepared?
11. Is the public startup API unable to enter `WeightsLoaded` without an
    unforgeable completed `AdoptedRankSetReceipt`, and is the internal mock
    bypass inaccessible to external production callers?
12. Must the adoption digest be zero before load, identical at
    `WeightsLoaded`, and immutable through all later startup stages, with
    every disagreement terminal?
13. Are claims correctly limited given that the lifecycle object models but
    does not own a CUDA allocation, stream, event, or physical free?
14. Re-run or independently audit all focused tests and verify the reported
    309-test full local gate was executed against exact candidate
    `65614928`, whose bytes are pinned in the proof.
15. Are all missing pieces explicit: reader-to-plan semantic construction,
    full four-rank fault coordination, pinned staging/event rings, H2D/device
    verification, persistent rank-worker ownership, complete native rank
    input, checkpoint smoke, and every GPU/model claim?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- canonical plan and receipt encodings are exact;
- arena interval and identity validation is fail-closed;
- streaming into quarantine is complete and nonpublishable;
- prepared/adopted rank-set consensus is process-safe;
- execution permits cannot precede global adoption;
- startup cannot bypass or drift the adoption digest;
- CPU proof claims and exclusions are accurate; and
- the implementation is safe to extend with a CUDA arena writer.

Only if all eight answers are unqualified `YES`, end with the requested
token. Withhold it for a stale candidate, encoding ambiguity, unchecked
interval, hash-domain collision, FS-verity opening, reorder/partial-adoption
success, forgeable or cloneable permit, reusable failed sink, startup bypass,
nonterminal disagreement, resource-ownership overclaim, missing test
coverage, or any CUDA/checkpoint/health/capacity/quality/performance
overstatement.

The token accepts only this CPU implementation boundary. It does not accept
the r2 design separately, open cn4, authorize CUDA work, or accept a
checkpoint smoke.
