# cn4 EXL3 source-container preparation

Date: 2026-07-29

Status: CPU and `sm_120f` compile proof passed; no CUDA device was exposed or
launched.

## Provenance

| Item | Identity |
|---|---|
| source commit | `0544e29b294883d7adb167146ab24d3cfd3a3839` |
| evidence root | `/home/derek/glmaxx/evidence/prepare-0544e29-r3` |
| container | `sha256:4eb9de29e5a532c672697762ca7b24e5e82316d6795dfdf616f129d35b794109` |
| CUDA compiler | `13.3.33` |
| Rust | `1.92.0` |
| CUTLASS | `e05f953a5b3d38adc240df2ff928e0421c2abba3` |

The source and CUTLASS mounts were read-only. The container was deliberately
started without `--gpus`; its log records that the NVIDIA driver was not
available. The two earlier fresh evidence paths stopped before compilation:
the first on container-local Git trust and the second on a missing Cargo cache
mount. Both incomplete directories were preserved. The `r3` path is the
successful immutable result.

## Result

- all 125 Rust tests passed in the pinned Linux container;
- the deterministic NVFP4 rank golden remained unchanged;
- EXL3 source planes round-tripped through the rank container and CPU oracle;
- SFB layout proof passed 393,216 comparisons;
- SFA layout proof passed 42,564,864 comparisons over 17 cases;
- the linked library retained exactly 128 owned
  `OMMA.SF.16864.F32.E2M1.E2M1.UE4M3.4X` instructions;
- the dense and grouped control launch symbols were both exported;
- the release Rust runner linked to the same freshly built native library.

Artifact identities:

| Artifact | SHA-256 |
|---|---|
| input hash record | `d1903d12cd17b253d1283d672fa9c29078af9c7a4285b863458759d4bf1ce83a` |
| Cargo test log | `5c38ac90e238a489b72fef647a7cd63c62f6064dd9ccab73d1c9fdf64cb20965` |
| CMake build log | `0b2a90d11ef21cbf2efcb24cc68d0498cc0124d7da3b0fdb4efcd9f8d98ec83b` |
| SFB proof | `d77e30c3875b5e410b443974419c21b8e002f05ae48c088cbc61ff26c5e53485` |
| SFA proof | `ab2d8233671a2fd5301db2c45c6edd0255d5a339c55d38aec547b1d5e080d794` |
| library SASS | `3819c64ca378448fb033244ecd0db352debfeb4ea9bf0b033d5549cf7b858f4f` |
| native library | `61fe1b1557136a8a7620d4fbb046fc1fd9c72f8afecb3d53527f87e3ad667bd3` |
| release runner | `1caf192cb9ba37aacd5efdd2eee7ba92186948608341269c022c7759f96cc3b0` |
| final verdict | `fd88a10c6fe54ace0d4cfc892e53bd8cbf26e543bbe8e6690ea440d9c4d58c34` |

The verdict is `PREPARED_NO_DEVICE_LAUNCH`. EXL3 remains an inspection and
CPU-proof codec only; this result neither closes its independent review nor
qualifies a GPU consumer. Reviewed Phase B remains separately gated.
