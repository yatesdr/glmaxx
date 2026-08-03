# Fable handoff: FC2 grouped-control scratch correction r2

Date: 2026-08-03

Status: corrective adversarial design review requested

GPU authorization conveyed by this handoff: none

A read-only hash/content check of the named cn4 evidence directory is allowed.
Do not compile, create a CUDA context, run the probe, or launch a kernel.

Review candidate commit:
`419c2b0832723f5ffaeecbbc39c9ad6fd8652be7`

Required result path:
`fable-fc2-grouped-control-scratch-r2.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`fc2-grouped-control-scratch-r2-design-accepted`

## Provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Withhold the token for any mismatch or incomplete input.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `docs/fc2-grouped-control-scratch-r2.md` | `3e5f2a307315089dad4e2ffd7db2a38983de0f08cff7cb9fe62a5c4d505c09b7` |
| `docs/cn4-fc2-scratch-probe-20260803.md` | `ea69477e78475f4bf1b06827c2789014275354b2ece91f37622a0cfcfad3b7b3` |
| `docs/fc2-grouped-control-scratch-r1.md` | `0e401f8cbae5b399a88dfdb0660533412551cb16f47e0017eda6b6edede69930` |
| `docs/fable-fc2-grouped-control-scratch-r1-handoff.md` | `716ab017da58630c8240a74a365609d0ef157d237b6406286a16605ac3bb8dec` |
| `docs/cn4-sm120-first-launch-20260803.md` | `0f971d1c56aa2fd05889c27224a55515dceff5fb86162b3768671ccf4a73e8e4` |
| `crates/glm-cuda/src/abi.rs` | `28905e69300a3a8c8105752ee9aaeb4d718cbe4387cab548139c257242ec68a4` |
| `crates/glm-cuda/src/ffi.rs` | `2a76ad51cb1c9b28a508dc4734bfeb6b6ad46103c3b437ec8e8ff8f6a6ff2f31` |
| `crates/glm-cuda/src/lib.rs` | `dfff79d944bacee30be686b8dda8e7c47f17926c316674c97d49e4c1984b7105` |
| `kernels/include/glmaxx_kernel.h` | `c5f5ceed453c901a63dfeecea0ec83a53b6485e98c32763650c708c699b56406` |
| `kernels/sm120/cutlass_nvfp4_fc2_control.cu` | `dba61fd6bc34b659543f1b64a329603ab01406a1505954ae16bc9626b8f7ff94` |
| `kernels/sm120/nvfp4_routed_fc2.cu` | `b72fff75bf4b0ee0ef06bf65286bad73678e4d396b2bdaad72bc784da738bb31` |

Run the complete CPU-only local gate and record its exit status:

```text
./scripts/local-checks.sh
```

If cn4 is reachable without disturbance, hash-verify the 14-entry stream at:

```text
/home/derek/glmaxx/evidence/20260803T191700Z-fc2-scratch-probe-c25e558
```

Its manifest SHA-256 must be
`44efef29ecfabd552345368d22785c054d79df702f616ae86630276b9396bda7`.
Inspect the retained probe source and output, but do not execute either.

## Required decisions

Independently derive every byte figure and inspect both host `-3` branches.
Then answer:

1. Does the pinned probe demonstrate that 3,072 bytes of metadata fit M1,
   while metadata plus 144,384 CUTLASS-workspace bytes require 147,456 bytes
   and fail the 24,576-byte combined check?
2. Does the retained probe use the pinned FC2 translation unit's exact
   `grouped_scratch`, CUTLASS argument, hardware, and workspace-size types
   without launching a kernel or treating its result as numerical evidence?
3. Does r2 fully supersede r1 and prevent the false metadata-only explanation
   or old token from opening implementation?
4. Is the exact shared helper domain—rows 1 through 65,536 and assignments 1
   through `min(rows*8, 65,535)`—consistent with the existing path domains
   and sufficient to eliminate the observed Rust/C++ divergence?
5. Are the 4,554,820-byte M1 aggregate and row-170/171 scratch crossover
   exact after both grouped-SFA and token-output replacement, with every old
   workspace term still charged?
6. Is temporal reuse of `token_output_f32` safe under same-stream ordering,
   with materialized BF16 and FP32 assignment output remaining in their
   separately charged, non-overlapping `assignment_down_f32` regions?
7. Is the 112-byte probe ABI exact, fail-closed, non-launching, and bound to
   the same sizing function the actual launch must call immediately before
   CUTLASS initialization?
8. Does the runtime required-versus-allocated check make the 4 MiB reserve
   safe for the pinned development control without claiming it bounds a
   future CUTLASS build or fused production kernel?
9. Does the CPU/native matrix cover domain edges, arithmetic, ABI mutations,
   range ownership, Rust/native equality, and every separately charged term
   before any new device run?
10. Does the qualification sequence preserve design review, implementation,
    implementation review, non-launching route probes, correctness controls,
    matrix execution, and only then later layer/checkpoint gates?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
answer every decision with an unqualified `YES` or `NO`. Only if every answer
is `YES`, attest the candidate and all twelve exact input hashes, then end
with the requested token as the only bare acceptance line.

Acceptance opens only the CPU/native scratch-helper, shared sizing function,
and probe implementation. It does not accept that implementation, authorize
a CUDA launch, or establish FC2, layer, TP4, checkpoint, quality, capacity,
or performance evidence.
