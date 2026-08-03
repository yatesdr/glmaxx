# cn4 W4A16/NF3 kernel-source audit

Date: 2026-08-03

Status: read-only implementation-source audit; no GPU or checkpoint-payload
evidence

## Scope and provenance

The audit used password-only SSH to cn4 at `192.168.13.34`. It read existing
source exports only. It did not create a CUDA context, launch a kernel, start a
container, write a remote file, or modify a vLLM/SparkInfer resource.

```text
adapter root    /home/derek/sol-stage-vllm-pr189
SparkInfer root /home/derek/glm52-opt-dynamic-build/workspace/b12x-nvfp4-dynamic-scale
```

The SparkInfer export contains a stale copied-worktree `.git` file whose
gitdir names a path on another host. Therefore no Git revision is asserted for
its contents. A nearby image recipe labels the base SparkInfer revision
`c3828fd7f807ce237a9ac36ef033659e6f6b6dd3`, but that label is context only;
the exact source-file hashes below are the authority.

| Source | SHA-256 |
|---|---|
| vLLM `nvfp4_nf3_hybrid.py` | `a8b4e19c5e776ece1d6c7ff2c48da236d1bd4032a3399f7b6a9563955c99f61b` |
| SparkInfer W4A16 `host.py` | `c6b41bf23b3d18024a1a8b4d19fff168fb51de164624a32c72c2e01571e9d2d4` |
| SparkInfer W4A16 `kernel.py` | `7bae99dfff0ab8f61f1d2a0f36a401543f32a39e2e3982668fcc89a44e882f05` |
| SparkInfer W4A16 `prepare.py` | `b54175a861730662350a2ef5ee63989c8afafc907b6a3c13a3331928cfb9285f` |
| SparkInfer W4A16 `route_pack.py` | `c9f6cccc8a74a708712a4cb0c76dc00e275a6017ee7a8bbc1f70aa70e88544c1` |
| SparkInfer `_lib/intrinsics.py` | `07dcf2deacafb4e10c08e5a76818387d3d8afc9afe5f8f278078e86c68b8bb33` |

The earlier inventory pins a different W4A16 `kernel.py` hash,
`11a4dedeb1ff8eee01c13314582081776059e719658dc4189eb6cdc76eb68c4d`.
That older source remains the authority for the r2 format derivation. The
newer hash above is a separate implementation precedent for the unified
hybrid decode route; it does not silently replace a reviewed input.

An attempted aggregate inspection of the real checkpoint's four-byte scalar
payloads was rejected by the execution environment's sensitive-data egress
policy. No scalar payload was read or exported. Tensor names, shapes, dtypes,
and source-component independence remain established by the existing
metadata-only inventory.

## Actual reference execution route

The source does not execute BF16 activations through the native FP4 x FP4
block-scaled MMA used by the current GLMAXX CUTLASS control. Its W4A16 route
does the following:

1. retain BF16 activations;
2. repack ModelOpt values/scales as `packed` plus `e4m3_k16`;
3. repack NF3 values/scales as `nf3_2p1` plus `e4m3_k32`;
4. reconstruct E2M1 or NF3 codebook weight fragments as packed BF16 register
   pairs;
5. issue
   `mma.sync.aligned.m16n8k16.row.col.f32.bf16.bf16.f32`; and
6. accumulate into FP32 before the BF16 output boundary.

For small-M GLM-5.2 decode, the adapter pins:

```text
hidden                         6144
rank-local intermediate        512
top-k                            8
tier 0 ModelOpt experts         64
tier 1 NF3 experts             192
fc1 tile K/N                 64/256
fc2 tile K/N                 64/256
per-tier direct-decode M       <= 8
unified E64/E192 grid M        <= 4
```

The unified kernel receives both tiers' FC1/FC2 values, block scales, and
global scales; global top-k IDs; a complete 256-entry `(tier, local-id)` map;
top-k weights; disjoint FC1/FC2 FP32 scratch; intermediate BF16 buffers; and
one output. It schedules whole tiles, uses direct top-k routing, and folds the
ordered top-k sum into the FC2 path. The source admission expects one block
per SM, 45,184 bytes of shared memory, and no local-memory spill for the
pinned specialization. These are source claims, not GLMAXX measurements.

Large-M uses stable per-tier route compaction and preplanned launches. One
process-wide scratch/buffer set is reused serially across layers. Compilation
is completed during eager preparation rather than graph capture.

## Correctness boundary GLMAXX must not copy

The adapter loads gate and up `weight_scale_2` into two columns, but its
ModelOpt preparation selects only column zero when constructing the fused FC1
global scale. Consequently the adapter source is not proof that collapsing
two independently named checkpoint scalars is numerically valid.

GLMAXX must retain the r2 contract's stricter rule:

- codec `0x0102` carries separate gate and up outer-scale bits;
- the FC1 epilogue applies each scale to its own projection before SwiGLU;
- FC2 carries its independent down scale; and
- equality in one checkpoint, if later established under an approved
  procedure, cannot weaken the ABI or source identity.

The adapter is therefore an implementation/performance precedent, not a
quality oracle for the fused-scalar boundary.

## Implementation consequence

The existing GLMAXX W4A4 CUTLASS control cannot consume the production
ModelOpt checkpoint under the selected W4A16 numerical policy. The first
production implementation needs a distinct Rust-owned launch ABI and
SM120-only kernel family that:

- consumes native codec `0x0102` and codec `0x0300` without persistent dense
  BF16 weights;
- decodes weights into BF16 register fragments and uses BF16 MMA with FP32
  accumulation;
- preserves separate ModelOpt gate/up outer scales;
- uses one rank-invariant direct-top-k work stream for the E64/E192 small-M
  path and a stable partition for prefill;
- owns disjoint scratch and deterministic reduction order;
- rejects resource spills, geometry drift, codec/layout drift, and
  rank-local fallback before launch; and
- binds the compiled module/config generation without moving resident
  weights.

The reference's pinned `(64,256,64,256)` tiles are the first measured-control
candidate, not a performance conclusion. GLMAXX must compare them against its
own actual-shape tile sweep on SM120 after the r2 design and CPU-proof gates.

## Nonclaims

This audit does not accept the pending r2 design, authenticate source exports
as dependencies, approve source copying, prove checkpoint scalar values,
implement a codec or kernel, qualify SM120 resources, or establish correctness,
quality, fit, capacity, cold start, or speed.
