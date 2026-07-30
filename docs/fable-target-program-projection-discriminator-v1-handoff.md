# Fable handoff: target-program projection discriminator v1

Date: 2026-07-30

Status: adversarial design review requested

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch GPU work for this review

Review candidate commit:
`39fbee5bf220467104535d86c00b49effe96c3a8`

Required result path:
`docs/reviews/fable-target-program-projection-discriminator-v1.md`

Requested acceptance token, only if every blocker and major is resolved:
`target-program-projection-discriminator-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |
| `docs/target-program-projection-discriminator-v1.md` | `c8585f4790a33dc98af0246b30de62ea61d6a7a70150b661dc4d0499ea7f50fe` |
| `docs/target-layer-execution-v1.md` | `7f16667d55a50441dbc81b7fe18285be5a6f0fb06c68ef008a40aad8f406f478` |
| `docs/small-checkpoint-runner-v1.md` | `720e07e3791ab1c5174aedc9aa449cfe048e6bc1b9d483798c0d83d8319050f6` |
| `docs/resident-tensor-device-binding-proof-v1.md` | `15a21ae2ff24758d2f115540a895191f7aeb2acf13c31f2972dcb0700adbab6d` |
| `crates/glm-format/src/checkpoint.rs` | `08450f0cb33e592ec76dfbe655b06580ba1743e60f3109a65218d052dbea406c` |
| `crates/glm-format/src/rank_manifest.rs` | `cb57a81ad3643a86992c4fd9c2166e9ee238cc7e66063732355d14e485f73410` |
| `crates/glm-engine/src/checkpoint_cuda.rs` | `0b5e411d68a61fa1a39ccb7cc6b36702b85b3d385098764fa2d33b18227efdbe` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-target-program-projection-discriminator-v1-handoff.md
cargo test --offline -p glm-format -p glm-engine
```

## Review boundary

This review covers only the design amendment that distinguishes routed gate,
up, down, and combined gate/up tensor bindings in the immutable target
program. It includes split-EXL3 versus combined-NVFP4 representation,
startup derivation, hashing, ordering, fail-closed mixtures, and M4 inventory
consequences.

It does not accept the broader target-layer design, implement a compiler,
change the on-disk format, choose a hybrid policy, authorize conversion, or
accept CUDA, checkpoint execution, serving, quality, capacity, or
performance.

## Required adversarial questions

1. Verify all candidate input hashes at review start and finish. Does the
   review stay pinned even if `main` advances?
2. Independently inspect the pinned capacity-EXL3 rank plan. Do gate and up
   have distinct tensor IDs/names but identical layer, role `0x0501`, expert,
   codec, and TP axis for all 256 experts?
3. Does that fact make the original `(tensor_id, role_id, expert_id,
   codec_id)` binding insufficient as a semantic lookup key, without making
   numeric tensor-ID execution itself ambiguous?
4. Is the closed projection enumeration complete and nonaliasing for
   protected, split gate, split up, down, and combined gate/up?
5. Recompute the amended record size. Is it exactly 16 bytes with fixed-width
   little-endian fields and five required-zero reserved bytes?
6. Does the changed target-program hash domain prevent old/new graph profile
   or step-input identity aliasing, or is another ABI/version field also
   required?
7. Are canonical name strings and SHA-256 derivation unambiguous for every
   layer/expert, including decimal formatting, terminator posture, and
   experts 0/255?
8. Is exact validated `name_sha256` a sufficient startup discriminator only
   when layer, role, expert, codec, TP axis, dtype, shape, and plane geometry
   are also checked?
9. Could a malicious/corrupt manifest swap gate/up names, IDs, metadata, or
   payloads and still compile? Trace every existing manifest/descriptor hash
   check plus the new required semantic check.
10. Is split EXL3 represented with two direct projections and no hidden
    combined allocation or repack? Are its rank/global shapes exact?
11. Is combined NVFP4 represented with one `[1024,6144]` rank matrix, and is
    the gate-then-up row convention sufficiently frozen to prevent a
    converter/kernel disagreement?
12. Are combined gate/up global geometry and all per-rank TP slices stated
    consistently with two source projections of global shape
    `[2048,6144]` each?
13. Are every forbidden mixture and duplicate sufficient to prevent missing,
    double-counted, cross-role, cross-codec, and rank-divergent expert
    programs?
14. Can a hybrid profile choose per-expert representation only as an
    immutable process-common startup policy, never per request, route, graph
    launch, or rank?
15. Is the canonical binding order total and deterministic? Does rejecting a
    duplicate `(role, expert, projection)` make tensor ID only an identity
    tie-breaker rather than an accidental discriminator?
16. Independently rederive 768 routed descriptors per split-EXL3 sparse layer
    and 512 per combined-NVFP4 sparse layer.
17. Is the M4 533-tensor inventory correctly limited to the combined-NVFP4
    laboratory representation, with no implication that a capacity-EXL3
    subset has the same count?
18. Can graph launch consume only fixed tensor/projection IDs through the
    resident resolver, with no string hash, metadata parse, map lookup,
    policy decision, or repack?
19. Is forbidding split NVFP4 and combined EXL3 in v1 an explicit, safe
    scope choice rather than an accidental inability to represent a required
    current checkpoint?
20. Are the CPU-proof requirements sufficient to distinguish adjacency
    inference, name-only acceptance, wrong row order, mixed
    representations, duplicate projection, cross-rank drift, and M4 count
    overclaim?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- the original key ambiguity is real;
- the 16-byte discriminator record is canonical and collision-safe;
- startup derivation is sufficient and request/rank independent;
- split EXL3 and combined NVFP4 are represented without repack;
- geometry and row order are exact;
- forbidden mixtures and cross-rank drift fail closed;
- the M4 inventory consequence is accurate;
- runtime graph launch needs no string/metadata lookup; and
- claims and exclusions match the exact candidate.

Only if all twenty answers are unqualified `YES`, end with the requested
token. Withhold it for stale provenance, false ambiguity, record-size or
endianness error, hash-domain aliasing, ambiguous name derivation, name-only
trust, wrong geometry, unfrozen row order, hidden repack, incomplete mixture
rejection, rank-local policy, nondeterministic ordering, wrong descriptor
counts, M4 overclaim, runtime string parsing, incomplete CPU gate, or evidence
overstatement.

The token accepts only this design amendment. It does not authorize cn4,
accept a target-program implementation, or establish model execution,
serving, quality, capacity, or performance.
