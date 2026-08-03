# Adversarial CPU review: indexer-key scale v1

Date: 2026-07-31

Reviewer: Fable (adversarial CPU implementation review, queue row 29)

Handoff: `docs/fable-indexer-key-scale-v1-handoff.md`

Result path note: the handoff requests `fable-indexer-key-scale-v1.md` at the
repository root; the operator directed review artifacts into `docs/reviews/`
instead, so this review is recorded at
`docs/reviews/fable-indexer-key-scale-v1.md`.

Review candidate commit:

13f0c598c192f389ae664a22ffc2f81e58bd9f31

Reviewed in a detached worktree pinned to exactly that commit. No GPU, no
CUDA, no cn4 connection was used.

## Input table (pinned SHA-256, verified at START and FINISH)

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a |
| prior `fable-manifest-abi-v022.md` (repository root) | 505bf452895cde7598e8e03141bd8bd381729f31f5ee95c11c036d26c79c8d42 |
| `crates/glm-cache/src/kv.rs` | b79994dfb6d83f848f0a6b0b3b23868b02f3cfffd779a5834ac9c46c026c206a |
| `crates/glm-format/src/float.rs` | e2f547b3ec5efae0d9fdb975136164f557e24a93770a5791c4ca7d7359e7e1de |
| `docs/indexer-key-scale-proof-v1.md` | fd7df63054ee31da37ea22b4bd46f0078bd81e11f28981f20731b5d5d621c215 |

## Provenance verification statement

START (review begin): `shasum -a 256` over all five input files in the
detached worktree at `13f0c598c192f389ae664a22ffc2f81e58bd9f31` matched every
pinned SHA-256 above, and
`cargo run --offline -q -p glm-cli --bin glmaxx -- review-proof
docs/fable-indexer-key-scale-v1-handoff.md` returned verdict `PASS` with all
expected/actual input hashes equal and candidate commit
`13f0c598c192f389ae664a22ffc2f81e58bd9f31`.

FINISH (review end): the same `shasum -a 256` pass over the worktree and the
same `review-proof` invocation were repeated after all analysis and
computation. Every hash again matched the pinned values and the verdict was
again `PASS`. The candidate is not stale; both hash sets are identical.

## Gate commands (run once, in the pinned worktree)

- `cargo test --offline -p glm-cache`: 44 passed, 0 failed, 0 ignored
  (plus 0 doc-tests), finished in 4.33s.
- `cargo clippy --offline -p glm-cache --all-targets -- -D warnings`:
  finished clean, no warnings, no errors.

## Independent computational verification

All checks used standalone harnesses in the reviewer scratchpad
(`verify_pow2.rs`, `verify_overflow.rs`, `rustc -O`), with the functions
under test copied byte-for-byte from the pinned worktree and references
constructed independently from an explicit ascending list of all 277 positive
finite binary32 powers of two (`2^-149 ..= 2^127`) built by direct bit
construction.

1. `ceil_positive_power_of_two`, exhaustive over every positive finite
   nonzero f32 bit pattern (`0x0000_0001 ..= 0x7f7f_ffff`, 2,139,095,039
   inputs) against a binary-search reference: 0 mismatches. Every input with
   a representable ceiling returns exactly the smallest power of two greater
   than or equal to it; every input in `(2^127, f32::MAX]` returns
   `Err(KvError::Scale)` — no infinity, no wraparound. Rejection classes
   (`0.0`, `-0.0`, sampled negatives, both infinities, NaN) all rejected.
2. `is_positive_finite_power_of_two`, exhaustive over all 2^32 bit patterns
   against list membership: 0 mismatches. Accepts exactly the 277 positive
   finite powers (23 subnormal, 254 normal); rejects `0`, `-0`, negatives,
   non-powers, infinities, NaN.
3. Record rule: `ceil(fl32(max(amax, 1e-4) / 448))` checked for 8,523,567
   amax values (every 251st positive finite bit pattern plus all
   per-exponent boundary offsets, `0.0`, and `f32::MAX`): scale is always a
   valid power of two, `scale >= raw`, and minimal (next lower power is
   `< raw`). 0 failures.
4. Overflow fail-closed: single-amax-lane encoder simulation exhaustive over
   both top binades (`0x7e80_0000 ..= 0x7f7f_ffff`, 16,777,216 patterns)
   plus a stride-1009 sweep of the full positive finite range: 18,356,316
   accepted with finite reconstruction in every lane, 524,288 rejected as
   `NonFinite` (exactly the near-`f32::MAX` band whose E4M3 code x scale
   product overflows), 0 anomalies. Decoder simulation over every
   syntactically finite (code, valid power-of-two scale) pair
   (255 x 277): all 560 overflowing products are caught by the
   reconstruction finiteness check; 0 escapes.
