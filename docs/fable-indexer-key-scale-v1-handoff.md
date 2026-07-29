# Fable handoff: indexer-key scale CPU proof v1

Date: 2026-07-29

Status: adversarial implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`13f0c598c192f389ae664a22ffc2f81e58bd9f31`

Required result path:
`fable-indexer-key-scale-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`indexer-key-scale-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| prior `fable-manifest-abi-v022.md` | `505bf452895cde7598e8e03141bd8bd381729f31f5ee95c11c036d26c79c8d42` |
| `crates/glm-cache/src/kv.rs` | `b79994dfb6d83f848f0a6b0b3b23868b02f3cfffd779a5834ac9c46c026c206a` |
| `crates/glm-format/src/float.rs` | `e2f547b3ec5efae0d9fdb975136164f557e24a93770a5791c4ca7d7359e7e1de` |
| `docs/indexer-key-scale-proof-v1.md` | `fd7df63054ee31da37ea22b4bd46f0078bd81e11f28981f20731b5d5d621c215` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-indexer-key-scale-v1-handoff.md
cargo test --offline -p glm-cache
cargo clippy --offline -p glm-cache --all-targets -- -D warnings
```

## Review boundary

This is the narrow corrective review for prior manifest-review finding `m7`.
It also covers the newly explicit fail-closed behavior for a key record whose
E4M3 value times its FP32 scale would overflow.

It does not re-review or accept the manifest gate's FC2 CUDA control, durable
sidecar model, profile budget, or any other prior finding. It does not accept
an SM120 kernel, authorize cn4, authorize checkpoint conversion, or establish
model quality or performance.

## Required adversarial questions

1. For every positive finite binary32 input with a representable finite
   power-of-two ceiling, does `ceil_positive_power_of_two` return exactly the
   smallest power of two greater than or equal to the input?
2. Are all subnormal cases correct, including exact subnormal powers,
   values between them, and promotion from the maximum subnormal interval to
   the minimum normal value?
3. Does the normal path preserve exact powers, round a nonzero fraction by
   exactly one exponent, and reject the interval above `2^127` instead of
   constructing infinity or wrapping?
4. Does `is_positive_finite_power_of_two` accept exactly the positive finite
   normal and subnormal powers of two while rejecting zero, negative zero,
   negative values, non-powers, infinities, and NaN?
5. Re-derive the section 16.1 record rule. Does applying the bit-exact
   ceiling to the already-rounded binary32 value
   `max(amax, 1e-4) / 448` implement the specified policy without a hidden
   `powf`, `log2`, host-libm, rounding-mode, or fast-math dependency?
6. Can any finite 128-element key produce a published record that decodes to
   a non-finite value? Can a corrupted but syntactically finite code/scale
   pair escape the decoder's new reconstruction check?
7. Do the boundary tests actually cover every binary32 exponent class and
   both sides of each applicable power boundary, or is any edge masked by a
   tautological expected-value calculation?
8. Is returning `KvError::NonFinite` for a finite input whose quantized
   reconstruction would overflow safely fail-closed without changing the
   132-byte ABI or the scale policy for valid model-range records?
9. Does this fully close prior finding `m7`, or is any floating
   transcendental operation still involved in indexer scale generation or
   validation?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- bit-exact power-of-two ceiling construction is accepted;
- bit-exact scale validation is accepted;
- encode/decode overflow handling is accepted;
- the CPU proof and its non-claims are accurate; and
- prior manifest-review finding `m7` is closed.

Only if all five answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, uncovered exponent class,
incorrect subnormal behavior, possible non-finite reconstruction, or a
tautological boundary proof.

The token accepts only this CPU record-path correction. It does not satisfy
the separate `manifest-abi-v0.2.2-accepted` gate.
