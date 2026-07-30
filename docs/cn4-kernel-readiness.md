# cn4 kernel readiness

Date: 2026-07-29

Phase: A complete only after the repository checks, commit, and push recorded
in the handoff

Target: four RTX PRO 6000 Blackwell GPUs, SM120, PCIe, no NVLink

Operator: TP4 rank-local GLM-5.2 routed-expert FC1/SwiGLU/FC2

Kernel ABI: `glmaxx.sm120.nvfp4.routed_moe.v2`

## Current verdict

The CPU/reference package, direct CUDA correctness baselines, and native
block-scaled FC1/FC2 controls are engineering-ready for cn4. Device execution
is still gate-blocked until an
independent reviewer accepts the generated operation manifest and the v0.2.2
physical/cache ABI amendment. Separate operator authorization is also
required. The kernels are not yet qualified as functional on SM120, and they
are not performance candidates yet. The authorized cn4 preparation pass
compiled native `sm_120f` cubins with pinned CUDA 13.3 and CUTLASS 4.6.1,
proved both CUTLASS scale layouts, linked both 224-byte Rust/native
descriptors, and ran all 153 Rust tests without creating a CUDA context. The
four compile-only dense/grouped FC1/FC2 controls contain exactly 256 native
SM120 block-scaled E2M1/UE4M3 `OMMA.SF` instructions. There is still no FC2
device launch, hardware numerical result, counter, or timing evidence. The
latest compact result is
`docs/cn4-routed-fc2-preparation-20260729.md`.

The first cn4 session must establish correctness before replacing the
CUDA-core dot product with the CUTLASS block-scaled MMA path. A source file
existing in Git is not a GPU pass.

## Proven locally

- Rust 1.92 workspace builds without a tensor framework.
- E2M1 codes, E4M3 finite classes, tie rounding, zero encoding, corrupt
  metadata, overflow checks, and direct byte accounting are tested.
- Value order is logical row-major with even-low/odd-high nibbles.
- The SFB closed form is bijective for the real `[1024,6144]` rank shard.
- The deterministic actual-shape fixture has:

  - 3,145,728 value bytes;
  - 393,216 scale bytes;
  - 128 metadata bytes;
  - packed digest
    `a84be06b6bf6192eb51324ee57a1b6a4c57924c78709bcbe275b9f56b547cab5`.

- The deterministic rank container has content-derived identities, zero
  timestamp, CRC32C, SHA-256 checks, fixed descriptors, corruption rejection,
  and complete range/overlap validation.
- The operation manifest freezes gate/up TP4 slicing, route order, operation
  order, reduction boundary, 75 sparse layers, all 21 target IndexShare
  groups, and the logical one-layer MTP recurrence. Independent review is
  pending.
- The 368-byte KV and 132-byte indexer records have CPU writers/readers.
- Packed indexer scoring, deterministic owner-local/global top-k, packed-KV
  decode, and fixed-rank log-sum-exp merge are exercised together against a
  direct sparse-attention control. This is a record/merge proof, not a
  model-layer replay.
- Round-robin DCP4 gives exactly 4,096 full pages per rank at 1M positions.
- Every page-state pair is checked against the transition table.
- MTP0 through MTP6 tentative/commit/rollback transitions are checked,
  including context-limit clamping.
- The cache calculator derives 33,529,266,176 aggregate bytes at 1M with MTP:
  30,098,325,504 target KV, 2,906,652,672 target indexer keys, 385,875,968
  draft KV, and 138,412,032 draft indexer keys.
- Rust and C agree on two 224-byte, 16-byte-aligned descriptors.
- The source baseline directly consumes packed weights, dynamically quantizes
  BF16 rows once, reuses them for gate/up, accumulates in FP32, fuses the two
  dot products with SwiGLU, and does not materialize a gate/up tensor. Decode
  uses a fixed persistent CTA pool; prefill uses a grouped two-dimensional
  schedule.
- A generated CPU matrix artifact freezes all nine row buckets, eight routing
  cases, eight numerical cases, actual-shape fixture hashes, 135 positive GPU
  launches, and nine required route rejections. The Rust GPU runner emits one
  immutable JSON record per case and retains every failing element.

## Unproven until the reviewed cn4 launch

