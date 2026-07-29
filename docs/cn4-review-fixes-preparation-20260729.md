# cn4 Fable review-fix preparation

Date: 2026-07-29

Verdict: `PREPARED_NO_DEVICE_LAUNCH`

Evidence root:

```text
/home/derek/glmaxx/evidence/prepare-c25e558-r1
```

## Provenance

| Input | Identity |
|---|---|
| source commit | `c25e55843062dd777c4778a9f5d19cd9221a3278` |
| source status | detached `HEAD`, clean before and after |
| container | `sha256:4eb9de29e5a532c672697762ca7b24e5e82316d6795dfdf616f129d35b794109` |
| Rust | `1.92.0 (ded5c06cf 2025-12-08)` |
| CUDA compiler | `13.3.33` |
| CUTLASS | `e05f953a5b3d38adc240df2ff928e0421c2abba3` |
| compilation target | `sm_120f` |

The preparation container was started without `--gpus`; `nvidia-smi` was not
available in it. The script did not create a CUDA context or launch a device
kernel.

## Results

- All 162 Rust tests passed on cn4.
- All five owned CUDA translation units compiled and linked into
  `libglmaxx_sm120.so`, including the corrected FC2 CUTLASS control and EXL3
  source projection control.
- The output contains five real `sm_120f` cubins and exactly 256 expected
  SM120 block-scaled NVFP4 OMMA instructions.
- The independent SFB layout probe passed 393,216 comparisons.
- The independent SFA layout probe passed 42,564,864 comparisons across 17
  cases.
- The shared library exports both FC1 launchers, both FC2 launchers, and all
  three EXL3 ABI/workspace/launch symbols.
- The Rust release binary linked against the newly built shared library.

This closes compileability and artifact-presence questions only. It is not
device-correctness or performance evidence.

## Built artifact hashes

| Artifact | SHA-256 |
|---|---|
| `libglmaxx_sm120.so` | `f75f533d0f5476594a5eb8671a555fc1004da00877a48d48e33261d8a6a5b40a` |
| `glmaxx_cutlass_layout_probe` | `6fb0f10b490d75d8e1e496534156a32d3d506eb08b0bc80a3a2917a1929d203e` |
| `glmaxx_cutlass_activation_layout_probe` | `10a163396701f1816b6694192fee9424a8e3447e7ce3ad2bef0dd90f5b9e86a8` |
| stock CUTLASS dense control | `0a2754b5c33a7e8c375235dd41e4059b339e32bf40437090d36aaa16a549f1dc` |
| release `glmaxx` | `21f213d6265c2749d57ee09e0a0876f8f98b9352a8ba14aff0da2e8e103ebf28` |

## Critical raw-record hashes

| Raw record | SHA-256 |
|---|---|
| `source-commit.txt` | `cf100731e05b5d4eda61c7a2e128dfb1b7cc8cfa3c4e791010aff8e1a49325d8` |
| `source-status.txt` | `ba051334ebc33f3162a4fd5be48615e03c8dac8ec7977e7c2752dbc1cf981841` |
| `input-sha256.txt` | `a672b47e22e19237c41a6bad31d59dbb606a334fec94149b6223e2564c0fb6cc` |
| `cargo-test.txt` | `2d0c52e1b64324da0d48251358a3f671f1519b1e9c27817d53c54fc02d15aca6` |
| `cmake-configure.txt` | `b8d9c79e6a0631e092d1a31fa831f8368b7a173ce0fc18e0fa95666a7e74fe63` |
| `cmake-build.txt` | `a6be178d5c0502fd3121db94b8fff8fb0da443012d9d9c66bb49b7d289a0634f` |
| `cutlass-layout-probe.txt` | `d77e30c3875b5e410b443974419c21b8e002f05ae48c088cbc61ff26c5e53485` |
| `cutlass-activation-layout-probe.txt` | `ab2d8233671a2fd5301db2c45c6edd0255d5a339c55d38aec547b1d5e080d794` |
| `cuobjdump-elf.txt` | `f0624b715970817948defda922dc3bbd9e0b882f947aa7a6674b5cd0a8a487f7` |
| `cuobjdump-resources.txt` | `a7c192b81dbda453f5ce9b607f595c09cdfc0ea700cd0be1eda606647c92692a` |
| `glmaxx-owned-omma-count.txt` | `f16c302d5d30e1d3fbe955cf4f637f58a871adeba597922e3baad0aeeb13f656` |
| `glmaxx-control-symbols.txt` | `303bbee15526368700d91090539ed8ad0be18b79e2714f469693dc2993a27b7e` |
| `glmaxx-fc2-control-symbol.txt` | `e19e5fdd6630e4101cbe630e3ed43350711695592851df7cd3225cfa9bd799d8` |
| `glmaxx-exl3-control-symbols.txt` | `1cc83bd32ed0b88841bbd70a7a260bf1f711ea66c5f8ec95d12e8b58d24a5d73` |
| `cargo-cuda-ffi-build.txt` | `5263e1e6990dcd33c169ab37fb5d22426c1798fe6926e0903f07d5819998b258` |
| `cuda-ffi-linkage.txt` | `bdb31d5222fd2b01c8578029a7bddc5797d04645799cbff27832f7a054a44cd2` |
| `build-artifact-sha256.txt` | `b812e81f977f04035721eb51933ddac402f122c7179b65d5b857afcf00780f72` |
| `source-status-after.txt` | `ba051334ebc33f3162a4fd5be48615e03c8dac8ec7977e7c2752dbc1cf981841` |
| `verdict.txt` | `fd88a10c6fe54ace0d4cfc892e53bd8cbf26e543bbe8e6690ea440d9c4d58c34` |

## Remaining boundary

Fable must re-review the exact corrected bytes. The source-projection and
manifest-ABI launch scripts remain fail-closed until their dedicated accepted
review artifacts are committed. The warp-decode v2 review opens only its CPU
proof; no v2 CUDA implementation exists yet.
