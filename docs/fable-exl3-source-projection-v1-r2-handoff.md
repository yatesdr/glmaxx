# Fable handoff — direct EXL3 source projection v1 re-review

Date: 2026-07-29

Candidate base commit:
`0edfc8d796aeaeb969668005149bcb6286aa1e85`

Required result path: `fable-exl3-source-projection-v1-r2.md` at the
repository root.

Review scope: the two MAJOR findings and related device-facing MINOR findings
from the first review. The first review's independent reconstruction and
arithmetic proofs stand unless a corrected byte invalidates them.

GPU authorization conveyed by this handoff: none.

## Required provenance procedure

Review the exact candidate commit in a detached worktree. Hash every input at
review start and finish. If either set differs from this table, report a stale
candidate and do not emit an acceptance token.

| Input | SHA-256 |
|---|---|
| prior `fable-exl3-source-projection-v1.md` | `c299371ec162f8d86acf323d5856657d99ebb0d81cea52401f2f128d43ed0298` |
| `docs/exl3-trellis-cpu-contract.md` | `7c5bfa0795aa6b78646b00b029f8d4edc7c15491d69ea19e423f770701b5cdc3` |
| `docs/exl3-sm120-source-projection.md` | `20e4f6007969d42d78aba586e0a2b2496fc19483cd94563ed366bcd3e1b2b389` |
| `crates/glm-format/src/exl3.rs` | `c290960591295a1dd440818f97a7317fb42a061f9bc71c87a31b5bbb4cfd3647` |
| `kernels/sm120/exl3_projection_control.cu` | `241730ceaf629d01101629cb3f107e8d13fe92019444f4b635f9aa1d8cbc819d` |
| `scripts/cn4-exl3-phase-b.sh` | `4f8baac179d34bad89c565487b5954c31ceddfa51cda23494f57dc85d7b4bd35` |
| `docs/cn4-review-fixes-preparation-20260729.md` | `b94156526800da97bc30f46e0315a64ab43c488c2007f510dedb86e71b3ec805` |

The raw cn4 preparation records are outside Git at:

```text
/home/derek/glmaxx/evidence/prepare-c25e558-r1
```

Hash-check the records against the compact preparation document. That run
compiled the corrected CUDA source but did not expose GPUs or launch a device
kernel.

## Decision 1: independent forward-scatter test

Determine whether
`forward_scatter_cross_checks_inverse_and_off_diagonal_tile_addressing`:

1. constructs its matrix from the forward lane/weight scatter instead of
   calling `decode_native_at`;
2. decodes each selected point through a separate direct lane/weight
   expression;
3. covers the off-diagonal tiles `(0,1)` and `(1,0)` in addition to the two
   transpose fixed points; and
4. proves its fixture actually distinguishes the correct tile address from
   the transposed address.

Do not repeat the first handoff's false claim that the old first/last-tile
test could detect transposition. Decide whether the replacement test provides
the missing non-tautological regression proof.

## Decision 2: executable gate and device checks

Verify directly that:

1. the SM120 inventory expression uses the single-backslash ERE
   `'^12\.0$'`, and demonstrate that it counts four `12.0` lines;
2. the v1 launcher queries the selected device and rejects anything other
   than compute capability 12.0 before enqueueing work;
3. every thread reaches each `__syncthreads()` and inactive rows are guarded
   only around loads/stores and arithmetic;
4. the input rotation uses explicit `__fmul_rn`, matching the already reviewed
   output-rotation posture; and
5. the cn4 preparation proves those exact CUDA bytes compile into a real
   `sm_120f` cubin with the expected EXL3 symbols.

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
answer separately:

1. Does the corrected independent test close the tile-transpose hole?
2. Is the corrected Phase-B gate executable and fail-closed on exactly four
   SM120 devices?
3. Are the v1 device-property, barrier, and explicit-multiply fixes accepted?
4. With the first review's standing mathematical/ABI findings, is the direct
   source-projection control accepted for its first synthetic correctness
   launch?

Only if every answer is an unqualified `YES`, include:

```text
exl3-cpu-contract-sha256=7c5bfa0795aa6b78646b00b029f8d4edc7c15491d69ea19e423f770701b5cdc3
exl3-sm120-design-sha256=20e4f6007969d42d78aba586e0a2b2496fc19483cd94563ed366bcd3e1b2b389
exl3-rust-oracle-sha256=c290960591295a1dd440818f97a7317fb42a061f9bc71c87a31b5bbb4cfd3647
exl3-cuda-control-sha256=241730ceaf629d01101629cb3f107e8d13fe92019444f4b635f9aa1d8cbc819d
```

Then end with:

```text
exl3-source-projection-v1-accepted
```

Do not emit the token for a conditional pass or stale input. Acceptance opens
only the synthetic v1 source-projection correctness gate. It does not
authorize a launch, qualify a real checkpoint, or establish performance.
