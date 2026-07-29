# cn4 CUDA-graph preparation result

Date: 2026-07-29

Verdict: `PREPARED_NO_DEVICE_LAUNCH`

This is a compact record of the fresh no-launch build after adding the
CUDA-graph correctness harness. Raw output remains outside Git at:

```text
/home/derek/glmaxx/evidence/prepare-20260729T062055Z
```

## Provenance

| Input | Identity |
|---|---|
| source commit | `3c41718a0a604444c9b8f65e25c865ffed1b188b` |
| source status | `main...origin/main`, clean |
| container | `sha256:4eb9de29e5a532c672697762ca7b24e5e82316d6795dfdf616f129d35b794109` |
| Rust | `1.92.0 (ded5c06cf 2025-12-08)` |
| CUDA compiler | `13.3.33` |
| CUTLASS | `e05f953a5b3d38adc240df2ff928e0421c2abba3` |
| target | four RTX PRO 6000 Blackwell Workstation Edition GPUs, SM120 |

The checkout, CUTLASS tree, and repository inputs were mounted read-only.
Cargo targets and native build output were directed into the fresh external
evidence directory.

## Results

- All 115 workspace tests passed in the pinned Linux container.
- CUDA 13.3 compiled the direct FC1 control and the new graph
  capture/instantiate/launch bridge with `-arch=sm_120f`.
- The CUTLASS SFB layout probe passed all 393,216 comparisons.
- `cuobjdump` retained a native `nvfp4_routed_fc1.sm_120.cubin`.
- Resource usage was unchanged:

  - `direct_fc1_swiglu`: 34 registers, 3,072 shared bytes;
  - `quantize_compacted_rows`: 30 registers, 2,048 shared bytes.

- The release Rust `cuda-ffi` executable linked to the newly built native
  library.
- The preparation script did not call `gpu-smoke`, `gpu-matrix`, or
  `gpu-graph`, and it launched no CUDA kernel.

## Built artifact hashes

| Artifact | SHA-256 |
|---|---|
| `libglmaxx_sm120.so` | `fcf64e91178723c8ea1c902c22cc02fffc5c646c6196127f38c299cc35e421ab` |
| `glmaxx_cutlass_layout_probe` | `622224a55f159afa506d0444b60e51768337bc7d4516c21cb9c7eecaa4df80d3` |
| release `glmaxx` | `4ac07933b6c940d52ed355bd80963c5dfa3f94dc5acf3bebce00454ce3566606` |

## Critical raw-record hashes

| Raw record | SHA-256 |
|---|---|
| `source-commit.txt` | `8447bb1b40cd1cb0d7498293761f47a0e890d2c64108ef7192d729ac7ca3422c` |
| `input-sha256.txt` | `f9fe92fab4b0558c5589e86b8c024863b12c2d6494ed47dbff31d5fe952dba72` |
| `cargo-test.txt` | `aea73f154863d4540c100d9a33b627d9b19d79f7d2d1cf031f8b6118bf1d60d9` |
| `cmake-build.txt` | `5927d1abf8185a7ff83b3c92b20aba6bfbfeab3b4d3a67629b976d229b3e6c6a` |
| `cutlass-layout-probe.txt` | `d77e30c3875b5e410b443974419c21b8e002f05ae48c088cbc61ff26c5e53485` |
| `cuobjdump-elf.txt` | `c5ddba11b8cfb75d262fa3ec759465d7d9d35ddbaf01618095428f9a6cadf612` |
| `cuobjdump-resources.txt` | `d0e9fc7893bf317e84df4330ccf4d60f6a051e01c2ccf41127484e89cccdfff1` |
| `cuda-ffi-linkage.txt` | `093b80bf348be82717c6ed04b6599c29c09112c958c3c4a17161ba5f41e21428` |
| `build-artifact-sha256.txt` | `3acfb845e57942ad46fc736c72d88e5c10fff7d503ef7befca11de43f8dc034f` |
| `verdict.txt` | `cee765aa6cc4fe7afe5057ca22d1f67b1e23973b5a793aec3ae2521b75f65f62` |

## Remaining gate

Compilation is not device correctness. The phase-B launch still requires the
committed independent review artifact containing the exact
`manifest-abi-v0.2.2-accepted` token. When that gate passes, the prepared
runner will execute the M1 smoke, 135-case eager matrix, and the two-case,
20-replay CUDA-graph matrix in that order.

## Follow-up activation-scale proof

