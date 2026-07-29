# Fable review — EXL3 warp-staged decode v2 design gate

Date: 2026-07-29
Reviewer: Fable (adversarial design-gate review per
`docs/fable-exl3-warp-decode-v2-handoff.md`)

## Provenance

Candidate commit actually reviewed:
`c1ce8846013ecdd643493610eb134855779f3fac` ("Design warp-staged EXL3
decode"), via a dedicated worktree at
`/private/tmp/claude-501/-Users-derek-glm5-native/f0e57b4e-b3ca-4b43-a75e-93057551ef6b/scratchpad/wt-warpdecode-c1ce884`.

Repository HEAD at review time was `830c6c8db61d93bd0916337b730f9264dd3fb3d1`.

Drift observed versus HEAD: exactly one pinned input differs at HEAD.
`kernels/include/glmaxx_kernel.h` hashes to
`da233563c6bfe92885c1a3101bcafa20292365b12ab788afb4d32d44a3ed2472` at HEAD
(pinned `8a365d0e…`). The diff is a five-line additive declaration of
`glmaxx_device_count` / `glmaxx_device_bind` from the concurrent
persistent-rank runtime work (commits 091ad0e..830c6c8, Sol's line of work).
All other six inputs are byte-identical at HEAD and at the pinned commit.
Per the handoff procedure, the entire review was conducted against the
pinned bytes in the worktree.

Verified SHA-256 table — identical at review start and review finish inside
the worktree (also verified once against `git show c1ce884…:<path>` before
worktree creation):

| Input | SHA-256 (start == finish) |
|---|---|
| `docs/exl3-sm120-warp-decode-v2.md` | `b73210fa756d1ec7f550970ac3b2fecb4c53f1b136ea9039418715b2747744d1` |
| `docs/exl3-sm120-source-projection.md` | `6a889c1987cbf9b0e69b8c99716acd753ad0626496a32d26a8b59135a17f22d7` |
| `docs/exl3-trellis-cpu-contract.md` | `7c5bfa0795aa6b78646b00b029f8d4edc7c15491d69ea19e423f770701b5cdc3` |
| `crates/glm-format/src/exl3.rs` | `8b771eb88eac20dae28917faf3cf640b58c3b12baa6193b9720a89d8bc1538b1` |
| `kernels/sm120/exl3_projection_control.cu` | `a50542774a585abeeb451c5248397da3b069296856ca8ae64423786ec5675857` |
| `kernels/include/glmaxx_kernel.h` | `8a365d0efecc65f24ae0722276e21ec01e6fd71d1a1dd7a8affcac9ace91ce47` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |

## Independent verification performed

- Re-derived the 256-position inverse trellis scatter mapping from the CPU
  contract's forward map (`lane`,`weight` → `row`,`col`) and confirmed the
  v2 doc's inverse formula, the v1 kernel's `decode_weight_bits`, and
  `inverse_trellis_slot` in `crates/glm-format/src/exl3.rs` agree on all
  256 positions with zero collisions (Python simulation).
- Re-derived the `+257` cyclic 24-word window extraction against an
  independent big-integer cyclic-bitstream reference for all 256
  (lane,weight) positions: zero mismatches. `end_bit` spans 771–1536;
  first/last word indices span 23–47, so the modulo-24 lookup is always
  live; 5 of 256 windows wrap the 24-word cyclic boundary and 136 fall in
  a single word — the 768-byte stage holds all 24 words of a tile, so
  every wrap case is covered.
- Simulated the complete v2 staged schedule (one CTA per N tile, groups of
  eight consecutive K tiles, threads 0–191 loading one U32 each via
  `stage[k_tile % 8][word] = trellis_u32[(k_tile*(N/16)+n_tile)*24+word]`)
  for BOTH real geometries at full scale — gate/up 384×32 tiles, down
  32×384 tiles — and compared every staged window against direct
  source-order decode: zero mismatches across all CTAs, all stage
  iterations, all 256 tile positions. Exactly 192 loads per stage.
- Verified byte arithmetic: 16×16×3-bit tile = 96 bytes = 24 U32; stage =
  8×24×4 = 768 bytes; per-CTA trellis reads = (K/16)×96 (36,864 gate/up;
  3,072 down); grid aggregate = (K/16)×(N/16)×96 = 1,179,648 bytes for
  both geometries, equal to the total trellis component size, i.e. every
  source byte addressed exactly once, independent of rows. K-tile counts
  384 and 32 are both exact multiples of eight (48 and 4 stage iterations).
- Verified the v1 descriptor already enforces `trellis_u16 % 4 == 0`
  (`exl3_projection_control.cu:249`), making the U32 view of the u16 plane
  aligned; a 96-byte tile keeps every tile base 4-byte aligned. Confirmed
  the little-endian half-pair assembly (`lo | hi<<16`) equals a direct U32
  load only on little-endian, which the doc correctly pins to the cn4 host.
- Verified index-width safety at the pinned shapes: max `source_u32` index
  294,911 and max byte offset < 1.2 MB; no overflow in 32-bit arithmetic,
  and the entry point rejects all other shapes.
- Verified the arithmetic invariant loop (`k = 16*k_tile + k_local`,
  ascending, `__fadd_rn(acc, __fmul_rn(activation, weight))`, `acc = 0.0f`)
  is operation-for-operation and order-for-order identical to the scalar
  control's inner loop at `exl3_projection_control.cu:149-165`, with the
  same `__float2half_rn` store boundary; the decode chain (wrapping U32
  multiply by `0xCBAC1FED`, `& 0x8FFF8FFF ^ 0x3B603B60`, FP16 half-sum) is
  byte-identical, with only the 24 source words relocated to shared memory.
- Confirmed the CPU oracle (`matmul_reference_f16`, `decode_native_at`,
  `hadamard_128` in `crates/glm-format/src/exl3.rs`) matches the same
  contract, and ran `cargo test -p glm-format` (49+3 tests) and
  `cargo test -p glm-cuda` (11 tests) in the worktree: all pass.
- Confirmed the workspace formula `rows*(K+N)*2` in the doc matches both
  the native `workspace_bytes` and the Rust `exl3_workspace_bytes`
  cross-check in `validate_native_exl3_library`, and that the 144-byte
  descriptor (8×u32 + 14×u64) leaves rows 1–8 expressible with no new
  fields, no new allocation, and only 768 bytes of static shared memory.
- Searched the entire pinned tree for the v1 SM120 device-property check
  the v2 fail-closed route requires to be "repeated" (see MAJOR-1).

## Findings

### BLOCKER

None.

### MAJOR

- **MAJOR-1 — The fail-closed route requires repeating a v1 SM120
  device-property check that does not exist in the pinned v1 EXL3 path.**
  `docs/exl3-sm120-warp-decode-v2.md:139` requires the v2 entry point to
  "repeat the v1 SM120 device-property check". At the pinned commit,
  `glmaxx_exl3_projection_launch` in
  `kernels/sm120/exl3_projection_control.cu` performs no
  `cudaGetDeviceProperties` / compute-capability check, and neither does
  any Rust code on the EXL3 launch path (`launch_native_exl3`,
  `NativeExl3Fixture`, `validate_native_exl3_library` in
  `crates/glm-cuda/src/ffi.rs` are ABI/workspace/shape checks only). Only
  the NVFP4 launchers have `sm120_properties`
  (`kernels/sm120/nvfp4_routed_fc1.cu:293`, `nvfp4_routed_fc2.cu:405`).
  As written the requirement is unexecutable (there is nothing to repeat)
  and risks being satisfied vacuously, in which case the claimed fail-closed
  property would not hold on non-SM120 devices; it also documents a false
  property of the retained control that the bitwise-equivalence gate
  (step 4) runs against. Fix before CPU-proof acceptance of Decision 3:
  either reword to "introduce an SM120 device-property check following the
  NVFP4 `sm120_properties` pattern, and add the same check to the retained
  v1 control", or land the v1 check first and then "repeat" it. (Note: the
  post-pin HEAD drift adds `glmaxx_device_bind` with a compute-capability
  out-parameter for the persistent rank runtime, but that is not on the v1
  EXL3 launch path and is outside the pinned bytes.)

### MINOR

- **MINOR-1 — Bitwise-equivalence gate validity is conditional on
  identical compilation of both kernels; pin it explicitly.**
  `__fmul_rn`/`__fadd_rn` are never FMA-contracted and per-thread
  sequential order is scheduler-independent, so the gate is implementable;
  but the FTZ behavior of single-precision ops follows module compile
  flags. The gate is unconditionally valid only when the v2 entry point is
  built in the same native library with the same flags as the retained
  scalar control (current repo practice; gate step 3 records compiler
  commands). The design doc should state this compilation-unit/flag
  pinning as part of the gate rather than leaving it implicit.
- **MINOR-2 — The thread-to-word bijection for the 192-word cooperative
  load is unspecified.** The doc pins "threads 0–191 load exactly 192 U32"
  and the per-(k_tile, word) source formula, but not which thread loads
  which word (e.g. `tile = t / 24, word = t % 24`). Any bijection is
  bit-correct (verified), but the CPU staged-tile proof and later
  coalescing analysis need one pinned mapping; the proof should declare it.
- **MINOR-3 — The U32 view of the trellis plane is C++ type-punning of a
  u16-typed address.** Alignment is guaranteed by the existing
  `trellis_u16 % 4` descriptor check and this is standard CUDA practice,
  but the eventual implementation should read via a `const uint32_t*`
  formed once from the raw 64-bit descriptor address rather than casting a
  dereferenced u16 pointer mid-kernel, keeping the "U32 view is legal"
  claim aligned with what is actually compiled.

### QUESTION

- **QUESTION-1 — Why a 256-thread CTA?** With two rows per warp and rows
  capped at 8, at most warps 0–3 (128 threads) compute; threads 0–191 (six
  warps) load; threads 192–255 (two warps) neither load nor compute and
  exist only to reach barriers. A 192-thread CTA satisfies both roles.
  If 256 is reserved headroom for the paired gate/up successor, the doc
  should say so; otherwise the two idle warps are unexplained occupancy
  cost. Not a correctness issue.
- **QUESTION-2 — Shared-stage bank behavior is unaddressed.** The 16 lanes
  of a subwarp index a 24-word tile through the inverse mapping, which
  produces non-uniform word indices per `k_local`; conflicts are a
  performance matter deferred to profiler step 7, but confirm the silence
  is intentional and that stage layout padding is allowed to change under
  the bitwise gate (it is, since it does not alter arithmetic).

## Required answers

**1. Is the CTA/subwarp/shared-stage schedule accepted for CPU proof?**
YES. The 256-thread CTA mapping, one CTA per 16-column N tile, two rows
per warp as 16-lane subwarps, eight-row bound, eight-tile stages, the
192-word cooperative load, and full-CTA barrier participation were
independently re-derived; the source addressing reads `[K/16,N/16,24]` in
original order with no transpose; the full staged schedule was simulated
bit-exactly against direct decode for both real geometries with zero
mismatches; the 1,179,648-byte aggregate is exact and row-independent; no
partial-warp, barrier-divergence, alignment, overflow, or down-projection
hazard was found. MINOR-2 (pin the load bijection) is discharged inside
the CPU proof this acceptance opens.

**2. Is the exact arithmetic-order and bitwise-equivalence gate accepted?**
YES. The lane-local loop preserves the scalar control's exact ascending-K
`__fmul_rn`-then-`__fadd_rn` sequence, accumulator initialization, decode
chain, cyclic lookup, FP16 reconstruction, projection store, and unchanged
rotations; shared-memory staging is bit-transparent; no compiler,
aliasing, subnormal, or scheduling behavior makes the claimed bitwise
equality of the intermediate and final FP16 planes unimplementable when
both kernels are built in the same module with the same flags, which the
existing gate-step-3 evidence requirement records (MINOR-1 asks that this
be stated explicitly).

**3. Is the v1 ABI reuse, fail-closed route, and traffic/claim boundary
accepted?**
NO — not as an unqualified YES. Descriptor and workspace reuse is
sufficient (no hidden allocation or metadata; 768 B static shared only),
rows outside 1–8 are rejected with no device-side or rank-local fallback,
the traffic statement is correctly limited to logical global-load
addresses, and the gate sequence keeps payload, timing, profiler, grouped
routing, prefill, and model-quality claims closed. But the fail-closed
route's "repeat the v1 SM120 device-property check" (MAJOR-1) references a
check that does not exist anywhere in the pinned v1 EXL3 path, so the
route as specified cannot be executed and its SM120 fail-closure is
asserted on a false premise. Decision 3 requires the one-line correction
in MAJOR-1 (and preferably the v1-side check) before acceptance.

## Token decision

Answers are YES / YES / NO. Per the handoff, no acceptance token is
emitted for a conditional or partial pass. **No token is emitted.**

The CPU staged-tile proof remains closed. Nothing in this review
authorizes implementation, compile, device execution, or timing.
