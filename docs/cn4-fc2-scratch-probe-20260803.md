# cn4 FC2 grouped scratch probe

Date: 2026-08-03

Status: source-pinned non-launching diagnostic; no FC2 correctness, kernel,
layer, checkpoint, quality, capacity, or performance claim

## Result

The original scratch contract attributed the M1 `-3` to grouped metadata
alone exceeding the 24,576-byte `token_output_f32` allocation. A host-side
probe compiled the exact accepted FC2 control source into one translation
unit and invoked the same `grouped_scratch` and
`GroupedGemm::get_workspace_size` expressions. It created a CUDA context only
to query the current device's SM count; it enqueued no kernel.

For M1, eight assignments, and eight active experts on the 188-SM device:

```text
grouped metadata          3,072 bytes
CUTLASS workspace       144,384 bytes
combined requirement    147,456 bytes
available capacity       24,576 bytes
metadata fits                 yes
combined requirement fits      no
```

Therefore the observed `-3` is the combined metadata-plus-CUTLASS-workspace
check in `enqueue_grouped_prepared`, not the earlier metadata-only check in
`prepare_grouped_control`. The capacity correction remains necessary, but
the r1 explanation is false and must not be reviewed as written.

The audit also found that the draft Rust helper rejects `rows=u32::MAX` when
`rows * 8` overflows `u32`, while its C++ counterpart computes that predicate
in `u64` and accepts it. The corrected contract explicitly caps the shared
path-independent helper at the production FC2 maximum of 65,536 rows.

## Provenance

```text
source commit       c25e55843062dd777c4778a9f5d19cd9221a3278
FC2 source SHA-256  dba61fd6bc34b659543f1b64a329603ab01406a1505954ae16bc9626b8f7ff94
library SHA-256     f75f533d0f5476594a5eb8671a555fc1004da00877a48d48e33261d8a6a5b40a
probe source SHA    a3d0ea9a12e8f2242c695f5f6c9551b764c90ccb9d2516ffbda735ff8c120ffe
probe binary SHA    66283b213898f054246be9d01c70d4584f74930b6be63edceb3b23beb4c0b832
container image     sha256:4a041313a952def9eb7353f055ee4061f5d76416e090aca04529a597b0bd549a
```

The successful 14-entry evidence stream is outside Git at:

```text
/home/derek/glmaxx/evidence/20260803T191700Z-fc2-scratch-probe-c25e558
```

Its `evidence-sha256.txt` SHA-256 is
`44efef29ecfabd552345368d22785c054d79df702f616ae86630276b9396bda7`.
Every entry revalidated after the run. cn4 returned to 2/2/2/10 MiB used,
0% utilization, and no compute process.

Two failed setup attempts are retained rather than hidden. The first mounted
the build directory over `/lib` and failed in the NVIDIA container hook before
compilation; its 13-entry stream hashes to
`8796555518987637e47f9f23681cb4e81004079530bb6c595873c2177ebb77b6`.
The second used an unsupported `nvcc` rpath spelling and failed compilation;
its 15-entry stream hashes to
`bec6083fe43533afa8ff2affc78e50ba1cb5ca6d652e4ac34d7bd0a5f84a55a2`.
Neither attempt created a probe binary or launched a kernel.
