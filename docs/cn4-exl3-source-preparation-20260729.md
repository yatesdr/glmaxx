# cn4 direct EXL3 source-projection preparation

Date: 2026-07-29

Verdict: `PREPARED_NO_DEVICE_LAUNCH`

Evidence roots:

```text
/home/derek/glmaxx/evidence/prepare-e4f0290
/home/derek/glmaxx/evidence/ffi-e4f0290
```

## Provenance

| Input | Identity |
|---|---|
| source commit | `e4f029045187743deed00bba80f757377ebbba39` |
| source status | detached `HEAD`, clean |
| container | `sha256:4eb9de29e5a532c672697762ca7b24e5e82316d6795dfdf616f129d35b794109` |
| Rust | `1.92.0 (ded5c06cf 2025-12-08)` |
| CUDA compiler | `13.3.33` |
| CUTLASS | `e05f953a5b3d38adc240df2ff928e0421c2abba3` |
| compilation target | `sm_120f` |

Both runs omitted `--gpus`; the container reported that the NVIDIA driver was
unavailable. No CUDA context or device kernel was created.

## Results

- All 156 Rust tests passed on cn4.
- The direct source-order EXL3 kernel compiled into its own `sm_120f` cubin.
- The H128 input stage uses 22 registers and 1,536 bytes shared memory.
- The scalar on-demand trellis projection uses 38 registers and no shared
  memory.
- The H128 output stage uses 24 registers and 1,536 bytes shared memory.
- The shared library exports exactly the EXL3 ABI identifier, checked
  workspace formula, and projection launcher.
- Rust/native ABI validation passed for the 144-byte, 16-byte-aligned EXL3
  descriptor and the 13,312-byte M1 gate scratch allocation.
- Adding the EXL3 control did not remove the 256 expected SM120 NVFP4
  block-scaled OMMA instructions.

The kernel reconstructs weights only at the scalar accumulation point and has
zero persistent dense-weight expansion. It is a deliberately slow correctness
control, not a performance result.

## Built artifact hashes

| Artifact | SHA-256 |
|---|---|
| `libglmaxx_sm120.so` | `0ef2e59946b5491fb531961bc225f2893cd89c4f779140776818fe7c6db5522a` |
| `glmaxx_cutlass_layout_probe` | `e48da6703fca45cf8621232b5f3148664e0e5c59f1598a6801202a570917ac7e` |
| `glmaxx_cutlass_activation_layout_probe` | `e0fd5642feb5236b4b35483e466b5e8aa7bd91114dcb33b594322910de43438c` |
| stock CUTLASS dense control | `60a9519dee3e830d94bdca3eaff6717f7911bd7baf7d46e47fa9c271fd9037b5` |
| release `glmaxx` | `c155d0bcd7e7e2c99ad3069f82188b19aa72a64c4bd088256c232d3067700299` |

## Critical raw-record hashes

| Raw record | SHA-256 |
|---|---|
| `source-commit.txt` | `904fa023f77926784cd8ee903689fcc4a0c56b6e6fc991490075689be373e5c2` |
| `input-sha256.txt` | `a672b47e22e19237c41a6bad31d59dbb606a334fec94149b6223e2564c0fb6cc` |
| `cargo-test.txt` | `e42ac686fe4593e8ae6b21479d2547dca3813713f80d610bc2df9a15063f248c` |
| `cmake-build.txt` | `585063ef3d9854d257f982794029ebd0e969415a87b383bb16f948ec38dbc7d0` |
| `cuobjdump-resources.txt` | `cd72c9ece7c5edeeda9ff9c7bcbeb8574841ade5494a00d75ece79ba9edbc8c3` |
| `glmaxx-library-sass.txt` | `290330cbcb228e6c4f6f95a09a764b45ac2f488d5e3468557e2385828c398464` |
| `glmaxx-owned-omma-count.txt` | `f16c302d5d30e1d3fbe955cf4f637f58a871adeba597922e3baad0aeeb13f656` |
| `glmaxx-exl3-control-symbols.txt` | `c73b24eaf6360e52b0a90c96c1d473bb90f075bcc5d11d9974e6940f489eccc8` |
| `cargo-cuda-ffi-build.txt` | `4b1c975ee01642095d8473fabb3bb24540d668dc07a5155a61817a2b27b60066` |
| `abi-check.json` | `bdf4059bb4a558b837d0942a62f51c86d21814657f9874c95960f657bb2f0e4f` |
| `verdict.txt` | `fd88a10c6fe54ace0d4cfc892e53bd8cbf26e543bbe8e6690ea440d9c4d58c34` |

## Remaining boundary

Compile evidence is not execution evidence. The EXL3-specific independent
review must accept the inverse scatter/window arithmetic, rounding order,
descriptor, scratch, and direct-source claim before the synthetic gate/up/down
M1 smokes run. Real-payload execution additionally waits for the pinned
checkpoint download and full 92-file SHA-256 verification.