Commit `bf24aaf8498a2139d0d27760bf78e1264739174c` added a separate host-only
probe for the dynamic activation `SFA` layout. A fresh preparation run is
preserved at:

```text
/home/derek/glmaxx/evidence/prepare-20260729T062811Z
```

The pinned CUTLASS
`Sm1xxBlockScaledConfig<16>::tile_atom_to_shape_SFA` mapping agrees with the
runtime activation-scale offset for 17 assignment shapes spanning 1 through
65,535 assignments: 42,564,864 offset comparisons passed. It also agrees that
storage is `round_up(assignments, 128) * 6144 / 16` bytes. This proves that
the direct control's activation workspace can feed the later MMA operand
without a scale transform.

| Follow-up record or artifact | SHA-256 |
|---|---|
| `cutlass-activation-layout-probe.txt` | `ab2d8233671a2fd5301db2c45c6edd0255d5a339c55d38aec547b1d5e080d794` |
| `cargo-test.txt` | `787e6455b97d804b3382c519b3aaf652a01b9dffa1d4e562e37d36016af6212b` |
| `cmake-build.txt` | `2aad2f1f42f67ad7f78e18d41616a0b27e37a58087bf6269e2c5e15c91e5c5d1` |
| `build-artifact-sha256.txt` | `6a894b511dd7a3879e8449069c1f06884155eed492e4bd3918dd17d803f610f7` |
| `verdict.txt` | `3d0e151f9f146ce7d391c425b23ca95d945d4ca0619cb2a59b95fcd42be9dc3b` |
| `libglmaxx_sm120.so` | `2c6306953bbf52e050f33722018dbaefe7f844096ae2cac1c9dd73f3c900d87a` |
| `glmaxx_cutlass_activation_layout_probe` | `eb9d5e7ecc68e00d32fc9a9b309405e8e1f6812fa9adff18deea67173bff5dd7` |

## Compile-only SM120 tensor-core control

Commit `0aa490d2af326c57f43cb39c5376837cee50a13a` added the pinned
CUTLASS 79a dense NVFP4 control to the no-launch build. The successful fresh
preparation record is:

```text
/home/derek/glmaxx/evidence/prepare-20260729T064629Z
```

The result remained `PREPARED_NO_DEVICE_LAUNCH`. All 117 Rust tests passed,
as did the 393,216 SFB and 42,564,864 SFA comparisons. CUDA 13.3 compiled and
linked a native `sm_120f` CUTLASS kernel. Its resource record reports 168
registers and 1,024 shared bytes. The retained SASS contains exactly 64
native:

```text
OMMA.SF.16864.F32.E2M1.E2M1.UE4M3.4X
```

instructions. This proves that the pinned source/toolchain combination emits
SM120 block-scaled E2M1 tensor-core instructions with UE4M3 scales. It does
not prove device execution or numerical agreement; the executable was not
run.

The preceding immutable attempt at commit
`b9ede575f952805773936931fc5f3cccc8bd723e` is retained at
`/home/derek/glmaxx/evidence/prepare-20260729T064429Z`. It stopped at compile
time because the example's `helper.h` include path was absent. No verdict file
was written and no CUDA kernel was launched. Commit `0aa490d` added only the
missing pinned CUTLASS `examples/common` include path before the fresh run.

| Successful record or artifact | SHA-256 |
|---|---|
| `source-commit.txt` | `a211777edb3a1103d4c3903438a7ace7ff7d39bcaacf70db58ef286a231ff79d` |
| `input-sha256.txt` | `9581c12bfb8c2e313273555e13756e4ea80cc6c8581c68b2b5c51dc0369e3093` |
| `cargo-test.txt` | `769a8115e956b0abfafb68b934bd7632b570a072e5b19cac3077dda591f445b5` |
| `cmake-build.txt` | `b133851dd0fc345b1bf00fbc7563df846906983c6fedd8f8d716e0f08abd0390` |
| `cutlass-dense-control-elf.txt` | `090d2e1a6a4035b2c8604b1252843fc826ca425446be335ddd734f9a50b6da5f` |
| `cutlass-dense-control-resources.txt` | `61e4fe21072e00d0f1da024ff9429a028833a417eb504f9f1682e0458ee61d2c` |
| `cutlass-dense-control-sass.txt` | `9b8c94845a48306b93dd4c29fc9f70ebe10f3c96e6aaa8f8da35c827fa2447b5` |
| `build-artifact-sha256.txt` | `021199bd6cbf4134c610f8ccadec0092e948e61b08920a26a3d7326d4bb86988` |
| `verdict.txt` | `46e37de26271cb0be15f5ace05d17611b678ec6f05e17543e60ce6f85f8686d4` |
| `libglmaxx_sm120.so` | `c84d0f38b72655d311b41fcbe33979b0fdb1663fc8cd412413f89282dd5d06b2` |
| `glmaxx_cutlass_nvfp4_dense_control` | `77f27b1dc3c2a762f28155fbfda59a29fa2ae5e1020ada0b890b71d0a32a2855` |
| release `glmaxx` | `daaf2d07f17a2a9fff41c0c418426ed01f879181f82d40cda62d72bf858aec68` |

