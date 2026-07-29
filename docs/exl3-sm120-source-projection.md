# SM120 direct EXL3 source-projection control

Date: 2026-07-29

Status: implementation candidate; CPU oracle complete; independent review,
clean `sm_120f` compilation, and device correctness are pending

Kernel ABI: `glmaxx.sm120.exl3.source_projection.v1`

## Purpose

The first EXL3 GPU boundary is one actual GLM-5.2 TP4 expert projection. It
exists to prove that the native source-order trellis can be consumed directly
without converting the checkpoint to another persistent representation.

The control supports exactly:

| Projection | K | N |
|---|---:|---:|
| gate | 6,144 | 512 |
| up | 6,144 | 512 |
| down | 512 | 6,144 |

Rows are bounded to 1–3,072. The first reviewed device gate uses M1 for gate,
up, and down, then expands to the decode and prefill row buckets. Only
three-bit MCG payloads with multiplier `0xCBAC1FED` are admissible.

## Direct source boundary

The kernel receives three pointers that alias the native container planes:

- little-endian I16 trellis in `[K/16,N/16,48]` source order;
- FP16 `suh[K]`;
- FP16 `svh[N]`.

There is no weight transpose, swizzle, repack, dense expansion, or
dequantization allocation. The scalar correctness control reconstructs a
single native FP16 weight immediately before its ordered FP32 multiply-add.
A later optimized kernel may reconstruct a tile into registers/shared memory
or directly into MMA fragments, but it may not persist an expanded expert.

The loader remains responsible for metadata CRC, component lengths, finite
rotations, projection identity, layer/expert/rank bounds, MCG marker, and
container hashes. The launch descriptor independently rejects shape,
version, pointer, alignment, reserved-field, and workspace lies.
Before any asynchronous operation, the native launcher also queries the
caller-bound CUDA device and rejects anything other than compute capability
12.0.

## Arithmetic

The operation reproduces the CPU contract:

```text
rotated_input =
  FP16(H128(FP16(input_f16 * suh)))

projected =
  FP16(ascending-FP32-dot(rotated_input, decoded_trellis_weight))

output =
  FP16(H128(FP16(projected)) * svh)
```

Each H128 output accumulates source indices `0..127` in ascending order and
multiplies by `1/sqrt(128)`. The projection uses explicit round-to-nearest
FP32 multiply and add instructions in ascending K order. Trellis decode uses
wrapping U32 multiplication, the pinned mask/XOR, and an FP16 add of the two
decoded halves.

The source CPU oracle remains the semantic authority. The first GPU tolerance
is deliberately broad:

```text
finite(gpu) and abs(gpu-cpu) <= 0.5 + 0.03 * abs(cpu)
```

Every result records maximum absolute/relative error, failing elements,
source/input/output hashes, and two-repeat bitwise determinism. This gate
detects layout and arithmetic failures; it is not the final model-quality
threshold.

## Descriptor and scratch

`glmaxx_exl3_descriptor` is 144 bytes, 16-byte aligned, POD-only, and mirrored
in Rust. Its eight 32-bit fields freeze version, byte size, flags, projection,
rows, K, N, and bits. It then carries source pointers, two temporary FP16
planes, output, a device validation word, total scratch bytes, sequence, and
four zero reserved words.

The exact temporary allocation is:

```text
rotated_input_f16 = rows * K * 2
projected_f16     = rows * N * 2
workspace_bytes   = rows * (K + N) * 2
```

Output and the four-byte validation word are caller-owned but not counted as
scratch. M1 gate/up scratch is 13,312 bytes; M1 down is the same. No allocator
operation occurs inside a launch.

## Launch stages

The retained control enqueues three kernels on one caller-owned stream:

1. one 128-thread CTA per `(row,K/128)` input rotation block;
2. a strided 256-thread scalar projection grid;
3. one 128-thread CTA per `(row,N/128)` output rotation block.

Both rotation kernels guard work while keeping every CTA thread live through
the shared-memory barrier. Their FP32 multiplies use explicit
round-to-nearest intrinsics.

Non-finite values set distinct device-validation bits for input rotation,
projection, and output rotation. A nonzero word fails the Rust result after
stream synchronization.

This three-launch scalar implementation is expected to be slow. It is not a
performance candidate and must remain available as the direct-source
correctness control while fragment-local decode, paired gate/up execution,
grouped routing, and SM120 MMA successors are developed.

## Required review and evidence

No EXL3 device launch is authorized by this document. Independent review must
first re-derive:

- the inverse trellis scatter mapping for all 256 positions of a 16×16 tile;
- the cyclic 24-word window extraction;
- FP16 and ordered-FP32 rounding boundaries;
- exact gate/up/down shapes and byte arithmetic;
- Rust/C descriptor layout and scratch parity;
- absence of a persistent reconstructed matrix.

After acceptance, a fresh external cn4 evidence directory must contain source
commit, clean status, container/toolchain/CUTLASS identities, compiler
commands, cubin/SASS/resource records, ABI-check output, and the three M1
gate/up/down reports. Real pinned checkpoint payloads replace the synthetic
trellis only after the complete checkpoint hash gate passes.
