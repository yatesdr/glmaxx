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
