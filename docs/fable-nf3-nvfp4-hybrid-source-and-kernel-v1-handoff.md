# Fable handoff: NF3/NVFP4 hybrid source and kernel contract v1

Date: 2026-08-03

Status: adversarial design review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4, inspect the external evidence tree, or run CUDA for this
review. The checked-in inventory is the review boundary for the earlier
read-only discovery.

Review candidate commit:
`d3a5acd91422845a4665898405f05466763b8525`

Required result path:
`fable-nf3-nvfp4-hybrid-source-and-kernel-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`nf3-nvfp4-hybrid-source-and-kernel-v1-design-accepted`

## Provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Withhold the token for any mismatch or incomplete input.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/nf3-nvfp4-hybrid-source-and-kernel-v1.md` | `bd4606d4335f4450b6f5611f54c95a06077ef4ed644812b4dac111aca1c7e01b` |
| `docs/cn4-hybrid-source-inventory-20260803.md` | `caa270096611f2acfdfdef5f8cafd743b49d3b675e9779813dc5aeb7c400e247` |
| `docs/cn4-experiment-isolation-v1.md` | `aab1dc4860fd2dde21e19b067b211f842387436d3d92a48b2fb31037a945d735` |
| `docs/local-inference-lab-decode-bench-20260803.md` | `cd1dfed287c04ea93af399b56fc74a67213850110ca2f5d9bdb09e17ffd36c77` |
| `crates/glm-format/src/float.rs` | `e2f547b3ec5efae0d9fdb975136164f557e24a93770a5791c4ca7d7359e7e1de` |
| `crates/glm-format/src/nvfp4.rs` | `af9211c7df2c74b446d234ed215580614ce58c415963a1038eb86df48ad8b11a` |
| `crates/glm-format/src/safetensors.rs` | `4a7d8d4a2121a2257a5e8b7ec531c98b4b83bddb6ea140ade697088a05009594` |
| `crates/glm-format/src/checkpoint.rs` | `08450f0cb33e592ec76dfbe655b06580ba1743e60f3109a65218d052dbea406c` |
| `crates/glm-format/src/container.rs` | `2fbe22c55d481e40699a02be9929f9a964f50bc08199853772dd314c85ade47f` |
| `crates/glm-format/src/rank_manifest.rs` | `cb57a81ad3643a86992c4fd9c2166e9ee238cc7e66063732355d14e485f73410` |
| `crates/glm-engine/src/weight.rs` | `d658cefefc17757a28258bafd0e13f5309e8adcbf2b30c4d2bdc97be9899ca19` |
| `docs/hybrid-serving-manifest-v1.md` | `934787ea37a5dbd9b6778844adbeb0b40fd365d4653991fc7cbfe77df3c685cf` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

Run the complete CPU-only local gate and record its exit status:

```text
./scripts/local-checks.sh
```

## Required independent work

Do not accept the discovery prose by inspection alone. Independently:

1. rederive all NF3 and NVFP4 source shapes and component counts from the
   documented global GLM-5.2 geometry and 192:64 membership;
2. brute-force the NF3 little-endian extraction for all eight positions and
   all code values, and independently round the decimal codebook to the stated
   BF16 bits;
3. rederive the TP4 slicing, native fused gate/up and down byte rows, per-expert
   totals, 14,400-expert payload total, and metadata-inclusive total;
4. prove the proposed 12-byte NF3 unit mapping is bijective for the two actual
   rank-local shapes and that fixed `tile_n=256` divides both N dimensions;
5. inspect the current format/container/manifest code to identify every place
   where a distinct NF3 codec/profile will be required and any unsafe alias
   with EXL3 or NVFP4; and
6. verify the benchmark commit, version, and file hash from the public Git
   repository without executing an engine benchmark.

## Required decisions

Answer every decision with an unqualified `YES` or `NO`:

1. Is the 75-layer 192:64 target map plus uniform-NVFP4 layer 78 complete,
   exact, and fail-closed without checkpoint-name inference?
2. Are component families, full shapes, counts, TP4 axes, and rank-local
   boundaries exact for NF3 and ModelOpt NVFP4?
3. Are NF3 source bit order, BF16 codebook, E4M3FN scale semantics, zero and
   invalid encodings precise enough for an independent CPU oracle?
4. Are ModelOpt NVFP4 nibble order, E2M1 values, block scales, distinct
   gate/up outer multipliers, and unused-but-authenticated input scale precise?
5. Does the proposed source-proof matrix remain independent of the native
   packer while covering real tensors and malformed boundaries?
6. Is the streaming conversion bounded, non-expanding, deterministic, and
   incompatible with a hidden dense or int32 checkpoint materialization?
7. Is the fixed-tile native NF3 permutation bijective and sufficiently bound
   to prevent a kernel/layout mismatch or runtime reinterpretation?
8. Is the 192-byte metadata unambiguous, checksum-complete, reserved-byte
   strict, and sufficient to bind codebook, geometry, TP ownership, layouts,
   source route, membership, and exact plane bytes?
9. Are all payload and metadata-inclusive arithmetic values exact and based on
   checked physical bytes rather than average bpw?
10. Does requiring a distinct codec/profile prevent NF3 from masquerading as
    EXL3, NVFP4, or the existing hybrid-serving policy?
11. Is the NF3 tensor-core route numerically anchored to BF16 weights and FP32
    accumulation without a persistent dense plane?
12. Do deterministic codec partitioning, per-route scratch, ordered reduction,
    and rank consensus forbid atomics or rank-local fallback as correctness
    shortcuts?
13. Does the resident-weight/cold-load boundary permit kernel and config hot
    tuning without rereading weights while preserving collective rollback?
14. Does the cn4 isolation contract prevent GLMAXX from mutating or cleaning
    ongoing vLLM work and preserve non-overwriting, hash-complete evidence?
15. Is the Local Inference Lab pin reproducible and separated from later
    benchmark execution and claims?
16. Does the gate sequence obey design review, CPU proof, SM120 microbenchmark,
    layer replay, checkpoint smoke, quality, then matched benchmark?
17. Are all stated nonclaims and the limited post-acceptance implementation
    scope accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, followed
by the independent derivations and all seventeen decisions. Only if every
decision is `YES`, attest the candidate commit and all thirteen exact input
hashes, then end with the requested token as the only bare acceptance line.

Acceptance opens only CPU source parsing, native NF3 plane packing, metadata,
and proof implementation. It does not accept the new complete rank-manifest
profile, the checkpoint, conversion publication, CUDA, quality, capacity,
cold-start, hot-reload, or performance evidence.
