# Fable handoff: fused NVFP4 routed-MoE v1 r2

Date: 2026-07-30

Status: adversarial corrective-design review requested

Review candidate commit:
`2afb205f7cfe5c90cdeac1262996b9fb9df0f726`

Required result path:
`docs/reviews/fable-nvfp4-fused-routed-moe-v1-r2.md`

Requested acceptance token, only if every blocker and major is resolved:
`nvfp4-fused-routed-moe-v1-r2-design-accepted`

GPU, host, process, container, network, model, checkpoint, or storage
authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4, query its current state, start or stop a
process/container, build, test, create a CUDA context, access a checkpoint,
or launch work for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if needed. Hash every candidate input at review start and finish.
Any mismatch must withhold the token as stale.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/nvfp4-fused-routed-moe-v1.md` | `a190a4546b2f3cc6506eaea7d6ffc2cb8de345e08c25365a66312511425a3083` |
| `docs/nvfp4-fused-routed-moe-v1-r2.md` | `f1cfa4e613f8a6ea959a57d30e4643bc65f9c2f3c2d6acef87b4b3d171b37e80` |
| `docs/nvfp4-physical-abi.md` | `d5a189ae06f8f39e828400e589fbd31f94f0245cb90488881369d6a806bd6d1e` |
| `docs/prefill-graph-profile-abi-v2.md` | `37154c9e31109acdf35a382c6be87b3a865e2b7f6ae8f801969526789dd41f91` |
| `docs/target-layer-execution-v1.md` | `7f16667d55a50441dbc81b7fe18285be5a6f0fb06c68ef008a40aad8f406f478` |
| `docs/production-punchlist.md` | `169a3f05662db9f7183709c4b9c5efb3a9f33fc54c3f11cecb4d67d7dfab6079` |
| `docs/results-index.md` | `dcfea404dc8ca58a10495c7d60ac0c6b95d256448b16ec6e76f9f815a86b21bf` |
| `crates/glm-format/src/nvfp4.rs` | `af9211c7df2c74b446d234ed215580614ce58c415963a1038eb86df48ad8b11a` |
| `crates/glm-format/src/container.rs` | `2fbe22c55d481e40699a02be9929f9a964f50bc08199853772dd314c85ade47f` |
| `crates/glm-cuda/src/abi.rs` | `28905e69300a3a8c8105752ee9aaeb4d718cbe4387cab548139c257242ec68a4` |
| `crates/glm-reference/src/routed_fc1.rs` | `55709be01710d08f5b13de22afdb24884d233cdfab1c5de132ad22edd304f40b` |
| `crates/glm-reference/src/routed_fc2.rs` | `4f34f5b89cd542f096269a7442da5289900d1e831e32fcf83462560dc410a40d` |
| `kernels/include/glmaxx_kernel.h` | `a7ddb56de39dbd22e25184be1a2a767dd43bc3ca5ecafd3dcc771aedebbdcf13` |
| `kernels/sm120/nvfp4_routed_fc1.cu` | `9786c68795c6e75f148192a49aec4a6845e81e3c5e1df1e561163da34204eb28` |
| `kernels/sm120/nvfp4_routed_fc2.cu` | `b72fff75bf4b0ee0ef06bf65286bad73678e4d396b2bdaad72bc784da738bb31` |
| `kernels/sm120/cutlass_nvfp4_dense_control.cu` | `1ec4abf4fc307f709aa5d2576ac80c84825d75a7eb9db7b11ae036fd67a34541` |
| `kernels/sm120/cutlass_nvfp4_fc2_control.cu` | `dba61fd6bc34b659543f1b64a329603ab01406a1505954ae16bc9626b8f7ff94` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-nvfp4-fused-routed-moe-v1-r2-handoff.md
git diff --check 2afb205f7cfe5c90cdeac1262996b9fb9df0f726^ \
  2afb205f7cfe5c90cdeac1262996b9fb9df0f726
```

The handoff and queue metadata are added after the candidate and are not
candidate inputs. This is a source-only design review. Do not connect to a
host, inspect a live runtime, read a checkpoint, or reproduce a proposed
device run.

## Review purpose

Determine whether r2 fully and correctly repairs the seven pre-implementation
gaps found by independently re-deriving v1 against the retained Rust
references, current native ABI, CUDA controls, graph profile, and target-layer
contract.

Attack especially:

1. physical/logical FC1 row identity across values, SFB, metadata, and the
   projection discriminator;
2. dense top-8 validity, route receipt ownership, and rank-common execution;
3. compact value rows versus independently expert-padded SFA rows;
4. validation/status visibility before any route-derived address;
5. exact activation quantization order and canonical encodings;
6. paired FC1 fragment ownership and SiLU rounding boundaries;
7. unweighted FC2 projection followed by slot-ordered weighted FMA;
8. every workspace maximum and production row ceiling; and
9. production ABI separation from development scratch aliases.

## Review boundary

Acceptance covers only the r2 layout, routing, quantization, epilogue,
workspace, ABI, CPU-proof, and later SM120-gate design.

Acceptance does not implement or accept layout `0x1202`, a converter,
container/manifest revision, route compiler, CPU proof, native descriptor,
CUDA kernel, checkpoint, cn4 run, quality result, capacity result, latency,
throughput, or production health. It does not establish all-NVFP4 physical
fit and does not pass C06.

