# Fable adversarial review — direct EXL3 source projection v1

Date: 2026-07-29
Reviewer: Fable (Claude), independent gate review
Handoff: `docs/fable-exl3-source-projection-handoff.md`

## Verdict

**Token withheld.** Decisions 2 and 3 are accepted. Decision 1 is accepted on
the mathematics — which I verified three independent ways — but cannot be an
unqualified YES because the handoff's required test property is demonstrably
false: the in-repo inverse-scatter test is tautological, and the two tiles it
checks are exactly the two fixed points of a tile-address transposition.
A one-test fix and re-pin should pass quickly. A second MAJOR: the pinned
`scripts/cn4-exl3-phase-b.sh` can never pass its own GPU-inventory check, so
the qualification run this gate feeds is unrunnable with the reviewed bytes.

## Provenance

Reviewed at the pinned candidate commit in a dedicated worktree:

- Candidate base commit: `731c3bb02104edad0e154dcc63a26fe6bf224d7d` (exists;
  reviewed exactly these bytes).
- Repository `HEAD` at review time was `33616f7f81a58c698bc4e2cdcee0671904091ea9`,
  9 commits ahead. At `HEAD`, 6 of 17 handoff inputs have drifted
  (`crates/glm-cuda/src/abi.rs`, `ffi.rs`, `lib.rs`,
  `kernels/include/glmaxx_kernel.h`, `crates/glm-cli/src/main.rs`,
  `scripts/cn4-exl3-phase-b.sh`). All 6 match the handoff table exactly at
  `731c3bb`, so this is forward drift from concurrent work, not a stale or
  corrupted handoff. **Any acceptance from this review is scoped to `731c3bb`
  and does not transfer to `HEAD`.**
- All 17 input SHA-256s were verified against the handoff table inside the
  `731c3bb` worktree at review start and re-verified at review finish; every
  hash matched both times (the worktree is a detached, unmodified checkout).
- Prior `fable-adversarial-v2.md` and `docs/fable-v2-disposition.md` hashes
  verified.
- cn4 raw evidence: all 11 critical raw-record hashes listed in
  `docs/cn4-exl3-source-preparation-20260729.md` were verified over ssh
  against `/home/derek/glmaxx/evidence/prepare-e4f0290` and `ffi-e4f0290`
  (read-only). Contents corroborate the prose: source commit `e4f0290`,
  verdict `PREPARED_NO_DEVICE_LAUNCH`, real `sm_120f` fatbin ELF resource
  records (rotate-in 22 reg / 1,536 B smem; projection 38 reg / 0 smem;
  rotate-out 24 reg / 1,536 B smem), exactly the three exported EXL3 symbols,
  `abi-check.json` with 144-byte / 16-aligned descriptor and 13,312-byte M1
  workspace, `gpu_launched: false`, and the NVIDIA-driver-unavailable banner.
- The preparation evidence was built at `e4f0290`, not `731c3bb`. Verified:
  `git diff e4f0290..731c3bb` touches only docs and adds
  `scripts/cn4-exl3-phase-b.sh`; every compiled kernel/crate source is
  byte-identical, so the build evidence carries over to the candidate.
- `cargo test --workspace` at `731c3bb` on this host: 156 passed, 0 failed —
  matching the count in the cn4 `cargo-test.txt` record.

## Independent verification performed

1. **Inverse mapping, all 256 positions.** I implemented the forward
   lane/weight scatter from `docs/exl3-trellis-cpu-contract.md` and the
   inverse from the handoff/CUDA/Rust from scratch and brute-forced all
   `(lane, weight)` and all `(row, column)`: the forward scatter is a
   bijection onto the 16×16 tile and the inverse is its exact inverse in both
   directions, including the four row quadrants, parity, and low/high column
   halves. The CUDA `decode_weight_bits` and Rust `inverse_trellis_slot` are
   expression-identical to the verified inverse.
2. **Full-matrix independent reconstruction.** I wrote an independent NumPy
   decoder that uses only the *forward* scatter (never the inverse under
   review), row-major tile addressing, little-endian half-to-word assembly,
   the `+257` window with 24-word cyclic indexing, wrapping `0xCBAC1FED`
   multiplication, `0x8FFF8FFF`/`0x3B603B60`, and FP32-add/FP16-RN rounding,
   ran it on the Rust fixture's xorshift trellis, and reproduced the pinned
   digest `72fd649c…5b4e15` for the whole 6,144×512 matrix exactly. This
   independently confirms the Rust decode path end to end — including tile
   addressing, where a transposition would have changed the digest.
3. **Normalization constant.** `f32(0.08838834764831845)` (CUDA literal),
   Rust `1.0 / 128f32.sqrt()`, and the correctly rounded f32 of the true
   `1/sqrt(128)` are all bit-identical: `0x3db504f3`.
