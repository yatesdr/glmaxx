# Fable handoff: fused NVFP4 routed-MoE v1 r3

Date: 2026-07-30

Status: adversarial corrective-integration design review requested

Review candidate commit:
`bcc8ebf0b951516acb63ebf2baea1825018bbed8`

Required result path:
`docs/reviews/fable-nvfp4-fused-routed-moe-v1-r3.md`

Requested acceptance token, only if every blocker and major is resolved:
`nvfp4-fused-routed-moe-v1-r3-design-accepted`

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
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `docs/nvfp4-fused-routed-moe-v1-r2.md` | `f1cfa4e613f8a6ea959a57d30e4643bc65f9c2f3c2d6acef87b4b3d171b37e80` |
| `docs/nvfp4-fused-routed-moe-v1-r3.md` | `f60d3adb777321ed715ae465abcf20895dbbf470c20970855636ed9b2b3a4db0` |
| `docs/target-program-projection-discriminator-v1.md` | `c8585f4790a33dc98af0246b30de62ea61d6a7a70150b661dc4d0499ea7f50fe` |
| `docs/nvfp4-physical-abi.md` | `d5a189ae06f8f39e828400e589fbd31f94f0245cb90488881369d6a806bd6d1e` |
| `docs/small-checkpoint-runner-v1.md` | `720e07e3791ab1c5174aedc9aa449cfe048e6bc1b9d483798c0d83d8319050f6` |
| `docs/target-layer-execution-v1.md` | `7f16667d55a50441dbc81b7fe18285be5a6f0fb06c68ef008a40aad8f406f478` |
| `docs/production-rank-manifest-validation-v2.md` | `542c48d969ddebc40a14aefe269deff85656054ef937053034718650d8eb0f45` |
| `docs/production-punchlist.md` | `43938ef310f56a4ec962a7d86e279818fda4396dc4087eb8e754452ba4d464b2` |
| `docs/results-index.md` | `a1a84c9e2ee2266315e06f42e3bd1449fe6a93f59d097c2184dcb87f70982ea3` |
| `crates/glm-format/src/nvfp4.rs` | `af9211c7df2c74b446d234ed215580614ce58c415963a1038eb86df48ad8b11a` |
| `crates/glm-format/src/container.rs` | `2fbe22c55d481e40699a02be9929f9a964f50bc08199853772dd314c85ade47f` |
| `crates/glm-format/src/rank_manifest.rs` | `cb57a81ad3643a86992c4fd9c2166e9ee238cc7e66063732355d14e485f73410` |
| `crates/glm-engine/src/weight.rs` | `d658cefefc17757a28258bafd0e13f5309e8adcbf2b30c4d2bdc97be9899ca19` |
| `crates/glm-engine/src/checkpoint_load.rs` | `052198f4265ab2569eb19feb074c96b83daa34fa0566575e9517ec59f7ca5957` |
| `crates/glm-reference/src/routed_fc1.rs` | `55709be01710d08f5b13de22afdb24884d233cdfab1c5de132ad22edd304f40b` |
| `crates/glm-reference/src/routed_fc2.rs` | `4f34f5b89cd542f096269a7442da5289900d1e831e32fcf83462560dc410a40d` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-nvfp4-fused-routed-moe-v1-r3-handoff.md
git diff --check bcc8ebf0b951516acb63ebf2baea1825018bbed8^ \
  bcc8ebf0b951516acb63ebf2baea1825018bbed8
