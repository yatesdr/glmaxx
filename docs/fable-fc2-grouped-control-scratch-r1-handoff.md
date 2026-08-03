# Fable handoff: FC2 grouped-control scratch correction r1

Date: 2026-08-03

Status: corrective design review requested

GPU authorization conveyed by this handoff: none

Review candidate commit:
`da65e63c8ebbe303335ca2636a3b56d7f1dfe028`

Required result path: `fable-fc2-grouped-control-scratch-r1.md` at the
repository root.

Requested acceptance token, only if every blocker and major is resolved:
`fc2-grouped-control-scratch-r1-design-accepted`

## Provenance

Review the exact commit in a detached worktree. Hash every input at start and
finish and withhold the token for any mismatch.

| Input | SHA-256 |
|---|---|
| `docs/cn4-sm120-first-launch-20260803.md` | `0f971d1c56aa2fd05889c27224a55515dceff5fb86162b3768671ccf4a73e8e4` |
| `docs/fc2-grouped-control-scratch-r1.md` | `68a9cfff163ff732c81af0e01a69de710c510a4dd0438e1d84f9cf026691b5b6` |
| `crates/glm-cuda/src/abi.rs` | `28905e69300a3a8c8105752ee9aaeb4d718cbe4387cab548139c257242ec68a4` |
| `crates/glm-cuda/src/ffi.rs` | `2a76ad51cb1c9b28a508dc4734bfeb6b6ad46103c3b437ec8e8ff8f6a6ff2f31` |
| `crates/glm-cuda/src/lib.rs` | `dfff79d944bacee30be686b8dda8e7c47f17926c316674c97d49e4c1984b7105` |
| `kernels/include/glmaxx_kernel.h` | `c5f5ceed453c901a63dfeecea0ec83a53b6485e98c32763650c708c699b56406` |
| `kernels/sm120/cutlass_nvfp4_fc2_control.cu` | `dba61fd6bc34b659543f1b64a329603ab01406a1505954ae16bc9626b8f7ff94` |
| `kernels/sm120/nvfp4_routed_fc2.cu` | `b72fff75bf4b0ee0ef06bf65286bad73678e4d396b2bdaad72bc784da738bb31` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

## Required independent work

1. Re-derive the M=1 allocation and grouped metadata placement and verify that
   the observed `-3` follows from the 24,576-byte `token_output_f32` capacity,
   rather than a driver or CUDA launch error.
2. Audit every allocation term and lifetime. Confirm that metadata/CUTLASS
   workspace and semantic FP32 token output may safely share the base only in
   the stated temporal order, and that no other pointer overlaps it.
3. Recompute the Rust/native shape domain, exact M=1 total `4,554,820`, the
   rows 170/171 crossover, grouped-SFA replacement, output replacement, and
   checked arithmetic.
4. Decide whether the 4 MiB pinned-build reserve plus exact runtime
   metadata/workspace measurement and fail-closed headroom is a sufficient
   development-control contract. It must not be interpreted as a future
   CUTLASS or production fused-kernel guarantee.
5. Verify that the required native probe and rerun retain maximum metadata,
   CUTLASS workspace, total required bytes, reserve headroom, pointer ranges,
   repeat determinism, numerical results, and post-run allocation state.

## Decisions

Answer each `YES` or `NO`:

1. Is the cn4 FC2 failure's root cause correctly identified?
2. Is the scratch ownership/lifetime contract race-free and non-overlapping?
3. Are the helper shape domain and Rust/native byte formulas complete and
   checked, including every pre-existing workspace term?
4. Does runtime sizing/headroom evidence make the fixed development reserve
   fail-closed without overstating it as a general bound?
5. Are the CPU proof and fresh SM120 rerun requirements sufficient to detect
   mismatch, under-allocation, nondeterminism, corruption, or leakage?
6. May this design enter CPU/native implementation and only then a separately
   gated cn4 rerun?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, followed
by independent derivations and all six decisions. Only if all are `YES`,
attest all nine hashes and end with the requested token as the only bare
acceptance line.