4. **Rounding-boundary walkthrough** of CUDA vs Rust at every FP16/FP32
   boundary (details under Decision 2).
5. **Transpose fixed-point analysis** of the 384×32 tile grid (details under
   Finding MAJOR-1).

## Findings

### BLOCKER

None.

### MAJOR

**MAJOR-1 — the handoff's required transpose-detection property is false,
and the in-repo test provides no independent coverage.**
The handoff requires: "Check that the Rust test's first and last tiles cannot
both pass if tile addressing is transposed." This is false twice over:

- The test `inverse_scatter_recovers_every_trellis_tile_position`
  (`crates/glm-format/src/exl3.rs:722`) compares `decode_native_at(...)`
  against `native[row * n + column]`, where `native` comes from
  `reconstruct_native_f16`, which fills every element by calling the very
  same `decode_native_at`. The comparison is a tautology; it verifies
  determinism, not the mapping.
- Even were the comparison independent, the tile bases it checks —
  `(0, 0)` and `(6_128, 496)`, i.e., tiles `(0,0)` and `(383,31)` — are
  exactly the only two fixed points of a tile-address transposition on the
  384×32 grid: `31r = 383c` with 383 prime has precisely those two solutions,
  which I confirmed by brute force. A transposed `tile_index` passes this
  test at both chosen tiles by construction.

The substantive risk is mitigated: `native_decode_has_stable_content_digest`
pins the full-matrix digest (any transposition changes it), my independent
forward-scatter reconstruction reproduces that digest, and the real-payload
proof recorded NumPy parity. The mapping is correct. But the confirmation the
handoff demands is unsatisfiable as written, and the only in-tree test that
claims to check the inverse checks nothing. Required fix: replace the
tautological assertion with a genuine forward-scatter cross-check (scatter
each decoded `(lane, weight)` value through the contract's forward mapping
and compare), and include at least one tile off the transpose-fixed-point
diagonal, e.g. tile `(0,1)` or `(1,0)`.

