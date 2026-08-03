# Fable adversarial re-review — direct EXL3 source projection v1 (r2)

Date: 2026-07-30
Reviewer: Fable (Claude), independent gate re-review
Handoff: `docs/fable-exl3-source-projection-v1-r2-handoff.md` (queue row 24)

Location note: the handoff declares the required result path at the
repository root; the operator directed all queue reviews into
`docs/reviews/`. The Phase-B script gates on a Git-tracked
`fable-exl3-source-projection-v1-r2.md` at the repository root — move this
file there (unmodified) when consuming the acceptance.

## Verdict

All four required answers are unqualified **YES**. Acceptance lines and the
gate token are included at the end. Acceptance opens only the synthetic v1
source-projection correctness gate; it does not authorize a launch by
itself, qualify a real checkpoint, or establish performance.

## Provenance

- Candidate `0edfc8d796aeaeb969668005149bcb6286aa1e85` reviewed in a
  detached worktree. All 7 handoff input hashes verified at review start and
  again at review finish; all matched. (The worktree was temporarily mutated
  for the mutation tests described below and restored via `git checkout`;
  finish hashes confirm byte-identical inputs.)
- The pinned prior review `fable-exl3-source-projection-v1.md` is
  byte-identical to the artifact I authored on 2026-07-29 (diffed against my
  original; no alteration in committing).
- cn4 r1 evidence (`/home/derek/glmaxx/evidence/prepare-c25e558-r1`): all
  19 raw-record hashes in `docs/cn4-review-fixes-preparation-20260729.md`
  verified over ssh. `source-commit.txt` = `c25e558`, clean status before
  and after; resource records show the three EXL3 kernels rebuilt
  (rotate-in 22 reg / 1,536 B smem, projection 38 reg, rotate-out 22 reg /
  1,536 B smem); exactly the three EXL3 symbols exported; 162 tests passed.
- The identical `input-sha256.txt` hash across the e4f0290 and c25e558-r1
  runs was investigated: that file attests only the prepare script's fixed
  doc/spec input set, which did not change; the compiled tree is attested by
  the source commit and clean status. Not an anomaly.
- Commit-offset check: the evidence was built at `c25e558`, the candidate is
  `0edfc8d`. Verified `crates/glm-format/src/exl3.rs` and
  `kernels/sm120/exl3_projection_control.cu` at `c25e558` hash exactly to
  the r2-pinned values, i.e., cn4 compiled the exact reviewed bytes. The
  `c25e558..0edfc8d` script diff is precisely the r2 attestation re-pin
  (result filename + three corrected hashes); docs otherwise.
- Diff scope re-check versus the first review's candidate `731c3bb`: the
  oracle change is confined to the test module (old tautological test
  removed, new independent test added); the kernel change is exactly the
  four reviewed fixes; the design doc adds two paragraphs documenting the
  device check and barrier/intrinsic posture. The first review's standing
  mathematical, arithmetic, and ABI proofs therefore carry over intact.

## Verification performed

1. **Independence of the new test.** `decode_forward_slot` decodes directly
   from `(lane, weight)` with an explicitly computed `tile_index` and never
   calls `inverse_trellis_slot` or `decode_native_at`;
   `forward_trellis_slot` is the contract's forward scatter. The test
   asserts `inverse_trellis_slot(forward_trellis_slot(l,w)) == (l,w)` for
   all 256 slots, covers tiles `(0,0)`, `(0,1)`, `(1,0)`, `(383,31)` — the
   two old fixed points plus two off-diagonal tiles — and proves the fixture
   distinguishes tile 1 from tile 32 (`off_diagonal_differences > 0`),
   which makes the canonical row/column-swap transposition provably fail
   rather than probabilistically fail.
2. **Mutation testing (decisive).** In the pinned worktree I injected both
   tile-transposition variants into `decode_native_at` and ran the new test:
   - `(column/16)*(N/16) + row/16` (row/col swap, same stride): **FAILED**
     as required;
   - `(column/16)*(K/16) + row/16` (full transpose, stride 384): **FAILED**
     as required.
   The file was restored and finish hashes re-verified. The clean suite
   passes (glm-format 51, glm-cuda 11 + 3, all green).
3. **Gate expression.** `scripts/cn4-exl3-phase-b.sh:144` now pipes through
   `tr -d '\r '` into `grep -c '^12\.0$'` (single backslash). Demonstrated:
   four `12.0` lines count to 4. Any non-12.0 device drops the count below
   4 → exit 70 before any GPU work; both `gpu_count` and `sm120_count` must
   equal exactly 4. The counts are also recorded to evidence
   (`gpu-counts.txt`).