5. `2.0_f32.powi(k)` exactness for the E4M3 exponent range `k in -9..=8`
   verified bit-identical to direct exponent-field construction (relevant to
   `decode_e4m3`, which is in the value path, not the scale path).

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. `docs/indexer-key-scale-proof-v1.md` line 6 states implementation commit
   `35a9d0358ae49bda3fcd571bc526b20cc7ec7d03`, one commit behind the pinned
   candidate `13f0c598c192f389ae664a22ffc2f81e58bd9f31` (which records the
   proof document itself). Verified `35a9d035...` is the direct ancestor and
   that `crates/glm-cache/src/kv.rs` carries the identical pinned SHA-256 at
   the candidate, so the reviewed bytes are the proved bytes. Cosmetic
   provenance wrinkle only; future proofs should state the commit at which
   the proof lands or pin the file hash as this one also does.
2. `decode_e4m3` (`crates/glm-format/src/float.rs` lines 43 and 47) uses
   `2.0_f32.powi(...)`. This is integer exponentiation lowered to exact
   binary32 multiplications, verified bit-exact for the whole E4M3 exponent
   range, and it sits in the value-code path, not in scale generation or
   validation. Not an m7-class defect; noted so nobody later mistakes it for
   one, and replacing it with `f32::from_bits` exponent construction would
   make the non-dependence self-evident.

### QUESTION

1. Binary32 rounding of `max(amax, 1e-4) / 448` can, for an exact quotient
   within one rounding ulp above a power of two, select a scale one step
   smaller than the smallest power of two >= the exact real quotient. Then
   `amax / scale` is at most `448 * (1 + 2^-24)` and nearest-even E4M3
   encoding clamps it to the 448 code. This is consistent with the spec
   16.1 wording ("saturated-finite E4M3") and with the handoff's own framing
   of the rule as the bit-exact ceiling of the already-rounded binary32
   value, so it is the specified policy, not a defect. Flagged only so the
   eventual GPU kernel implements the same rounding order (binary32 divide,
   then bit ceiling) rather than an exact-quotient ceiling.

## Answers to the nine required questions

1. Yes. Verified exhaustively over all 2,139,095,039 positive finite nonzero
   binary32 patterns against an independent 277-power reference: for every
   input with a representable finite ceiling, `ceil_positive_power_of_two`
   returns exactly the smallest power of two >= the input, with zero
   mismatches.
2. Yes. All subnormal behavior is correct: the 23 exact subnormal powers are
   preserved, every value strictly between consecutive subnormal powers
   rounds up exactly one position, the minimum subnormal `f32::from_bits(1)`
   is preserved as `2^-149`, and every value in the maximum subnormal
   interval (up to `0x007f_ffff`) promotes to the minimum normal `2^-126`
   via `fraction.next_power_of_two()` producing bit pattern `1 << 23`.
   Covered by the exhaustive sweep, not just the unit tests.
3. Yes. Exact normal powers return unchanged (`fraction == 0` path); any
   nonzero fraction advances the exponent field by exactly one step
   (`+0x0080_0000`); and every input in `(2^127, f32::MAX]` hits
   `next_exponent == F32_EXPONENT_MASK` and returns `Err(KvError::Scale)`.
   The exhaustive sweep confirms no input constructs an infinity bit pattern
   and no exponent wraparound occurs.
4. Yes. Exhaustive over all 2^32 bit patterns: `is_positive_finite_power_of_two`
   accepts exactly the 277 positive finite powers of two (subnormal single-bit
   fractions and normal zero-fraction values) and rejects `0.0`, `-0.0`, all
   negatives, all non-powers, both infinities, and every NaN pattern, with
   zero mismatches against independent list membership.
5. Yes. The pinned rule is `scale = ceil_positive_power_of_two(amax.max(1.0e-4) / 448.0)`
   (`crates/glm-cache/src/kv.rs` lines 160-161): one binary32 `max`, one
   binary32 divide by the exactly representable constant 448, then pure
   integer bit manipulation. Workspace-wide grep confirms no `powf`, `log2`,
   `exp2`, `ln`, or `log10` anywhere in `crates/` outside softmax `exp()` in
   attention accumulation; there is no libm call, no rounding-mode
   sensitivity (only IEEE-754 default round-nearest-even division, which
   Rust guarantees), and no fast-math contraction (Rust does not enable
   fast-math). The re-derivation matches spec section 16.1 as framed by the
   handoff (bit-exact ceiling of the already-rounded binary32 quotient; see
   QUESTION 1 for the one-ulp boundary note, which is the specified policy).
