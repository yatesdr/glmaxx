# Fable handoff: SM120 EXL3 grouped paired gate/up decode v1

Date: 2026-08-04

Status: adversarial design review requested

GPU authorization conveyed by this handoff: none

Read-only cn4 artifact verification: permitted; do not launch CUDA or modify
the retained evidence tree

Review candidate commit:
`7e2e4e360133fcf591c94bdf21eb8aa2ab1d6f91`

Required result path:
`fable-exl3-grouped-gate-up-sm120-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`exl3-grouped-gate-up-sm120-v1-design-accepted`

## Required provenance procedure

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Report a stale candidate and withhold the token if either
set differs from this table. Do not substitute current `main`, the later
handoff commit, or an untracked review-inbox file.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/exl3-grouped-gate-up-sm120-v1.md` | `f618027ca386c1976052e9ad5259f94180ac1cd73d7e78a6f17ffd9126847547` |
| `docs/exl3-sm120-warp-decode-v2.md` | `67fb3bcb5b839cc50f3462990f1ef6056ca7c9d991851efc5e668fed9d0b3325` |
| `docs/exl3-warp-staging-cpu-proof-v2.md` | `5c77b5721885da708d0240e9eeb6537e9ed74a25a6940cf92e00bc79de494b31` |
| `docs/cn4-exl3-staged-k3-ncu-20260804.md` | `cc7592fd6da2b4bc589cefd664819c4d04c03dfb0e1a782582ad54fdefda0865` |
| `docs/target-layer-execution-v1.md` | `89c6cf7397a3dc6b0c01383e679dcc4b51e20e3c45057bef0928dc24b866a819` |
| `docs/target-layer-execution-v1-r2.md` | `808da35c2e54eb5692512996650839fb6f127cb91658603eb2fb5ce049c56ed2` |
| `crates/glm-reference/src/routed_fc1.rs` | `faa4631d3c3cf90eb94e6d0b09ca32143a80dc620b1f7238f798d52e01a3e0af` |
| `crates/glm-cuda/src/abi.rs` | `28905e69300a3a8c8105752ee9aaeb4d718cbe4387cab548139c257242ec68a4` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

Run the complete CPU-only gate and record its exit status:

```text
./scripts/local-checks.sh
```

This design has no implementation. A green repository gate does not answer
the design questions or authorize CUDA work.

## Decision 1: measured premise and scope

Hash-verify the retained cn4 `.ncu-rep`, raw CSV, and control/profile output
records named by the diagnostic. Confirm independently that the staged K=3
M1 gate kernel launched 32 CTAs on 188 SMs, took 280.26 microseconds under
counter replay, achieved 16.66% occupancy and 0.26% DRAM throughput, and kept
the same output digest inside and outside profiling. Determine whether those
facts support under-parallelism as the first hypothesis without treating NCU
duration as unprofiled latency or claiming a grouped result that does not yet
exist.

## Decision 2: route and mixed-K correctness

Prove from the pinned route contracts that each real token selects eight
distinct experts and stable compaction is expert/token/slot ordered. Attack
the K=3 filtering rule with duplicate expert/slot, unsorted active experts,
nonmonotonic offsets, an offset tail unequal to assignment count, token or slot
overflow, a K=4 expert admitted as K=3, and rank-local policy divergence.

Confirm that only an all-K3 M1 selection guarantees eight active experts and
256 projection CTAs. The real TR3 3.25-bpw profile contains K=3 and K=4
experts; this design must not imply that every real route obtains that grid or
that K=4 is implemented.

## Decision 3: ABI and resident binding

Independently lay out the proposed descriptor in C and Rust. Verify every
offset, exactly 256 bytes, 16-byte alignment, no implicit tail padding, the
inline digest, and zeroed reserved fields. Re-derive the exact workspace term
`26,624 * assignments`, including 212,992 bytes at A=8 and 1,703,936 bytes at
A=64.

Determine whether six resident pointer tables can be bound safely to the
immutable adopted weight generation and target-program identity. Check that
the host-side route-digest/upload receipt plus device-side bounds checks are a
sufficient fail-closed boundary, or report a concrete missing device identity
check. No kernel may inspect a pointer before proving its index and alignment
safe.

## Decision 4: paired staging and arithmetic

Exhaustively simulate the load loop:

```text
for linear = thread; linear < 384; linear += 256
```

Prove it writes every `(projection,stage_tile,word)` exactly once, gives two
loads to threads 0..127 and one to threads 128..255, and never aliases the two
768-byte stages. For rows 1..8 and every legal per-expert assignment count,
prove warp and half-warp ownership is unique and all 256 threads reach both
barriers.

Compare the lane-local grouped recurrence expression-by-expression with the
accepted predecessor. Confirm gate and up retain separate ascending-K FP32
multiply/add sequences, one FP16 projection rounding, unchanged input/output
rotations, and no shuffle, split-K, reduction, FMA contraction, or cross-lane
dependency that could change bits.

## Decision 5: launch and traffic arithmetic

Re-derive all three grids and their model-shape limits. Confirm the projection
grid is `32 * active_expert_count` CTAs, so the all-K3 M1 case has 256 CTAs,
and that one grouped entry point needs no host expert/projection loop. Re-derive
2,359,296 logical trellis bytes per active expert and 18,874,368 bytes at eight
experts. Keep logical address traffic distinct from observed DRAM sectors.

Check whether the three-kernel order and shared validation word remain safe on
one caller-owned stream for every failure. A zero-K3 route must be a common
no-launch decision, not a malformed descriptor or rank-local shortcut.

## Decision 6: gates and nonclaims

Confirm the design requires adversarial acceptance, independent CPU proof,
ABI proof, SM120 compile/inspection, synthetic bitwise comparison, real
checkpoint comparison, and matched profiling before target-layer integration.
It must not claim K=4, down projection, SwiGLU, route-weight scatter, TP4,
checkpoint smoke, quality, concurrency above M8, or serving throughput.

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
answer separately and unambiguously:

1. Is the measured under-parallelism premise accurately and narrowly used?
2. Is K=3 route filtering deterministic, mixed-K honest, and rank invariant?
3. Is the 256-byte ABI, workspace arithmetic, and resident binding accepted?
4. Is the paired stage bijective and bitwise-equivalent to two isolated
   ascending-K projections?
5. Are the grids, logical traffic, launch order, and empty route safe?
6. Are the gate sequence and all nonclaims exact?
7. Is the design accepted for an independent Rust CPU proof?

Only if every answer is an unqualified `YES`, include the candidate commit and
all nine exact input SHA-256 values from the provenance table in the result,
then end with the requested acceptance token named in the header as the only
bare acceptance line.

Withhold the token for stale bytes, a conditional pass, an unproven device
pointer boundary, route divergence, K=4 scope leakage, a nonbijective stage,
barrier early return, changed accumulation order, or any performance claim not
supported by the retained evidence.
