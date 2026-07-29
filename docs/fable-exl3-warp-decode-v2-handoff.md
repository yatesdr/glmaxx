# Fable handoff — EXL3 warp-staged decode v2 design

Date: 2026-07-29

Candidate base commit:
`c1ce8846013ecdd643493610eb134855779f3fac`

Review scope: design gate only, before CPU staged-tile proof or CUDA
implementation.

GPU authorization conveyed by this handoff: none.

## Required provenance procedure

Hash every input at review start and finish. If either set differs from this
table, report a stale candidate and do not emit an acceptance token.

| Input | SHA-256 |
|---|---|
| `docs/exl3-sm120-warp-decode-v2.md` | `b73210fa756d1ec7f550970ac3b2fecb4c53f1b136ea9039418715b2747744d1` |
| `docs/exl3-sm120-source-projection.md` | `6a889c1987cbf9b0e69b8c99716acd753ad0626496a32d26a8b59135a17f22d7` |
| `docs/exl3-trellis-cpu-contract.md` | `7c5bfa0795aa6b78646b00b029f8d4edc7c15491d69ea19e423f770701b5cdc3` |
| `crates/glm-format/src/exl3.rs` | `8b771eb88eac20dae28917faf3cf640b58c3b12baa6193b9720a89d8bc1538b1` |
| `kernels/sm120/exl3_projection_control.cu` | `a50542774a585abeeb451c5248397da3b069296856ca8ae64423786ec5675857` |
| `kernels/include/glmaxx_kernel.h` | `8a365d0efecc65f24ae0722276e21ec01e6fd71d1a1dd7a8affcac9ace91ce47` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |

## Decision 1: CTA schedule and source addressing

Independently derive the proposed 256-thread CTA mapping:

- one CTA per 16-column N tile;
- two rows per warp as low/high 16-lane subwarps;
- exactly eight rows maximum;
- eight K tiles per shared stage;
- threads 0–191 loading exactly 192 U32 words; and
- all threads reaching both barriers even for inactive rows.

Verify both real geometries. Check that the proposed formula reads source
layout `[K/16,N/16,24]` in its original order and that each CTA logically
loads exactly 1,179,648 aggregate trellis bytes across the complete grid,
independent of rows. Look for transpose mistakes, partial-warp hazards,
barrier divergence, alignment assumptions, overflow, and down-projection
edge cases.

## Decision 2: arithmetic and equivalence gate

Determine whether the lane-local loop can preserve the retained scalar
control's exact ascending-K `__fmul_rn` then `__fadd_rn` sequence. Check every
operation involved in staged U32 decode, cyclic word lookup, FP16
reconstruction, accumulator initialization, FP16 projection store, and the
unchanged rotations.

Decide whether bitwise equality of the intermediate and final FP16 planes is
a valid predeclared gate. Identify any compiler flag, aliasing, subnormal,
shared-memory, or scheduling behavior that would make the claimed equivalence
unimplementable.

## Decision 3: ABI, routing, and claim boundary

Verify that reuse of the frozen v1 descriptor and workspace is sufficient for
a second explicit entry point with rows 1–8, and that no new allocation or
metadata is being hidden. Confirm the design rejects unsupported rows and
does not permit device-side or rank-local fallback.

Assess whether the traffic statement is correctly limited to logical
global-load addresses rather than asserted DRAM transactions. Check that the
gate sequence keeps real payload, timing, profiler, grouped routing, prefill,
and model-quality claims closed.

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
answer separately:

1. Is the CTA/subwarp/shared-stage schedule accepted for CPU proof?
2. Is the exact arithmetic-order and bitwise-equivalence gate accepted?
3. Is the v1 ABI reuse, fail-closed route, and traffic/claim boundary accepted?

Only if every answer is an unqualified `YES`, include:

```text
exl3-warp-decode-v2-design-sha256=b73210fa756d1ec7f550970ac3b2fecb4c53f1b136ea9039418715b2747744d1
```

Then end with:

```text
exl3-warp-decode-v2-design-accepted
```

Do not emit the token for a conditional pass or stale input. Acceptance opens
only the CPU staged-tile proof; it does not authorize implementation, compile,
device execution, or timing.
