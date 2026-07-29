# cn0 SM86 non-acceptance bring-up

Date: 2026-07-29

Status: PASS for CUDA compilation, CUTLASS layout, Linux Rust tests, native
linking, and no-device failure propagation; no GPU launch was attempted

This record is deliberately not M2 or SM120 acceptance evidence. The operator
authorized GPU0 on cn0, but read-only inventory found it occupied. The
repository safety contract therefore prohibited overlap.

## Provenance

- host: `cn0`, Ubuntu 24.04.4 LTS, Linux 6.8.0-136-generic x86_64;
- authorized device: GPU0, NVIDIA RTX A6000, SM86;
- GPU UUID: `GPU-a03fe082-9021-4938-17a2-e1e098bd19c9`;
- driver: 590.48.01;
- inventory at start and finish: 30,755 MiB allocated on GPU0 by one Python
  process and eight Blender processes;
- final source revision:
  `e76e726f553b4d7b43d768f2825aebf4821e104a`;
- CUTLASS revision:
  `e05f953a5b3d38adc240df2ff928e0421c2abba3`;
- CUDA compiler: 12.9.86;
- Rust/Cargo: 1.92.0, installed under the isolated build directory;
- source checkout: `/home/derek/glmaxx-cn0-b82c5ee`;
- CUTLASS checkout: `/home/derek/cutlass-e05f953`;
- no model weights, datasets, checkpoints, caches, or raw benchmark output
  were copied.

The CUDA library was compiled immediately before the Rust-only ABI-reporting
fix in `e76e726`. Its kernel and C-header inputs are byte-identical at the
final revision:

| Input | SHA-256 |
|---|---|
| `kernels/sm120/nvfp4_routed_fc1.cu` | `3b90e48baaeedd3b9dffc3a010a60da646d5e3ed83abcadaf43e96a8b4f9bbe8` |
| `kernels/include/glmaxx_kernel.h` | `e24424dd4040d5ed63ce925b917c4d271e2cc82b85e7b5512fee0ac8da056af6` |

## Build

The host had CUDA 12.9 under `/usr/local/cuda`, but no CMake or Ninja.
Therefore this non-acceptance pass used the equivalent direct compiler
command rather than weakening the pinned cn4 harness:

```bash
/usr/local/cuda/bin/nvcc \
  -std=c++17 -shared -Xcompiler=-fPIC -lineinfo \
  -arch=sm_120f --threads=0 \
  -I kernels/include \
  -I /home/derek/cutlass-e05f953/include \
  -I /home/derek/cutlass-e05f953/tools/util/include \
  kernels/sm120/nvfp4_routed_fc1.cu \
  -o build-cn0/libglmaxx_sm120.so
```

`cuobjdump -lelf` reports two `nvfp4_routed_fc1.sm_120.cubin` entries. The
library exports every required `glmaxx_*` C symbol.

The host-only CUTLASS probe was compiled for `sm_120f` and run with no visible
CUDA devices:

```text
CUTLASS_SFB_LAYOUT_PASS comparisons=393216
```

Artifact hashes:

| Artifact | SHA-256 |
|---|---|
| `libglmaxx_sm120.so` | `f8a79d2f4653e2bd1e7771dad1e8088a411b65a3e575c064e81a545b6e1ddf0d` |
| `glmaxx_cutlass_layout_probe` | `907498a68c599f6ba7b5ffda4ead09a4e4319ab31e0c8a79fe882d634469dafb` |
| Rust `glmaxx` binary at `e76e726` | `8151116e3fbd5116ebedd81cb30ff9c55aea8101cd343e2878d30a14a680a9c3` |

## Rust and native boundary

All 41 workspace tests passed on x86_64 Linux. The release Rust CLI linked
against the compiled CUDA library. With `CUDA_VISIBLE_DEVICES` empty,
`abi-check` reported:

```json
{
  "cuda_ffi_feature": true,
  "descriptor_alignment": 16,
  "descriptor_bytes": 224,
  "gpu_launched": false,
  "kernel_abi": "glmaxx.sm120.nvfp4.routed_fc1.v1",
  "m128_workspace_bytes": 20321284,
  "native_abi_verified": true
}
```

The no-visible-device smoke failed closed as `Driver(100)` with exit status
1. This validates error propagation without creating a context or touching
an occupied GPU.

The bring-up exposed and fixed one evidence bug: `abi-check` had hardcoded
`cuda_compiled: false` even in a CUDA-feature build. Revision `e76e726`
replaced that field with independently meaningful feature, native-ABI, and
GPU-launch booleans.

## What remains unproven

- CUDA 13.3 and the CMake/Ninja cn4 build path;
- loading or executing either cubin on SM120;
- activation quantization or FC1 numerical correctness on a GPU;
- decode or prefill functionality;
- asynchronous device-error behavior after an actual launch;
- eager/graph determinism, leaks, hidden repacking, counters, or timing;
- any performance claim.

An A6000 cannot close those gates. The next useful GPU action remains the
authorized, idle SM120 session on cn4.
