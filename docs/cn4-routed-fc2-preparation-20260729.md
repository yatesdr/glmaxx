# cn4 routed-MoE v2 preparation

Date: 2026-07-29

Verdict: `PREPARED_NO_DEVICE_LAUNCH`

The fresh immutable evidence directory is:

```text
/home/derek/glmaxx/evidence/prepare-22d03fc-r3
```

Two earlier directories with the same prefix are incomplete setup attempts:
one lacked the container safe-directory setting and one lacked the pinned
Cargo cache. Neither launched a device kernel and neither is evidence.

## Provenance

| Input | Identity |
|---|---|
| source commit | `22d03fcce921483bbf71da5a51e80131326217b7` |
| source status | detached `HEAD`, clean |
| container | `sha256:4eb9de29e5a532c672697762ca7b24e5e82316d6795dfdf616f129d35b794109` |
| Rust | `1.92.0 (ded5c06cf 2025-12-08)` |
| CUDA compiler | `13.3.33` |
| CUTLASS | `e05f953a5b3d38adc240df2ff928e0421c2abba3` |
| compilation target | `sm_120f` |

The source and CUTLASS mounts were read-only. The container was deliberately
started without `--gpus`; its log confirms that the NVIDIA driver was not
detected. No CUDA context or device kernel could be created by this run.

## Results

- All 153 Rust tests passed on cn4, including the same-size in-place shard
  mutation test on cn4's filesystem.
- The SFB proof passed 393,216 comparisons.
- The SFA proof passed 42,564,864 comparisons.
- Rust and C agree on the 224-byte FC1 and FC2 descriptors.
- The release Rust binary with `cuda-ffi` links to the exact newly built
  native library.
- The shared library exports the FC1 dense/grouped and FC2 dense/grouped
  control launchers.
- The cubins retain exactly 256
  `OMMA.SF.16864.F32.E2M1.E2M1.UE4M3.4X` instructions across those four
  SM120 controls.
- FC2 direct and grouped M1 correctness runners are compiled into the Rust
  binary but were not invoked.

The CUDA-core and CUTLASS implementations remain correctness controls. The
CUTLASS paths still have named BF16 development boundaries and are not fused
production operators or performance evidence.

## Built artifact hashes

| Artifact | SHA-256 |
|---|---|
| `libglmaxx_sm120.so` | `c754fce262030530bb2364148ec5bd49aa31a3ab5e3d0973ea92cc923cd3e8f4` |
| `glmaxx_cutlass_layout_probe` | `d58c0db3fc389d3ef7b6dc9a3f73560fd951877211b4eff09935958b4cfb64b7` |
| `glmaxx_cutlass_activation_layout_probe` | `1c7deb17647df41132bd15578d4f9f9d54f4e282f616b3dcfb1af9e4d3805be5` |
| stock CUTLASS dense control | `02d18c49adc5bd318867303a6f512d097343495db59e7e1d657f6d4942b08886` |
| release `glmaxx` | `97d6d141a922aee6aa6589642119df8d6d7acf8ad2fa20aca50df40416ddda56` |

## Critical raw-record hashes

| Raw record | SHA-256 |
|---|---|
| `source-commit.txt` | `19cfc7ab23481a982289b78f2f914f2e7adf198c138e99638cff17904727757b` |
| `input-sha256.txt` | `29e323ae70c4063d194ffa8514964ebf3414968d4ae76767943f93c8dc2dff0d` |
| `cargo-test.txt` | `0c86e10921c0f29a45b3840ea1e63b163466b9499bb3ce5de07ade6175f5ae3d` |
| `cmake-configure.txt` | `1f355fff4869187061ca38d277566c1626058ac5cd1ea2ea79270642d6339649` |
| `cmake-build.txt` | `955d5d88929914074ddb2040b860ccdf5216be971442001864cb6bd1dabf8bd5` |
| `cutlass-layout-probe.txt` | `d77e30c3875b5e410b443974419c21b8e002f05ae48c088cbc61ff26c5e53485` |
| `cutlass-activation-layout-probe.txt` | `ab2d8233671a2fd5301db2c45c6edd0255d5a339c55d38aec547b1d5e080d794` |
| `cuobjdump-resources.txt` | `13cf81b5628acc5ae287128630a6bca3eb0f288da1a5ac2f39152d8cd9eb763a` |
| `glmaxx-library-sass.txt` | `2e98fe357d822be989e04afb6120489b769987163008d6ea8d806c507d384cbe` |
| `glmaxx-owned-omma-count.txt` | `f16c302d5d30e1d3fbe955cf4f637f58a871adeba597922e3baad0aeeb13f656` |
| FC1 control symbols | `47e47d3c4e0ce89ec7df6e78629cf71cbfaec13fa530ff023793e55ce6d2889e` |
| FC2 control symbols | `46f48855e1de984a0d99e7244b811492376572fe4908fa8ccf1633e3cde70eb7` |
| `cargo-cuda-ffi-build.txt` | `993d314e4570c5b70c43fe30ce066631e3ddd52d24179a1dd7af63468787398e` |
| `cuda-ffi-linkage.txt` | `e38176acd8e3a37b71f4b3eaa253bd984339a09e0bcabb97adb03e80cb43e391` |
| `build-artifact-sha256.txt` | `90386be1a9b942d8a8f473ffd8bbab8e8776edf8381625d836813f57b6458933` |
| `verdict.txt` | `fd88a10c6fe54ace0d4cfc892e53bd8cbf26e543bbe8e6690ea440d9c4d58c34` |

## Remaining boundary

The root review result `fable-manifest-abi-v022.md` is still absent. Device
correctness therefore remains blocked by the independent-review half of the
M2 gate even though operator authorization is present. Once the exact reviewed
artifact is committed, Phase B starts with the direct FC1 and FC2 M1 smokes
before any matrix, graph, or timing work.