| Failed-attempt record | SHA-256 |
|---|---|
| `source-commit.txt` | `155330fdcc509fc09cef54b102f798c824430843d6419fda4de0c62993077bf2` |
| `cmake-build.txt` | `3c7188c37d6062bc4201d5fcf3e8cf4c7acc252f2635f43f6dc022c3ffdcafe4` |

## GLMAXX-owned packed-byte control

Commit `55c5997404f6c9990606d3884df578e71cb5d369` completed the
compile-only successor to the stock example. The fresh record is:

```text
/home/derek/glmaxx/evidence/prepare-20260729T071000Z
```

The shared library now owns a void-C CUTLASS GEMM that directly accepts the
frozen GLMAXX activation-value/SFA and expert-value/SFB pointers. It writes a
BF16 1,024-column gate/up development intermediate, then applies the
per-assignment activation global scale, per-expert weight global scale, and
SwiGLU into the final 512-column BF16 output. It performs no weight repack,
scale transform, or persistent dequantization.

The no-launch gate proved all of the following:

- 117 Rust tests pass and the release Rust runner links the new native
  function;
- the shared library exports `glmaxx_nvfp4_dense_control_launch`;
- its native `sm_120f` cubin contains exactly 64
  `OMMA.SF.16864.F32.E2M1.E2M1.UE4M3.4X` instructions;
- the MMA kernel uses 168 registers and 1,024 shared bytes;
- the scale-and-SwiGLU development epilogue uses 31 registers and 1,024
  shared bytes;
- the SFB and SFA host proofs still pass 393,216 and 42,564,864 comparisons.

The M1/M256 Rust correctness runner and 20-repeat determinism gate are built
but unexecuted. This materialized gate/up path is a development control, not
the production fused kernel or a performance result.

| Record or artifact | SHA-256 |
|---|---|
| `source-commit.txt` | `7e6fc9e9ccbc3e01d31eab026aa85b423861190a8c1b308bed4e981479af9700` |
| `input-sha256.txt` | `ff97ff3b550c1ca3e15565d3de0fe36ee60bdd50ea86d2f5ebce054153f43d1d` |
| `cargo-test.txt` | `cfdb2e46642147c61bc4637093cb61559adc49fc26f1d2e27f000f7083d4c029` |
| `cmake-build.txt` | `357b967afbcdcec032c6628fe036cd37c3ae3a5d871e68ce9f6fbad3a6603cab` |
| `cuobjdump-resources.txt` | `871207f8f0823094fdf7efdcc613dc7b972f10989061aefa591177049f07e026` |
| `glmaxx-library-sass.txt` | `c93f0e02a6ed3b73b336d85b733dc0bee5dda2e8878500b32c671cd62b4540c5` |
| `glmaxx-owned-omma-count.txt` | `913f5d1da2feaf4deeccc9e55cbb350a20f12b3f507e87be85dbb77fdd3cb9bc` |
| `glmaxx-dense-control-symbol.txt` | `600d84f9d89df0c8c697d78224e917a443dea12a1b10cb649967ced302e74559` |
| `cuda-ffi-linkage.txt` | `c25a44e07c1a8738958d8f69a727f7b3fdb7b7532daa8bb3e019cfc505a884df` |
| `build-artifact-sha256.txt` | `d9497a54ca8e297c5307872b93cf9d875bcf94403f948572fcc371b0c5b0130b` |
| `verdict.txt` | `46e37de26271cb0be15f5ace05d17611b678ec6f05e17543e60ce6f85f8686d4` |
| `libglmaxx_sm120.so` | `5c9d3bf56e06b6945bc31fdec5ebf85f922c7578f65547cc4cb4bd71d8ae3c5a` |
| release `glmaxx` | `b4d936a3dbc2638a622c9a2abc4bb52dfb5a73b53ebfd69640f287349bf33f8d` |
