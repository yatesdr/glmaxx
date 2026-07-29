# cn4 persistent TP4 rank-bind result

Date: 2026-07-29

Verdict: `SM120_TP4_RANK_BIND_PASS`

The first native Rust-to-CUDA startup proof passed on all four target GPUs.
Raw evidence remains outside Git at:

```text
/home/derek/glmaxx/evidence/rank-bind-8114b11-r5
```

## Provenance

| Input | Identity |
|---|---|
| source commit | `8114b115018d6ef539a2797eafebb3e346482b55` |
| source status | detached clean checkout |
| container | `sha256:4eb9de29e5a532c672697762ca7b24e5e82316d6795dfdf616f129d35b794109` |
| Rust | `1.92.0 (ded5c06cf 2025-12-08)` |
| Cargo | `1.92.0 (344c4567c 2025-10-21)` |
| CUDA compiler | `13.3.33` |
| CUTLASS | `e05f953a5b3d38adc240df2ff928e0421c2abba3` |
| driver | `595.71.05` |
| target | four RTX PRO 6000 Blackwell Workstation Edition GPUs, compute capability 12.0 |

The four devices reported 188 SMs each. Devices 0–2 reported
101,973,491,712 bytes and device 3 reported 101,964,644,352 bytes through the
CUDA runtime. The inventory independently found exactly four visible devices
and exactly four compute-capability 12.0 devices.

## Result

- All 158 workspace tests passed in the pinned Linux container.
- CUDA 13.3 compiled the native library for `sm_120f`.
- The library exported exactly `glmaxx_device_count` and
  `glmaxx_device_bind` for the new device-discovery boundary.
- The release Rust binary linked the native library with the `cuda-ffi`
  feature.
- Four persistent host threads bound ranks 0–3 to visible devices 0–3.
- Every rank created, synchronized, and destroyed one nonblocking CUDA stream
  on its owner thread.
- The proof emitted four distinct rank/device identities and the required
  `SM120_TP4_RANK_BIND_PASS` verdict.
- No CUDA device kernel was launched, and the post-run idle check found no
  retained compute process.

## Built artifact hashes

| Artifact | SHA-256 |
|---|---|
| `libglmaxx_sm120.so` | `15ad4720f7ee1a38ef696fdc68d9552f044f0b91e928aa1d9ec2976fd671337e` |
| release `glmaxx` | `c97e80194f2c29199a03aa1d4ecaba122728730671d98ec3ae4b53030168b15f` |
| `rank-bind.json` | `e295575d39cdb7aa81cfb204e1481fc8e9bca03aa53400c5e064163e9cd942a0` |

## Critical raw-record hashes

| Raw record | SHA-256 |
|---|---|
| `source-commit.txt` | `dfd1b7d7620424320500a9086f8ea6df9276adbf40937505e442072de1a00ab2` |
| `source-status.txt` | `ba051334ebc33f3162a4fd5be48615e03c8dac8ec7977e7c2752dbc1cf981841` |
| `gpu-inventory.csv` | `3da4688bbed0a1ef888409d8c943005ef486f449700aa144153c745be1d661d3` |
| `gpu-counts.txt` | `cc934056263d7c42f88f96764c172426a7c42844ff314a253d1f30720710739b` |
| `topology.txt` | `0fbe47144ace86e0f32a0df4d1f785f2374685a562606313d5355c530b57e85d` |
| `input-sha256.txt` | `6ff08ef17a279821bdd7389da2784bcb4aad78bdb1250320f3b69cd14f92391c` |
| `cargo-test.txt` | `eac35afd799e9e1b8d58a06fbcf1b7cfac3902e93764aae0bc9d7a941fb30e25` |
| `cmake-configure.txt` | `abbfbe5b33dc137f3361527482a7106699ad1a52e2195cd9811a83b346e87f5a` |
| `cmake-build.txt` | `730f4a5b05489e46ca80c6a2e32a348c5d15facd101c3512609c20af1f9a07a3` |
| `rank-bind-symbols.txt` | `ea79f0eea324091ef0468908de9c870687db92ee5fa61586a9d9de6d470e5b34` |
| `cargo-cuda-ffi-build.txt` | `598f4221fe2054d0c0b0ebddd44488674be7c7495ba7c9d45526c8e7c0415fb2` |
| `artifact-sha256.txt` | `c10c6b7d9757dcb132f8bf55df4280b415fd18712370b94defcb7d73b50d228f` |
| `verdict.txt` | `9778f4f0070abc34c8d9c62dbd56ac5b1df378975af455e411817c2d0df45491` |

## Gate boundary

This qualifies only deterministic rank-to-device binding and per-rank stream
ownership. It does not qualify weight loading, GPU kernels, NCCL/collective
execution, graph capture, checkpoint execution, correctness, or performance.
