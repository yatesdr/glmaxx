# Fable handoff: fused NVFP4 routed-MoE v1

Date: 2026-07-30

Status: adversarial design review requested

Review candidate commit:
`803ae518424aef00d98e09e73b940f6b2c9832ca`

Required result path:
`docs/reviews/fable-nvfp4-fused-routed-moe-v1.md`

Requested acceptance token, only for an unqualified pass:
`nvfp4-fused-routed-moe-v1-design-accepted`

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch GPU/container work for this
review

## Provenance

Review in a detached worktree and hash every input at start and finish.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/nvfp4-fused-routed-moe-v1.md` | `a190a4546b2f3cc6506eaea7d6ffc2cb8de345e08c25365a66312511425a3083` |
| `docs/nvfp4-physical-abi.md` | `d5a189ae06f8f39e828400e589fbd31f94f0245cb90488881369d6a806bd6d1e` |
| `crates/glm-format/src/nvfp4.rs` | `af9211c7df2c74b446d234ed215580614ce58c415963a1038eb86df48ad8b11a` |
| `crates/glm-format/src/container.rs` | `2fbe22c55d481e40699a02be9929f9a964f50bc08199853772dd314c85ade47f` |
| `crates/glm-cuda/src/abi.rs` | `28905e69300a3a8c8105752ee9aaeb4d718cbe4387cab548139c257242ec68a4` |
| `crates/glm-reference/src/routed_fc1.rs` | `55709be01710d08f5b13de22afdb24884d233cdfab1c5de132ad22edd304f40b` |
| `crates/glm-reference/src/routed_fc2.rs` | `4f34f5b89cd542f096269a7442da5289900d1e831e32fcf83462560dc410a40d` |
| `kernels/include/glmaxx_kernel.h` | `a7ddb56de39dbd22e25184be1a2a767dd43bc3ca5ecafd3dcc771aedebbdcf13` |
| `kernels/sm120/cutlass_nvfp4_dense_control.cu` | `1ec4abf4fc307f709aa5d2576ac80c84825d75a7eb9db7b11ae036fd67a34541` |
| `kernels/sm120/cutlass_nvfp4_fc2_control.cu` | `dba61fd6bc34b659543f1b64a329603ab01406a1505954ae16bc9626b8f7ff94` |
| `benchmarks/sm120-fc1-matrix-v1.json` | `d5a0286d0c9d06ce1036085d4f1712d929273fb533601806b26b9f774c360e74` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `docs/production-punchlist.md` | `db721e747e22898c38766f0c23f39b46396630d0b53e2b55a4f3ea0f6a3b9e2b` |
| `scripts/local-checks.sh` | `56f728cdf3f047f9633509a57341d25a977efa802f0d5b371c9716830517db59` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-nvfp4-fused-routed-moe-v1-handoff.md
git diff --check 803ae518424aef00d98e09e73b940f6b2c9832ca^ \
  803ae518424aef00d98e09e73b940f6b2c9832ca
```

## Review boundary

Review only the production kernel/layout design. Acceptance permits a
separate CPU byte-permutation/route/epilogue proof. It does not accept or
implement layout `0x1202`, modify a checkpoint, accept a CUDA kernel, authorize
cn4, pass C06, establish all-NVFP4 fit, or claim quality/performance.

## Required adversarial questions

1. Do all candidate hashes match twice in a detached worktree?
2. Is the diagnosed `0x1201` gate/up epilogue problem real: paired values are
   512 columns apart and cannot be safely fused by ordinary independent N
   tiles without a global boundary or a custom paired mainloop?
3. Is `gate c -> 2c`, `up c -> 2c+1` bijective over 1,024 rows?
4. Does applying the existing SFB formula to physical row preserve complete
   scale coverage, byte count, and each row's values/scales?
5. Can `0x1201` transform to `0x1202` and invert byte-exactly without
   requantization, runtime repack, or changed global scales?
6. Are the new layout discriminator, four-rank agreement, offline conversion,
   control retention, and mixed-layout rejection sufficient?
7. Does the three-kernel per-expert scan produce stable expert/token/slot
   order, exact empty experts, no floating atomics, and rank-identical
   compaction?
8. Is `O(256*rows*8)` explicitly a measured bounded design rather than an
   unproven speed claim?
9. Does expert-local activation quantization preserve the accepted
   global/block/E2M1 arithmetic and canonical 128-row-padded SFA slabs?
10. Can the FC1 epilogue own adjacent gate/up fragments locally, apply both
    global scales, exact SiLU/final BF16 rounding, and write no 1,024-column
    intermediate without cross-CTA synchronization?
11. Does FC2 apply global scales and route weight after projection, write each
    validated `(token,slot,hidden)` destination once, and reduce slots 0..7
    deterministically without floating atomics?
12. Is the FP32 slot-partial workspace arithmetic explicit and correctly
    classified as a memory-plan term rather than hidden storage?
13. Are decode/verify/prefill variant choices rank-common and immutable, with
    no route-local fallback?
14. Are all per-step buffers preallocated and all aliasing constrained by
    nonoverlapping lifetimes?
15. Do retained CUDA-core/dense/grouped/BF16 controls permit matched
    correctness and performance comparisons despite the row permutation?
16. Does the CPU proof independently catch pair, scale-row, nibble, padding,
    route-order, SFA-boundary, epilogue-ownership, reducer, and workspace
    defects?
17. Does the SM120 gate cover every required row/route regime, intermediate
    boundary, graph behavior, resource counter, and timing component?
18. Is quality protected by exact arithmetic first and separate evidence for
    any approximate activation or reduced partial precision?
19. Does the contract avoid implying that an all-NVFP4 checkpoint fits or
    that a 4-bit path already beats EXL3?
20. Are the C06, implementation, CUDA, cn4, quality, capacity, and speed
    exclusions accurate?

## Required answer

Return `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION` findings, then answer:

1. `0x1202` is a sound offline byte permutation and format discriminator;
2. deterministic compaction and expert-local quantization are complete;
3. FC1 paired epilogue and FC2 slot reduction have no cross-CTA race;
4. workspace/variant/allocation rules are bounded and rank-common;
5. controls plus CPU/SM120 gates are adequate; and
6. acceptance opens only CPU proof, not C06 or any performance claim.

Only if all twenty questions and six statements are unqualified `YES`, end:

```text
nvfp4-fused-routed-moe-v1-design-accepted
```

Withhold for stale provenance, nonbijective layout, changed quantization,
runtime repack, unfusable epilogue, route nondeterminism, SFA mismatch,
cross-CTA race, floating atomic reduction, hidden workspace, rank-local
variant choice, inadequate gates, fit/speed overclaim, or scope leakage.
