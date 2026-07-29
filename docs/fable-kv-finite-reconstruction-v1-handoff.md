# Fable handoff: KV finite reconstruction v1

Date: 2026-07-29

Status: adversarial implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`757d5cf44074a167a6434f708939719ef8550e1e`

Required result path:
`fable-kv-finite-reconstruction-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`kv-finite-reconstruction-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/kv.rs` | `fe5f4b8e07c8a32c6534f6217d62057f3ddd7c4b1abfcc00489c550a39660721` |
| `docs/indexer-key-scale-proof-v1.md` | `fd7df63054ee31da37ea22b4bd46f0078bd81e11f28981f20731b5d5d621c215` |
| `docs/kv-finite-reconstruction-proof-v1.md` | `7c76c8b4690ab08e13a3814e63ba6fe4a2c23253ea29dc7ce7c8b3bfde716af2` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-kv-finite-reconstruction-v1-handoff.md
cargo test --offline -p glm-cache
cargo clippy --offline -p glm-cache --all-targets -- -D warnings
```

## Review boundary

This gate covers only finite-reconstruction enforcement in the CPU
target/draft KV and indexer-key record paths. It supplements, but does not
replace, the separate indexer power-of-two-scale correction.

It does not accept CUDA KV kernels, attention, cache transfer, prefix
publication, a checkpoint, model output, quality, capacity, or performance.
It does not authorize cn4.

## Required adversarial questions

1. Enumerate every multiply used to reconstruct NoPE, RoPE, and indexer-key
   values. Does every scale intermediate and final value now have an
   effective finite check before successful return?
2. Can operation ordering, zero times an extreme scale, subnormal
   underflow, signed zero, NaN, or infinity allow a non-finite value to pass
   either encoder or decoder?
3. Can a crafted record with individually finite FP32 scales and finite
   E2M1/E4M3 codes still yield infinity or NaN while returning `Ok`?
4. Do writer-side checks prevent creation of a record that the hardened
   decoder would reject for reconstruction overflow?
5. Is accepting positive and negative `f32::MAX` NoPE/RoPE input safe in
   the exact candidate, and does its decoded output remain finite?
6. Are error classifications fail-closed and appropriate:
   `NonFinite` while encoding and `Encoding` while decoding externally
   supplied bytes?
7. Does the candidate preserve the exact 368-byte and 132-byte ABIs,
   numerical scale policies, finite E4M3/E2M1 encoding, and ordinary valid
   record bytes?
8. Are the crafted overflow tests independent enough to prove the new
   checks, or can they pass without executing the vulnerable products?
9. Are the proof document's 231-test count, 31-handoff count, skipped
   tokenizer statement, and all non-claims accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- target/draft KV encoder finite enforcement is accepted;
- target/draft KV decoder finite enforcement is accepted;
- indexer encoder/decoder finite enforcement is accepted;
- ABI and finite model-range behavior are preserved; and
- the CPU proof and its non-claims are accurate.

Only if all five answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, unchecked product,
non-finite successful output, ABI drift, or a test that does not reach the
claimed path.

The token accepts only this CPU record-path hardening. It does not satisfy
any SM120, model-quality, or end-to-end gate.
