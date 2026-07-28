# First SM120 NVFP4 physical ABI

Status: Phase-A candidate frozen; independent review and SM120 layout probe
still required

Kernel ABI: `glmaxx.sm120.nvfp4.routed_fc1.v1`

CUTLASS pin: `e05f953a5b3d38adc240df2ff928e0421c2abba3`

Pinned CUTLASS layout-header SHA-256:
`598e054bef21edf94b1fd6bb1447cfa9cfcf5a5907ab370128102448dbb6d530`

Quant-policy fragment SHA-256:
`cd909579334405ecd4cd8d9a6c2dfcba7f0124c4c4ba92bc40c976d574be05a3`

## First operator

The first operator is rank-local routed expert FC1 for TP4:

```text
input A:                  [assignments, 6144]
source gate projection:   [2048, 6144]
source up projection:     [2048, 6144]
rank gate shard:           [512, 6144]
rank up shard:             [512, 6144]
packed rank gate/up W:    [1024, 6144]
rank SwiGLU output:        [assignments, 512]
```

Gate and up are independently column-sharded, then concatenated gate-first.
The source tensor is not sliced after concatenation.

## Values

`value_layout_id = 0x1201`. Logical `W[N,K]` is contiguous row-major and is
therefore the same address sequence as CUTLASS column-major operand `B[K,N]`.
Even linear elements use the low nibble; odd elements use the high nibble.
N pads to 128 and K pads to 64.

## Scales

`scale_layout_id = 0x1201`. The natural tensor is
`S[N_padded,K_padded/16]`. For `(n,g)`:

```text
offset = 512 * ((n / 128) * (K_padded / 64) + g / 4)
       + 16 * (n % 32)
       + 4 * ((n % 128) / 32)
       + g % 4
```

The Rust proof exhaustively checks that all 393,216 FC1 scale offsets are
unique and cover the plane. The cn4 build additionally compiles and runs
`glmaxx_cutlass_layout_probe`, which compares every offset with pinned
`Sm1xxBlockScaledConfig<16>::tile_atom_to_shape_SFB`.

## Arithmetic

Weights use one tensor-shard FP32 global scale, one saturated-finite E4M3
scale per K-consecutive block of 16, E2M1 values, round-to-nearest-even, and
FP32 accumulation. Activations use one dynamic global scale per BF16 row and
one block-16 E4M3 scale. The packed activation row is reused for gate and up.
The first direct CUDA baseline computes both dot products together and stores
BF16 `SiLU(gate) * up` without a global gate/up intermediate.

The current device dot product is the correctness control. It uses CUTLASS
numeric types and the pinned physical layout but not block-scaled MMA yet.
Decode uses a fixed multiprocessor-sized persistent CTA pool that strides over
assignment/column work; prefill uses one grouped CTA per assignment/column.
Replacing it with the CUTLASS SM120 MMA collective is the first required M2
performance optimization; both results must be retained.

## Rust/C boundary

The descriptor is 224 bytes, aligned to 16, versioned, POD-only, and checked
by both languages. Rust validates fixed geometry, path, pointers, alignment,
workspace arithmetic, reserved fields, and overflows before launch. Native
allocations and streams have RAII owners. A launch is asynchronous; the
caller must synchronize or query before consuming output.

No runtime weight transpose, swizzle, repack, or persistent dequantization is
permitted.
