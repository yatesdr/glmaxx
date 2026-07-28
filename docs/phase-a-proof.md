# Phase A CPU preparation proof

Status: PASS for host-available checks; GPU and post-manifest review gates
remain open

Recorded UTC: 2026-07-28T23:18:29Z

Implementation commit:
`7fdd600eb55bd8c66c1474e79192baf9549e2da8`

Model: `zai-org/GLM-5.2`

Model revision: `b4734de4facf877f85769a911abafc5283eab3d9`

Read-only evidence repository revision:
`d213925ee6701072f117aec59ca94f1bf00d5e7f`

CUTLASS revision:
`e05f953a5b3d38adc240df2ff928e0421c2abba3`

## Command

From a clean checkout of the implementation commit:

```bash
./scripts/local-checks.sh
```

The script ran formatting, all workspace tests, warnings-as-errors Clippy,
the CUDA-FFI Rust CLI/driver type check and Clippy pass, release actual-shape
CPU proof, deterministic correctness-matrix regeneration comparison, manifest
regeneration comparison, deterministic rank-file pack/read, budget report,
Rust/C header ABI syntax check, and CUDA-toolchain detection.

## Host toolchain

- host kernel: Darwin 25.5.0 arm64;
- Rust host triple: `x86_64-apple-darwin`;
- `rustc 1.92.0 (ded5c06cf 2025-12-08)`, LLVM 21.1.3;
- `cargo 1.92.0 (344c4567c 2025-10-21)`;
- CMake 3.31.5;
- Apple clang 21.0.0;
- `nvcc`: unavailable.

No cn4 access, CUDA compilation, GPU launch, or GPU measurement occurred.

## Results

- 41 Rust tests passed: 15 cache/attention, 3 CUDA ABI/ownership, 11 format,
  3 format-integration, and 9 reference/manifest/matrix tests.
- Actual rank-local gate/up shape: `[1024,6144]`.
- Value bytes: 3,145,728.
- Scale bytes: 393,216.
- Codec metadata bytes: 128.
- Packed fixture SHA-256:
  `a84be06b6bf6192eb51324ee57a1b6a4c57924c78709bcbe275b9f56b547cab5`.
- The generated qualification matrix expands all nine row buckets into 135
  isolated positive GPU cases plus nine fail-closed duplicate-route cases.
- Matrix proof SHA-256:
  `5ebf329ee29e4cd95e2c92a41a99625808dcf4212f996c874d651d637cdb6eef`.
- Packed indexer selection and packed-KV attention state merge match a direct
  CPU control across DCP4 owner subsets.
- Rank-0 laboratory file SHA-256:
  `ea706d83c4aa89fda26f977f03e7fa72862b71cf36c2c77cead70d68bc7b3093`.
- File UUID: `f411c5cf2e7ef452dcab5c8715f5116d`.
- Conversion UUID: `a1e0b6dcc961821ec8900eb2d3d4bd54`.
- Rust/C descriptor: 224 bytes, alignment 16.
- M128 reserved workspace: 20,321,284 bytes.
- Full 1M MTP cache: 33,529,266,176 bytes aggregate and 8,382,316,544
  bytes per DCP4 rank before slack.

## Input hashes

| Input | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `fable-adversarial-v2.md` | `f0019b96d5b35bdca6d026691629b56fbeb0c3c4528e1ae4ff9c1aa06817953e` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `spec/format-v0.md` | `fbe147ebe5c8a88b2fe81dfdb871bd32d43395f1b5ccf5c7162779a3f8cf7b77` |
| operation manifest | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| physical ABI note | `8936c8a60a1d6a7a2038fcd7f3f4a352b80477c359a6f3f2f89ea3903d2a9e99` |
| correctness matrix | `d5a0286d0c9d06ce1036085d4f1712d929273fb533601806b26b9f774c360e74` |
| generated matrix proof | `5ebf329ee29e4cd95e2c92a41a99625808dcf4212f996c874d651d637cdb6eef` |
| fixture recipe | `cce63cdf83d918e49f2f3fdc1d73fa14ed30e630a871d2ea1ac54087e070bf09` |
| C ABI header | `e24424dd4040d5ed63ce925b917c4d271e2cc82b85e7b5512fee0ac8da056af6` |
| CUDA source | `3b90e48baaeedd3b9dffc3a010a60da646d5e3ed83abcadaf43e96a8b4f9bbe8` |
| local proof script | `3d48d6a16fec3cfa0dc96ecd7da99f0580060b2a8e2ae660b3c33513b4e24517` |
| cn4 harness | `86fc49a99eac1fe9724b11c3a777e46eb5038fcb4eb2b291928113208f9a9aa7` |

## Remaining gates

An independent reviewer must accept the generated operation manifest and
v0.2.2 physical/cache ABI amendment. The operator must then separately
authorize cn4. Only an authorized SM120 compile and correctness run can
establish that the kernel is functional.
