# Fable handoff — EXL3 warp-staged decode v2 re-review

Date: 2026-07-29

Candidate base commit:
`0edfc8d796aeaeb969668005149bcb6286aa1e85`

Required result path: `fable-exl3-warp-decode-v2-r2.md` at the repository
root.

Review scope: the single MAJOR finding from the first review. Its bit-exact
schedule and cyclic-window proofs stand unless the corrected design text or
v1 source invalidates them.

GPU authorization conveyed by this handoff: none. This remains a design gate.

## Required provenance procedure

Review the exact candidate commit in a detached worktree. Hash every input at
review start and finish. If either set differs from this table, report a stale
candidate and do not emit an acceptance token.

| Input | SHA-256 |
|---|---|
| prior `fable-exl3-warp-decode-v2.md` | `943fc05276e3efe8fa31959c5ad872168ac46cb0ce257bda0c5042c5a137768b` |
| `docs/exl3-sm120-warp-decode-v2.md` | `67fb3bcb5b839cc50f3462990f1ef6056ca7c9d991851efc5e668fed9d0b3325` |
| `docs/exl3-sm120-source-projection.md` | `20e4f6007969d42d78aba586e0a2b2496fc19483cd94563ed366bcd3e1b2b389` |
| `docs/exl3-trellis-cpu-contract.md` | `7c5bfa0795aa6b78646b00b029f8d4edc7c15491d69ea19e423f770701b5cdc3` |
| `crates/glm-format/src/exl3.rs` | `c290960591295a1dd440818f97a7317fb42a061f9bc71c87a31b5bbb4cfd3647` |
| `kernels/sm120/exl3_projection_control.cu` | `241730ceaf629d01101629cb3f107e8d13fe92019444f4b635f9aa1d8cbc819d` |
| `docs/cn4-review-fixes-preparation-20260729.md` | `b94156526800da97bc30f46e0315a64ab43c488c2007f510dedb86e71b3ec805` |

## Decision: realizable fail-closed inheritance

Verify directly that:

1. the retained v1 launcher now performs an actual current-device
   compute-capability check and accepts only 12.0;
2. the v2 design explicitly requires its new entry point to repeat that same
   check before enqueueing work;
3. the design no longer describes a check absent from its pinned v1 base;
4. all threads, including inactive-row threads, are required to reach both
   barriers; and
5. v2 remains a separate, collective-plan-selected route with no
   device-side or rank-local fallback.

The preparation evidence is compile evidence for the corrected v1 base only.
There is still no v2 CUDA implementation or v2 device evidence.

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
answer separately:

1. Do the first review's CTA/schedule and arithmetic-equivalence answers
   remain `YES` for the corrected bytes?
2. Does the real v1 device check make the v2 fail-closed inheritance
   requirement implementable?
3. Is the v1 ABI reuse, explicit route selection, and claim boundary accepted
   for CPU proof?

Only if every answer is an unqualified `YES`, include:

```text
exl3-warp-decode-v2-design-sha256=67fb3bcb5b839cc50f3462990f1ef6056ca7c9d991851efc5e668fed9d0b3325
```

Then end with:

```text
exl3-warp-decode-v2-design-accepted
```

Do not emit the token for a conditional pass or stale input. Acceptance opens
only the CPU staged-tile proof; it does not authorize implementation, device
execution, or timing.
