# FC1 direct-control oracle correction r1

Date: 2026-08-03

Status: corrective design candidate; adversarial acceptance required before
CPU implementation or an SM120 rerun

## Problem

The FC1 matrix compares the CUDA-core direct control with a semantic CPU
oracle that accumulates K sequentially. The CUDA control instead has 256 FMA
lanes and a fixed tree reduction. At M=256 the deterministic-random fixture
contains a cancellation boundary where the semantic result rounds to BF16
`-172` and the specified device schedule rounds to BF16 `-177`. cn4 returned
`-177` exactly. The current 2% rule therefore reports 43 failures even though
the device follows its declared arithmetic.

This correction must not loosen the frozen tolerance, turn a schedule-specific
control into the model semantic definition, or bless CUTLASS accumulation by
analogy.

## Dual-oracle contract

Keep `routed_fc1_oracle` unchanged as the portable semantic oracle. Add a
separate `routed_fc1_direct_control_oracle` used only for the retained
CUDA-core direct control. For every row and gate/up column it must reproduce:

1. BF16 input admission and the existing byte-exact NVFP4 activation pack;
2. the existing packed weight bytes and decoded scales;
3. 256 independent FP32 accumulators using explicit fused multiply-add at
   `k = lane, lane+256, ...`;
4. explicit FP32 additions at strides `128,64,32,16,8,4,2,1`;
5. the ordinary non-fast-math SiLU expression; and
6. BF16 round-to-nearest-even output.

The direct-control matrix evaluates every finite device element as follows:

```text
semantic_ok = abs(device - semantic) <= 0.5 + 0.02 * abs(semantic)
schedule_exact = device_bf16_bits == direct_control_oracle_bf16_bits
accepted = semantic_ok || schedule_exact
```

Non-finite semantic, schedule, or device values fail. A schedule match may
rescue a semantic deviation only for the named direct CUDA-core backend and
only when the device bits are exact. It may not qualify the dense or grouped
CUTLASS controls, graphs of a different kernel, fused production kernels, or
quality.

## Evidence and schema

Rev the case and summary schemas. Preserve both reference bit patterns, both
absolute/relative errors, all semantic deviations, all schedule mismatches,
the number of exact schedule rescues, and unresolved failures. The corrected
first-run fixture must report exactly 43 semantic deviations, 43 exact
schedule rescues, and zero unresolved failures. Do not discard the original
failed evidence.

CPU proof must pin the exact row-239/column-20 boundary and independently
prove sequential `0xc32c`, scheduled `0xc331`, difference `5.0`, and the
complete 256-lane index partition. Mutations to lane count, FMA use, stride
order, input rounding, scale multiplication, gate/up pairing, SiLU, or BF16
rounding must be detected.

After review and CPU proof, rerun the full 135-positive/nine-negative matrix
in a fresh cn4 worktree. Then run graph and CUTLASS controls under their own
unchanged gates. This correction qualifies no layer, checkpoint, quality,
capacity, or performance result.