- hardware conversion agreement for every scale/value boundary;
- successful descriptor launch and asynchronous error behavior;
- numerical agreement for decode M `1,2,4,8,16,32,64,128` and prefill M 256;
- graph capture, repeatability, leak freedom, and route-edge GPU behavior;
- physical bytes read, achieved bandwidth, occupancy, register pressure,
  shared memory, launch overhead, or speed;
- block-scaled tensor-core use;
- numerical agreement for the GLMAXX-owned block-scaled tensor-core control;
- any comparison with BF16, FP8, vLLM, SGLang, llama.cpp, or EXL3.

## First authorized session

Do not run these commands until the manifest/ABI review gate passes and the
operator separately authorizes cn4.
Start from an exclusive shell on cn4. The script performs read-only inventory
first and exits without launching if `nvidia-smi` reports a compute PID.

```bash
cd /path/to/glmaxx
export CUTLASS_DIR=/path/to/cutlass-4.6.1
export GLMAXX_EVIDENCE_DIR=/path/outside/repo/glmaxx-m2-$(date -u +%Y%m%dT%H%M%SZ)
export GLMAXX_CONTAINER_DIGEST=sha256:<64-lowercase-hex-container-digest>
export GLMAXX_REVIEW_GATE=manifest-abi-v0.2.2-accepted
export GLMAXX_REVIEW_ARTIFACT=/path/to/glmaxx/fable-manifest-abi-v022-r2.md
export GLMAXX_CN4_AUTHORIZATION=phase-b-authorized
./scripts/cn4-phase-b.sh
```

The review artifact must be committed in the source repository and contain
the exact acceptance token on its own line. The script records its SHA-256 at
launch and verifies that neither it nor the source tree changed during the
run. An environment token without that artifact cannot open the gate.

The expected environment is Rust 1.92, CMake at least 3.28, Ninja, CUDA
13.3, and CUTLASS commit
`e05f953a5b3d38adc240df2ff928e0421c2abba3`. A mismatch stops the session; do
not edit pins after seeing results.

## Provisional direct-control timing

Only after the complete phase-B eager and graph gates pass on the same source
commit, `scripts/cn4-phase-c-baseline.sh` may time the retained CUDA-core
control. It revalidates both summary files, the review artifact, source
commit, native library, runner, and GPU idleness before launch.

```bash
export GLMAXX_PHASE_B_EVIDENCE=/evidence/<successful-phase-b>
export GLMAXX_EVIDENCE_DIR=/evidence/direct-baseline-<UTC timestamp>
export GLMAXX_REVIEW_GATE=manifest-abi-v0.2.2-accepted
export GLMAXX_REVIEW_ARTIFACT=/workspace/fable-manifest-abi-v022-r2.md
export GLMAXX_CN4_AUTHORIZATION=phase-c-authorized
./scripts/cn4-phase-c-baseline.sh
```

The runner records activation quantization, fused direct-core/SwiGLU,
inclusive eager, inclusive CUDA-graph, and host enqueue time separately over
20 warmups and 200 measured iterations for all nine frozen M buckets. Routing
remains the named CPU fixture control outside the timed CUDA boundary. Every
result is labeled `PROVISIONAL_CONTROL_ONLY`; it is a baseline for the later
CUTLASS MMA candidate, not a performance win.

## Expected evidence

The external evidence directory must contain:

- GPU names, UUIDs, PCI addresses, driver, memory, and topology;
- container digest and before/after GPU clocks, power limits, and persistence
  mode;
- source commit and clean status;
- Rust/Cargo/CMake/nvcc versions and CUTLASS commit;
- SHA-256 of both specs, the operation manifest, and test matrix;
- complete Rust test output;
- CMake configure/build output and compiler command lines;
- the 393,216-comparison CUTLASS layout-probe result;
- a shared-library SASS record proving exactly 256 expected SM120 NVFP4
  `OMMA.SF` instructions and all four exported FC1/FC2
  dense/grouped-control symbols;
- one JSON correctness report for each of the 135 positive cases, a summary
  proving all nine negative route cases were rejected, two 20-repeat eager
  determinism gates, and SHA-256 for every report;
