# cn4 checkpoint-ingest preparation

Date: 2026-07-29

Result: `PREPARED_NO_DEVICE_LAUNCH`

The strict safetensors ingest revision was reproduced on cn4 in
`/home/derek/glmaxx/evidence/prepare-98c2957`. The container was deliberately
started without `--gpus`; its log records that no NVIDIA driver was visible.

## Provenance

| Item | Value |
|---|---|
| source commit | `98c2957acb6f7061e050471bf2b1cc24834463ad` |
| image | `glmaxx-dev:cuda13.3-rust1.92` |
| image digest | `sha256:4eb9de29e5a532c672697762ca7b24e5e82316d6795dfdf616f129d35b794109` |
| CUDA compiler | `13.3.33` |
| Rust | `1.92.0` |
| CUTLASS | `e05f953a5b3d38adc240df2ff928e0421c2abba3` |
| native library SHA-256 | `f101f7057810fe3bf70d87858038ceef7e91df9efd9bf4e54020b67035e85469` |
| Rust runner SHA-256 | `ca3e03820ed203a2b4752790f1a645107f93f6037227b142ab9c43177f5ac2b8` |
| owned SASS SHA-256 | `3819c64ca378448fb033244ecd0db352debfeb4ea9bf0b033d5549cf7b858f4f` |

All 132 tests at that revision passed on Linux. The SFB probe compared
393,216 positions; the SFA probe compared 42,564,864 positions over 17
cases. The owned library contains exactly 128
`OMMA.SF.16864.F32.E2M1.E2M1.UE4M3.4X` instructions and exports both dense
and grouped controls.

## Evidence hashes

| File | SHA-256 |
|---|---|
| `source-commit.txt` | `d4a87782ee003e3c50d0bcd8cc9d51286cb35de5d681b1ebc7f380573d9d91f8` |
| `input-sha256.txt` | `d1903d12cd17b253d1283d672fa9c29078af9c7a4285b863458759d4bf1ce83a` |
| `cargo-test.txt` | `41009242c7a66975df9a5430beda3334fd4a2bfbb38ad80062f23c2fe5b916e8` |
| `cmake-configure.txt` | `46f24df24ee71e6d06b7726fe41be5f53ea633b1dde0eec9f31573e8ac15bb8a` |
| `cmake-build.txt` | `dc1f0c89d8575eb0780b9f27de7ab54c1d2944c41673eea168aff96b6d3d6eb6` |
| `cutlass-layout-probe.txt` | `d77e30c3875b5e410b443974419c21b8e002f05ae48c088cbc61ff26c5e53485` |
| `cutlass-activation-layout-probe.txt` | `ab2d8233671a2fd5301db2c45c6edd0255d5a339c55d38aec547b1d5e080d794` |
| `cuobjdump-resources.txt` | `cbdf0aaa841777cef4709c31a3200786345a95dd4f7b045dfda6145b63cd38e3` |
| `glmaxx-library-sass.txt` | `3819c64ca378448fb033244ecd0db352debfeb4ea9bf0b033d5549cf7b858f4f` |
| `glmaxx-owned-omma-count.txt` | `56292515f7d3a7110811eb8de26b3f75f82a0766aa5a1fd66ebcfcb84fe6d5ff` |
| `glmaxx-control-symbols.txt` | `ef5393385ce9afe66d878bc5af30ac1dfb469f0fad55b0100b63581cf51eea29` |
| `cuda-ffi-linkage.txt` | `b9cc4e142630b1eb28ce3c3995943dbdb87ec6d8355514c7eff6b1575d57bd70` |
| `build-artifact-sha256.txt` | `011a456a90214b18fee02d031097e4ef6f881e1112ee8b7e8e249ffb5c418994` |
| `verdict.txt` | `fd88a10c6fe54ace0d4cfc892e53bd8cbf26e543bbe8e6690ea440d9c4d58c34` |

This is compile and CPU evidence only. The reviewed Phase-B gate was not
satisfied, no device was exposed to the container, and no GPU kernel was
launched.
