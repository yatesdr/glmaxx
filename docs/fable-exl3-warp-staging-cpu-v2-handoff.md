# Fable handoff: EXL3 warp-staging CPU proof v2

Date: 2026-07-30

Status: adversarial CPU-proof review requested

Review candidate commit:
`21f9881859a9c8b95c211e792c1dfe5c97d7cc9f`

Required result path:
`docs/reviews/fable-exl3-warp-staging-cpu-v2.md`

Requested acceptance token, only for an unqualified pass:
`exl3-warp-staging-cpu-v2-accepted`

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch CUDA for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if necessary, run `review-proof`, and hash every input at review
start and finish. A mismatch is a stale candidate and must withhold the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |
| `docs/exl3-sm120-warp-decode-v2.md` | `67fb3bcb5b839cc50f3462990f1ef6056ca7c9d991851efc5e668fed9d0b3325` |
| `docs/exl3-warp-staging-cpu-proof-v2.md` | `5c77b5721885da708d0240e9eeb6537e9ed74a25a6940cf92e00bc79de494b31` |
| `fable-exl3-warp-decode-v2-r2.md` | `c26236ec0e57b56d90028edb8396dd5521e5cec75174401d04469eefd33990b5` |
| `crates/glm-format/src/exl3.rs` | `f6fa1b25311d78e13e22a0c7c908da7abca636948218fef1987c89850e974edb` |
| `crates/glm-format/src/exl3/warp_proof.rs` | `93dff6fc1e0190efb387e3ee9359bcf196ce6510550f11df73433ac06d34be73` |
| `crates/glm-format/src/lib.rs` | `3d527c9c185d58c176350daf0880676fdcad28b39a320d08c2cd4c1e5dbc7576` |
| `crates/glm-cli/src/main.rs` | `e5e0fd98d222f8e6744de15901c67027ffd1ce7d81b2ca4fbca8f35297f76c77` |
| `fixtures/exl3-warp-staging-proof-v2.json` | `cdc650dd2c70dcbb8c3cb2e5e5659b42f429dc84a62e32bacb0c629ad66f1f45` |
| `scripts/local-checks.sh` | `2d1882be9afd91f4a54c1d3ff9b9f02cd5087357eeb5668d4094c2114c3003ce` |

Run:

```text
proof_dir="$(mktemp -d "${TMPDIR:-/tmp}/glmaxx-exl3-warp-proof.XXXXXX")"
trap 'rm -rf "${proof_dir}"' EXIT
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-exl3-warp-staging-cpu-v2-handoff.md
cargo test --offline -p glm-format exl3::warp_proof
cargo clippy --offline -p glm-format --all-targets -- -D warnings
cargo run --release --offline -p glm-cli --bin glmaxx -- \
  exl3-warp-proof "${proof_dir}/release.json"
cmp fixtures/exl3-warp-staging-proof-v2.json \
  "${proof_dir}/release.json"
cargo run --offline -p glm-cli --bin glmaxx -- \
  exl3-warp-proof "${proof_dir}/debug.json"
cmp fixtures/exl3-warp-staging-proof-v2.json \
  "${proof_dir}/debug.json"
cmp "${proof_dir}/release.json" "${proof_dir}/debug.json"
```

The fresh directory prevents a prior review's output from being mistaken for
either invocation. Both modes must independently match the canonical fixture
and each other byte for byte.

## Review boundary

This review covers only gate 2 of the accepted EXL3 warp-decode-v2 sequence:
the deterministic CPU simulation of staged loads, source decode, row
ownership, barriers, traffic arithmetic, and ascending-K projection
accumulation.

Acceptance opens implementation of the separately selected CUDA v2 entry
point. It does not accept CUDA source that does not yet exist, authorize cn4,
accept a compile or launch, or establish real-payload, prefill, routing,
one-layer, checkpoint, model-quality, serving, capacity, or performance
evidence.

## Required adversarial questions

1. Do all candidate hashes match at review start and finish in a detached
   worktree, even if `main` advances?
