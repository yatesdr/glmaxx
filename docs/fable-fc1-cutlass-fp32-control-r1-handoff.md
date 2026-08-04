# Fable handoff: FC1 CUTLASS FP32 materialization control r1

Date: 2026-08-04

Status: corrective design review requested

GPU authorization conveyed by this handoff: none

Review candidate commit:
`18f4132535133a4830cc54e1e86ecb04e44fe42b`

Current HEAD or the working tree may drift; use a detached worktree and
verify every SHA-256 at the start and finish of the review.

Required result path:
`fable-fc1-cutlass-fp32-control-r1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`fc1-cutlass-fp32-control-r1-design-accepted`

Emit it only if all three questions receive an unqualified **YES** and there
is no blocker or major finding. Otherwise withhold it and give the smallest
corrective scope.

## Pinned inputs

| Path | SHA-256 |
|---|---|
| `docs/fc1-cutlass-fp32-control-r1.md` | `f53429b0fbfb08953198132bbff4b69ce624aceba6445721f33502c95138fd3c` |
| `docs/cn4-fc1-reduction-order-probe-86fe811-20260804.md` | `c7b974a911cfe04a19214ac57bf7874b8394f25718b2aa570b3b844d3cbc36b0` |
| `docs/sm120-cutlass-fc1-successor.md` | `ae6a5378b012192b456a35df0ddf8419f01fde278ad06075e4b85be3f2b719be` |
| `docs/cn4-kernel-readiness.md` | `6bb35c6b18aaf2b9582f5fde3a291e9a01518ac80db2d457fdd9ee81e9933b1c` |
| `benchmarks/sm120-fc1-matrix-v1.json` | `d5a0286d0c9d06ce1036085d4f1712d929273fb533601806b26b9f774c360e74` |
| `kernels/sm120/cutlass_nvfp4_dense_control.cu` | `1ec4abf4fc307f709aa5d2576ac80c84825d75a7eb9db7b11ae036fd67a34541` |
| `crates/glm-cuda/src/abi.rs` | `28905e69300a3a8c8105752ee9aaeb4d718cbe4387cab548139c257242ec68a4` |
| `crates/glm-cuda/src/ffi.rs` | `2a76ad51cb1c9b28a508dc4734bfeb6b6ad46103c3b437ec8e8ff8f6a6ff2f31` |
| `crates/glm-cli/src/main.rs` | `fdd02d400004dece8a335c48d6e4f655fa1c958293472dd20057fd1c5853dc00` |

Raw cn4 evidence is retained at
`/home/derek/glmaxx/evidence/20260804T005300Z-fc1-reduction-probe-86fe811`.
The probe JSON SHA-256 is
`86392ac300f02dd9525a2e729b6627ec9a3cb1992206ea6c998dca597b193b86`.

## Questions

1. Does the design isolate the avoidable BF16 materialization boundary with
   an FP32-D control while preserving the exact packed operands, global-scale
   placement, semantic oracle, and frozen tolerance?
2. Are the allocation, pointer-disjointness, reporting, ordering, and
   fail-closed stop conditions sufficient to prevent the diagnostic from
   qualifying a production kernel or hiding a tensor-core numerical defect?
3. Does the proposed CPU proof and SM120 sequence provide enough independent
   evidence to authorize implementation without weakening quality, format,
   routing, rank-consensus, or no-repack requirements?

Classify findings as blocker, major, or minor and explicitly report the
start/end candidate and input-hash checks.