**MAJOR-2 — `scripts/cn4-exl3-phase-b.sh` can never pass its GPU-inventory
check.** Line 144: `grep -c '^12\\.0$'` inside single quotes sends the
pattern `^12\\.0$`, which matches a literal backslash; `12.0` never matches,
so `sm120_count` is always 0 and the script exits 70 ("requires exactly four
visible compute-capability 12.0 GPUs") on every host, including a correct
4×SM120 cn4. Verified by direct execution. The failure is fail-closed (no
GPU work runs), but the qualification run this review gates cannot proceed
with the pinned script, and fixing it changes a hash-pinned reviewed input —
so the fix must land before or with the re-pin from MAJOR-1. The pattern
should be `'^12\.0$'`.

### MINOR

**MINOR-1 — the three device validation bits are never exercised.** Bits 1,
2, and 4 (`atomicOr` on non-finite in `rotate_input_f16`,
`project_native_f16`, `rotate_output_f16`) can only fire on non-finite
values, which no synthetic fixture produces; the Rust `download` fail-closed
path (`KernelError::DeviceValidation`) is likewise untested end to end. Not
required for the M1 synthetic gate (the CLI separately fails on any
non-finite comparison), but a deliberate overflow fixture — e.g. saturating
`suh` — is worth adding before real-payload execution so the fail-closed path
has been seen to fire at least once.

**MINOR-2 — early return before `__syncthreads()` in both rotation
kernels.** `if (row >= descriptor.rows || output_offset >= kH128) return;`
precedes `__syncthreads()`. Under the actual launch geometry
(`gridDim.x = rows`, `blockDim.x = 128`) neither condition can be true, so
this is dead code today — but if the geometry is ever changed, threads
exiting before a barrier that others reach is undefined behavior. Prefer
guarding the work, not skipping the barrier.

**MINOR-3 — one plain multiply on the rotation path.** `rotate_input_f16`
computes `scaled` with the plain `*` operator rather than `__fmul_rn`
(`exl3_projection_control.cu:102`). With the pinned CMake flags (no
fast-math, no FTZ) and no adjacent add to contract into an FMA, nvcc emits
`mul.rn.f32` and the semantics match Rust exactly — I checked the flag set —
but it is the one spot that relies on compiler defaults rather than the
explicit-intrinsic discipline the design doc claims. Cheap to make explicit
at the next touch.

**MINOR-4 — preparation evidence commit offset.** The cn4 evidence is pinned
to `e4f0290` while the review candidate is `731c3bb`. Verified harmless (the
diff is docs plus the new phase-b script; compiled sources byte-identical),
but future handoffs should either rebuild at the candidate or state the
equivalence and its proof in the preparation record itself.

### QUESTION

**Q-1** — `compare_f16_output` reports `maximum_relative_error` with
denominator `max(|cpu|, 1e-6)` but the pass/fail tolerance is the absolute
form `0.5 + 0.03·|cpu|` only. Intentional that the relative metric is
informational-only? (No action needed if so; the frozen gate is the absolute
formula, which matches the design doc.)

## Required answers

**1. Is the inverse source-trellis reconstruction accepted for a first SM120
correctness launch?**
Not as an unqualified YES. The mathematics is verified correct — exhaustive
256-position inverse↔forward proof, independent full-matrix forward-scatter
reconstruction matching the pinned digest, bit-exact window/cyclic/multiplier
/mask/rounding semantics, and no signedness, overflow, or rounding divergence
between the CUDA and Rust expressions (CUDA u32 arithmetic wraps identically
to `wrapping_mul`; all bit-index arithmetic fits u32; `tile_base` is u64 on
both sides). But the handoff's mandated check on the Rust test fails as
stated (MAJOR-1): the test is tautological and its tiles are the transpose
fixed points. Fix the test, re-pin `crates/glm-format/src/exl3.rs`, and this
decision should pass without further review risk.

**2. Are the rotation/projection arithmetic and first-launch comparison
accepted?**
YES. The sequence `FP16(input·suh) → H128 → FP16 store → ascending-K
FP32 `__fmul_rn`/`__fadd_rn` → FP16 store → H128 → FP16(H·svh)` is
implemented identically on both sides at every rounding boundary; H128
accumulates indices 0..127 ascending with exact ±negation on both sides; the
normalization constant is bit-identical (`0x3db504f3`); compile flags carry
no fast-math/FTZ and every contractable mul+add uses explicit RN intrinsics
so FMA contraction cannot occur; Rust f16↔f32 conversions are IEEE RN-even
with subnormal support (exhaustively round-trip tested) matching
`__half2float`/`__float2half_rn`; each kernel output is computed by exactly
one thread in fixed order, so two-run bitwise determinism is structural. The
broad first-launch tolerance plus all-position comparison, plane hashes, and
repeat determinism is a defensible layout/arithmetic gate. MINOR-1 (an
overflow fixture for the validation bits) is recommended before real-payload
execution but not required for the synthetic launch.

**3. Is the v1 descriptor, Rust ownership, fail-closed behavior, and
direct-source/no-persistent-expansion claim accepted?**
YES. Gate/up are frozen at K=6,144/N=512 and down at K=512/N=6,144 in both
`validate_exl3_descriptor` and `descriptor_valid`, with rows 1–3,072 enforced
on both sides. The 144-byte descriptor is independently frozen (Rust
`repr(C, align(16))` + layout test; C `static_assert(sizeof==144, alignof==16)`;
cn4 `abi-check.json` concurs) and field order matches exactly. Version,
struct size, flags, bits, projection/shape, all eight pointers non-null,
alignments (2/2/2/2/2/2/2 and 4 for trellis and validation word), reserved
words, row limit, and `workspace ≥ rows·(K+N)·2` are validated consistently
in Rust and C; the Rust side additionally checks the trellis byte formula,
and the FFI layer independently cross-checks the native library's ABI string
and workspace formula before any launch. Scratch is exactly
`rows·(K+N)·2` (M1 = 13,312 for all three projections), with output and the
4-byte validation word allocated and owned separately. `NativeExl3Case`
holds every buffer through `download`, which synchronizes the stream before
returning and fails closed on any nonzero validation word — covering all
three bits. Trellis/SUH/SVH upload is a byte-exact `memcpy` of the
little-endian source words with no transpose, swizzle, repack, or persistent
reconstruction; per-launch scratch holds only rotated and projected
activations. The CLI control compares every output position, hashes all six
planes, and enforces two-run bitwise determinism plus the frozen tolerance.
The preparation evidence contains a real `sm_120f` ELF cubin, symbol and
descriptor parity, and resource records, and claims no device correctness or
performance. (MAJOR-2, the unrunnable phase-b script, is an operational
defect in the launch process rather than in the items enumerated by this
decision, but it must be fixed in the same re-pin.)

## Gate disposition

One answer is not an unqualified YES, so per the handoff the acceptance hash
lines and the gate token are **not** emitted. No GPU launch is authorized by
this review.

Path to acceptance (small, mechanical):

1. Replace the tautological inverse-scatter test with a genuine
   forward-scatter cross-check including at least one off-diagonal tile
   (MAJOR-1).
2. Fix the `sm120_count` grep pattern in `scripts/cn4-exl3-phase-b.sh`
   (MAJOR-2).
3. Optionally add the non-finite/overflow fixture (MINOR-1) and barrier
   hygiene (MINOR-2).
4. Re-issue the handoff with updated hashes for the two touched files; every
   other reviewed input can carry its verified hash forward.

All substantive device-facing content — the mapping, the arithmetic, the
ABI, the ownership model, and the direct-source claim — passed adversarial
verification at `731c3bb`.
