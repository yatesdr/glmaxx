# FC2 grouped-control scratch correction r1

Date: 2026-08-03

Status: corrective design candidate; CPU implementation and adversarial review
required before an SM120 rerun

## Observed failure

The first review-gated cn4 device run launched the NVFP4 FC1 M1 kernel
successfully, then `gpu-fc2-smoke 1` failed closed with `Driver(-3)`. The
failing path is the grouped CUTLASS FC2 development control. Its metadata and
CUTLASS workspace are placed at `token_output_f32`, but the fixture allocates
only `rows * 6,144 * sizeof(f32)` bytes there. At M1 that is 24,576 bytes and
is smaller than the grouped metadata. The total workspace formula includes
other independently allocated planes and therefore cannot prove this pointer's
capacity.

This is a real device-found capacity bug, not a CUDA error and not a numerical
failure. FC2 remains unqualified until the corrected path passes.

## Corrected contract

Add one pure ABI helper and matching Rust function:

```text
fc2_grouped_control_scratch_bytes(rows, assignments) =
    max(rows * 6,144 * sizeof(f32), 4 MiB)
```

The helper accepts exactly the ordinary FC2 shape domain: nonzero rows and
assignments with `assignments <= rows * 8` and `assignments <= 65,535`; all
arithmetic is checked in Rust and bounded in C++. Invalid input returns an
error/zero respectively.

For grouped-control-capable allocations:

- `token_output_f32` owns exactly that capacity;
- the final `rows * 6,144 * sizeof(f32)` bytes remain the semantic output
  extent at the same base pointer;
- before reduction, the same extent is temporary grouped metadata plus the
  CUTLASS workspace;
- reduction overwrites it with final output only after the grouped GEMM and
  scale expansion have consumed the temporary state; and
- the grouped total-workspace formula replaces its old output term with this
  larger term. No hidden padding may satisfy the charge.

The 4 MiB floor is a development-control reserve, not a production fused-
kernel requirement. The pinned CUTLASS build must still compute its exact
grouped metadata and workspace at runtime and fail closed if they do not fit;
the reserve is not evidence that an arbitrary future CUTLASS build fits. At
M1 the corrected grouped workspace is 4,554,820 bytes. At rows whose output
extent is already at least 4 MiB, the formula adds no reserve. The native
helper, Rust helper, descriptor validation, fixture allocation, and total-
workspace formula must agree exactly.

## Required proof and rerun

CPU tests must prove invalid-shape rejection, checked arithmetic, the exact M1
value, the reserve/output crossover, native/Rust helper equality, pointer
non-overlap, and that every existing FC2 workspace term remains charged. A
pinned native probe must retain the per-route maximum of grouped metadata,
CUTLASS workspace, required total, and reserve headroom for every matrix case;
zero or negative headroom fails qualification.

After adversarial acceptance, rerun only in a fresh isolated cn4 worktree and
evidence directory:

1. FC1 M1 regression;
2. FC2 M1 direct then grouped control twice;
3. FC2 M8;
4. the full FC2 positive/negative matrix; and
5. allocation/leak and post-run GPU-idle checks.

Any helper mismatch, `-3`, invalid route acceptance, nondeterminism, numerical
tolerance failure, allocation leak, or source/evidence drift fails closed.
This correction does not qualify full layers, TP4, a checkpoint, quality,
capacity, or performance.
