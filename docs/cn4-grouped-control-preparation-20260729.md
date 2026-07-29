# cn4 grouped NVFP4 control preparation

Date: 2026-07-29

Verdict: `PREPARED_NO_DEVICE_LAUNCH`

The fresh immutable evidence directory is:

```text
/home/derek/glmaxx/evidence/prepare-20260729T073500Z
```

## Provenance

| Input | Identity |
|---|---|
| source commit | `3979aaaf5c8429f567dff40dae775dfb3c9b3120` |
| source status | `main...origin/main`, clean |
| container | `sha256:4eb9de29e5a532c672697762ca7b24e5e82316d6795dfdf616f129d35b794109` |
| Rust | `1.92.0 (ded5c06cf 2025-12-08)` |
| CUDA compiler | `13.3.33` |
| CUTLASS | `e05f953a5b3d38adc240df2ff928e0421c2abba3` |
| compilation target | `sm_120f` |

The source and CUTLASS mounts were read-only. The container was deliberately
started without `--gpus`; its log confirms that the NVIDIA driver was not
detected. No CUDA context or device kernel could be created by this run.

## Results

- All 119 Rust tests passed.
- The SFB proof passed 393,216 comparisons.
- The SFA proof passed 42,564,864 comparisons.
- Rust tests cover exact expert-major grouping, expert-local padded SFA
  slabs, worst-case capacity, monotonicity, and overflow rejection.
- The shared library exports both
  `glmaxx_nvfp4_dense_control_launch` and
  `glmaxx_nvfp4_grouped_control_launch`.
- The shared-library cubin retains exactly 128
  `OMMA.SF.16864.F32.E2M1.E2M1.UE4M3.4X` instructions: 64 in the dense
  control and 64 in the pointer-array grouped control.
- The grouped resource record contains the device-side metadata initializer,
  grouped CUTLASS kernel, and grouped scale/SwiGLU control.
- The release Rust binary with `cuda-ffi` links to the exact newly built
  native library.
- The runtime runner is prepared for 14 positive cases across M1/M256 and all
  non-rejected frozen routing postures, two duplicate-route rejections, and
  two 20-repeat all-expert determinism cases.

Both CUTLASS controls still write the 1,024-column BF16 gate/up development
intermediate. They prove the direct packed-byte boundary only; they are not
the fused production operator or performance evidence.

## Built artifact hashes

| Artifact | SHA-256 |
|---|---|
| `libglmaxx_sm120.so` | `6c65a11984f893dc5ed7d14b4a8aed665fc07c621e7883abd169d3c2fa83365b` |
| `glmaxx_cutlass_layout_probe` | `f8932daa7370e2be03ec66a6c58feec7a4f36107b8f560fe8105c65b84ff950f` |
| `glmaxx_cutlass_activation_layout_probe` | `2593fcd93980d7b2dcfa7adf7468fb5bb28fb918799b5d6b244f8d268e649134` |
| stock CUTLASS dense control | `bf33dbe87c841538aeba493f6087c3b3873efd99c93404efb1af1c56162946e9` |
| release `glmaxx` | `459f4024082f0ff95d0eaabd3517dca1bd002d7e02c1bb8b14d0c54b9e799110` |

## Critical raw-record hashes

| Raw record | SHA-256 |
|---|---|
| `source-commit.txt` | `fb11de6706c6b0fde81556b42b8ccfea80ed173e46681fef281c0ac1dc0c72e1` |
| `input-sha256.txt` | `14cf778c712df6e61b5f6f5b04211051fbcdb21748ccff41f3e5ad6a259b9e8c` |
| `cargo-test.txt` | `519d916de719cdbb5a05f6897bf16f1a8fba4a0f65f4c810ab8931a5e2e2882e` |
| `cmake-build.txt` | `59503f5652ea934820626b90a6e6b27644afb1e138f03986e20ed4a7baf4212e` |
| `cutlass-layout-probe.txt` | `d77e30c3875b5e410b443974419c21b8e002f05ae48c088cbc61ff26c5e53485` |
| `cutlass-activation-layout-probe.txt` | `ab2d8233671a2fd5301db2c45c6edd0255d5a339c55d38aec547b1d5e080d794` |
| `cuobjdump-resources.txt` | `cbdf0aaa841777cef4709c31a3200786345a95dd4f7b045dfda6145b63cd38e3` |
| `glmaxx-library-sass.txt` | `3819c64ca378448fb033244ecd0db352debfeb4ea9bf0b033d5549cf7b858f4f` |
| `glmaxx-owned-omma-count.txt` | `56292515f7d3a7110811eb8de26b3f75f82a0766aa5a1fd66ebcfcb84fe6d5ff` |
| `glmaxx-control-symbols.txt` | `aae3203293dfb428a9671542257baa23b5981b06533763466f9a3bf8aa1d13f4` |
| `cuda-ffi-linkage.txt` | `556c8e91eda9e88f6cff05b96c6f9a60948da1f81a52413246c988ed6855a827` |
| `build-artifact-sha256.txt` | `ee44ee7b248abae114921d7277ea56c3631752783c4305af3807e4613ac12d90` |
| `verdict.txt` | `fd88a10c6fe54ace0d4cfc892e53bd8cbf26e543bbe8e6690ea440d9c4d58c34` |

## Remaining boundary

Compilation is not device correctness. Phase B still requires the committed
independent result `fable-manifest-abi-v022.md` with an unqualified manifest
and cache-ABI acceptance plus the exact gate token. The instructional handoff
is explicitly rejected as a review artifact. Once that file exists, the
authorized Phase B sequence runs the direct eager matrix, graph replay, dense
CUTLASS control, and grouped CUTLASS control before any timing.