6. No published record can decode to a non-finite value, and no corrupted
   syntactically finite pair escapes. The encoder checks
   `(decode_e4m3(code) * scale).is_finite()` for every one of the 128 lanes
   before publication (kv.rs lines 164-167) and returns
   `KvError::NonFinite` otherwise; the decoder independently validates the
   scale as a positive finite power of two, each code as finite-decoding,
   and each reconstructed product as finite (kv.rs lines 174-192). The
   255 x 277 exhaustive pair sweep found all 560 overflowing
   code-times-scale products caught, zero escapes; finite x finite cannot
   produce NaN, so the finiteness check is complete.
7. Yes, coverage is genuine and non-tautological. The pinned boundary tests
   (`power_of_two_ceiling_is_exact_at_every_f32_exponent_boundary`,
   `power_of_two_validation_is_bit_exact_and_fail_closed`) construct every
   expected value by independent bit arithmetic (`1 << shift`,
   `exponent << 23`, `(exponent + 1) << 23`), never by calling the function
   under test: all 23 subnormal power positions, all 254 normal exponent
   fields, one ULP below and above each applicable boundary, the maximum
   subnormal to minimum normal promotion (exponent 1, `exact_bits - 1`), and
   rejection above `2^127` (exponent 254 `above` arm). The `shift > 1`
   skip of the below case is correct, because `f32::from_bits(1)` is itself
   a power. My exhaustive sweep independently confirms there is no masked
   edge anywhere in the domain.
8. Yes. Returning `KvError::NonFinite` for a finite key whose quantized
   reconstruction would overflow is safely fail-closed: the record is
   rejected before any bytes are published, the layout remains exactly 132
   bytes (128 E4M3 codes plus one little-endian FP32 scale) for every
   accepted record, and the scale policy for model-range inputs is
   untouched — rejection occurs only for `amax` above roughly
   `248 * 2^120` (about 3.3e38, within 3 percent of `f32::MAX`), far
   outside any model-range activation. The encoder sweep shows exactly the
   524,288 top-band patterns rejected and every accepted record
   reconstructing finite.
9. Yes, m7 is fully closed. The defective
   `2.0f32.powf(raw_scale.log2().ceil())` is gone; scale generation is pure
   binary32 field manipulation and scale validation is a pure bit predicate.
   A workspace-wide search finds no `powf`, `log2`, or `exp2` anywhere in
   `crates/`. The only remaining exponentiation near the record path is
   `powi` inside `decode_e4m3` (value codes, not scale; verified bit-exact,
   see MINOR 2) and softmax `exp()` in attention accumulation, which is
   outside indexer scale generation and validation.

## Five required statements

- Bit-exact power-of-two ceiling construction is accepted: YES.
- Bit-exact scale validation is accepted: YES.
- Encode/decode overflow handling is accepted: YES.
- The CPU proof and its non-claims are accurate: YES (accurate on defect,
  mechanism, coverage, and non-claims; the one-commit implementation-commit
  wrinkle is recorded as MINOR 1 and does not alter the reviewed bytes).
- Prior manifest-review finding m7 is closed: YES.

## Architecture & maintainability

The correction is well-shaped: the two helpers are private, total over their
domain, and expressed directly in the IEEE-754 bit vocabulary
(`F32_EXPONENT_MASK` / `F32_FRACTION_MASK` / `F32_EXPONENT_STEP`) so the
proof obligation is local to 38 lines. Reusing `u32::next_power_of_two` for
the subnormal coefficient is the right altitude — the promotion to minimum
normal falls out of the representation instead of being a special case, and
the comment at kv.rs lines 18-20 records why. Encoder and decoder enforce
the invariant independently, so neither trusts the other across the
serialization boundary, which is the correct posture for a cache record that
survives eviction and restore. Two small future-proofing notes: the helpers
are duplicated verification targets for the eventual SM120 kernel, so
consider promoting them (with their exhaustive-test obligation) to
`glm-format` alongside the E4M3 code if the GPU path will need a host-side
oracle; and MINOR 2's `powi` in `decode_e4m3` would read more obviously
bit-deterministic as exponent-field construction. Neither affects
correctness at this commit.

## Token decision

Every input hash matched at start and finish, `review-proof` passed twice,
the gate commands passed cleanly, the exhaustive and dense independent
verifications found zero mismatches, there are no blockers and no majors,
and all five required statements are an unqualified YES. This acceptance
covers only the CPU indexer-key record-path correction for finding m7 and
the added overflow fail-closed behavior; it does not satisfy the separate
`manifest-abi-v0.2.2-accepted` gate, accept any SM120 kernel, authorize cn4
or checkpoint conversion, or establish model quality or performance.

indexer-key-scale-v1-accepted
