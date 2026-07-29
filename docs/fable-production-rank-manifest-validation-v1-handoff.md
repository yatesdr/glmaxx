# Fable handoff: production rank-manifest validation v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested; GPU authorization
conveyed by this handoff: none

Review candidate commit:
`46bff28aaf950ea15fdfc69ac074412cbd46c9c4`

Required result path:
`fable-production-rank-manifest-validation-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`production-rank-manifest-validation-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run the repository
`review-proof` command on this handoff, then hash every input at review start
and finish. If either hash set differs, report a stale candidate and do not
emit the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `crates/glm-format/src/rank_manifest.rs` | `288402a5c0f965d690df9325ab219117790d07e1f90c3f6543a0bd3663377ba2` |
| `crates/glm-format/src/native_reader.rs` | `24eef8432a8dff2e830a8ec63e4e46bffcfafd94486e64fbb467945825ab0089` |
| `crates/glm-format/src/lib.rs` | `1e30ccde5a59005f7ce6db05d6a895283558ee3bcca812dba59ca32b5b11e905` |
| `crates/glm-cli/src/main.rs` | `8190a701f4ae757408bd9048783709d3525ffa398d865d66653a2746f80d732b` |
| `crates/glm-format/src/checkpoint.rs` | `b9b616eae24541556753599bba670dafb4989d61f61174c9bccdfb76663b2aca` |
| `crates/glm-format/src/stream.rs` | `4cd4cb23d68ef4280a9a9a00270fc7dad4091ade058fd1165f353d6c95772c8f` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `docs/checkpoint-ingest.md` | `186ce5985ca1adbf280a011a7692ae07780736f842c786dccb599ed8a458d07d` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `docs/fable-manifest-abi-v022-r2-handoff.md` | `d13839b369b22b0614fea641836c755cb544411c8343ca2ab6f78cc0a603f0e0` |
| `docs/fable-checkpoint-load-transaction-v1-r2-handoff.md` | `24cd8f8502d6a6f2a34c0e28cb46083ef2585924e1fc60935dc0cae0f1c3118f` |
| `crates/glm-cli/src/review.rs` | `d2c2d2756b94df8fb5555f578e7c907bef7c09b7b10fb3f310f45566f73c1c45` |
| `docs/review-provenance-verifier-v1.md` | `c4be2415ad0b13cea7fc154ce10c7aea839bd47b57af3e42fa6f329b92f3cb4e` |
| `scripts/local-checks.sh` | `378a717bc9d592bac68ac999c6a91d4655b633177a9005529288096a3d2a029b` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-production-rank-manifest-validation-v1-handoff.md
```

## Required adversarial questions

1. Is schema dispatch fail-closed for every future production schema without
   breaking deliberately non-production CPU fixtures?
2. Does recursive `deny_unknown_fields`, canonical JSON checking, exact typed
   parsing, and digest parsing eliminate extension and normalization
   ambiguities?
3. Recompute the converter's tokenizer-bundle preimage. Is the reader
   expression-for-expression identical, including its domain separator and
   ordered filenames?
4. Does the 92-file source-map validation bind every required nested digest
   and reject unsafe names, missing entries, extra entries, malformed hashes,
   and inconsistent publisher metadata?
5. Compare every emitted production-manifest field in
   `rank_conversion_manifest` and `PinnedRankPlan::manifest_tensors` with the
   reader. Can a current valid capacity-EXL3 conversion be rejected due to
   reader/writer drift?
6. Conversely, can a re-signed manifest lie about a descriptor, name, codec
   metadata, physical bytes, source path/slice, rank/global/source shape,
   reconstruction, collective, operation identity, profile, toolchain, or
   review provenance and still pass?
7. Independently test replicated, contiguous-TP, and explicit EXL3 source
   geometry for actual protected and routed GLM-5.2 tensors on all four ranks.
8. Is rejecting NVFP4 descriptors and all unreviewed hybrid/NVFP4 profile
   names correct for this exact v0.2.2 production-manifest schema?
9. Is the tensor-contract digest correctly rank-specific, and does
   four-rank consensus compare exactly the fields that must be common without
   equating rank-specific source contracts?
10. Does `native-rank-proof` fail closed on non-production manifests,
    operation-manifest drift, wrong tensor count, current-binary kernel-ABI
    drift, rank divergence, or payload corruption before reporting success?
11. Are manifest/control-region allocation limits and parsing order adequate
    against malformed-input memory or integer-exhaustion attacks?
12. Do the tests contain any tautological oracle, fixed-point-only sample, or
    self-hash-only check analogous to the defects found in earlier handoffs?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then state
separately whether:

- the writer and strict reader contracts are exact inverses for the current
  capacity-EXL3 manifest;
- re-signed semantic divergence fails closed;
- four-rank production identity handling is accepted; and
- this CPU manifest-validation implementation may enter the reviewed
  checkpoint-load implementation after that design's own r2 token is present.

Only if all four answers are unqualified `YES`, end with the requested token.
Do not emit it for a conditional pass or stale input.

Acceptance of this implementation does not accept the separately pending
manifest-ABI r2 gate or checkpoint-load transaction r2 gate. It does not
authorize cn4, a full conversion, CUDA upload, checkpoint startup, quality, or
performance claims.

