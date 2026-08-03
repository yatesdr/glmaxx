# FC2 grouped-control scratch correction r2

Date: 2026-08-03

Status: corrective design candidate; implementation is blocked on adversarial
acceptance

## Supersession and observed failure

This document replaces `docs/fc2-grouped-control-scratch-r1.md`. The r1
candidate incorrectly said the 24,576-byte M1 output allocation was smaller
than grouped metadata. The pinned probe in
`docs/cn4-fc2-scratch-probe-20260803.md` proves that metadata is 3,072 bytes
and fits. The additional 144,384-byte CUTLASS workspace makes the combined
147,456-byte requirement exceed capacity and triggers `-3` before GEMM.

The first reviewed cn4 run therefore found a real capacity defect, but not the
one described by r1. No FC2 numerical result exists from that failed run.

## Exact helper domain and accounting

Add matching pure Rust and C ABI helpers:

```text
valid domain:
  1 <= rows <= 65,536
  1 <= assignments <= min(rows * 8, 65,535)

fc2_grouped_control_scratch_bytes(rows, assignments) =
  max(rows * 6,144 * sizeof(f32), 4 MiB)
```

The 65,536-row ceiling is the maximum of the existing decode and prefill FC2
descriptor domains. Path-specific descriptor validation still restricts
decode to 128 rows. Rust performs the predicate and byte arithmetic in
checked `u64`; invalid inputs return `KernelError::Shape`. C++ performs the
same arithmetic in `uint64_t` and returns zero for the identical invalid
domain. Values above 65,536 may not be accepted by one helper and rejected by
the other.

For grouped-control-capable fixtures:

- `token_output_f32` owns exactly the helper's returned capacity;
- its first `rows * 6,144 * sizeof(f32)` bytes are the final semantic output;
- before reduction, the full allocation is temporary grouped metadata
  followed by CUTLASS workspace;
- same-stream ordering requires initialization, grouped GEMM, and scale
  expansion to finish consuming that state before reduction overwrites the
  semantic output extent; and
- the aggregate grouped-workspace formula replaces its old token-output term
  with this exact scratch term. No padding or another allocation may satisfy
  the charge.

At M1/eight assignments the aggregate grouped-workspace result is exactly
4,554,820 bytes. The helper returns 4,194,304 bytes through row 170 and
4,202,496 bytes at row 171. The 4 MiB floor is a reserve for this pinned
development control, not a bound for arbitrary CUTLASS revisions or the
future fused production kernel.

## Shared native probe ABI

The implementation must factor metadata layout and CUTLASS argument
construction into one host function shared by the actual launch path and an
exported non-launching probe. The probe accepts the validated descriptor and
active-expert count, performs the same SM120 device check, and returns one
fixed 112-byte, 16-byte-aligned record containing:

```text
abi_version:u32, struct_bytes:u32,
rows:u32, assignments:u32, active_experts:u32, sm_count:u32,
flags:u32, reserved0:u32,
metadata_bytes:u64, cutlass_workspace_bytes:u64,
required_scratch_bytes:u64, allocated_scratch_bytes:u64,
headroom_bytes:u64, semantic_output_bytes:u64,
reserved:[u64;4]
```

All flags and reserved fields are zero in v1. Required scratch is the checked
sum of metadata and CUTLASS workspace; allocated scratch is the helper result;
headroom is their checked difference; semantic output is the exact row output
extent. Zero headroom is allowed, negative headroom fails before enqueue. The
probe may query CUDA device properties and CUTLASS workspace sizing but must
not enqueue a kernel or mutate device memory.

The actual grouped launch calls the shared sizing function again immediately
before CUTLASS initialization and fails closed if its required size exceeds
the allocation. A previously retained probe record cannot authorize a later
binary, device, route, or CUTLASS revision.

## CPU/native proof

Before an SM120 rerun, tests must prove:

1. every boundary and mutation of the exact shared shape domain, including
   rows 65,536/65,537 and assignments 65,535/65,536;
2. Rust/native helper equality across all required decode rows and boundary
   prefill rows;
3. exact M1 aggregate 4,554,820, row-170/171 crossover, grouped-SFA
   replacement, output replacement, and checked aggregate accounting;
4. distinct allocation ranges for every persistent plane and exact temporal
   reuse only within `token_output_f32`;
5. a byte-exact 112-byte probe ABI with reserved/flag rejection;
6. mutation checks that independently perturb metadata, CUTLASS workspace,
   allocated scratch, required scratch, and headroom; and
7. every pre-existing FC2 workspace term remains charged.

## Fresh cn4 qualification

After design acceptance, implementation, and an implementation review, run in
a fresh isolated worktree and evidence directory:

1. a non-launching probe for every FC2 matrix route, retaining all probe
   records and requiring positive headroom;
2. FC1 M1 regression;
3. FC2 M1 direct and grouped controls twice;
4. FC2 M8 and the full positive/negative FC2 matrix; and
5. repeat determinism, allocation/leak checks, and post-run GPU-idle checks.

Any helper/probe mismatch, zero or negative headroom, `-3`, invalid route
acceptance, nondeterminism, tolerance failure, allocation leak, or provenance
drift fails qualification. This correction does not qualify a fused operator,
full layer, TP4, checkpoint, quality, capacity, or performance result.
