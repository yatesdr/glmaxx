# Fable handoff: SM120 profiler package v1 r2

Date: 2026-08-04

Status: full adversarial implementation review requested; supersedes the
unexecuted v1 handoff

GPU authorization conveyed by this handoff: none

Read-only cn4 artifact verification: permitted; do not launch CUDA or modify
the retained evidence tree

Review candidate commit:
`b3ee484c8dda2cc0baed986362fbb6c83d78256a`

Required result path:
`fable-sm120-profiler-package-v1-r2.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`sm120-profiler-package-v1-r2-accepted`

## Why r2 exists

The v1 handoff pinned `ncu --force-overwrite false`. Nsight Compute 2026.2
defines `--force-overwrite` as a valueless option, so it treated `false` as the
target program and could never capture a kernel. R2 removes the option because
evidence paths are fresh, moves the complete NCU invocation into a focused
wrapper, captures all 33 arguments in a CPU self-test, and makes preflight
require both that test and this corrective review. No v1 token should be
issued.

## Required provenance procedure

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Report a stale candidate and withhold the token if either
set differs from this table. Do not substitute current `main`, the later
handoff commit, or an untracked review-inbox file.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/sm120-profiler-package-v1.md` | `71edba4a79b0ed31f68e87f2e9d755ab4f9e3f85540b584f7893f667441a2c7e` |
| `docs/cn4-exl3-staged-k3-ncu-20260804.md` | `cc7592fd6da2b4bc589cefd664819c4d04c03dfb0e1a782582ad54fdefda0865` |
| `crates/glm-cli/src/profile.rs` | `a03bb7d17aff7f21cf5ffbbef5ac67d181500e856ee84b005f54bd83aa33ea47` |
| `crates/glm-cli/src/main.rs` | `e5e0fd98d222f8e6744de15901c67027ffd1ce7d81b2ca4fbca8f35297f76c77` |
| `crates/glm-cuda/src/ffi.rs` | `2a76ad51cb1c9b28a508dc4734bfeb6b6ad46103c3b437ec8e8ff8f6a6ff2f31` |
| `crates/glm-cuda/src/lib.rs` | `dfff79d944bacee30be686b8dda8e7c47f17926c316674c97d49e4c1984b7105` |
| `kernels/include/glmaxx_kernel.h` | `c5f5ceed453c901a63dfeecea0ec83a53b6485e98c32763650c708c699b56406` |
| `kernels/sm120/nvfp4_routed_fc1.cu` | `67d954f2ba1bf28f0eca30c42ab18c014b19353b4102e89edd7089a1ad9770c5` |
| `kernels/sm120/nvfp4_routed_fc2.cu` | `b72fff75bf4b0ee0ef06bf65286bad73678e4d396b2bdaad72bc784da738bb31` |
| `kernels/sm120/exl3_projection_control.cu` | `241730ceaf629d01101629cb3f107e8d13fe92019444f4b635f9aa1d8cbc819d` |
| `kernels/CMakeLists.txt` | `9c695447b180e67f49c3c320be1f6b6be99501c661cd479726cb20695ce048c5` |
| `scripts/cn4-profiler-preflight.sh` | `242457b2c24e7890599ffb9dc2e3a589de5f1589b88fae2b8c8a7b9455340452` |
| `scripts/cn4-profiler-suite.sh` | `3ef5b9fd4620a14f0aee9d5ee1b96ee86bf5e28e0c7d0d099411bc490f13df47` |
| `scripts/cn4-ncu-capture.sh` | `d449c711c8530228e89a08deead691a0860f44e6bd4f3583e4a14010d57b1864` |
| `scripts/cn4-ncu-capture-selftest.sh` | `2c8b83497c5cf3dab3a23bb13db328a5f180d9e6f99eaf3ea3877cc65ed43c63` |
| `scripts/local-checks.sh` | `1675ca5bac9bda032ab6db206629bc0052ad6afc5cb6f59a7b6697e4e5c779d0` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

Run the complete CPU-only gate and record its exit status:

```text
./scripts/local-checks.sh
```

## Decision 1: deterministic plan and route controls

