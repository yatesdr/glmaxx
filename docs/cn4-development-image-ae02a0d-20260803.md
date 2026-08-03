# cn4 development image validation at ae02a0d

Date: 2026-08-03

Status: full host gate and `sm_120f` compilation pass; no device launch

Commit `ae02a0daea99b1b71e23fe561814acf9a8e56356` corrects the pinned
cn4 development image so it contains the tools required by
`scripts/local-checks.sh`: Rust 1.92, rustfmt 1.8, Clippy 0.1.92, Clang
18.1.3, and CUDA 13.3.

The image was built under a new tag without replacing or deleting any image:

```text
tag       glmaxx-dev:cuda13.3-rust1.92-ae02a0d
image ID  sha256:0b400cb8ba8dc58d8ae9729702260b5c3d1abaa063a8ef9e14380d72df773842
```

Inside that exact image, with `--network none`, no `--gpus`, and isolated
GLMAXX worktree/build/cache/evidence paths:

- `scripts/local-checks.sh` passed completely, including formatting, 413
  release tests, workspace Clippy with warnings denied, CUDA-FFI checks,
  deterministic proof comparison, review provenance, native C/C++ header
  parsing, and all shell syntax checks;
- a full CMake/Ninja CUDA build completed with `-arch=sm_120f`; and
- `cuobjdump` retained the five expected SM120 cubins for NVFP4 FC1, NVFP4
  FC2, EXL3 projection, and the two CUTLASS controls.

Pinned hashes:

| Artifact | SHA-256 |
|---|---|
| Dockerfile | `423f89c3ef9b931d678cbecc756b40afbb7c99d11d5bb54eda456d27b3d297b5` |
| Docker build log | `562519a94364b8e030cd2ff305f9de61a24351db4f7d258aba9f109f85577232` |
| Complete local gate log | `fb85c8b85ad69e57aaeed4b783f701755eedf70038e1cb7a84660d0475016543` |
| SM120 build log | `ced751f760e82839b8a5893cd0e7eec84b5eb003b05b586ce2f412d5f8a8a695` |
| `libglmaxx_sm120.so` | `b491e95e15103702354f5e400cb97efbc9b9b19fd52a9e7db4c42c3e63d2e465` |
| `summary.json` | `f1c520fdafdf6db7352ffc20df1cb2b5fcb0aee1004bc85aafd81839567e44f9` |
| Ordered 11-record hash manifest | `d78b5f31267829ac980fccc7d8fa841d389191b491ab26f5931ae2d2c24abc1e` |

Raw evidence is retained outside Git at:

```text
/home/derek/glmaxx/evidence/20260803T204500Z-image-ae02a0d
```

All four GPUs remained idle at 2/2/2/10 MiB and zero utilization. This
accepts only the repeatable development toolchain. It is not a pending source
or execution review, device correctness result, real-weight layer replay,
checkpoint smoke, quality result, capacity allocation, or performance claim.
