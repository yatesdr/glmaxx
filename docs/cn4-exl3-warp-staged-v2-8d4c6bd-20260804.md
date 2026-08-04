# cn4 EXL3 warp-staged v2 qualification

Date: 2026-08-04

Status: informational SM120 microbenchmark passed; route promotion withheld
pending the separately requested CPU-proof and CUDA implementation reviews

Source commit: `8d4c6bddc661d57643c781422b437e3ccffbc24d`

Branch: `perf/exl3-warp-staged-v2`

## Result

The dedicated Rust harness compared the retained scalar-v1 launcher with the
new warp-staged-v2 launcher in one native library. All twelve gate/up/down
M1/M2/M4/M8 cases produced bitwise-identical FP16 output and zero device
validation bits. Each route retained 1,000 interleaved CUDA-event samples
after 50 warmups.

| Projection | Rows | Scalar p50 (us) | Staged p50 (us) | Speedup |
|---|---:|---:|---:|---:|
| gate | 1 | 461.408 | 201.344 | 2.292x |
| gate | 2 | 461.728 | 203.072 | 2.274x |
| gate | 4 | 465.536 | 203.392 | 2.289x |
| gate | 8 | 467.552 | 206.400 | 2.265x |
| up | 1 | 461.344 | 201.312 | 2.292x |
| up | 2 | 463.680 | 203.616 | 2.277x |
| up | 4 | 465.856 | 204.384 | 2.279x |
| up | 8 | 467.680 | 207.680 | 2.252x |
| down | 1 | 47.712 | 25.184 | 1.895x |
| down | 2 | 47.744 | 25.216 | 1.893x |
| down | 4 | 47.744 | 25.184 | 1.896x |
| down | 8 | 68.224 | 29.088 | 2.345x |

The staged kernel compiled to a real SM120 cubin with 64 registers, zero
local/stack bytes, and 1,792 reported shared bytes. The run preserved full
SASS, resource usage, exported symbols, the exact binary hashes, all raw
samples, GPU state before and after, and a self-excluding artifact manifest.
GPU0 returned to 2 MiB used and no compute process after the run.

## Provenance

- Host: cn4, four RTX PRO 6000 Blackwell Workstation Edition GPUs, compute
  capability 12.0, driver 595.71.05.
- Container:
  `sha256:0b400cb8ba8dc58d8ae9729702260b5c3d1abaa063a8ef9e14380d72df773842`.
- Toolchain: CUDA 13.3.33, Rust 1.92.0, CMake 3.28.3, pinned CUTLASS
  `e05f953a5b3d38adc240df2ff928e0421c2abba3`.
- Evidence:
  `/home/derek/glmaxx/evidence/20260804T043059Z-exl3-staged-v2-8d4c6bd-r1`.
- Evidence-manifest SHA-256:
  `927e51a6c2e2003035a613de1c8590ec2083fffd83cfb3dbe5cc57c66e40868d`.
- Suite summary SHA-256:
  `6a704799d8430a0d3c982929984c248ee0faf7b8cc819437daf52155de2a4e6b`.
- Shared library SHA-256:
  `46be15d5a53104a979d148f136f24d31653f31fd024b0c6c05a1b092b66eb401`.
- Rust runner SHA-256:
  `85166a398e71868f318367fd8c29a463becc450f68cdf8c15f0e85e7bcd7eebd`.
- Full SASS SHA-256:
  `59c3b481df7d678a1c2326b0a7ed07f476b45f300fac1bc979e07c3aa1e33ec1`.

## Gate correction and claim boundary

The operator-authorized device run occurred before the separate CPU-proof
review requested by `docs/fable-exl3-warp-staging-cpu-v2-handoff.md` returned
its acceptance token. The accepted warp design explicitly opened only the CPU
proof. Therefore this record is retained as reproducible information but is
not admissible route-promotion evidence yet. The scalar route remains the
engine control, and no additional staged-v2 optimization or integration may
rely on this result until the CPU proof and CUDA implementation are reviewed.

The payloads were deterministic synthetic values at the exact GLM-5.2 expert
projection shapes. This does not establish full-hash-gated real-payload
correctness, profiler-counter traffic, grouped expert execution, TP4 layer
replay, checkpoint output, quality, serving throughput, or KV capacity.

After review acceptance, the next evidence gates are real TR3 K=3 payload
comparison, inclusive and kernel-only profiler replay, then paired gate/up and
grouped-expert successors. The 201 us M1 gate/up result is a useful control but
is not fast enough to serve as the final GLM-5.2 MoE kernel.
