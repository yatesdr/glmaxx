# Fable handoff: cn4 TP4 memory baseline 2026-08-03

Date: 2026-08-03

Status: diagnostic implementation and evidence review requested

GPU authorization conveyed by this handoff: none

cn4 posture: read-only hash verification of the named GLMAXX evidence path is
permitted; do not launch a GPU, container, build, model, or storage operation

Review candidate commit:
`28993cf858357ee5b697e8fca1f94d2136c6e233`

Required result path: `fable-cn4-tp4-memory-baseline-20260803.md` at the
repository root.

Requested acceptance token, only if every blocker and major is resolved:
`cn4-tp4-memory-baseline-20260803-diagnostic-accepted`

## Provenance

Review the exact commit in a detached worktree. Hash every input at start and
finish and withhold the token for any mismatch.

| Input | SHA-256 |
|---|---|
| `docs/cn4-tp4-memory-baseline-20260803.md` | `2f4eed1617919427045bcc101c5a6e1a1b68d97b626ee889cd3ac4dbb71971fc` |
| `docs/hybrid-mtp3-capacity-ledger-v1.md` | `465af873bd388166beee5e84e3d2d4272f9501813bdfe95c1e5a3c05b062c2a8` |
| `docs/cn4-hybrid-source-inventory-20260803.md` | `caa270096611f2acfdfdef5f8cafd743b49d3b675e9779813dc5aeb7c400e247` |
| `docs/nf3-nvfp4-hybrid-source-and-kernel-v1.md` | `bd4606d4335f4450b6f5611f54c95a06077ef4ed644812b4dac111aca1c7e01b` |
| `crates/glm-cli/src/main.rs` | `381a74d0ef7311a95a2c5996be80b39eb76442489edcaf8a2f934beaf00cf518` |
| `crates/glm-cuda/src/ffi.rs` | `2a76ad51cb1c9b28a508dc4734bfeb6b6ad46103c3b437ec8e8ff8f6a6ff2f31` |
| `kernels/sm120/nvfp4_routed_fc1.cu` | `67d954f2ba1bf28f0eca30c42ab18c014b19353b4102e89edd7089a1ad9770c5` |
| `kernels/CMakeLists.txt` | `9c695447b180e67f49c3c320be1f6b6be99501c661cd479726cb20695ce048c5` |
| `docs/results-index.md` | `4f294d59338c8a96ae04448ff91f0c0b827814470fd9f9a34952d214b4ecefc1` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

Raw evidence path:

```text
/home/derek/glmaxx/evidence/20260803T162522Z-memory-baseline-36584b0
```

The SHA-256 of its sorted `evidence-sha256.txt` is
`5f52baf8af7d0ca8060825962884696eafbe0e29680cd1aab642a138d49d9d41`.
Hash-verify every listed raw record over read-only SSH if cn4 is available.
An unavailable host must be reported as an evidence-verification limitation,
not replaced by the checked-in prose.

## Required independent work

1. Audit the Rust thread/barrier path. Confirm that all four exact SM120
   contexts and streams remain alive during every `cudaMemGetInfo` call, the
   call occurs on the owner thread with the correct current device, and rank
   disagreement or an error cannot produce a success report.
2. Trace the native path from `glmaxx_device_bind` through stream creation,
   synchronization, and `glmaxx_device_memory_info`. Confirm that no GLMAXX
   device kernel or user allocation is launched and that the report does not
   mislabel CUDA/runtime-owned memory as a measured module or graph charge.
3. Independently verify the source commit/status, container, toolchains,
   CUTLASS revision, input hashes, binary/library hashes, `sm_120f` cubin,
   before/after GPU state, empty post-run compute process list, and exact
   commands from the raw evidence.
4. Parse all five JSON samples independently. Confirm their full-file hashes
   are identical, every rank identity is stable, rank 3 is the minimum at
   101,367,742,464 bytes, and `summary.json` contains no aggregation error.
5. Re-derive the weight/cache subtotal, post-context residual, seven old
   non-context/non-staging terms, and 474,189,824-byte provisional remainder.
   Check units and ensure the result remains subordinate to native alignment,
   module, graph, collective, workspace, fragmentation, full allocation, and
   escrow evidence.
6. Look for any contamination of vLLM/production worktrees, images,
   containers, caches, ports, checkpoints, or evidence. Verify every mutable
   path is under `/home/derek/glmaxx`.
7. Confirm the checked-in result claims only a stable post-context diagnostic,
   not hybrid fit, physical KV capacity, model execution, quality, latency,
   throughput, or a qualified kernel.

## Decisions

Answer each `YES` or `NO`:

1. Does the Rust/native implementation measure four simultaneous owner-thread
   SM120 contexts without a GLMAXX kernel or user device allocation?
2. Are all five samples and the minimum 101,367,742,464-byte result authentic,
   stable, and correctly summarized?
3. Are the build, source, environment, command, and post-run cleanup records
   complete and hash-consistent for this diagnostic scope?
4. Is the 474,189,824-byte sensitivity remainder exact and explicitly
   provisional rather than a fit or capacity pass?
5. Is the cn4 work isolated from every production/vLLM resource named by the
   repository contract?
6. Does the result avoid implying module/graph residency, physical KV,
   checkpoint, quality, or performance evidence?
7. May this diagnostic be used as the post-context input to the separately
   reviewed hybrid MTP3 CPU planner and later full physical-capacity gate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, followed
by independent derivations and all seven decisions. Only if every decision is
an unqualified `YES`, attest all ten hashes and the raw evidence-list hash,
then end with the requested token as the only bare acceptance line.
