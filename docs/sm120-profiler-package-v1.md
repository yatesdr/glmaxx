# SM120 profiler package v1

Status: corrective implementation candidate; r2 adversarial review required

GPU evidence: isolated EXL3 staged diagnostic only; no package qualification

The first package handoff pinned an Nsight Compute command that treated the
token `false` as the target executable. The corrected package omits the
valueless overwrite flag, regression-tests the complete target boundary, and
requires the corrective r2 review token in preflight. The isolated diagnostic
in `docs/cn4-exl3-staged-k3-ncu-20260804.md` proves the corrected command form
can retain an SM120 report, but it did not execute this complete package.

## Purpose

This package makes the GLM-5.2 TP4 NVFP4/EXL3 kernels ready for the first
repeatable SM120 correctness and optimization cycle. It is target-only: four
PCIe-connected compute-capability 12.0 GPUs, the pinned GLM-5.2 tensor shapes,
NVFP4 direct/grouped FC1 and FC2, and the direct-source EXL3 gate/up/down
projections. It does not establish support for another GPU or model.

The package is deliberately split into:

1. `gpu-time-case`, which retains one CUDA-event latency sample per launch and
   never starts an Nsight profiler;
2. `gpu-profile-case`, which launches one named phase inside nested NVTX and
   `cudaProfilerStart`/`cudaProfilerStop` boundaries and records no CUDA-event
   timing; and
3. `cn4-profiler-suite.sh`, which runs correctness before timing, timing before
   counter replay, and publishes a content manifest only after the source and
   device are rechecked.

No timing or counter result exists until the suite is run on authorized SM120
hardware. This implementation therefore makes no performance claim.

## Deterministic matrix

`glmaxx profile-plan` emits 571 sorted, unique cases. Re-emission is byte
deterministic. `profile-plan-validate` rejects any missing, added, reordered,
or altered case.

The NVFP4 row domain is exactly decode M=1,2,4,8,16,32,64,128 and prefill
M=256,512,1024,2048,3072. The EXL3 CPU-control domain is M=1,2,4,8. Grouped
cases include empty-expert, one-hot, uniform, Zipf, and maximally-skewed
routing. Direct controls accept only one-hot routing. The matrix contains:

- direct FC1 quantize, fused core/SwiGLU, eager inclusive, and graph inclusive;
- grouped FC1 expert-local quantize, grouped fused core/SwiGLU, and inclusive;
- direct and grouped FC2 quantize, core, reduce, and inclusive; and
- EXL3 gate, up, and down projection.

The first-cycle shell schedule runs all phases and all five grouped routing
cases at M=1,128,256,3072, plus EXL3 at M=1,8: 178 retained timing cases. It
runs nsys and ncu in separate processes for 36 phase/shape representatives.
The single-case commands remain available for every one of the 571 cases.

## Timing boundary

Every retained sample owns a distinct start/end CUDA-event pair. All pairs
are recorded on the fixture stream, the final event is synchronized once, and
each elapsed interval is read after completion. Reports retain the original
sample order plus minimum, nearest-rank p50/p90/p95/p99, maximum, mean, and
population standard deviation in microseconds. Host enqueue samples are also
retained where the existing aggregate benchmark commands report them.

Warmups complete before the measured event sequence. FC1 core timing performs
the required activation quantization before warmup; FC2 reduce timing performs
the required quantize and core preparation before warmup. Grouped metadata and
graph instantiation occur before timing. Route generation/compaction, fixture
packing, upload, downloads, output hashing, JSON encoding, and artifact hashing
remain outside the timed interval.

Routed `gpu-time-case` reports additionally retain a separate host-clock
distribution for deterministic route generation, sorting, and compaction.
That CPU distribution is never combined with the CUDA-event distribution.

`gpu-time-case` launches only the selected phase during its warmup and measured
sequence. The untimed output-hash launch occurs afterward. It cannot silently
substitute an inclusive launch for a requested isolated phase.

## Profiler boundary

The native library exports four profiler controls:

- `glmaxx_profiler_start` and `glmaxx_profiler_stop`;
- `glmaxx_nvtx_range_push`; and
- `glmaxx_nvtx_range_pop`.

The outer push/pop range is exactly `glmaxx-profile`. The inner range names the
selected backend phase, for example `glmaxx.fc1.grouped-core-swiglu` or
`glmaxx.fc2.reduce`. Warmups and prerequisite kernels complete before
`cudaProfilerStart`. The selected launches and their terminal stream
synchronization occur inside both ranges. Cleanup pops both ranges and stops
the profiler even when a launch fails; the original launch or synchronization
error remains authoritative.

