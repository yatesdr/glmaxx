# Fable handoff: NVFP4 metadata and padding canonicality v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`9d30aa17cc60de9215b598e4056c880446702f94`

Required result path:
`fable-nvfp4-metadata-canonicality-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`nvfp4-metadata-canonicality-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `crates/glm-format/src/nvfp4.rs` | `785d2daeb13517893fa6604c5a1424079111ac822821b1d16ec23ce6fde0e440` |
| `crates/glm-format/src/container.rs` | `802cd4eee7090ebcad9cce11127bc09271038614466198a84e5045271bdeeb25` |
| `crates/glm-format/tests/nvfp4_proof.rs` | `74b312d65566db5414dd012c2d9b5222aa39808dfb5e979b11c6dadb7c45c734` |
| `docs/nvfp4-metadata-canonicality-proof-v1.md` | `5c3e6710f89a829d40a3ed6d38c398fd5abf04f42c0bf68821ff9ee5cb0e839c` |
| `docs/production-punchlist.md` | `0ad54195dc91714f55746499b37fb7969a6b7d5a7d650e4170f761c9afa6a1c4` |
| `docs/results-index.md` | `12cb2a7c1071a63172c38788f7d2390cb032a6dc5f46e30ba2b14a63ad0f2fad` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-nvfp4-metadata-canonicality-v1-handoff.md
```

Independently derive the legal value and scale coordinates from
`spec/format-v0.md`. Do not use the Rust decoder as the only reference for
padding or partial-tile behavior.

## Review boundary

This review covers only canonical CPU decoding of the existing NVFP4 format:
fixed metadata values, reserved bytes, the `global_amax`/`global_scale`
relation, zero-scale value codes, value padding, scale padding, partial 2D
tiles, and invocation from the rank-file reader.

It does not accept the quantizer's model-quality policy, kernel ABI, CUDA
loading, SM120 execution, block-scaled MMA, checkpoint conversion, profile
fit, output quality, capacity, or performance.

## Required adversarial questions

1. Does metadata decode enforce exact values at bytes 32 through 35 and zero
   at every byte in both reserved ranges 52–55 and 124–127?
2. Is the `global_amax`/`global_scale` relation bit-exact for every accepted
   input, including the positive-zero-only amax rule and the exact `1.0`
   all-zero special case?
3. Does a positive-zero block scale require positive-zero E2M1 values without
   accidentally accepting negative zero or another nonzero nibble?
4. Are value nibbles outside logical N or K rejected for both odd and even
   logical dimensions?
5. Are 1D scale rows and groups outside the logical tensor forced to zero?
6. Is the 2D exception limited to repeated shared scales in the final partial
   16-row tile, while groups wholly beyond logical K remain zero and padded
   values remain zero?
7. Can a malformed shape, truncated plane, oversized plane, or arithmetic
   overflow evade the joint plane validator?
8. Does `RankFile::read` invoke the joint value/scale validation after
   descriptor/metadata agreement for every NVFP4 tensor?
9. Does the fully resigned container regression recompute every enclosing
   checksum, hash, UUID, and header identity that its mutation affects, so it
   distinguishes semantic validation from integrity validation?
10. Are the byte-by-byte metadata mutations and one-ULP scale mutation
    distinguishing rather than self-referential?
11. Do the randomized 1D and 2D pack/dequant proofs still cover the final
    partial tile and remain deterministic?
12. Are the reported test counts, host exclusions, and absence of GPU,
    model-quality, capacity, or performance claims accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- all fixed metadata and reserved bytes are canonical;
- amax/global-scale validation is complete and bit-exact;
- zero-scale/value and value-padding validation is complete;
- 1D and 2D scale-padding rules match the frozen layout;
- the rank-file reader cannot bypass the semantic checks;
- the resigned regressions distinguish semantic from integrity checks;
- randomized pack/dequant coverage remains valid; and
- the proof results and exclusions are accurate.

Only if all eight answers are unqualified `YES`, end with the requested
token. Withhold it for a conditional pass, stale input, unvalidated fixed or
reserved byte, alternate zero encoding, inexact scale relation, nonzero
padding, incorrect partial-tile rule, reader bypass, self-referential test,
unrecomputed enclosing identity, false test count, or overstated GPU/model/
quality/capacity/performance claim.

The token accepts only the canonical CPU decoder correction. It does not
accept the complete manifest/EXL3 current tree, open cn4, authorize CUDA work,
or accept production serving.
