# KV finite-reconstruction CPU proof v1

Date: 2026-07-29

Implementation commit:
`abd851d91d371846c824d4a6a7208c2e89821166`

Status: CPU/reference implementation passed; independent review pending

GPU claim: none

## Invariant

Every successful `KvRecord` and `IndexerKeyRecord` encode or decode now
guarantees that every intermediate scale and every reconstructed FP32 value
is finite.

The record decoders already rejected non-finite stored FP32 scales and
non-finite E4M3 codes. That was insufficient: two individually finite
factors can multiply to infinity. A corrupted 368-byte record could
therefore pass its factor checks and publish non-finite NoPE or RoPE output.

The candidate checks:

- E4M3 group scale times the NoPE outer scale;
- E2M1 value times that reconstructed group scale;
- E4M3 RoPE value times the RoPE scale; and
- E4M3 indexer value times its power-of-two scale.

The writer performs the equivalent checks before returning a serializable
record. An encoder-side overflow returns `KvError::NonFinite`; a
decoder-side overflow or non-finite reconstruction returns
`KvError::Encoding`.

This does not change the 368-byte target/draft KV ABI, the 132-byte indexer
ABI, scaling formulas, rounding, or finite model-range record bytes.

## Proof

New fixtures include:

- a finite `f32::MAX` NoPE outer scale, finite E4M3 group scale, and finite
  E2M1 value whose products overflow;
- a finite `f32::MAX` RoPE scale and finite maximum E4M3 value whose product
  overflows;
- the corresponding indexer-key overflow record; and
- an encoder input containing positive and negative `f32::MAX` in NoPE and
  RoPE, which successfully round-trips to finite output.

The full local gate passed 231 Rust tests with zero failures, workspace
formatting and Clippy with warnings denied, CUDA FFI type checks, all
deterministic CPU proofs, and all 31 then-present review handoff proofs.

Commands:

```text
cargo fmt --check
cargo test -p glm-cache
cargo clippy -p glm-cache --all-targets -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-cache/src/kv.rs
fe5f4b8e07c8a32c6534f6217d62057f3ddd7c4b1abfcc00489c550a39660721

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a

docs/indexer-key-scale-proof-v1.md
fd7df63054ee31da37ea22b4bd46f0078bd81e11f28981f20731b5d5d621c215
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its pinned fixture was not changed.
No CUDA compiler or GPU was used. This proof does not establish device KV
encoding, model attention correctness, long-context quality, capacity, or
performance, and it does not authorize cn4.
