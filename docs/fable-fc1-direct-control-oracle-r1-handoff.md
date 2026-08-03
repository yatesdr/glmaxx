# Fable handoff: FC1 direct-control oracle correction r1

Date: 2026-08-03

Status: corrective design review requested

GPU authorization conveyed by this handoff: none

Review candidate commit:
`da65e63c8ebbe303335ca2636a3b56d7f1dfe028`

Required result path: `fable-fc1-direct-control-oracle-r1.md` at the repository
root.

Requested acceptance token, only for an unqualified pass:
`fc1-direct-control-oracle-r1-design-accepted`

## Provenance

Review the exact commit in a detached worktree. Hash every input at start and
finish and withhold the token for any mismatch.

| Input | SHA-256 |
|---|---|
| `docs/cn4-sm120-first-launch-20260803.md` | `0f971d1c56aa2fd05889c27224a55515dceff5fb86162b3768671ccf4a73e8e4` |
| `docs/fc1-direct-control-oracle-r1.md` | `1bbf242755f6bdb47901dfb1f9687c271e849d4288ee6143a982c1829f9cabd5` |
| `crates/glm-reference/src/matrix.rs` | `bad7f19ff2cfa3f9d4e3abcf0f9de549f2acbdcbbaae5508347add5943f9df39` |
| `crates/glm-reference/src/routed_fc1.rs` | `55709be01710d08f5b13de22afdb24884d233cdfab1c5de132ad22edd304f40b` |
| `crates/glm-cli/src/main.rs` | `04537c79fe4bcac67627483e96fcedc783702d08a16db8c10f3894964fe99afc` |
| `kernels/sm120/nvfp4_routed_fc1.cu` | `67d954f2ba1bf28f0eca30c42ab18c014b19353b4102e89edd7089a1ad9770c5` |
| `benchmarks/sm120-fc1-matrix-v1.json` | `d5a0286d0c9d06ce1036085d4f1712d929273fb533601806b26b9f774c360e74` |
| `docs/cn4-kernel-readiness.md` | `6bb35c6b18aaf2b9582f5fde3a291e9a01518ac80db2d457fdd9ee81e9933b1c` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

The raw device evidence remains at the two cn4 paths pinned in the result
record. Hash-verify it over SSH if available; do not launch a kernel.

## Required independent work

1. Recreate the M=256 deterministic-random row 239, column 20 from source.
   Independently pack/dequantize it and derive the sequential and 256-lane
   tree BF16 results. Confirm or refute `0xc32c` versus `0xc331` and the `5.0`
   difference.
2. Trace the CUDA kernel expression by expression. Verify lane membership,
   explicit FMA, tree strides, scale multiplication, gate/up pairing, SiLU,
   and BF16 RNE, including compiler contraction/fast-math posture.
3. Determine whether the dual-oracle acceptance can conceal a layout, scale,
   nibble, route, or accumulation defect. In particular, check that a rescue
   requires exact schedule bits and is restricted to this retained direct
   backend.
4. Verify that the semantic oracle and original tolerance remain unchanged;
   both error views and every rescued/unresolved element remain in evidence;
   and CUTLASS, graphs of other kernels, layers, quality, and performance
   cannot inherit this exception.
5. Mutation-test lane count, lane indexing, FMA, stride order, rounding,
   scales, gate/up selection, and report accounting. Each mutation must be
   caught by CPU proof or the specified matrix evidence.

## Decisions

Answer each `YES` or `NO`:

1. Does the cn4 result establish a schedule/oracle mismatch rather than a
   device arithmetic defect for all 43 reported elements?
2. Is the direct-control oracle independently reproducible and exact for the
   pinned CUDA schedule?
3. Does `semantic_ok || schedule_exact` retain the frozen semantic tolerance
   while preventing an approximate schedule match from rescuing a failure?
4. Are schema/evidence requirements sufficient to expose every semantic
   deviation, exact rescue, schedule mismatch, and unresolved failure?
5. Is the exception bounded so it cannot qualify CUTLASS, a production
   kernel, a layer, checkpoint, quality, or performance?
6. May the design enter CPU implementation and only then a separately gated
   SM120 rerun?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, followed
by independent derivations and all six decisions. Only if all are `YES`,
attest all nine hashes and end with the requested token as the only bare
acceptance line.

