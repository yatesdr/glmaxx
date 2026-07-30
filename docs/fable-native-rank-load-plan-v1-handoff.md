# Fable handoff: native-rank load plan v1

Date: 2026-07-30

Status: adversarial implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`7681210af6c93d7a2cb644a80d8aa001e8e8cc02`

Required result path:
`docs/reviews/fable-native-rank-load-plan-v1.md`

Requested acceptance token, only if every blocker and major is resolved:
`native-rank-load-plan-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `crates/glm-engine/src/checkpoint_load.rs` | `dc0947540fd4e7d692ff2fe1222040536c68aecfb1a0bd0a8043e4bc34554a9e` |
| `crates/glm-engine/src/lib.rs` | `538fb1aabb3354d7b2dde4256ecd46ca7bcb7856e7bd8e3896520834f71c4959` |
| `crates/glm-format/src/container.rs` | `2fbe22c55d481e40699a02be9929f9a964f50bc08199853772dd314c85ade47f` |
| `crates/glm-format/src/lib.rs` | `c37aa179cc9cf56f64c57f7e8b50ba6b226eac74af58066630a53c700bb0f60a` |
| `crates/glm-format/src/native_reader.rs` | `89e61aff8541ddbb48fd28a2458b10bb3e90dcf2d654a2fb134217521a6fdc5e` |
| `crates/glm-format/src/rank_manifest.rs` | `cb57a81ad3643a86992c4fd9c2166e9ee238cc7e66063732355d14e485f73410` |
| `crates/glm-format/src/stream.rs` | `b6d7dae8adf6fbb7ebd0f08c79c3d7f9dbba6269408f6b760fa43b18028a22fb` |
| `docs/native-rank-load-plan-proof-v1.md` | `19558f87ef912c8ead99c31cf0f1a1867dcc384ab79d8efbbee96f66abfe0e63` |
| `docs/checkpoint-load-transaction-v1.md` | `79d9c376201f3540f247344c24c37dd7d819d629f10459899a73c15f8b27015f` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/production-punchlist.md` | `a338c9391db3f46e7ca0c684124cbfab2955f6508574067ccf9f16b6d34513dc` |
| `docs/results-index.md` | `6f814aecaf33f5da9e37c23ac78ec37fa7fe42cd9d1fb3c315ac6b91513a43ea` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-native-rank-load-plan-v1-handoff.md
cargo test --offline -p glm-format rank_manifest
cargo test --offline -p glm-engine checkpoint_load
cargo clippy --offline -p glm-format -p glm-engine --all-targets -- -D warnings
```

## Review boundary

This review covers the production capacity-EXL3 semantic-catalog projection
and the construction of a deterministic four-rank load plan from four
authenticated native readers plus a process-supplied environment.

It does not accept the earlier load-transaction design or CPU-core review by
implication. It does not accept NVFP4-laboratory or hybrid manifests, actual
full rank files, a CUDA sink, device allocation or verification, a checkpoint
smoke, production health, SM120 execution, capacity, quality, or performance.

## Required adversarial questions

1. Can the public `build_rank_set_load_plan` path construct a plan without
   first passing the unchanged-file and complete four-rank validation in
   `NativeRankReader::validate_rank_set`?
2. Does `ValidatedTensorSemantic::encode` match every reviewed
   `TensorSemanticEntry.v1` offset, integer width, signed encoding, reserved
   zero, and field source exactly?
3. Are the source-kind, reconstruction, collective-after, safetensors dtype,
   and EXL3 source-dtype mappings complete, collision-free, and fail-closed?
4. Does the catalog preimage use the exact domain, little-endian tensor
   count, and tensor-ID order? Independently reproduce the pinned
   `bb08eef4...f222deb` digest from all 59,585 compiled tensors on all four
   ranks.
5. Can a rank-local source path, slice, codec-metadata hash, plane length,
   descriptor flag, alignment, or tensor-contract difference be erased by
   the normalized catalog rather than rejected by full rank validation?
6. Are physical metadata, primary, and auxiliary lengths and device
   alignment derived only from the compiled capacity contract, with every
   reader descriptor checked against it? Can a self-consistent re-signed rank
   file choose its own arena sizes?
7. Recompute the tensor-ID-order arena construction. Are starts aligned,
   empty metadata/auxiliary offsets canonical, arithmetic checked, intervals
   nonoverlapping, and final arena extents exact?
8. Independently reproduce 81,605,027,840 weight bytes, 14,942,048 metadata
   bytes, and rank-0 arena-layout digest
   `140274b8...f81df039`.
9. Does every plan header and rank entry come from the correct authenticated
   common or rank-local source? Are conversion, model, tokenizer, template,
   policy, ABI, operation, budget, device, file, descriptor, payload, and
   tensor-contract identities all bound exactly once without substitution?
10. Is the private authenticated-source projection inaccessible to external
    callers and used only to positively test planning logic? Does any public
    exported helper provide an equivalent forgeable bypass?
11. Do profile, rank, identity, tensor-count, semantic, descriptor-contract,
    overflow, overlap, and tail failures all remain fail-closed? In
    particular, do unsupported NVFP4/hybrid manifests remain closed rather
    than masquerading as capacity EXL3?
12. Are the proof's local-test count, exact-candidate claim, exclusions, and
    absence of full rank files, allocation, CUDA, smoke, GPU, capacity,
    quality, and performance evidence accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- the public reader-to-plan path is non-forgeable and fail-closed;
- the normalized semantic catalog is byte-exact and rank invariant;
- full rank-local validation prevents normalization from hiding drift;
- physical plane lengths and alignments come only from the compiled contract;
- complete actual-shape arena arithmetic and hashes are exact;
- all common and rank-local plan identities are correctly sourced;
- unsupported profiles remain closed; and
- CPU proof claims and exclusions are accurate.

Only if all eight answers are unqualified `YES`, end with the requested
token. Withhold it for a stale candidate, public validation bypass, encoding
ambiguity, missing semantic field, dynamic enum alias, concealed rank drift,
file-defined plane size, unchecked arithmetic, wrong identity source,
unsupported profile opening, test-coverage gap, or evidence overstatement.

The token accepts only this CPU reader-to-plan boundary. It does not accept
the transaction design separately, open cn4, authorize CUDA work, or accept
a checkpoint smoke.
