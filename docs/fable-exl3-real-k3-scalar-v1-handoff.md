# Fable handoff: real TR3 K=3 scalar-v1 qualification

Date: 2026-08-04

Status: adversarial implementation/evidence review requested

GPU authorization conveyed by this handoff: none. Review retained cn4 records
read-only if desired; do not launch CUDA.

Review candidate commit:
`7c90d66560217e4d2b18763059a1e1ac71c4bff3`

Required result path:
`fable-exl3-real-k3-scalar-v1.md` at the repository root.

Requested token, only if every required answer is an unqualified `YES`:
`exl3-real-k3-scalar-v1-accepted`

## Provenance

Review the exact candidate in an isolated worktree. Hash every input at review
start and finish and withhold the token on any mismatch.

| Input | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `crates/glm-cli/Cargo.toml` | `6ab8d2dbe3033e0944c8bd26b3717d5d0fdf7431f52bf55d00d641bbd0984106` |
| `crates/glm-cli/src/bin/exl3_real_k3_v1.rs` | `39be37583a28701eac8cde5c3df52b25397faf6c3f47f0333cc284758d456ff7` |
| `scripts/cn4-exl3-real-k3-v1.sh` | `b6718d3c04ddfc5facdda1f8d23134e0143c412865d2caf94a7f5329ff234404` |
| `docs/cn4-exl3-real-k3-scalar-v1-20260804.md` | `48ca42b88c43e94dd6f939d925ea11d229b10c6b4de94028a77ca271897275ca` |
| `docs/cn4-tr3-k3-real-cpu-proof-20260804.md` | `a00f62ee5bae846b0b2de295e7964ffbb30db88a550aabf41170daca825fdd2f` |
| `fable-exl3-source-projection-v1-r2.md` | `cac885880345fb2f02e940bcf0cd32420acf5ac8a6a3e34fc76e7971a5aa2964` |
| `docs/exl3-trellis-cpu-contract.md` | `7c5bfa0795aa6b78646b00b029f8d4edc7c15491d69ea19e423f770701b5cdc3` |
| `docs/exl3-sm120-source-projection.md` | `20e4f6007969d42d78aba586e0a2b2496fc19483cd94563ed366bcd3e1b2b389` |
| `crates/glm-format/src/exl3.rs` | `f6fa1b25311d78e13e22a0c7c908da7abca636948218fef1987c89850e974edb` |
| `crates/glm-format/src/safetensors.rs` | `f15097989389dc8eebfad95bf7aa71977f1a43d5688c2c87273b047a2876149e` |
| `crates/glm-cuda/src/abi.rs` | `28905e69300a3a8c8105752ee9aaeb4d718cbe4387cab548139c257242ec68a4` |
| `crates/glm-cuda/src/ffi.rs` | `2a76ad51cb1c9b28a508dc4734bfeb6b6ad46103c3b437ec8e8ff8f6a6ff2f31` |
| `kernels/sm120/exl3_projection_control.cu` | `241730ceaf629d01101629cb3f107e8d13fe92019444f4b635f9aa1d8cbc819d` |
| `kernels/include/glmaxx_kernel.h` | `c5f5ceed453c901a63dfeecea0ec83a53b6485e98c32763650c708c699b56406` |

The external evidence root is:

```text
/home/derek/glmaxx/evidence/20260804T052506Z-exl3-real-k3-e8e4593-r1
```

Its `artifact-manifest.txt` SHA-256 is
`e41a282967c2ce747d08888fe3b59c6a5e3c5eec936a1c91cdea7e38c7d4ad61`.
The compact `summary.json` SHA-256 is
`561aa20a00f5d592ec326b38c7da39957ea13a355d44e122d679b840d99e3f59`.
Hash-verify every manifest row if cn4 is reachable. Lack of cn4 access is a
provenance limitation and cannot be replaced by trusting the preparation
document.

## Required decisions

1. Does the harness fail closed unless the ABI is exactly scalar source
   projection v1, the source is one explicit safetensors file, its complete
   SHA-256 matches, and the tensor validates as K=3?
2. Is use of caller-constructed K=3 metadata acceptable only for this
   K=3-only qualification, with no checkpoint-admission or K=4 claim?
3. Does every M=1,2,4,8 case compare every FP16 output element against the
   accepted Rust oracle, require three bitwise-identical device repetitions,
   and fail before reporting a pass on any mismatch or non-finite value?
4. Are source components, inputs, CPU outputs, GPU outputs, upload bytes,
   zero-repack posture, toolchain/source identities, and raw timing sample
   counts represented without silently reconstructing a persistent dense
   matrix on the GPU?
5. Is nearest-rank p50/p95/p99 implemented correctly over the 1,000 retained
   CUDA-event samples, and are the timing results labeled only as scalar
   controls rather than an optimized-route or end-to-end claim?
6. Does the run cover gate/up/down, rank 0/rank 3, and M=1/2/4/8 on a real
   hash-pinned K=3 expert, while the real K=4 expert fails closed at its
   trellis contract?
7. Do the script and records prove exact four-SM120 inventory, zero compute
   PIDs before each launch, isolated worktree/build/evidence paths, clean
   source before/after, read-only checkpoint/source mounts, and a complete
   post-run evidence manifest?
8. Are the result document's 24 exact-hash matches, timing table, evidence
   hashes, K=4 exclusion, and remaining-gate statements accurate?
9. Does this evidence close only the real K=3 scalar projection control,
   leaving K=4, staged implementation, TP4 replay, checkpoint smoke, quality,
   serving, KV capacity, MTP, and end-to-end performance unaccepted?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
answer all nine decisions separately. Include the requested token as the only
bare token line only if every answer is `YES`, candidate/input/evidence hashes
match at start and finish, and no blocker or major remains.

Acceptance permits this run to serve as the real-K3 scalar control for later
matched K3/K4 qualification. It does not accept or authorize any broader
engine milestone.
