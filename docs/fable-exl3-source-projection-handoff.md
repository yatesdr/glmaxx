# Fable handoff — direct EXL3 source projection v1

Date: 2026-07-29

Candidate base commit:
`731c3bb02104edad0e154dcc63a26fe6bf224d7d`

Review scope: the independent design/CPU/physical-ABI gate before the first
synthetic EXL3 gate/up/down kernel launch on cn4.

GPU authorization conveyed by this handoff: none. Operator authorization is
separate and already recorded outside this review.

## Required provenance procedure

Hash every input at review start and finish. If either set differs from this
table, stop and report a stale candidate rather than reviewing inferred bytes.

| Input | SHA-256 |
|---|---|
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `docs/exl3-trellis-cpu-contract.md` | `7c5bfa0795aa6b78646b00b029f8d4edc7c15491d69ea19e423f770701b5cdc3` |
| `docs/exl3-sm120-source-projection.md` | `6a889c1987cbf9b0e69b8c99716acd753ad0626496a32d26a8b59135a17f22d7` |
| `docs/cn4-exl3-source-preparation-20260729.md` | `4759330deb6df491a2379c44105b991e3140e816645bf27c39787a2d55d7ad99` |
| `crates/glm-format/src/exl3.rs` | `8b771eb88eac20dae28917faf3cf640b58c3b12baa6193b9720a89d8bc1538b1` |
| `crates/glm-cuda/src/abi.rs` | `8001cbbe8fbd8a4ae915a1e5793a79716b1a77e9d7a35fdfe767e73ce2fa89be` |
| `crates/glm-cuda/src/ffi.rs` | `9e1a0e8d5e2694a68f4534279611c11052bb2b2d2e938391b94ce72424a5a471` |
| `crates/glm-cuda/src/ownership.rs` | `5ef1c916c356d84a55b00168fd5d69e80dc76ff5cf369d7a21a259002834e5ec` |
| `crates/glm-cuda/src/lib.rs` | `801a0630f2b25367c09a21307fac7b96b3d8f277108e44b4153c9f9971ffde67` |
| `kernels/include/glmaxx_kernel.h` | `8a365d0efecc65f24ae0722276e21ec01e6fd71d1a1dd7a8affcac9ace91ce47` |
| `kernels/sm120/exl3_projection_control.cu` | `a50542774a585abeeb451c5248397da3b069296856ca8ae64423786ec5675857` |
| `kernels/CMakeLists.txt` | `aac96117cf7cd7e7262c91a93a64aca49072b71b3c91e6813316041641ba0b98` |
| `crates/glm-cli/src/main.rs` | `963a7797e470f0d1c7aa499bc256781c0a15ae06bc56bc4996e5a32f107ba19a` |
| `scripts/cn4-phase-b-prepare.sh` | `ec10e66a028f8859504007edb03be2361fae6782c41d5bdefa97055dada08a9d` |
| `scripts/cn4-exl3-phase-b.sh` | `740f21dad51f3712220f13bb6ceaba97dca559be39c2495115f3e68118610573` |
| prior `fable-adversarial-v2.md` | `f0019b96d5b35bdca6d026691629b56fbeb0c3c4528e1ae4ff9c1aa06817953e` |
| `docs/fable-v2-disposition.md` | `fd60c89ec188fc6467507ad054f114a379625b0eec40b863cb61c5ace5b1783b` |

The cn4 raw evidence remains outside Git at the two paths named by the compact
preparation record. Verify its listed hashes rather than treating prose as
the evidence.

## Decision 1: inverse source-trellis consumption

Independently re-derive the inverse mapping used by the CUDA kernel for all
256 `(row,column)` positions of one 16×16 tile:

```text
q          = (row & 7) >> 1
row_sel    = 2*(row >= 8) + (row & 1)
col_group  = (column >> 1) & 3
parity     = column & 1
lane       = 8*col_group + 4*parity + q
weight     = 4*(column >= 8) + row_sel
```

Confirm that this is the exact inverse of the reviewed lane/weight scatter,
including the four row quadrants, parity, and low/high column halves. Check
that the Rust test's first and last tiles cannot both pass if tile addressing
is transposed.

Then re-derive:

1. 48 I16 halves and 24 U32 words per three-bit tile;
2. the `+257` window position and 24-word cyclic indexing;
3. little-endian half-to-word assembly;
4. wrapping U32 multiplication by `0xCBAC1FED`;
5. mask `0x8FFF8FFF`, XOR `0x3B603B60`, and the final
   FP32-add/FP16-round boundary.

Report any place where the CUDA expression changes signedness, overflow, or
rounding relative to Rust.

## Decision 2: rotations and projection arithmetic

Verify the exact sequence:

```text
FP16(input * suh)
  -> direct normalized H128
  -> ascending-K FP32 multiply then add
  -> FP16 projection store
  -> direct normalized H128
  -> FP16(H * svh)
```

Check the CUDA intrinsics and compiler flags rather than inferring intent from
comments. In particular, assess whether explicit `__fmul_rn`/`__fadd_rn`,
the decimal normalization constant, half conversion behavior, subnormal
handling, and the CPU oracle establish a defensible first-launch comparison.
Flag any missing boundary fixture required before device execution.

## Decision 3: physical ABI, ownership, and fail-closed behavior

Verify:

1. gate/up are exactly K=6,144/N=512 and down is exactly K=512/N=6,144;
2. Rust and C independently freeze a 144-byte, 16-byte-aligned POD descriptor;
3. every pointer, alignment, version, shape, bit-width, reserved field, row
   limit, and scratch term is validated consistently;
4. scratch is exactly `rows*(K+N)*2`, with output and the four-byte validation
   word separately owned;
5. the Rust fixture keeps every allocation and stream alive until
   synchronization and detects all three non-finite validation bits;
6. source trellis/SUH/SVH are uploaded byte-for-byte with no transpose,
   swizzle, repack, or persistent reconstruction;
7. the synthetic CLI control compares all output positions, hashes every
   plane, and checks two-run bitwise determinism; and
8. the preparation evidence contains a real `sm_120f` cubin, symbol/descriptor
   parity, and resource records but makes no device-correctness or performance
   claim.

Real pinned payload execution is deliberately outside this decision until
the full 92-file source hash gate passes.

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
answer these three questions separately:

1. Is the inverse source-trellis reconstruction accepted for a first SM120
   correctness launch?
2. Are the rotation/projection arithmetic and first-launch comparison
   accepted?
3. Is the v1 descriptor, Rust ownership, fail-closed behavior, and
   direct-source/no-persistent-expansion claim accepted?

Only if all three answers are unqualified `YES`, include these exact lines:

```text
exl3-cpu-contract-sha256=7c5bfa0795aa6b78646b00b029f8d4edc7c15491d69ea19e423f770701b5cdc3
exl3-sm120-design-sha256=6a889c1987cbf9b0e69b8c99716acd753ad0626496a32d26a8b59135a17f22d7
exl3-rust-oracle-sha256=8b771eb88eac20dae28917faf3cf640b58c3b12baa6193b9720a89d8bc1538b1
exl3-cuda-control-sha256=a50542774a585abeeb451c5248397da3b069296856ca8ae64423786ec5675857
```

Then end with the exact gate token:

```text
exl3-source-projection-v1-accepted
```

Do not emit that token for a conditional pass or stale input. Acceptance
opens only the synthetic projection correctness gate; it does not authorize a
GPU launch, qualify a real checkpoint, or establish performance.