4. **Kernel fixes.** `require_sm120_device()` queries the bound device and
   returns `cudaErrorInvalidDevice` for anything other than compute
   capability 12.0, called after descriptor validation and before the
   validation-word memset and all launches; property-query failures also
   fail closed. Both rotation kernels now compute an `active` predicate,
   guard only the shared-memory store (and later loads/stores/arithmetic),
   and every CTA thread reaches `__syncthreads()`; inactive threads return
   only after the barrier. Inactive threads leave their shared slot
   unwritten, but an inactive thread implies an entirely inactive block
   under the launch geometry, so no active thread ever reads a stale slot.
   The input rotation now uses explicit `__fmul_rn`, matching the
   output-rotation posture; numerical semantics are unchanged from the
   already-accepted arithmetic (the first review established the plain
   multiply was already RN under the pinned flags).

## Findings

### BLOCKER / MAJOR

None.

### MINOR

**MINOR-1 (standing from r1, out of r2 scope).** The three device
validation bits and the `KernelError::DeviceValidation` fail-closed path
remain unexercised by any fixture. Not required for the synthetic M1 gate
(the CLI independently fails on any non-finite comparison); still
recommended before real-payload execution.

**MINOR-2 (observation).** `require_sm120_device()` calls
`cudaGetDeviceProperties` on every launch; on some CUDA versions this is a
milliseconds-scale call. Irrelevant for the deliberately slow correctness
control, but cache it (or use `cudaDeviceGetAttribute`) if the check is
carried into performance-path launchers.

### QUESTION

None.

## Required answers

1. **Does the corrected independent test close the tile-transpose hole?**
   YES — independent forward path, off-diagonal tiles, proven fixture
   distinguishability, and both transposition mutations demonstrably fail
   the test.
2. **Is the corrected Phase-B gate executable and fail-closed on exactly
   four SM120 devices?** YES — corrected ERE verified at the pinned bytes
   and demonstrated to count four `12.0` lines; anything other than exactly
   four visible CC 12.0 devices exits 70 before any GPU work.
3. **Are the v1 device-property, barrier, and explicit-multiply fixes
   accepted?** YES — verified as described above, and the cn4 r1 evidence
   proves those exact CUDA bytes compile into a real `sm_120f` cubin with
   the expected EXL3 symbols.
4. **With the first review's standing mathematical/ABI findings, is the
   direct source-projection control accepted for its first synthetic
   correctness launch?** YES — the r1 proofs (exhaustive inverse↔forward
   verification, independent NumPy full-matrix digest reproduction,
   bit-identical constants, descriptor/ownership/fail-closed verification)
   carry over to byte-identical or strictly-corrected inputs, and both r1
   MAJORs are closed.

## Acceptance

exl3-cpu-contract-sha256=7c5bfa0795aa6b78646b00b029f8d4edc7c15491d69ea19e423f770701b5cdc3
exl3-sm120-design-sha256=20e4f6007969d42d78aba586e0a2b2496fc19483cd94563ed366bcd3e1b2b389
exl3-rust-oracle-sha256=c290960591295a1dd440818f97a7317fb42a061f9bc71c87a31b5bbb4cfd3647
exl3-cuda-control-sha256=241730ceaf629d01101629cb3f107e8d13fe92019444f4b635f9aa1d8cbc819d

## Machine provenance appendix

Reissued per docs/fable-kernel-r2-attestation-repair-request.md; findings, answers, and the substantive verdict above are unchanged. The candidate commit and every handoff input SHA-256 below were verified at review start and verified again at review finish; all matched both times.

0edfc8d796aeaeb969668005149bcb6286aa1e85

c299371ec162f8d86acf323d5856657d99ebb0d81cea52401f2f128d43ed0298
7c5bfa0795aa6b78646b00b029f8d4edc7c15491d69ea19e423f770701b5cdc3
20e4f6007969d42d78aba586e0a2b2496fc19483cd94563ed366bcd3e1b2b389
c290960591295a1dd440818f97a7317fb42a061f9bc71c87a31b5bbb4cfd3647
241730ceaf629d01101629cb3f107e8d13fe92019444f4b635f9aa1d8cbc819d
4f8baac179d34bad89c565487b5954c31ceddfa51cda23494f57dc85d7b4bd35
b94156526800da97bc30f46e0315a64ab43c488c2007f510dedb86e71b3ec805

exl3-source-projection-v1-accepted
