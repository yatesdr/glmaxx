# Fable handoff: NVFP4 streaming canonicality v2

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`9262007eaf675d2bc1754c0f17a3ae8a871abb18`

Required result path:
`fable-nvfp4-streaming-canonicality-v2.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`nvfp4-streaming-canonicality-v2-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `crates/glm-format/src/nvfp4.rs` | `af9211c7df2c74b446d234ed215580614ce58c415963a1038eb86df48ad8b11a` |
| `crates/glm-format/src/container.rs` | `802cd4eee7090ebcad9cce11127bc09271038614466198a84e5045271bdeeb25` |
| `crates/glm-format/src/native_reader.rs` | `937ad3883af69d956213492afdf8fa21db304809c3c3fb1c1ebff7518a18c965` |
| `crates/glm-format/src/stream.rs` | `363969e454fb7e851d4b73a355bbc4ebc33c79b710326aa2c7ddf1d17e9aff94` |
| `crates/glm-format/tests/nvfp4_proof.rs` | `74b312d65566db5414dd012c2d9b5222aa39808dfb5e979b11c6dadb7c45c734` |
| `docs/nvfp4-metadata-canonicality-proof-v1.md` | `5c3e6710f89a829d40a3ed6d38c398fd5abf04f42c0bf68821ff9ee5cb0e839c` |
| `docs/nvfp4-streaming-canonicality-proof-v2.md` | `fae7d1f1b0b06fee2671733a14d6df77c67a6d1b5cf925310412aace7d98803b` |
| `docs/production-punchlist.md` | `c1cc7863a7ca49ac9c5c33cc577585bb40eeab7e59563893a7b8eabc95da1f2c` |
| `docs/results-index.md` | `484ead409d655384b3c47a2b8d30ed20476307c95134cf25d15d326ff002800b` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-nvfp4-streaming-canonicality-v2-handoff.md
```

Independently derive the row-major value-block sequence, swizzled scale
offset, and 2D scale-sharing classes from `spec/format-v0.md`. Do not use the
Rust validator as the only reference.

## Review boundary

This review covers canonical CPU semantics for in-memory and file-backed
NVFP4 planes, including bounded streaming conversion and the tentative
direct-upload reader. It extends the narrower v1 metadata/container review.

It does not accept a device allocation, CUDA loading, SM120 execution,
block-scaled MMA, complete checkpoint conversion, profile fit, model quality,
capacity, or performance.

## Required adversarial questions

1. Does each sequential 8-byte value block correspond to exactly one logical
   `(row,K-group-of-16)` before its scale offset is swizzled?
2. Do odd logical K, padded K groups, padded N rows, low nibbles, and high
   nibbles all receive the correct padding decision?
3. Is one bit per scale sufficient to retain exactly the predicate needed for
   zero-scale/nonzero-value rejection without retaining the value plane?
4. Are negative, nonfinite, short, long, overlapping, skipped, or
   out-of-order scale/value chunks rejected?
5. Does codec `0x0101` require every one of the 16 physical scale replicas to
   equal the tile's first-row scale, including a final partial logical tile
   and a fully padded tile?
6. Does codec `0x0100` avoid accidentally imposing 2D sharing?
7. Is scratch exactly
   `scale_plane_bytes + ceil(scale_plane_bytes/8)`, allocation-fail-closed,
   and 442,368 bytes for actual TP4 FC1?
8. Does `NativeRankReader` construct the validator before exposing tensor
   chunks, feed both planes in exact order, include scratch in its maximum,
   and finalize semantics before `finish_tensor`?
9. Is the direct-upload sink contract sufficient to keep already-received
   chunks unreachable after a late semantic failure?
10. Does `StreamingRankWriter` validate new writes before descriptor
    publication and revalidate every completed descriptor on resume?
11. After a failed converter validation, can stale payload bytes become
    visible without a complete overwrite and successful descriptor commit?
12. Do the resigned file-backed regressions recompute every affected
    descriptor/header identity and therefore distinguish semantic rejection
    from hash rejection?
13. Do the chunk-split, out-of-order, zero-scale, padding, 2D-sharing, and
    exact-scratch tests exercise separate failure modes?
14. Are the 65 targeted unit tests, 3 external proof tests, 291 workspace
    tests, 73 handoffs, 54 configured results, host exclusions, and absence of
    device/model/quality/capacity/performance claims accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- value-block tracking and padding validation are complete;
- zero-scale/value coupling is complete;
- 1D and 2D scale semantics match format-v0;
- chunk sequencing and allocation are bounded and fail-closed;
- direct-upload validation and tentative ownership are sound;
- conversion publication and resume validation are sound;
- the regressions distinguish semantic from integrity checks; and
- proof results and exclusions are accurate.

Only if all eight answers are unqualified `YES`, end with the requested
token. Withhold it for a conditional pass, stale input, incorrect row/group
mapping, padding hole, zero-scale bypass, missing 2D replica check,
cross-codec rule leak, unchecked allocation, uncharged scratch, upload
publication before final validation, converter descriptor publication before
validation, resume bypass, nondistinguishing resigned test, false count, or
overstated device/model/quality/capacity/performance claim.

The token accepts only canonical bounded CPU conversion and tentative loading
for NVFP4. It does not accept the complete manifest/EXL3 current tree, open
cn4, authorize CUDA work, or accept production serving.
