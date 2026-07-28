# Phase A CPU preparation proof

Status: PASS for host-available checks; GPU and post-manifest review gates
remain open

Recorded UTC: 2026-07-28T22:54:35Z

Implementation commit:
`534370717d1b74c60d7407ef5c14fa5d13a97e77`

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
the CUDA-FFI host type check, release actual-shape CPU proof, manifest
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

- 33 Rust tests passed: 12 cache, 3 CUDA ABI/ownership, 10 format, 3
  format-integration, and 5 reference/manifest tests.
- Actual rank-local gate/up shape: `[1024,6144]`.
- Value bytes: 3,145,728.
- Scale bytes: 393,216.
- Codec metadata bytes: 128.
- Packed fixture SHA-256:
  `a84be06b6bf6192eb51324ee57a1b6a4c57924c78709bcbe275b9f56b547cab5`.
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
| correctness matrix | `223a5de30f0716e94767ed34bf9128639cc6785a1b1d65e5f06e4dc7fc451fb5` |
| fixture recipe | `cce63cdf83d918e49f2f3fdc1d73fa14ed30e630a871d2ea1ac54087e070bf09` |
| C ABI header | `060cc29912ce941630f74372a8c1681f3c5ba7ea5347c437807cca53907df239` |
| CUDA source | `f657b9359289cbf826258082a28feb8716f706a5a293f869b3892c727ef997ee` |
| local proof script | `9644d34b96a8dc87ec06ed0345ae3bd1d5b347cf956a4262ad6f3d36811dcee8` |
| cn4 harness | `ac79725249bb62fe5e4c25db250ef087d6bec73338700301c83794ea54eaee2e` |

## Remaining gates

An independent reviewer must accept the generated operation manifest and
v0.2.2 physical/cache ABI amendment. The operator must then separately
authorize cn4. Only an authorized SM120 compile and correctness run can
establish that the kernel is functional.
