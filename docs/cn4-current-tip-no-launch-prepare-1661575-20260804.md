# cn4 current-tip no-launch preparation

Date: 2026-08-04

## Result

The clean integration tip
`1661575c23dfea8c8d1be10da004b1a095da2b33` was rebuilt on cn4 in an
isolated GLMAXX worktree and container without GPU device access. All 429 Rust
tests passed, five real `sm_120f` cubins were produced, both CUTLASS layout
probes passed, the expected 256 Blackwell OMMA instructions were present, and
the Rust release binary linked against the newly built CUDA library.

This is a build/readiness result only. It did not create a CUDA context, launch
a kernel, read model payloads, run TP4, execute a checkpoint smoke, or measure
quality, KV capacity, latency, or throughput.

## Provenance

- Host: cn4 (`192.168.13.34`)
- Worktree:
  `/home/derek/glmaxx/worktrees/integration-1661575-20260804T134700Z`
- Source commit:
  `1661575c23dfea8c8d1be10da004b1a095da2b33`
- Evidence:
  `/home/derek/glmaxx/evidence/20260804T135047Z-phase-b-prepare`
- Evidence manifest SHA-256:
  `8d430acfeae11c8bc68efd00e1e0b5da42eabcbe8761ffdbe587c72c4c26c219`
- Evidence verifier: `evidence-run-verify=pass state=COMPLETE files=4428`
- Container:
  `glmaxx-dev@sha256:2e401388dcc9c180401cb9997e3e8394c2db695c7bf4e3139ff8a9a517940719`
- Rust: `1.92.0`
- CUDA: `13.3.33`
- CUTLASS: `e05f953a5b3d38adc240df2ff928e0421c2abba3`
- Network: disabled
- NVIDIA devices/driver: not exposed to the container

The host GPUs were observed idle before and after the run. Those observations
are operator-side receipts, not part of the no-device container claim.

## Checks

- Rust tests: 429 passed
- Cubins: `nvfp4_routed_fc1`, `nvfp4_routed_fc2`,
  `exl3_projection_control`, `cutlass_nvfp4_dense_control`, and
  `cutlass_nvfp4_fc2_control`
- SFB layout: `CUTLASS_SFB_LAYOUT_PASS comparisons=393216`
- SFA layout: `CUTLASS_SFA_LAYOUT_PASS cases=17 comparisons=42564864`
- Expected Blackwell OMMA instruction count: 256
- Required exports: FC1 dense/grouped, FC2 dense/grouped, EXL3 kernel ABI,
  projection, and workspace symbols present

Selected artifact SHA-256 values:

- `libglmaxx_sm120.so`:
  `e385b6504a8f19643fa59661a0ef7005ec7fb986d764f8b0939ca7bc51d205bd`
- SFB layout probe:
  `19979ea29f2a1bba595cf7b935938d08ea2d2d3bc447a68e9ef7eb0a4df35638`
- SFA layout probe:
  `0ae6a5e4f7820f491f1b1ae398cecfc604fd0660f0ca6a9b81bb8560cdda3c81`
- CUTLASS dense control:
  `9bc1722fee3b178ba5943a79ee59af5aae929c588e08f281428e7c165e9dc681`
- Rust release binary:
  `69d8a73be61771df7ce5c5ddf02241693ce182b3f6b6ba34c852e8dcf12c12fc`

## Failed attempts

Three earlier evidence directories are retained and must not be reused:

- `20260804T134553Z-phase-b-prepare`: cn4 host had no `rustc`
- `20260804T134819Z-phase-b-prepare`: the unprivileged container could not
  read the root-owned Cargo registry
- `20260804T134905Z-phase-b-prepare`: CUTLASS hit Git safe-directory rejection

The successful run used container-local safe-directory configuration only. It
did not alter host Git configuration or any vLLM path, image, cache, process,
or result.
