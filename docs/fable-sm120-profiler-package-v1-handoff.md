# Fable handoff: SM120 profiler package v1

Date: 2026-08-01

Status: adversarial implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: unavailable to this review; do not connect to cn4 or launch CUDA

Review candidate commit:
`fdbd91647a3ea23031ebd562e3d57676d7eb5d9a`

Required result path:
`fable-sm120-profiler-package-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`sm120-profiler-package-v1-accepted`

## Required provenance procedure

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Report a stale candidate and withhold the token if any byte
does not match this table. Do not substitute current `main`, an untracked
review-inbox file, or prose about a later commit for the pinned candidate.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/sm120-profiler-package-v1.md` | `3f4165a44e8b10ea0e1e8c1bb5613ed1a2a04633c5a6f819ec63481ae79ad0ca` |
| `crates/glm-cli/src/profile.rs` | `a03bb7d17aff7f21cf5ffbbef5ac67d181500e856ee84b005f54bd83aa33ea47` |
| `crates/glm-cli/src/main.rs` | `04537c79fe4bcac67627483e96fcedc783702d08a16db8c10f3894964fe99afc` |
| `crates/glm-cuda/src/ffi.rs` | `2a76ad51cb1c9b28a508dc4734bfeb6b6ad46103c3b437ec8e8ff8f6a6ff2f31` |
| `crates/glm-cuda/src/lib.rs` | `dfff79d944bacee30be686b8dda8e7c47f17926c316674c97d49e4c1984b7105` |
| `kernels/include/glmaxx_kernel.h` | `c5f5ceed453c901a63dfeecea0ec83a53b6485e98c32763650c708c699b56406` |
| `kernels/sm120/nvfp4_routed_fc1.cu` | `67d954f2ba1bf28f0eca30c42ab18c014b19353b4102e89edd7089a1ad9770c5` |
| `kernels/sm120/nvfp4_routed_fc2.cu` | `b72fff75bf4b0ee0ef06bf65286bad73678e4d396b2bdaad72bc784da738bb31` |
| `kernels/sm120/exl3_projection_control.cu` | `241730ceaf629d01101629cb3f107e8d13fe92019444f4b635f9aa1d8cbc819d` |
| `kernels/CMakeLists.txt` | `9c695447b180e67f49c3c320be1f6b6be99501c661cd479726cb20695ce048c5` |
| `scripts/cn4-profiler-preflight.sh` | `ce12448e082ac95e2b600ad401aa79938684534dbe121a11c7d20beadf5613d6` |
| `scripts/cn4-profiler-suite.sh` | `dfa44d59986a2949940f4bd7ec67a18c7a62e8e0fc67c2db32ca36a425b22ce0` |
| `scripts/local-checks.sh` | `2d1882be9afd91f4a54c1d3ff9b9f02cd5087357eeb5668d4094c2114c3003ce` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

Run the complete CPU-only gate and record its exit status:

```text
./scripts/local-checks.sh
```

It must run the complete release workspace test suite, both Clippy/type-check
postures, deterministic plan regeneration, existing CPU proofs, header syntax,
and every shell syntax check. CUDA compilation and execution are not possible
on this review host and are not implied by a CPU pass.

## Decision 1: deterministic case contract

Independently reconstruct the expected case count. Verify that the 571 cases
are unique, sorted by the emitted case ID, cover every specified decode and
prefill row, include all five grouped routing postures, constrain direct
controls to one-hot, constrain EXL3 to M1/2/4/8, and reject every unsupported
backend/mode/phase/routing combination. Mutate one field, remove one case, add
one case, and reorder two cases; `profile-plan-validate` must fail each
mutation.

Verify the route fixtures, including expert-major sorting, top-k uniqueness,
route weights, empty-expert placement, uniform distribution, Zipf skew, and
maximal skew. Confirm the separately retained host routing distribution cannot
be mistaken for CUDA-event latency.

## Decision 2: retained timing correctness

Audit `time_cuda_launches`, `time_cuda_case`, every FC1/FC2 phase-specific
method, and EXL3 timing. Determine whether:

1. every measured launch has its own start/end event pair;
2. the final synchronization makes all elapsed reads valid without a sync per
   launch;
