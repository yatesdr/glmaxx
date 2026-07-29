# SM120 EXL3 warp-staged decode projection

Date: 2026-07-29

Status: design candidate; no CPU proof, compiled kernel, device result, or
performance claim

Predecessor ABI: `glmaxx.sm120.exl3.source_projection.v1`

## Purpose and boundary

This is the first performance successor to the retained scalar direct-source
control. It optimizes only the projection stage for decode rows 1–8 while
retaining the reviewed input/output rotations, descriptor, source bytes, and
rounding order. Gate, up, and down continue to consume the pinned
three-bit source trellis with no persistent reconstructed matrix.

The first candidate deliberately does not address prefill, grouped expert
routing, paired gate/up, or tensor-core execution. Those are separate
successors. A narrow decode kernel is useful because it can prove the
source-byte traffic floor and establish whether fragment-local reconstruction
is viable before more arithmetic or routing is fused into the boundary.

## Frozen launch geometry

The projection launches one 256-thread CTA for each 16-column output tile:

```text
grid.x  = N / 16
block.x = 256
rows    = 1..8
```

Each warp owns two logical rows. Its low and high 16-lane subwarps own one
row each; lane `0..15` within a subwarp owns one output column. Inactive row
subwarps still reach every CTA barrier but do not load activations, decode
weights, accumulate, validate, or store.

For each group of eight consecutive K tiles, threads 0–191 cooperatively load:

```text
8 tiles * 24 U32 words/tile = 192 U32 = 768 bytes
```

into one CTA-local shared-memory stage. The remaining 64 threads issue no
trellis load. All threads synchronize, active subwarps consume the eight
tiles in ascending order, and all threads synchronize before the stage is
overwritten.

Both real K tile counts are exact multiples of eight:

| Projection | K tiles | N tiles | K-stage iterations |
|---|---:|---:|---:|
| gate/up | 384 | 32 | 48 |
| down | 32 | 384 | 4 |

## Exact source addressing

The source trellis is unchanged:

```text
I16[K/16, N/16, 48]
U32[K/16, N/16, 24]
```

For `k_tile`, CTA-owned `n_tile`, and `word`:

```text
tile_index = k_tile * (N / 16) + n_tile
source_u32 = tile_index * 24 + word
stage[k_tile % 8][word] = trellis_u32[source_u32]
```

The 4-byte source alignment already required by the v1 descriptor makes the
U32 view legal. A tile is 96 bytes, so every tile base is also 4-byte aligned.
Little-endian half-to-word assembly is therefore identical to the scalar
control on the pinned little-endian cn4 host.

Within a staged tile, the existing inverse mapping remains authoritative:

```text
q          = (k_local & 7) >> 1
row_sel    = 2*(k_local >= 8) + (k_local & 1)
col_group  = (n_local >> 1) & 3
parity     = n_local & 1
lane       = 8*col_group + 4*parity + q
weight     = 4*(n_local >= 8) + row_sel
```

The `+257` window position, 24-word cyclic indexing, wrapping U32 multiply,
mask/XOR, and FP16 reconstruction are byte-for-byte the v1 operations. Only
the location of the 24 source words changes from global memory to shared
memory.

## Arithmetic invariant

Every output lane processes:

```text
for k_tile in 0..K/16:
  for k_local in 0..16:
    k = 16*k_tile + k_local
    accumulator =
      __fadd_rn(accumulator,
                __fmul_rn(rotated_input[row,k],
                          decode(stage_tile,k_local,n_local)))
```

This preserves the scalar control's exact ascending-K FP32 multiply/add order.
It must therefore be bitwise identical at the intermediate FP16 projection
plane, not merely within a tolerance. The unchanged output rotation must then
be bitwise identical as well. Any difference is a correctness failure; the
candidate may not relax the v1 threshold after observing hardware output.

## Traffic and capacity claims

Each CTA loads every K tile for its one N tile exactly once, independent of
row count. Both real projections therefore read exactly:

```text
(K/16) * (N/16) * 96 = 1,179,648 trellis bytes
```

from the kernel's logical global-load schedule. This is an address-count
claim, not yet a DRAM-byte claim; cache-line overfetch, replay, and measured
device traffic remain hardware evidence.

The candidate adds 768 bytes of static shared memory and no persistent or
dynamic global allocation. It reuses the v1 rotated-input and projected-FP16
scratch planes, so the external workspace remains exactly
`rows*(K+N)*2`.

## Fail-closed route

The optimized entry point must:

- accept only the unchanged v1 descriptor and rows 1–8;
- reject all other rows rather than silently selecting another kernel;
- repeat the v1 SM120 device-property check;
- retain all pointer, shape, bit-width, reserved-field, and workspace checks;
- clear and preserve the three-stage device validation word;
- enqueue only on the caller-owned stream; and
- leave the scalar v1 launcher independently callable as the control.

Rust selects the entry point for the whole launch. There is no device-side or
rank-local fallback. A later TP4 executor must select the same path on all
four ranks before any collective schedule begins.

## Gate sequence

1. independent adversarial review of this address and arithmetic schedule;
2. CPU staged-tile proof for every local tile position and both real
   gate/down geometries;
3. clean `sm_120f` compile, SASS/resource inspection, and native ABI parity;
4. synthetic M1/M2/M4/M8 bitwise comparison with the retained scalar kernel;
5. full-hash-gated real gate/up/down payload comparison;
6. kernel-only and inclusive timing with source bytes and rotations matched;
7. profiler counters for global/shared traffic, occupancy, replay, and stalls.

No timing result is admissible until steps 1–5 pass. No result from this
kernel establishes prefill, grouped MoE, one-layer, checkpoint, or model
quality correctness.