Independently reconstruct the 571-case plan, 178 retained timing cases, and 36
separate nsys/ncu representative cases. Verify uniqueness, sort order, every
decode/prefill row, all five grouped routing postures, direct one-hot
restriction, and EXL3 M1/2/4/8 restriction. Mutate, remove, add, and reorder
cases and confirm plan validation fails.

Audit route generation, stable expert-major compaction, top-k uniqueness,
empty experts, uniform/Zipf/maximal skew, and the separately retained host
routing distribution.

## Decision 2: retained timing boundary

Trace every selected FC1, FC2, graph, and EXL3 timing path. Confirm every
launch owns one start/end event pair, the final synchronization makes every
sample readable, original sample order is retained, warmups and prerequisites
are outside the interval, and a selected phase launches only that phase.
Check nearest-rank percentiles, mean, population standard deviation, invalid
input rejection, and that output hashing occurs after timing. Counter replay
must never be presented as latency.

## Decision 3: profiler command and NVTX boundary

Audit the CUDA profiler/NVTX ABI and every cleanup path. Confirm warmups finish
before `cudaProfilerStart`, nested ranges enclose only selected launches plus
terminal synchronization, and cleanup cannot replace an earlier error.

Independently execute or parse the exact CUDA 13.3 Nsight Systems and Nsight
Compute 2026.2 command lines. Run the focused self-test and inspect its
NUL-separated 33-argument capture. It must prove:

- neither `false` nor any `--force-overwrite` spelling reaches NCU;
- `--export` receives the fresh report base;
- all required sections, NVTX push/pop filter, all-process, and kernel replay
  options precede the target; and
- the GLMAXX runner is the first target token with every target argument
  unchanged.

Hash-verify the retained isolated cn4 reports and controls. Confirm that the
successful diagnostic used the corrected no-overwrite form and produced real
SM120 kernel records, while remaining explicitly outside complete-package
qualification.

## Decision 4: byte, resource, and evidence accounting

Re-derive representative FC1, FC2, and EXL3 M1/M3072 byte ledgers. Verify all
arithmetic is checked and contract GiB/s is labeled a logical lower bound.
Runtime weight repack and persistent dequantization must truthfully remain
zero for these fixture paths.

Attack the evidence manifest with modified, added, removed, renamed, linked,
unsupported, non-UTF-8, escaped, and self-manifest files. Confirm resource
usage, SASS, raw nsys/ncu reports, CSV, samples, power/clocks, binaries, tool
hashes, source identity, review proofs, command self-test, and correctness all
have an immutable evidence home.

## Decision 5: preflight and suite ordering

Trace every preflight rejection path. It must verify authorization, full source
identity, tracked cleanliness, pinned CUTLASS/toolchains, all prerequisite
reviews including this r2 handoff, exactly four idle SM120 GPUs, and fresh
external evidence/build roots before build or CUDA work. Confirm the wrapper
files are executable and their argument self-test runs before device launch.

Trace correctness before timing, timing before profiler replay, occupancy
checks before every GPU command, final source/device checks, manifest creation,
and exact revalidation. Identify any redirection that pre-creates a directory
which a Rust command requires empty.

## Decision 6: claim boundary

Confirm collective, isolated SwiGLU, target layer, MTP, checkpoint serving, KV
offload/prefix caching, concurrency, quality, and end-to-end performance remain
explicit nonclaims. The isolated K=3 report is a diagnostic, not package or
engine acceptance evidence.

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
answer separately and unambiguously:

1. Is the deterministic 571-case plan and route generation accepted?
2. Is retained, phase-isolated CUDA-event timing accepted?
3. Is the corrected profiler/NVTX and separate nsys/ncu boundary executable
   and accepted?
4. Are byte accounting, resource capture, and evidence validation accepted?
5. Is preflight fail-closed before device work and is suite ordering accepted?
6. Are every performance and serving nonclaim exact?
7. Is r2 accepted for its first complete authorized SM120 package cycle?

Only if every answer is an unqualified `YES`, include the candidate commit and
all eighteen exact input SHA-256 values from the provenance table in the
result, then end with the requested acceptance token named in the header as
the only bare acceptance line.

Withhold the token for a conditional pass, stale input, unexecutable option,
target-boundary ambiguity, timing/counter confusion, missing cleanup,
unmanifested artifact, or any path that reaches CUDA before authorization and
review checks.
