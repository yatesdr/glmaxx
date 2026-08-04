# Fable handoff: recurrent-draft K=3 scalar-v1 qualification

Date: 2026-08-04

Status: adversarial implementation/evidence review requested

GPU authorization conveyed by this handoff: none. Review retained cn4 records
read-only if desired; do not launch CUDA.

Review candidate commit:
`6e0c6bfa6a85eda2eae4cd588ebb413e33a9a0b6`

Required result path:
`fable-exl3-recurrent-draft-k3-scalar-v1.md` at the repository root.

Requested token, only if every required answer is an unqualified `YES`:
`exl3-recurrent-draft-k3-scalar-v1-accepted`

## Provenance

Review the exact candidate in an isolated worktree. Hash every input at review
start and finish and withhold the token on any mismatch.

| Input | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `crates/glm-cli/Cargo.toml` | `6ab8d2dbe3033e0944c8bd26b3717d5d0fdf7431f52bf55d00d641bbd0984106` |
| `crates/glm-cli/src/bin/exl3_real_k3_v1.rs` | `39be37583a28701eac8cde5c3df52b25397faf6c3f47f0333cc284758d456ff7` |
| `scripts/cn4-exl3-real-k3-v1.sh` | `b91b5937413a62c46fa4095393c0078a3cd84bcf885c5d5c988dd9914f92b146` |
| `docs/cn4-exl3-recurrent-draft-k3-scalar-v1-20260804.md` | `364f0157c656ab6500062cf821c1d3959ddc1fb71a78d69be5a78dab5ec61889` |
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
/home/derek/glmaxx/evidence/20260804T054043Z-exl3-draft-k3-99ba366-r1
```

Its `artifact-manifest.txt` SHA-256 is
`812aa10a7989350df90537cb06961b78abe9f004ed7098dfdb8acba1e8ec27f3`.
The compact `summary.json` SHA-256 is
`50f014660a86b5dc0ef07b153d2fdefbaa084de57bcb9ebd7491b2a1ca41f0c0`.
Hash-verify every manifest row if cn4 is reachable. Lack of cn4 access is a
provenance limitation and cannot be replaced by trusting the preparation
document.

The retained manifest does not include the originating Docker command,
container-inspect record, mount table, or network-mode record. The result
document's read-only/network-disabled-container sentence is therefore an
unverified operator-setup statement, not independently retained evidence and
not part of the requested acceptance. The manifest does retain the complete
source-shard hash, offline build inputs, source cleanliness, and a script with
no source-write operation; judge only those recorded properties.

## Required decisions

1. Does the runner fail closed unless the caller supplies one explicit,
   complete-hash-matched safetensors shard and the selected projection is K=3
   under the scalar source-projection ABI?
2. Is caller-constructed K=3 metadata acceptable only for this narrow K=3
   projection qualification, with no checkpoint-admission, K=4, or MTP claim?
3. Does every M=1,2,4,8 case compare every FP16 output element against the
   accepted Rust oracle, reject non-finite/mismatched output, and require three
   bitwise-identical device repetitions?
4. Does the run cover layer 78 expert 0 gate/up/down on TP ranks 0 and 3 using
   real hash-pinned source payloads, with zero runtime repack and no persistent
   reconstructed GPU weight?
5. Are the 1,000-sample nearest-rank p50/p95/p99 summaries implemented and
   labeled only as scalar projection controls?
6. Do the script and records prove four SM120 devices, no compute PIDs before
   each launch, isolated paths, clean source before/after, exact source-shard
   identity, no source-write operation in the retained script, and a complete
   manifest that verifies all rows, while making no independent acceptance
   claim for the unrecorded mount/network posture?
7. Are the result document's 24 exact-hash matches, timing table, identities,
   evidence hashes, and K=4 posture accurate?
8. Does the evidence remain explicitly insufficient for MTP execution,
   proposal/verification semantics, TP4 replay, checkpoint smoke, serving,
   quality, KV capacity, or end-to-end performance?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
answer all eight decisions separately. Include the requested token as the only
bare token line only if every answer is `YES`, candidate/input/evidence hashes
match at start and finish, and no blocker or major remains.

Acceptance permits this result to serve only as a recurrent-draft K=3 scalar
projection control for later composition. It does not accept a draft layer or
MTP execution.
