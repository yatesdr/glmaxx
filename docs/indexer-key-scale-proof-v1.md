# Indexer-key scale CPU proof v1

Date: 2026-07-29

Implementation commit:
`35a9d0358ae49bda3fcd571bc526b20cc7ec7d03`

Status: CPU/reference implementation passed; independent review pending

GPU claim: none

## Defect closed by the candidate

The first manifest/ABI review identified that
`IndexerKeyRecord::encode` used:

```text
2.0f32.powf(raw_scale.log2().ceil())
```

Neither `powf` nor `log2` is required to return a correctly rounded result.
An off-by-one-ULP library result could therefore serialize a scale that was
not an exact power of two and that the decoder subsequently rejected.

The candidate constructs the ceiling directly from the IEEE-754 binary32
exponent and fraction fields. It:

- preserves an exact positive finite power of two;
- rounds every other representable positive value to the smallest
  representable power of two above it;
- handles subnormal powers by rounding the integer subnormal coefficient;
- promotes a subnormal result with coefficient `1 << 23` to the minimum
  normal value;
- rejects a value above `2^127` when no finite binary32 power-of-two ceiling
  exists; and
- uses a bit predicate, rather than `log2`, to validate decoded scales.

This implements the scale rule in `spec/engine-v0.md` section 16.1:

```text
smallest power of two greater than or equal to
max(max(abs(k)), 1e-4) / 448
```

## Additional fail-closed behavior

The same boundary proof exposed a separate extreme-input case. A finite key
near `f32::MAX` can select a valid finite scale and E4M3 code while the
decoded `code * scale` overflows binary32. The encoder now rejects such a
record before publication, and the decoder rejects an externally supplied
record whose reconstructed value is non-finite.

This is a failure-safety change, not a different scale policy. Ordinary
model-range records retain the same on-disk 132-byte ABI.

## Test coverage

The focused `glm-cache` gate passed 44 tests. New cases cover:

- all 23 positive subnormal power-of-two positions;
- all 254 positive normal exponent fields;
- the representable value immediately below and above each applicable
  power-of-two boundary;
- positive/negative zero, negative values, non-powers, infinities, and NaN;
- a serialized non-power-of-two scale;
- the maximum finite-key overflow path; and
- a crafted valid-code/valid-scale record whose reconstruction overflows.

The full local gate passed 230 Rust tests with zero failures, workspace
Clippy with warnings denied, format checks, CUDA FFI type checks, and all
deterministic CPU proof commands. It also verified 29 review handoffs; no
independent acceptance result was inferred.

Commands:

```text
cargo fmt --check
cargo test -p glm-cache
cargo clippy -p glm-cache --all-targets -- -D warnings
scripts/local-checks.sh
```

Toolchain:

```text
rustc 1.92.0 (ded5c06cf 2025-12-08)
cargo 1.92.0 (344c4567c 2025-10-21)
```

Relevant hashes:

```text
crates/glm-cache/src/kv.rs
b79994dfb6d83f848f0a6b0b3b23868b02f3cfffd779a5834ac9c46c026c206a

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a

fable-manifest-abi-v022.md
505bf452895cde7598e8e03141bd8bd381729f31f5ee95c11c036d26c79c8d42
```

No CUDA toolchain or GPU was used. This proof does not accept the wider
manifest ABI gate, authorize cn4, establish device correctness, or establish
model quality or performance.
