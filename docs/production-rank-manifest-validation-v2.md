# Production rank-manifest validation v2

Date: 2026-07-29

Status: CPU/reference implementation passed; adversarial implementation review,
full-rank evidence, CUDA upload, and checkpoint startup remain open.

This supersedes v1. Before external review, a continuation audit found that v1
proved internal manifest/descriptor agreement but did not compare the
re-signed pair with an engine-owned complete GLM-5.2 inventory. V2 closes that
gap.

## Candidate

The proved implementation is commit
`4bf7bb5e817e01cc299058b56a488b35011fd79d`.

| Input | SHA-256 |
|---|---|
| `crates/glm-format/src/rank_manifest.rs` | `3b94f2306e2c0ee82f342b66b945f48767441463929e12b575648f9ccda99d6b` |
| `crates/glm-format/src/native_reader.rs` | `24eef8432a8dff2e830a8ec63e4e46bffcfafd94486e64fbb467945825ab0089` |
| `crates/glm-format/src/checkpoint.rs` | `08450f0cb33e592ec76dfbe655b06580ba1743e60f3109a65218d052dbea406c` |
| `crates/glm-format/src/lib.rs` | `52aea8d21b3c3a504a46aa3c9517233fb8b438374ce5fb2e9ad07dc316ee7c0b` |
| `crates/glm-cli/src/main.rs` | `3b76e43b81a1bf7a540565e4b8356999a1b2dcc9e5c8dd1036d4e3b17708128c` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `scripts/local-checks.sh` | `378a717bc9d592bac68ac999c6a91d4655b633177a9005529288096a3d2a029b` |

## Engine-owned anchors

Production schema v0.2.2 now requires exactly 59,585 tensors and
81,590,319,104 primary-plus-auxiliary source bytes per rank. The canonical
tensor arrays generated from the complete pinned plans have these fixed
rank-specific SHA-256 values:

| Rank | Tensor-contract SHA-256 |
|---:|---|
| 0 | `460883a09b5c247bba5458078e4036c090a78cb33824399b7a1e94be5330b12f` |
| 1 | `a86485e73ebc4ece3066518543c590b3adb75f8749d78d7adbdf048874311386` |
| 2 | `47ce5c931ccf44008345ddaaf8d72ca82a56508c92f0b2a47ed1cc4ec25a9a79` |
| 3 | `6fc13c23ac6e9c0aeb9365afb5a05ff284478220a2425e285a64434885a046d4` |

The complete observed 92-file source map is also fixed independently. Its
basis is the 8,528-byte `MANIFEST.sha256` at model revision
`9297b9f1d53af5c67cffa01e30cc071a1ff7144b`, whose SHA-256 is the already
compiled
`bfb6dc39f28da08c1cfc5b89603414046adf7003152d69e9ee350e11f7a1fa63`.
Replace only the two reviewed publisher exceptions with their immutable
revision digests, encode the resulting sorted filename-to-lowercase-hex map as
compact JSON, and hash it. The required digest is
`ad1e4fb286adbc261a2800ab17e4abde5bcd13efb22b150d65ec42b47e2af5fe`.

The source verifier recomputes this map after hashing every source file. The
native reader recomputes it from the production manifest. Thus merely claiming
the pinned source-manifest digest while supplying a rewritten 92-file map no
longer passes.

## Additional v2 hardening

- Protected BF16, FP16, and FP32 codecs now require exactly matching source
  dtypes; EXL3 requires the exact component source dtype.
- Codec class and source-binding kind are coupled. EXL3 cannot masquerade as a
  replicated protected tensor.
- Replicated bindings require the tensor TP axis itself to be `-1`.
- The full four-rank golden-contract test serializes and validates every
  generated tensor on all ranks, including source geometry and reconstruction.
- `native-rank-proof` compares the header/manifest weight-policy digest with
  the engine's compiled capacity-EXL3 policy in addition to operation-manifest,
  kernel-ABI, tensor-count, rank-set, and payload checks.
- The one-tensor typed fixture is explicitly a core-parser proof. The
  production entry point proves that it fails the pinned-inventory gate.

## Proof

The complete local gate ran with:

```text
GLMAXX_TOKENIZER_DIR=/tmp/glmaxx-tokenizer.uQIinY \
  scripts/local-checks.sh
```

Result:

- 225 Rust tests passed;
- formatting and workspace Clippy with warnings denied passed;
- CUDA-FFI type checking and Clippy passed against the local stub boundary;
- deterministic CPU, matrix, operation-manifest, native fixture, engine,
  serving, and tokenizer proofs matched checked-in fixtures; and
- all candidate-based review handoffs hash-verified.

The pinned public source manifest was fetched only to independently derive the
source-map constant. Its exact byte count and compiled SHA-256 matched. A
temporary Rust derivation independently reproduced the Ruby-derived canonical
map digest and was removed. No checkpoint shard or model weight was
downloaded.

## Explicit exclusions

This remains a CPU parser/provenance result. It does not accept the separately
pending format-ABI r2 or checkpoint-load transaction r2 gates. It does not
validate a published complete four-rank conversion, allocate pinned memory or
HBM, upload weights, create a CUDA context, launch a kernel, adopt an arena,
reach model startup, establish output quality, or measure performance.

cn4 remained released to another development workload and was not contacted.

