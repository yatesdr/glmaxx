# Fable handoff: EXL3 warp-staged v2 implementation

Date: 2026-08-04

Status: adversarial CUDA implementation and evidence review requested after
the CPU-proof precondition is accepted

Review candidate commit:
`6af8060c17b89b241a786448024093b31a4d504f`

Required result path:
`fable-exl3-warp-staged-v2-implementation.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`exl3-warp-staged-v2-implementation-accepted`

GPU authorization conveyed by this handoff: none

cn4 posture: read-only hash verification of the named evidence directory is
allowed; do not compile, launch CUDA, stop a process, or modify cn4

## Required precondition

First complete the corrected CPU-proof review in
`docs/fable-exl3-warp-staging-cpu-v2-handoff.md`. The authoritative handoff is
published at commit `4824f5aff4870cffdfba96c68bd4d2a990a96229` with SHA-256
`eca5fb5aada09227520693193c3e2f0faf79f5f31decfe0264db8ac1883ce00e`.
It requires result path `docs/reviews/fable-exl3-warp-staging-cpu-v2.md` and
bare token `exl3-warp-staging-cpu-v2-accepted`.

The older CPU handoff present in the implementation candidate is retained in
the candidate hash table for historical reproducibility, but it is superseded
as the acceptance precondition. Withhold this implementation token unless the
corrected review independently returns its exact bare token. Do not treat the
design token or the later GPU result as a substitute.

## Candidate provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish; any mismatch with this table withholds the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |
| `fable-exl3-source-projection-v1-r2.md` | `cac885880345fb2f02e940bcf0cd32420acf5ac8a6a3e34fc76e7971a5aa2964` |
| `fable-exl3-warp-decode-v2-r2.md` | `c26236ec0e57b56d90028edb8396dd5521e5cec75174401d04469eefd33990b5` |
| `docs/exl3-sm120-warp-decode-v2.md` | `67fb3bcb5b839cc50f3462990f1ef6056ca7c9d991851efc5e668fed9d0b3325` |
| `docs/exl3-warp-staging-cpu-proof-v2.md` | `5c77b5721885da708d0240e9eeb6537e9ed74a25a6940cf92e00bc79de494b31` |
| `docs/fable-exl3-warp-staging-cpu-v2-handoff.md` | `56259aaa8f2bc0dcc06c7d49ca9fed8ccb8335aa4c7cbc1e9688131a2b4730a1` |
| `fixtures/exl3-warp-staging-proof-v2.json` | `cdc650dd2c70dcbb8c3cb2e5e5659b42f429dc84a62e32bacb0c629ad66f1f45` |
| `crates/glm-format/src/exl3/warp_proof.rs` | `93dff6fc1e0190efb387e3ee9359bcf196ce6510550f11df73433ac06d34be73` |
| `crates/glm-cli/Cargo.toml` | `6d5bea9f87e29cd1deaf1ddad42ec5dafd1ac96a6e31babb12da12fdc189dcaa` |
| `crates/glm-cli/src/bin/exl3_staged_v2.rs` | `8cfeaad0afbcb71f81f34be7baf439a4b200227ccfc9c7e691d72a54465a8dc4` |
| `crates/glm-cuda/src/abi.rs` | `28905e69300a3a8c8105752ee9aaeb4d718cbe4387cab548139c257242ec68a4` |
| `kernels/include/glmaxx_kernel.h` | `c5f5ceed453c901a63dfeecea0ec83a53b6485e98c32763650c708c699b56406` |
| `kernels/include/glmaxx_exl3_staged_v2.h` | `a874f4b836b526f9de00337e965c32008627a8a8f0397a6a59c47907b7142235` |
| `kernels/sm120/exl3_projection_control.cu` | `dd7d20d439cdb58fb8531b5c03697fbfb44458905acf59abff7f45ef76c52b48` |
| `kernels/CMakeLists.txt` | `9c695447b180e67f49c3c320be1f6b6be99501c661cd479726cb20695ce048c5` |
| `docs/cn4-exl3-warp-staged-v2-8d4c6bd-20260804.md` | `bb5d56ed02a948ec4eeea7a63c5ae9e9151137aed2ce50921ae4e0d6f6f48cda` |
| `scripts/local-checks.sh` | `2d1882be9afd91f4a54c1d3ff9b9f02cd5087357eeb5668d4094c2114c3003ce` |

Run the complete local gate and directly inspect the CUDA source and Rust
harness. A local host without CUDA may verify CPU tests and source properties;
it must not claim to have reproduced the device run.

## cn4 evidence provenance

Read-only evidence directory:

```text
/home/derek/glmaxx/evidence/20260804T043059Z-exl3-staged-v2-8d4c6bd-r1
```

Required top-level manifest SHA-256:
`927e51a6c2e2003035a613de1c8590ec2083fffd83cfb3dbe5cc57c66e40868d`.

On cn4, run `sha256sum --check artifact-sha256.txt` from that directory and
verify the source commit, empty source diff, container digest, exact build
command, toolchain, GPU occupancy records, binary hashes, full SASS, resource
usage, raw samples, summary, terminal record, and post-run process state. The
key pinned artifact hashes are:

| Artifact | SHA-256 |
|---|---|
| `artifact-sha256.txt` | `927e51a6c2e2003035a613de1c8590ec2083fffd83cfb3dbe5cc57c66e40868d` |
| `build/build.log` | `61daf3984919186e04ecc97526fdb0f3db221f3b127856f8df02c4a2f37feb5a` |
| `build/device-image-resources-symbols.txt` | `2a701ff58e0e10af341e75f58445f83dd27cf8021ad5eea06332007eed9cae9a` |
| `build/device-sass.txt` | `59c3b481df7d678a1c2326b0a7ed07f476b45f300fac1bc979e07c3aa1e33ec1` |
| `cases/summary.json` | `6a704799d8430a0d3c982929984c248ee0faf7b8cc819437daf52155de2a4e6b` |
| `run.sh` | `570df3f4a75e801b97b2a3dd1c7303192c07712dc46d9402c805d417512ed121` |
| `libglmaxx_sm120.so` | `46be15d5a53104a979d148f136f24d31653f31fd024b0c6c05a1b092b66eb401` |
| `glmaxx-exl3-staged-v2` | `85166a398e71868f318367fd8c29a463becc450f68cdf8c15f0e85e7bcd7eebd` |

## Required adversarial decisions

1. Does the CUDA implementation exactly realize the accepted 256-thread,
   eight-tile stage, thread-to-word mapping, row ownership, source address,
   cyclic decode, and ascending-K arithmetic without out-of-bounds access?
2. Do all 256 threads reach both barriers for every rows-1-through-8 route?
   Inactive row subwarps must perform no activation loads, decode, arithmetic,
   validation, or stores; this must not be misread to forbid the accepted
   row-independent cooperative trellis loads issued by threads 0 through 191.
3. Are v1 and v2 built in one CUDA translation unit with the same flags, and
   are explicit RN multiply/add and FP16 boundaries preserved without fast
   math, contraction, FTZ, or a changed output rotation?
4. Does the staged entry point repeat all descriptor and SM120 checks, reject
   rows outside 1..8, use only the caller stream, expose a distinct exact ABI,
   and leave scalar v1 independently callable with no fallback?
5. Does the implementation add no persistent reconstructed weights or runtime
   repack and retain the exact v1 workspace boundary?
6. Does the Rust harness own and synchronize every stream, event, and device
   allocation safely; compare every FP16 output element; verify both device
   validation words; and retain all 1,000 interleaved samples per route?
7. Are percentile and speedup calculations exact, are warmups excluded, and
   are scalar and staged launches matched for inputs, rotations, workspace,
   build, stream, and event method?
8. Does the cubin contain real SM120 SASS and both entry points, and do the
   reported 64 registers, zero local/stack bytes, and 1,792 shared bytes match
   the resource record?
9. Do all twelve retained reports prove bitwise equality and zero validation
   errors, with p50 speedups ranging from 1.893x to 2.345x?
10. Does the evidence script fail closed on source/review/hash/GPU-occupancy
    drift, refuse overwrite, and finalize both success and failure records
    without touching non-GLMAXX state?
11. Does the result document explicitly disclose the gate-order error and
    keep the evidence informational rather than using it to promote the route?
12. Are the remaining nonclaims exact: no real checkpoint payload, profiler
    counters, grouped routing, TP4 layer, model quality, serving, or KV result?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then answer
each decision separately. Only if the corrected CPU-proof precondition is
accepted, all candidate and evidence hashes match, and all twelve decisions
are unqualified `YES`, end with the requested bare token. Withhold it for stale
provenance, an unreviewed CPU proof, arithmetic or address drift, ineffective
comparison, unsafe lifetime, unmatched timing, non-SM120 code, missing raw
evidence, gate-order concealment, or an overstated claim.
