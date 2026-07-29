# Review provenance verifier v1

Date: 2026-07-29

Implementation commit:
`59e11e5b14737020f72659b8a49d8c82982deba8`

This record covers a GPU-independent, fail-closed verifier for adversarial
review handoffs. It addresses the recurring failure where a review was run
against stale bytes or a handoff made an incorrect claim about its own
candidate.

It is evidence infrastructure. It does not accept any pending gate and does
not establish CUDA correctness, model quality, or performance.

## Contract

`glmaxx review-proof`:

- requires exactly one full, lowercase 40-hex candidate commit;
- resolves that object as a Git commit;
- parses the first provenance table after the candidate label;
- requires unique clean repository-relative paths and lowercase SHA-256
  values;
- reads every input from `candidate:path`, not from the working tree;
- recomputes every SHA-256 and fails on the first mismatch;
- derives the requested acceptance token from either the current labeled form
  or one legacy bare-token form;
- classifies an optional review artifact as accepted only when the requested
  token appears exactly once as a complete line; and
- rejects unexpected or duplicate bare acceptance tokens.

`glmaxx review-proof-all` sorts and verifies every
`docs/fable-*-handoff.md` with a candidate label. Only the two predating the
candidate-pinning convention are explicitly exempt:

- `docs/fable-phase-a-engine-handoff.md`;
- `docs/fable-review-handoff.md`.

Any newly added handoff without a candidate label fails the repository-wide
check. The suite verifies pinned candidate bytes; review-token classification
remains an explicit per-review command because a handoff does not define a
unique future review filename.

Both commands emit deterministic JSON for identical repository state and
inputs. The repository-wide command is part of `scripts/local-checks.sh`.

## Pinned implementation bytes

| Artifact at implementation commit | SHA-256 |
|---|---|
| `crates/glm-cli/src/review.rs` | `d2c2d2756b94df8fb5555f578e7c907bef7c09b7b10fb3f310f45566f73c1c45` |
| `crates/glm-cli/src/main.rs` | `fab1e7886d71576473cfb279b8f6ace09633ec5da2a70cbf968c890355f39337` |
| `scripts/local-checks.sh` | `378a717bc9d592bac68ac999c6a91d4655b633177a9005529288096a3d2a029b` |

## Verification

The following commands passed from the clean implementation commit:

```text
cargo test -p glm-cli review::tests --offline
cargo clippy -p glm-cli --all-targets --offline -- -D warnings
cargo run --offline -p glm-cli --bin glmaxx -- review-proof-all . <temporary-output>
cargo run --offline -p glm-cli --bin glmaxx -- review-proof docs/fable-exl3-source-projection-handoff.md fable-exl3-source-projection-v1.md
GLMAXX_TOKENIZER_DIR=<pinned-external-tokenizer> ./scripts/local-checks.sh
```

Observed results:

- 5 verifier unit tests passed;
- 19 candidate-based handoffs and every pinned input hash passed;
- the EXL3 source-projection review was correctly classified
  `token_state=withheld`, with zero exact token lines;
- the full workspace gate passed 216 Rust tests, formatting, Clippy with
  warnings denied, CUDA-FFI host compilation, deterministic fixture
  regeneration, CPU/engine/serving proofs, and the pinned external-tokenizer
  proof; and
- CUDA compilation was skipped because the local host has no `nvcc`.

Toolchain:

| Component | Identity |
|---|---|
| Host | macOS 26.5.2 build 25F84 |
| Rust | `rustc 1.92.0 (ded5c06cf 2025-12-08)`, LLVM 21.1.3 |
| Cargo | `cargo 1.92.0 (344c4567c 2025-10-21)` |
| C++ syntax check | Apple clang 21.0.0 |

The tokenizer bundle and generated proof directory remained external to Git.
No model weights, checkpoint bytes, raw benchmark output, CUDA context, or GPU
process was used.

## Exclusions

This verifier does not prove that a handoff's prose is true, that its requested
review questions are satisfiable, or that a reviewer reached the correct
technical verdict. It proves which committed bytes the handoff pins and
whether an explicitly supplied review contains exactly the requested token.
The substantive adversarial and execution gates remain unchanged.