2. Does the pinned r2 design review contain both the exact design SHA-256 and
   `exl3-warp-decode-v2-design-accepted` token, and does the proof bind that
   design while allowing only its gate/up `6144x512` and down `512x6144`
   geometries?
3. Is the staged slot table actually derived from the forward scatter while
   the scalar decoder uses the inverse map, or do the two paths reduce to the
   same helper/tautology?
4. Does the forward construction write every one of the 256 local tile
   positions exactly once and then independently cross-check all positions
   against the inverse mapping?
5. Is the little-endian U16-to-U32 source view identical to the accepted CUDA
   address contract, with no alignment, signedness, or byte-order drift?
6. Does the 256-thread simulation map exactly threads 0 through 191 to all
   eight by twenty-four stage words once each, while threads 192 through 255
   issue no load?
7. Re-derive the global source index. Is it exactly
   `(k_tile * n_tiles + n_tile) * 24 + word`, with the eight consecutive K
   tiles staged for one fixed N tile?
8. Does the off-diagonal transpose mutation test genuinely fail if the source
   address is changed to N-major tile addressing?
9. Does staged decode preserve the `+257` cyclic-window position, modulo-24
   word selection, wrapping U32 multiply, mask/XOR, and FP16 reconstruction
   for every local position, including wrapped windows?
10. Independently multiply the iteration counts. Does the proof compare
    exactly 3,145,728 weights for each real geometry and 6,291,456 total,
    without sampling or duplicate-only coverage?
11. Are the 48 gate/up and 4 down stage iterations exact, and does each shape
    schedule exactly 1,179,648 logical trellis bytes?
12. For every output, does each scalar and staged accumulator visit K in
    exact ascending order, with an explicit rounded product boundary before
    addition rather than a contracted FMA?
13. Are all eight deterministic activation rows finite FP16 values, and are
    the final projection hashes computed in logical row-major order rather
    than CTA traversal order?
14. Does comparing every FP32 accumulator bit pattern before FP16 conversion,
    followed by matching row-major FP16 hashes, prove the stated numerical
    scope without silently using a tolerance?
15. For each row count 1 through 8, does the ownership proof give one owner
    for every active `(row,column)`, no owner for inactive rows, and exactly
    16 active threads per row?
16. Do all 256 threads reach both simulated barriers for every row count, and
    would the active-only early-return mutation be observable?
17. Do the two mutation tests fail for the intended independent reasons
    rather than merely comparing a value with itself?
18. Is the fixture deterministic and byte-identical between debug and release
    on the review host, with the exact documented SHA-256 and field values?
19. Can any proof failure be converted into a passing report, or do mapping,
    arithmetic, overflow, and mismatch errors fail closed before a verdict is
    returned?
20. Do the proof document, CLI command, unit tests, local gate, canonical
    fixture, and nonclaims all describe the exact implementation rather than
    a future CUDA behavior?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer each statement separately:

1. the forward-derived staged mapping is independent from and exhaustive
   against the scalar inverse source decoder;
2. both real projection geometries and all local tile positions are covered;
3. load mapping, source addressing, cyclic decode, and traffic counts are
   exact;
4. row ownership and barrier totality are exact for rows 1 through 8;
5. ascending-K FP32 accumulation and row-major FP16 projection results are
   bitwise identical;
6. mutation tests would expose tile transposition, slot permutation, and
   active-only barrier errors;
7. the canonical fixture and debug/release reproducibility claims are valid;
   and
8. the claim boundary is accurate and sufficient to open CUDA v2
   implementation.

Only if all twenty questions and all eight statements are unqualified `YES`,
end with:

```text
exl3-warp-staging-cpu-v2-accepted
```

Withhold the token for stale provenance, shared-path tautology, incomplete
tile coverage, wrong U32 assembly, transposed addressing, cyclic-window
drift, wrong counts, contracted arithmetic, traversal-order output hashing,
inactive barrier omission, ineffective mutations, nondeterministic fixture,
pass-on-error behavior, or evidence overstatement.
