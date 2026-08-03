# cn4 reviewed kernel Phase B at `db91038`

Date: 2026-08-03

Status: fail-closed qualification; NVFP4 FC1 M1 passed on SM120 and FC2 M1
stopped at the known grouped-control scratch boundary

## Result

The isolated qualification commit
`db91038d8a652675968a61938d72d85cb14fbeb6` is the reviewed kernel candidate
`0edfc8d796aeaeb969668005149bcb6286aa1e85` plus exactly three tracked Fable
acceptance artifacts. The delta contains no compiled Rust or CUDA source:

```text
A fable-exl3-source-projection-v1-r2.md
A fable-exl3-warp-decode-v2-r2.md
A fable-manifest-abi-v022-r2.md
```

The run passed all 163 committed Rust tests, both independent CUTLASS layout
probes, real `sm_120f` compilation, the exact 256 owned-NVFP4 OMMA count, and
the required NVFP4, EXL3, and FC2 exported-symbol checks.

The first device gate, NVFP4 FC1 at M1, launched on cn4 and passed:

| field | value |
| --- | --- |
| ABI | `glmaxx.sm120.nvfp4.routed_moe.v2` |
| shape | `[1, 6144, 1024]` |
| failed elements | `0` |
| maximum absolute error | `2.0` |
| maximum relative error | `0.027397260069847107` |
| frozen tolerance | `abs <= 0.5 + 0.02 * abs(reference)` |

The next gate, `gpu-fc2-smoke 1`, returned:

```text
glmaxx: Driver(-3)
```

The phase stopped immediately. It did not execute or accept the FC2 matrix,
graph, dense-control, grouped-control, or timing gates. The separately pinned
host sizing proof explains this exact failure: M1 needs 3,072 bytes of
grouped metadata plus 144,384 bytes of CUTLASS workspace, while the retained
descriptor exposes only the 24,576-byte token-output plane. The required
147,456 bytes exceed that plane by 122,880 bytes. This run does not accept a
scratch correction; `docs/fc2-grouped-control-scratch-r2.md` still requires
its dedicated Fable token before implementation qualification.

The final verdict is:

```text
SM120_PHASE_B_FAIL_CLOSED_FC2_DRIVER_MINUS_3
```

## Provenance

```text
qualification commit  db91038d8a652675968a61938d72d85cb14fbeb6
reviewed candidate    0edfc8d796aeaeb969668005149bcb6286aa1e85
manifest review SHA   b95fc05837ef1de91fda44bc1b3df49224dde93c76554efbeef3a9a58a70d882
source review SHA     cac885880345fb2f02e940bcf0cd32420acf5ac8a6a3e34fc76e7971a5aa2964
warp review SHA       c26236ec0e57b56d90028edb8396dd5521e5cec75174401d04469eefd33990b5
CUTLASS commit        e05f953a5b3d38adc240df2ff928e0421c2abba3
container digest      sha256:0b400cb8ba8dc58d8ae9729702260b5c3d1abaa063a8ef9e14380d72df773842
kernel library SHA    389e8fad937bb610ea125532e21f7a8ef6dbd0ae91e825aca92fed6578c9c335
Rust binary SHA       45c0c9c4f580c65da1e00ee44264f0fe9b50b1c9d0b5736f1d087c5066e1d7aa
```

The pinned input hashes were:

```text
engine-v0.md                 efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
format-v0.md                 619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a
glm52-operation-v1.json      8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6
sm120-fc1-matrix-v1.json     d5a0286d0c9d06ce1036085d4f1712d929273fb533601806b26b9f774c360e74
matrix-proof-v1.json         5ebf329ee29e4cd95e2c92a41a99625808dcf4212f996c874d651d637cdb6eef
```

Toolchains were CUDA 13.3 (`nvcc` 13.3.33), Rust 1.92.0, and CMake 3.28.3.
The detached source remained clean before and after the run.

Raw evidence is retained outside Git at:

```text
/home/derek/glmaxx/evidence/20260803T225000Z-phase-b-db91038
```

Selected receipt hashes are:

```text
cargo-test.txt             66fc9ca06d4ab2db438c383940cff2d90767020698c5ca8427a143685723d6ef
cmake-build.txt            8f7b366bc7b9f5b4fe831a275a342ce98f35778d8baf89128adad09cd20e4a19
gpu-smoke-m1.json          c62f3979f3daaa741740d435bb530200b97494b6f2d2611ef91f8b05fe7e3618
gpu-fc2-smoke-m1.json      a3c979253567cd9ba7fa4827fabd192a354344e197682a1519c591f2f7f2a11e
failure-verdict.txt        75304a861e57f93655d70559a3c09cc86cce0432b6a27d3d568285df76708e9e
gpu-inventory-after.csv    b0a97edab3d845183c0c4d04013d5c01fa64caf4a2ecd0e41436e8e7cf05b266
timestamp-verdict.txt      9b8334e7bf7feff5dfb4557b4fbe444478d5031c9938cc57a1477c1149288451
evidence-manifest.sha256   f80c0a72bcb222944859c1592b806b2e6818abdce2f0d39416fad9142d73835c
```

Postflight showed 2/2/2/10 MiB used, no compute application, and no retained
CUDA context. The small nonzero utilization field observed on GPU 3 during
the final inventory is not a GLMAXX compute process and is not used as timing
or idleness evidence beyond the empty compute-application list.

## Timestamp defect

The directory basename is not a valid start timestamp. It embeds 22:50Z, but
filesystem birth records the directory at 22:03:48.548Z, FC1 at 22:05:18.888Z,
FC2 at 22:05:29.921Z, and the seal at 22:14:14Z. The external
`timestamp-verdict.txt` preserves this discrepancy. The basename is treated
only as an opaque run identifier, no duration is derived from it, and future
run directories must derive their UTC name at creation.

## Claim boundary and next gate

This is reproducibility evidence for the reviewed FC1 pass and FC2
fail-closed boundary. It is not a complete fused-MoE result, real checkpoint
replay, TP4 layer execution, model smoke, KLD result, KV-capacity result,
serving result, or throughput measurement.

The next device gate is a fresh immutable Phase B after the FC2 scratch r2
design is accepted, implemented, CPU-proved, and independently rebound to the
exact source. Real TR3 admission additionally remains behind the separate
safetensors `total_size` and mixed-K EXL3 r2 reviews.
