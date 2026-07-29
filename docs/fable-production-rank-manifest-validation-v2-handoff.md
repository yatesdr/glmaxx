# Fable handoff: production rank-manifest validation v2

Date: 2026-07-29

Status: superseding adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

Review candidate commit:
`4bf7bb5e817e01cc299058b56a488b35011fd79d`

Required result path:
`fable-production-rank-manifest-validation-v2.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`production-rank-manifest-validation-v2-accepted`

The v1 handoff and token are superseded. Sol's own continuation audit found
that v1 authenticated a manifest against its native descriptors but allowed an
attacker to rewrite both consistently. V2 adds immutable engine-owned complete
rank-contract and source-map identities.

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof` on this
handoff, then hash every input at review start and finish. If either set
differs, report a stale candidate and do not emit the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `crates/glm-format/src/rank_manifest.rs` | `3b94f2306e2c0ee82f342b66b945f48767441463929e12b575648f9ccda99d6b` |
| `crates/glm-format/src/native_reader.rs` | `24eef8432a8dff2e830a8ec63e4e46bffcfafd94486e64fbb467945825ab0089` |
| `crates/glm-format/src/checkpoint.rs` | `08450f0cb33e592ec76dfbe655b06580ba1743e60f3109a65218d052dbea406c` |
| `crates/glm-format/src/lib.rs` | `52aea8d21b3c3a504a46aa3c9517233fb8b438374ce5fb2e9ad07dc316ee7c0b` |
| `crates/glm-cli/src/main.rs` | `3b76e43b81a1bf7a540565e4b8356999a1b2dcc9e5c8dd1036d4e3b17708128c` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `docs/checkpoint-ingest.md` | `186ce5985ca1adbf280a011a7692ae07780736f842c786dccb599ed8a458d07d` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `docs/fable-manifest-abi-v022-r2-handoff.md` | `d13839b369b22b0614fea641836c755cb544411c8343ca2ab6f78cc0a603f0e0` |
| `docs/fable-checkpoint-load-transaction-v1-r2-handoff.md` | `24cd8f8502d6a6f2a34c0e28cb46083ef2585924e1fc60935dc0cae0f1c3118f` |
| `scripts/local-checks.sh` | `378a717bc9d592bac68ac999c6a91d4655b633177a9005529288096a3d2a029b` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-production-rank-manifest-validation-v2-handoff.md
```

## Independent source-map derivation

Fetch only:

```text
https://huggingface.co/brandonmusic/GLM-5.2-EXL3-TR3-3.0bpw/resolve/9297b9f1d53af5c67cffa01e30cc071a1ff7144b/MANIFEST.sha256
```

It must be exactly 8,528 bytes, 92 newline-terminated records, and SHA-256
`bfb6dc39f28da08c1cfc5b89603414046adf7003152d69e9ee350e11f7a1fa63`.
Parse its lowercase hashes, then replace:

- `.gitattributes` with
  `5bb36c320417db43af1dc6af8bd0fcc154bb7276eddaf96b12c395bdafed634d`;
- `README.md` with
  `e60e023082ee175a11f51e79e8dd88f5e4ed9975fc904e64cdeabbbcf8abe225`.

Compact-JSON encode the lexicographically sorted string map. Independently
confirm its SHA-256 is
`ad1e4fb286adbc261a2800ab17e4abde5bcd13efb22b150d65ec42b47e2af5fe`.
Do not fetch model shards for this review.

## Required adversarial questions

1. Recompute all four canonical tensor-contract digests from
   `PinnedRankPlan::manifest_tensors`. Do they exactly match the fixed
   constants, and does every rank differ for the expected source paths,
   slices, and codec metadata?
2. Does the production entry point require exact count, exact source bytes,
   its rank's fixed tensor contract, and the fixed full source map—not merely
   self-consistent hashes?
3. Independently mutate a valid generated tensor policy and re-sign both
   descriptor and manifest. Does the fixed inventory reject every name, role,
   shape, source, dtype, codec, flag, byte, and collective change?
4. Recompute the source-map identity using the procedure above. Does both the
   source verifier and native reader use precisely that representation?
5. Are BF16/F16/F32 protected source dtypes and EXL3 component dtype bound
   exactly, with codec/source-kind masquerading impossible?
6. Are replicated, contiguous-TP, and explicit-component axes and global,
   rank, and source shapes independently derived for all four actual plans?
7. Compare every current writer field with the reader. Can an honestly
   converted capacity-EXL3 manifest fail due to writer/reader drift?
8. Does four-rank consensus compare all common identities while deliberately
   retaining four different tensor-contract digests?
9. Does `native-rank-proof` bind current-binary operation manifest, weight
   policy, kernel ABI, exact rank filenames/order, full payloads, and the
   pinned inventory before reporting success?
10. Can unbounded allocation, integer overflow, duplicate JSON keys,
    normalization, unknown fields, or unsupported future schema/profile data
    bypass or exhaust the control path?
11. Is the test oracle independent enough to detect a wrong emitter rather
    than merely hashing it? Identify any tautological or fixed-point-only
    assertion.
12. Does this implementation conflict with the pending format-ABI r2 or
    checkpoint-load transaction r2 contracts?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then state
separately whether:

- the fixed complete four-rank tensor inventory is accepted;
- the independently derived full source-map identity is accepted;
- re-signed semantic divergence now fails closed;
- writer/reader and four-rank identity handling are accepted; and
- this CPU validation may enter checkpoint-load implementation only after
  that design's own r2 token is present.

Only if all five answers are unqualified `YES`, end with the requested token.
Do not emit it for a conditional pass or stale input.

This token would accept only the CPU manifest validator. It does not accept
the separately pending format-ABI or load-transaction gates, authorize cn4,
approve full conversion, or establish checkpoint startup, quality, or speed.