- two CUDA-graph JSON reports for M1 and M256, each proving numerical
  agreement and bitwise identity across 20 replays, a summary, and SHA-256
  for every report;
- M1 and M256 CUTLASS packed-byte control reports, each proving numerical
  agreement and bitwise identity across 20 eager repeats, plus a summary and
  SHA-256 for every report;
- fourteen positive expert-grouped CUTLASS reports over M1/M256 routing,
  two negative route rejections, 20-repeat determinism for the all-expert
  cases, a summary, and SHA-256 for every report;
- later, separate kernel and inclusive timing, control results, profiler
  reports, and a provenance/result manifest.

Raw outputs remain outside Git. Only a compact, reviewed result record and
artifact hashes may be committed.

## Frozen correctness gate

Before looking at GPU output, the element rule is:

```text
finite(gpu) and abs(gpu - cpu) <= 0.5 + 0.02 * abs(cpu)
```

This intentionally broad first-launch threshold detects layout, scale,
nibble, and gross accumulation failures without pretending to be the final
quality threshold. The report retains maximum absolute/relative error and
every failing element. No NaN/Inf, illegal fallback, runtime weight repack, or
persistent dequantization is allowed. The frozen M1 decode and M256 prefill
representatives each run twenty eager iterations and must be bit-identical.
The phase-B runner then captures those same representatives into CUDA graphs;
each must remain bit-identical and within the same numerical tolerance over
twenty graph replays. An eager result cannot satisfy the graph gate. Initial
eager baselines may be timed after both correctness gates pass, but must
remain labeled eager and provisional.

The matrix and controls are frozen in
`benchmarks/sm120-fc1-matrix-v1.json`. Passing M1 and M256 is the minimum
functional definition; all listed M and route/numerical cases are required
for the complete M2 correctness matrix.

## Risks and rollback

- CUTLASS/CUDA may reject `sm_120f` or expose a changed layout. Stop and record
  the build; do not repack around it silently.
- The baseline may disagree because CPU and GPU conversion ties differ.
  Preserve bytes and failing indices, add a minimal boundary fixture, and
  revise only after a reviewed arithmetic decision.
- The direct baseline may be very slow. That is expected and is not a reason
  to skip correctness or relabel it as a performance result.
- The source currently assumes compacted route arrays. Route-compaction GPU
  qualification is a separate matrix line; CPU compaction is the initial
  deterministic control.
- If cn4 becomes occupied or unstable, synchronize the active stream if one
  was created, exit, preserve the external evidence directory, and leave
  other processes untouched.

Rollback means returning to the Phase A commit and its fixture digest. No
checkpoint or cache migration exists, and the laboratory rank file is
regenerable. Never delete shared model weights or neighboring repository
artifacts.

## Ordered performance punchlist

1. Pass the complete direct-layout correctness matrix, retaining M1 and M256
   as the first decode/prefill milestones.
2. Pass the M1/M256 CUDA-graph numerical and 20-replay determinism gate.
3. Add the frozen timing ledger for quantization, routing, core FC1,
   epilogue, launch, and inclusive eager time; retain the direct baseline.
4. Replace the CUDA-core dot product with pinned CUTLASS SM120 block-scaled
   MMA, keeping FP32 accumulation and the same bytes.
5. Feed the exact SFA swizzle to MMA without a transform and attribute
   activation-quantization time separately.
6. Add deterministic GPU route histogram/prefix-sum/compaction and tune the
   retained persistent small-M schedule; keep the CPU-compacted control.
7. Fuse the CUTLASS epilogue into BF16 SwiGLU without a gate/up global
   intermediate and prove it against the retained accumulator control.
8. Sweep decode tiles, stages, warp specialization, cluster shape, register
   caps, and persistent CTA count on SM120 only.
9. Add grouped prefill scheduling and tune M `128..3072` independently from
   decode.
10. Before closing M2, run leak/error/repack checks, hardware counters, and
    matched BF16/FP8 controls.
11. Implement FC2 direct-packed consumption, route weighting/scatter, shared
    expert combination, and the single TP4 reduction boundary.
12. Run TP4 one-layer replay with exact routes and downstream logits.
13. Provenance-pin the NVFP4 result, then begin EXL3 codec work. EXL3 still
    precedes every capacity-serving gate.
