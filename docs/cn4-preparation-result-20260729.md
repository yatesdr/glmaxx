# cn4 SM120 qualification preparation

Date: 2026-07-29

Verdict: `PREPARED_NO_DEVICE_LAUNCH`

The pinned CUDA/Rust environment compiled the native library for `sm_120f`,
proved the CUTLASS scale-factor layout, linked the Rust CUDA-FFI executable,
and passed the complete CPU workspace suite. This record is preparation
evidence only. No CUDA device kernel was launched.

## Provenance

- Source commit:
  `7cb680525d158255bfc92a05620219739f71be0f`
- Container image:
  `sha256:4eb9de29e5a532c672697762ca7b24e5e82316d6795dfdf616f129d35b794109`
- CUDA base:
  `nvidia/cuda@sha256:ef2203909e80b8b976cfc672f7e2ae2b00bc0e25c404ee86d89e10a3802f1c52`
- Rust base:
  `rust@sha256:e90e846de4124376164ddfbaab4b0774c7bdeef5e738866295e5a90a34a307a2`
- CUTLASS:
  `e05f953a5b3d38adc240df2ff928e0421c2abba3`
- Rust/Cargo: 1.92.0
- CUDA compiler: 13.3.33
- CMake: 3.28.3
- Ninja: 1.11.1
- External raw evidence:
  `/home/derek/glmaxx/evidence/prepare-20260729T054203Z` on cn4
  (482 MiB; not stored in Git)

The visible target was four NVIDIA RTX PRO 6000 Blackwell Workstation Edition
GPUs, each reporting 97,887 MiB. GPU0/GPU1 and GPU2/GPU3 are `PIX` pairs;
traffic between the pairs is `NODE`. No NVLink is present.

## Results

- All 115 Rust tests passed under the pinned Linux toolchain.
- `nvcc -O3 -lineinfo -arch=sm_120f` built `libglmaxx_sm120.so`.
- The library contains `nvfp4_routed_fc1.sm_120.cubin`.
- The CUTLASS SFB probe passed all 393,216 comparisons.
- The direct FC1 baseline reports 34 registers and 3,072 bytes shared memory.
- Activation-row quantization reports 30 registers and 2,048 bytes shared
  memory.
- The Rust release executable resolves the freshly built
  `libglmaxx_sm120.so`.

Build artifact SHA-256 values:

| Artifact | SHA-256 |
|---|---|
| `libglmaxx_sm120.so` | `e0ea084c75ff8ec8c952bc35ff90327ac86c45b1726b142df21539435c1f8046` |
| `glmaxx_cutlass_layout_probe` | `c465bebe02db173dd5fc81e0eabe0b332bba7d04b4f15948f5b69105f4e1c655` |
| Rust `glmaxx` release binary | `15735a56ca17e403297fd813701c0e8eb5423189611ef42d9e9a3fff6392580a` |

Critical raw-record SHA-256 values:

| Record | SHA-256 |
|---|---|
| `verdict.txt` | `cee765aa6cc4fe7afe5057ca22d1f67b1e23973b5a793aec3ae2521b75f65f62` |
| `input-sha256.txt` | `4b3fafa0c64e8a117c4148791024857488086f26109d218302b0e492d49fb13f` |
| `cargo-test.txt` | `b989afe148768b70586c531abedf27af3257af5cbd13f786b88bafd3302f38bd` |
| `cmake-configure.txt` | `c5ed9ea7f5c85e1dc5d3f370ac071fc36539f5c1262d82056fee54de78994863` |
| `cmake-build.txt` | `0175fd5af811271d1ccea8b039fc10d186270d37da27cc450b8824e4a2cfaec1` |
| `cutlass-layout-probe.txt` | `d77e30c3875b5e410b443974419c21b8e002f05ae48c088cbc61ff26c5e53485` |
| `cuobjdump-elf.txt` | `c5ddba11b8cfb75d262fa3ec759465d7d9d35ddbaf01618095428f9a6cadf612` |
| `cuobjdump-resources.txt` | `d0e9fc7893bf317e84df4330ccf4d60f6a051e01c2ccf41127484e89cccdfff1` |
| `cuda-ffi-linkage.txt` | `b7ecab0eb8e70a8ff5b849c4fc489172e126069cc70d225a27cda108f4d50629` |
| `build-artifact-sha256.txt` | `37c6d34533d2dae916f428a7aab6eacb04822cdc105390b9690aec76b3821fcd` |
| `gpu-inventory.csv` | `aaa42977cc320a6546263dfb871f8c825a173a92ac02b319454cecc69fe95fa8` |
| `topology.txt` | `0fbe47144ace86e0f32a0df4d1f785f2374685a562606313d5355c530b57e85d` |

## Remaining gate

Operator authorization is present. Independent acceptance of the generated
operation manifest and the v0.2.2 combined draft-KV/draft-indexer cache ABI is
not present in the repository. Fable's checked-in v2 review explicitly
authorizes M1 and M2 preparation but explicitly does not authorize GPU work.
`scripts/cn4-phase-b.sh` must remain closed until that review artifact exists.