3. the sample vector retains launch order and cannot hide outliers;
4. warmup, graph instantiation, grouped metadata, and prerequisite quantize or
   core work are outside the selected phase interval;
5. a selected time case launches only that phase during warmup and timing;
6. nearest-rank percentile, mean, and population-standard-deviation arithmetic
   is correct for all valid sample counts; and
7. invalid, empty, nonfinite, negative, oversized, or overflowing inputs fail
   closed.

Check that the later output-hash launch is outside timing and that timing
reports never claim profiler counter replay as latency.

## Decision 3: profiler and NVTX boundary

Verify the CUDA/C ABI, NVTX3 header-only/link posture, Rust FFI, and cleanup
logic. Determine whether warmups finish before `cudaProfilerStart`; both root
and phase ranges enclose only the selected launches plus terminal sync; all
failure paths pop/stop as far as safely possible; and a cleanup error never
overwrites an earlier launch error.

Independently check the exact CUDA 13.3 nsys/ncu command lines. Confirm the
`cudaProfilerApi` capture range, `glmaxx-profile/` push/pop filter, kernel replay
posture, required section names, report/export suffixes, CSV imports, and
force-overwrite syntax are executable. Do not accept plausible-looking flags
without checking the pinned tool versions.

## Decision 4: bytes, resources, and evidence

Re-derive FC1, FC2, and EXL3 input/value/scale/metadata/output/workspace byte
ledgers for representative M1 and M3072 cases. Verify all arithmetic is checked
and that reported contract GiB/s is explicitly a logical lower bound, not an
observed DRAM claim. Confirm runtime repack and persistent dequant bytes can
truthfully remain zero for these fixture paths.

Attack the evidence manifest with a modified byte, added file, removed file,
renamed file, symlink, non-UTF-8 path, unsupported file type, path escape,
invalid source commit, and self-manifest replacement. Determine whether exact
reconstruction rejects each mutation without recursively hashing itself.

Verify that resource usage, full SASS, raw nsys/ncu reports, CSV exports,
latency samples, power/clocks, binaries, exact tool hashes, source identity,
review proofs, and correctness results all have an evidence home. Identify any
shell redirection that pre-populates a target directory which the Rust runner
requires to be empty.

## Decision 5: preflight and suite ordering

Trace every preflight rejection path. It must verify explicit authorization,
full source identity, tracked cleanliness, permitted untracked names, pinned
CUTLASS, committed review results, exact tool capabilities, four SM120 devices,
and GPU idleness before building or launching. Preflight may inventory and
compile but must not create a CUDA context or launch a kernel.

Trace the suite sequence and independently count 178 retained-timing cases and
36 separately replayed nsys/ncu cases. Confirm all correctness commands precede
all timing, all timing precedes counter replay, occupancy is checked before
each GPU command, source identity is rechecked at finish, and the final
manifest is not mutated after validation.

Confirm collective, isolated SwiGLU, layer end-to-end, MTP, checkpoint serving,
KV offload/prefix caching, concurrency, and quality remain explicit non-claims.
The target-layer and TP4 layer-replay contracts are still review-blocked; this
package must not cross those gates merely to fill a profiler category.

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then answer
separately and unambiguously:

1. Is the deterministic 571-case plan and route generation accepted?
2. Is retained, phase-isolated CUDA-event timing accepted?
3. Is the CUDA-profiler/NVTX and separate nsys/ncu replay boundary accepted?
4. Are byte accounting and evidence-manifest validation accepted?
5. Is preflight fail-closed before device work and is suite ordering accepted?
6. Are all performance and serving non-claims exact?
7. Is the package accepted for its first authorized SM120 correctness and
   optimization cycle?

Only if every answer is an unqualified `YES`, include the candidate commit and
all fifteen exact input SHA-256 values from the provenance table in the result,
then end with the requested acceptance token named in the header as the only
bare acceptance line.

Withhold the token for a conditional pass, stale input, missing attestation,
untested shell premise, mixed timing/counter evidence, incomplete cleanup,
unmanifested artifact, or any rejection path that can reach a CUDA kernel
before authorization and review checks. Acceptance authorizes no GPU access;
it only opens the already separately authorized first-cycle script.