Nsight Systems uses the CUDA profiler API as its capture range. Nsight Compute
uses the exact outer NVTX push/pop range, kernel replay, and the required
LaunchStats, Occupancy, SpeedOfLight, MemoryWorkloadAnalysis, SchedulerStats,
WarpStateStats, and InstructionStats sections. Counter replay is never used as
latency evidence. The suite gives Nsight Compute a fresh export base and omits
`--force-overwrite`: CUDA 13.3's Nsight Compute 2026.2 defines that option as a
valueless flag, so an added `false` token would become the executable instead
of an option value. The CPU self-test captures the complete argument vector and
requires the GLMAXX runner to be the first target token.

## Byte and throughput records

Each case report records input, direct-packed value, direct-packed scale,
routing metadata, output, and temporary/workspace bytes. Checked arithmetic is
mandatory. The reported contract traffic adds logical reads and writes; it is
an explicit lower-bound ledger, not an assertion about observed DRAM sectors.
The derived GiB/s value uses the retained p50 only. NCU supplies measured memory
counters separately.

Runtime weight repack and persistent dequantization bytes are pinned to zero.
EXL3 reports source trellis and rotation bytes rather than a reconstructed
matrix.

## Preflight and fail-closed gates

`cn4-profiler-preflight.sh` creates no CUDA context and launches no kernel. It
requires explicit `sm120-profiler-cycle-authorized` operator authorization and
fails before build or execution unless:

- the full expected source commit, pinned origin, tracked tree, and index
  match; the only permitted untracked subtree is the read-only
  `docs/reviews/` operator inbox;
- CUTLASS is clean at `e05f953a5b3d38adc240df2ff928e0421c2abba3`;
- all manifest, EXL3 source, EXL3 warp, NVFP4 fused, current-tree, and profiler
  handoffs verify their committed result bytes and exact acceptance token;
- Rust is exactly 1.92.0 and nvcc exactly 13.3.33;
- nsys, ncu, cuobjdump, nvdisasm, CMake, Ninja, CUDA, Rust, and hashing tools
  exist, and ncu exposes every required section and replay option;
- exactly four visible GPUs report compute capability 12.0 and no compute PID
  is active; and
- evidence and build roots are fresh external paths.

Preflight builds the shared library and Rust runner, verifies an SM120 device
image and all profiler ABI symbols, captures resource usage and full SASS,
records exact executable/tool/review hashes, emits and validates the 571-case
plan, and rechecks source and GPU idleness. Its only successful verdict is
`PREFLIGHT_PASS_NO_DEVICE_LAUNCH`.

## Correctness-before-performance sequence

The one-command suite runs, in order:

1. preflight and reviewed-byte verification;
2. four-rank binding;
3. FC1 numerical, graph, dense-control, and grouped-control correctness;
4. FC2 M1 and M256 smoke;
5. EXL3 gate/up/down M1 and M8 smoke;
6. retained CUDA-event timing;
7. nsys trace capture;
8. ncu counter replay;
9. final source/device-state checks; and
10. evidence manifest construction followed by exact revalidation.

Any failed command stops the suite. A profiler report must be nonempty before
CSV extraction. Source drift, new tracked changes, GPU occupancy, missing
review results, an existing evidence path, missing sections, or an artifact
set that differs from its manifest is terminal.

## Evidence ownership

Raw evidence and build intermediates stay outside Git. The evidence manifest
recursively rejects symlinks and unsupported file types, hashes regular files
in sorted relative-path order, records lengths and SHA-256 values, and excludes
only its own fixed filename. Validation reconstructs the entire manifest and
requires exact equality.

The suite records GPU identity, topology, clocks, power, temperature, memory,
tool versions and executable hashes, review hashes/proofs, source commit,
container digest, CUTLASS commit, binary/library hashes, resource usage, SASS,
correctness results, raw latency samples, nsys reports/statistics, ncu
reports/raw CSV, and before/after device state.

## Boundaries not silently claimed

Routing is a CPU fixture control outside kernel timing. FC1 core and SwiGLU are
currently fused by the reviewed ABI, so this package labels that boundary
`core-swiglu`; an isolated SwiGLU kernel would require a new reviewed ABI.
Collective and full layer end-to-end boundaries remain blocked on acceptance
of the target-layer/TP4 layer-replay contracts. The deterministic plan records
that status rather than fabricating collective timings.

MTP, checkpoint serving, KV paging/offload, prefix caching, scheduler
concurrency, and quality acceptance are outside this microbenchmark package.
They remain required serving milestones and cannot inherit a kernel profiler
pass.