## Required adversarial questions

1. Do all twenty candidate-input hashes match at review start and finish in
   a detached worktree?
2. Are all seven diagnosed v1 gaps real when checked independently against
   the pinned Rust, CUDA, graph, and target-layer sources?
3. Is the `0x1202` forward map a bijection over all 1,024 FC1 physical rows,
   and is the stated inverse exact?
4. Does copying each complete value row and every independently addressed
   SFB location preserve all value/scale bytes without requantization?
5. Do `value_layout_id=0x1202`, `scale_layout_id=0x1202`, the combined
   projection discriminator, and changed layout-source digest prevent a
   one-plane or split/combined identity lie?
6. Is keeping FC2 at `0x1201` complete and unambiguous?
7. Does the dense token-major `R*8` route contract reject missing slots,
   out-of-range experts, duplicate experts, nonfinite weights, negative
   finite weights, and negative zero while permitting canonical positive
   zero?
8. Is the route receipt byte encoding complete, canonical, and bound to the
   exact step, layer, immutable buffer generation, rows, slots, experts, and
   raw weight bits?
9. Does upstream four-rank receipt agreement plus immutable generation,
   fixed compaction, and status checks safely replace v1's second per-layer
   digest collective? Is the compacted digest correctly retained only as
   qualification evidence?
10. Do the validate, count, prefix, and scatter stages have complete
    ownership and deterministic expert/token/slot ordering without an
    ordering-dependent atomic?
11. Is stream-ordered status visibility sufficient to prevent any malformed
    route from causing a data-dependent out-of-bounds read or write?
12. Are compact assignment-major value rows and exact grouped `M=count[e]`
    sufficient to prevent reads into another expert or dead bucket capacity?
13. Is only SFA independently expert-padded, with shared padded-row offsets
    and complete zeroed padding?
14. Is `P_max=A_bucket+127*min(A_bucket,256)` a tight safe maximum for all
    route distributions, including empty, singleton, maximally skewed, and
    all-active experts?
15. Does the fixed activation arithmetic preserve the accepted global,
    block-16 E4M3, and E2M1 policy without reassociation, contraction,
    fast-math, FTZ, negative-zero, or zero-code ambiguity?
16. Can the paired FC1 epilogue prove local ownership of `2*c` and `2*c+1`
    for every selected tile, with no global gate/up intermediate or
    cross-CTA synchronization?
17. Are the FC1 global-scale, ordinary `expf`, SiLU, multiply, and BF16
    rounding boundaries specified tightly enough for a CPU proof and later
    device qualification without falsely demanding host/device
    transcendental bit identity?
18. Does writing unweighted FP32 `slot_projection` and applying route weights
    only in an ascending-slot `fma_rn` reducer match the retained control?
19. Can any missing, stale, duplicated, or unwritten slot reach the reducer
    under the production profile?
20. Is combining routed and shared-expert FP32 partials before exactly one
    TP4 reduction consistent with the target-layer contract?
21. Re-derive every listed byte term and the maxima for `R=1,32,128,448,3072`.
    Do independent alignment charges and build-generated CUTLASS workspace
    close every hidden-storage route?
22. Is the 3,072-row/24,576-assignment production ceiling consistent with
    the graph-profile contract, and does the design reliably reject the
    development 65,536-row posture?
23. Are dedicated production ABI planes, explicit alignment/capacity, fixed
    profile identity, and no simultaneous semantic alias sufficient to keep
    current development scratch aliases out?
24. Does the revised CPU gate independently mutation-test every layout,
    route, code, rounding, capacity, overlap, and one-byte-short boundary
    before CUDA implementation?
25. Does the revised SM120 gate require actual generated-address, fragment
    ownership, status, workspace, graph-replay, resource, correctness, and
    timing evidence without implying that evidence already exists?
26. Are all no-implementation, no-cn4, no-checkpoint, no-quality, no-fit,
    no-capacity, and no-speed nonclaims accurate?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately whether:

1. the dual-plane `0x1202` FC1 identity is bijective, byte-preserving, and
   substitution-resistant;
2. dense top-8 validation, receipt ownership, rank agreement, and four-stage
   deterministic compaction are complete;
3. compact values and independently padded SFA are unambiguous, bounded, and
   safe at every expert distribution;
4. activation quantization arithmetic and canonical encodings are exact;
5. paired FC1 ownership and its numerical boundary are implementable;
6. FC2 unweighted projection and slot-ordered FMA match the retained control;
7. production row ceilings and all workspace charges are exact and complete;
8. the production ABI excludes scratch aliasing and hidden workspace;
9. the CPU and later SM120 gates can independently catch the diagnosed
   defects; and
10. no implementation, device, checkpoint, quality, fit, capacity, or
    performance evidence is implied.

Only if all twenty-six questions and all ten statements are unqualified
`YES`, end with exactly one bare line containing the requested acceptance
token shown above.

Withhold for stale provenance, an incorrect row permutation, inconsistent
value/scale identity, incomplete route validity, a digest/receipt ownership
gap, nondeterministic compaction, unsafe status visibility, padded-value
ambiguity, changed quantization or epilogue rounding, hidden workspace,
scratch aliasing, a wrong production ceiling, an incomplete proof gate, or
any implementation/hardware/performance overstatement.