```

The handoff and queue metadata are added after the candidate and are not
candidate inputs. This is a source-only design review. Do not connect to a
host, inspect a live runtime, read a checkpoint, or reproduce a proposed
device run.

## Review purpose

Independently determine whether:

1. r2's row permutation is valid for canonical 1D scales but unsound as a
   blanket claim for canonical 2D block-16x16 scale replicas;
2. r3's 1D-only fused FC1 boundary closes that defect without weakening
   quality or silently requantizing;
3. exact logical-policy-to-physical-descriptor realization closes current
   codec, gate/up, count, and byte-accounting gaps;
4. the 16-byte target record prevents an incompatible layout from selecting
   a fused graph; and
5. laboratory/hybrid execution remains fail-closed while the only accepted
   production manifest is capacity-EXL3-only.

## Review boundary

Acceptance covers r3's codec/layout restriction, source conversion cases,
policy-v2 physical realization, exact byte arithmetic, layout-bound target
record, authenticated derivation, manifest prerequisites, M4 scoping, and
revised CPU/SM120 gates. R2's retained routing, activation, epilogue, and
workspace corrections are in scope where r3 inherits them.

Acceptance does not implement or accept a format, converter, policy,
manifest, compiler, descriptor, kernel, checkpoint, graph, quality result,
fit result, capacity result, or performance result. It does not authorize
cn4 or any live action and does not pass C06 or C08.

## Required adversarial questions

1. Do all twenty candidate-input hashes match at review start and finish in
   a detached worktree?
2. Reproduce from the pinned codec that 2D rows `16t..16t+15` require equal
   per-K-group scales.
3. For distinct gate scale `a` and up scale `b`, does r2 map logical rows
   `0,512,1,513,...` into physical rows `0..15`, producing
   `a,b,a,b,...` and violating the current 2D replica rule?
4. Is the defect independent of the scale-plane byte swizzle and visible
   even when all value bytes and scale locations are copied losslessly?
5. Is limiting combined `0x1202` FC1 to `CODEC_NVFP4_1D` sufficient and
   fail-closed?
6. Does r3 correctly avoid claiming that a future separately identified
   interleaved 2D layout is impossible?
7. Are the three source-conversion cases complete? Can any 2D source be
   mislabeled as a byte-permuted 1D output?
8. Does a reviewed 2D-to-1D requantization necessarily carry a distinct
   conversion policy and quality evidence rather than inherit r2's
   unchanged-quantization claim?
9. Does the current weight backend enum fail to distinguish 1D and 2D, and
   can current policy construction admit mixed gate/up backends?
10. Must combined NVFP4 gate/up be one expert-atomic physical realization
    while down remains independently selectable?
11. Are split EXL3, combined NVFP4-1D, and down realization records complete
    and canonically hashable?
12. Independently rederive every payload, metadata, total, 128-byte delta,
    and 5,308,672-byte all-NVFP4 expert figure.
13. Does charging physical descriptors once fix the current conservative but
    inexact `3*1,769,600` policy arithmetic without allowing hidden bytes?
14. Does the exact policy still reject all-NVFP4 full-model serving and
    require both EXL3 and NVFP4 in a hybrid profile?
15. Is the revised target binding exactly 16 bytes at the stated offsets,
    with every reserved/layout byte hashed?
16. Do zero EXL3 layout IDs, combined `0x1202/0x1202`, and down
    `0x1201/0x1201` form a complete closed table?
17. Can any `0x1201` combined tensor, torn layout pair, shape-only inference,
    or caller-supplied ID enter the fused serving graph?
18. Are authenticated codec metadata, exact layout-source digest,
    quantization-policy digest, projection, role, policy realization, and
    four-rank record consensus sufficient derivation authority?
19. Do the target-program v2 domain and downstream graph/step/resident
    bindings prevent v1/v2 collision or captured-graph reuse after a layout
    mutation?
20. Confirm from pinned Rust that `glmaxx.rank-manifest.v0.2.2` has only
    `CapacityExl3` and explicitly rejects every NVFP4 descriptor.
21. Does r3 correctly leave that validator unchanged and require distinct
    reviewed laboratory and hybrid schemas before either branch is reachable?
22. Are the laboratory prerequisites sufficient to keep the 533-tensor M4
    subset from reaching production health or masquerading as capacity EXL3?
23. Re-derive the M4 1,358,954,496 routed payload and 65,536 routed metadata
    bytes; are payload and metadata consistently distinguished?
24. Are the hybrid prerequisites sufficient to derive variable descriptor
    membership/count from one exact policy while rejecting v1, all-NVFP4,
    incomplete protected inventory, and rank divergence?
25. Do the revised CPU and SM120 gates independently catch 2D permutation,
    codec/layout, physical-count, byte, record, manifest, graph, and fallback
    defects before any speed claim?
26. Are every no-format, no-implementation, no-manifest, no-checkpoint,
    no-cn4, no-quality, no-fit, no-capacity, and no-speed nonclaim accurate?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately whether:

1. the 2D counterexample is real and the fused FC1 1D restriction closes it;
2. source conversion cannot silently change codec, layout, quantization, or
   quality identity;
3. policy v2 distinguishes exact codecs, enforces gate/up realization, and
   charges exact physical bytes;
4. the 16-byte target record and authenticated derivation bind the exact
   physical layout to graph selection;
5. v1/v2 target and graph identities cannot collide;
6. the current capacity manifest remains correctly closed to NVFP4 and M4;
7. laboratory and hybrid prerequisites expose rather than conceal their
   missing manifest/load-plan work;
8. M4 payload/metadata arithmetic and scope are exact;
9. retained r2 routing/numerical/workspace corrections plus r3 CPU/SM120
   additions form a sufficient next gate; and
10. no implementation, manifest, device, checkpoint, quality, fit, capacity,
    or performance evidence is implied.

Only if all twenty-six questions and all ten statements are unqualified
`YES`, end with exactly one bare line containing the requested acceptance
token shown above.

Withhold for stale provenance, a false 2D counterexample, an unsupported
codec/layout path, silent requantization, mixed gate/up realization, inexact
physical charging, a target record that can select the wrong graph,
caller-derived layout identity, current-manifest leakage, an under-specified
laboratory/hybrid prerequisite, M4 arithmetic drift, an incomplete proof
gate, or any implementation/hardware/performance overstatement.
