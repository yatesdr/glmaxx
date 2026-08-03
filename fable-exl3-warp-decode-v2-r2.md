# Fable adversarial re-review — EXL3 warp-staged decode v2 (r2)

Date: 2026-07-30
Reviewer: Fable (Claude), independent design-gate re-review
Handoff: `docs/fable-exl3-warp-decode-v2-r2-handoff.md` (queue row 26)

Location note: the handoff declares the required result path at the
repository root; the operator directed all queue reviews into
`docs/reviews/`. Move this file to the root path (unmodified) when
consuming the acceptance.

## Verdict

All three required answers are unqualified **YES**. The acceptance line and
design token are included at the end. Acceptance opens only the CPU
staged-tile proof; it does not authorize implementation, device execution,
or timing. No v2 CUDA implementation exists yet.

## Provenance

- Candidate `0edfc8d796aeaeb969668005149bcb6286aa1e85` reviewed in a
  detached worktree. All 7 handoff input hashes verified at review start
  and re-verified at review finish; all matched. The worktree received no
  writes during this review.
- The pinned prior review `fable-exl3-warp-decode-v2.md` is byte-identical
  to the committed 2026-07-29 artifact (diffed; unaltered).
- Five of the seven inputs are shared with the source-projection r2 review
  (same candidate commit) and were already deeply verified there, including
  all 19 cn4 `prepare-c25e558-r1` raw-record hashes over ssh and the proof
  that the pinned `exl3_projection_control.cu` bytes are exactly what cn4
  compiled into a real `sm_120f` cubin with the three EXL3 symbols. That
  evidence is compile evidence for the corrected v1 base only, as the
  handoff states.

## Decision verification (all five items)

1. **Real v1 device check.** `require_sm120_device()` exists in the pinned
   `exl3_projection_control.cu`, queries the current device via
   `cudaGetDeviceProperties`, accepts only compute capability 12.0
   (`cudaErrorInvalidDevice` otherwise, property-query failures also fail
   closed), and runs after descriptor validation and before the
   validation-word memset and every enqueue.
2. **v2 must repeat the check.** The design's fail-closed route section now
   requires the optimized entry point to "repeat the retained v1 control's
   `cudaGetDeviceProperties` check requiring compute capability 12.0" —
   and that referenced check is now real.
3. **No phantom references.** The `c1ce884 → 0edfc8d` design diff was
   reviewed line-by-line: it adds the fixed load mapping, the U32 pointer
   formation, the same-module/same-flags compile rule, the idle-warp
   rationale, and the corrected device-check sentence. Nothing in the
   corrected text references a check, test, or artifact absent from the
   pinned base.
4. **Barrier totality.** The design requires inactive-row subwarps to
   "reach every CTA barrier" while doing no loads/decodes/accumulation/
   stores, and both stage barriers are explicit ("All threads synchronize,
   active subwarps consume … and all threads synchronize before the stage
   is overwritten"). This matches the corrected v1 kernels' realized
   barrier posture (active-predicate guarding, verified in the
   source-projection r2 review).
5. **Separate collective route.** "Rust selects the entry point for the
   whole launch. There is no device-side or rank-local fallback," with the
   TP4 executor required to select the same path on all four ranks.

Carry-over check for the standing proofs: the design deltas do not touch
the CTA/subwarp schedule, cyclic-window arithmetic, or the ascending-K
`__fmul_rn`/`__fadd_rn` sequence the first review verified bit-exactly for
both geometries (384×32 and 32×384; 5/256 windows wrapping the cyclic
boundary all covered by the 768-byte stage). The oracle change at this
candidate is test-module-only and the kernel change is the four v1 fixes;
the arithmetic sequence the v2 equivalence gate compares against is
unchanged. The newly pinned load mapping is a bijection of threads 0–191
onto 8 tiles × 24 words, and both real K-tile counts (384, 32) are exact
multiples of eight (48 and 4 stage iterations).

The first review's three MINORs and one QUESTION are all addressed by the
corrected text: compile flags are now pinned to same-module/same-rule
compilation; the thread→word load bijection is now fixed; the U32
type-punning is now specified as a `const uint32_t*` formed from the
descriptor's 64-bit address (legal under the v1 descriptor's 4-byte trellis
alignment); and the 256-vs-192-thread question is answered with an explicit
obligation to measure the idle-warp cost before the performance gate.

## Findings

### BLOCKER / MAJOR / MINOR

None.

### QUESTION

**Q-1 (non-blocking).** The load mapping assigns thread `t` word `t % 24`
of tile `t / 24`, so consecutive threads within a warp read consecutive
U32s of one 96-byte tile and cross a tile boundary every 24 threads —
slightly uncoalesced at warp granularity. Fine for a design gate; worth a
note in the eventual implementation review if the performance gate is
tight.

## Required answers

1. **Do the first review's CTA/schedule and arithmetic-equivalence answers
   remain YES for the corrected bytes?** YES — the deltas are additive
   specification and the corrected v1 fixes; nothing invalidates the
   bit-exact schedule or the equivalence target.
2. **Does the real v1 device check make the v2 fail-closed inheritance
   requirement implementable?** YES — the requirement now names an
   existing, verified check.
3. **Is the v1 ABI reuse, explicit route selection, and claim boundary
   accepted for CPU proof?** YES — descriptor reuse is unchanged from the
   accepted v1 ABI, route selection is host-side and collective with no
   fallback, and the design claims nothing beyond the CPU proof.

## Acceptance

exl3-warp-decode-v2-design-sha256=67fb3bcb5b839cc50f3462990f1ef6056ca7c9d991851efc5e668fed9d0b3325

## Machine provenance appendix

Reissued per docs/fable-kernel-r2-attestation-repair-request.md; findings, answers, and the substantive verdict above are unchanged. The candidate commit and every handoff input SHA-256 below were verified at review start and verified again at review finish; all matched both times.

0edfc8d796aeaeb969668005149bcb6286aa1e85

943fc05276e3efe8fa31959c5ad872168ac46cb0ce257bda0c5042c5a137768b
67fb3bcb5b839cc50f3462990f1ef6056ca7c9d991851efc5e668fed9d0b3325
20e4f6007969d42d78aba586e0a2b2496fc19483cd94563ed366bcd3e1b2b389
7c5bfa0795aa6b78646b00b029f8d4edc7c15491d69ea19e423f770701b5cdc3
c290960591295a1dd440818f97a7317fb42a061f9bc71c87a31b5bbb4cfd3647
241730ceaf629d01101629cb3f107e8d13fe92019444f4b635f9aa1d8cbc819d
b94156526800da97bc30f46e0315a64ab43c488c2007f510dedb86e71b3ec805

exl3-warp-decode-v2-design-accepted
