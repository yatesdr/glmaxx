# cn4 NVFP4 FC1 Nsight Compute diagnostic

Date: 2026-08-03

Status: synthetic accepted-artifact optimization diagnostic; no fused-MoE,
real-payload, TP4, serving, or performance acceptance

## Result

The exact SM120 library and Rust executable from the reviewed manifest/ABI
Phase-B tree were reused without rebuilding. Its retained M1 FC1 smoke had
already passed with zero failures, maximum absolute error 2.0, and maximum
relative error 0.0273973. Nsight Compute 2026.2 then profiled the two kernels
separately with basic-set kernel replay:

| kernel | grid CTAs | duration | SM throughput | L2 throughput | active warps | registers/thread |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| activation quantization | 1 | 14.464 us | 0.102% | 0.418% | 13.742% | 31 |
| direct FC1 + SwiGLU | 376 | 75.872 us | 14.816% | 2.243% | 27.230% | 34 |

The two isolated durations sum to 90.336 us. Quantization is 16.0% of that
pair; the CUDA-core FC1 control is 84.0%. The direct kernel already exposes a
large persistent grid, so the first-order FC1 problem is not the two-CTA
under-parallelization observed in the EXL3 gate control. It is scalar E2M1/
E4M3 decode, FP32 FMA, and CTA tree reduction in place of the intended SM120
block-scaled MMA path.

## Optimization decision

The measured NVFP4 FC1 order is:

1. connect the already compiled SM120 block-scaled MMA primitive to the
   reviewed routed FC1 byte/layout contract;
2. group routed assignments by expert so weight/source reuse and epilogues are
   amortized across concurrent rows without a rank-local route choice;
3. pipeline or fuse the one-CTA activation quantizer into the MMA producer
   boundary after the core path is correct; and
4. profile FC2 only after its pending grouped-scratch correction is accepted
   and passes correctness.

The retained direct path remains an independent correctness control. No
throughput projection is derived from this single synthetic assignment: a
real GLM-5.2 token has top-8 routing, rank ownership, FC2, communication, and
the rest of the target layer.

## Provenance

```text
source commit       8aa70cc5b10e0d0217c79f1aa601bd6349ec5653
Rust binary         5d43cfe66a2eb9d78f9d00c530febfce667d05e0b2c6c220735723336f92f17d
SM120 library       3ef1f5c214cb3453770183fc7793a118d77a6d62057231d4fe8cdcbc32f8bde8
container image     sha256:2e401388dcc9c180401cb9997e3e8394c2db695c7bf4e3139ff8a9a517940719
Nsight Compute      2026.2.0.0 build 37790515
```

The target command was the exact retained binary:

```text
/phase-b/cargo-target/release/glmaxx gpu-smoke 1
```

Nsight selected `direct_fc1_swiglu(glmaxx_fc1_descriptor)` and
`quantize_compacted_rows(glmaxx_fc1_descriptor, bool)` in separate runs,
using `--set basic --replay-mode kernel --launch-count 1`. Profiling ran as
root only inside disposable, network-disabled, GLMAXX-named containers with
`SYS_ADMIN`; CSV export used a no-GPU container. No host profiling policy was
changed.

Raw reports, CSVs, and logs are outside Git at:

```text
/home/derek/glmaxx/evidence/20260803T182000Z-nvfp4-fc1-ncu-8aa70cc
```

The sorted relative-path six-file hash stream is
`508c8b0b7bf4cea8940dc29f98d137d7308c1b87ff4986f575041fb56e9b5d3b`.
The direct and quantizer `.ncu-rep` hashes are respectively
`f96ee43896f17a6665f8e3617a7581cd53de012be6a0df903180b3b7fcdb835c`
and
`1ff6f950afbd92e3149ddd8c1293a3ef1254c85c7c5cff2addef7bc764c0090e`.
cn4 returned to 2/2/2/10 MiB used, 0% utilization, and no compute process.

## Scope

These are one-sample, replay-perturbed kernel diagnostics. They exclude
allocation, transfers, routing construction, FC2, collectives, layer/runtime
overhead, and checkpoint I/O. The pending fused-routed-MoE and profiler-package
reviews govern any implementation and acceptance-grade comparison.
