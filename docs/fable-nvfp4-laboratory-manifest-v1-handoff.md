# Fable handoff: NVFP4 laboratory rank manifest and M4 load plan v1

Date: 2026-07-30

Status: adversarial design review requested

Review candidate commit:
`0e084967f6750253c584ca3b0221dbc6de382a30`

Required result path:
`docs/reviews/fable-nvfp4-laboratory-manifest-v1.md`

Requested acceptance token, only if every blocker and major is resolved:
`nvfp4-laboratory-manifest-v1-design-accepted`

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
| `docs/nvfp4-laboratory-manifest-v1.md` | `8a0adb54dedfab1dba0afcf09579614ce567da92fda43134b0c404af5aafb0ee` |
| `docs/small-checkpoint-runner-v1.md` | `720e07e3791ab1c5174aedc9aa449cfe048e6bc1b9d483798c0d83d8319050f6` |
| `docs/nvfp4-fused-routed-moe-v1-r3.md` | `f60d3adb777321ed715ae465abcf20895dbbf470c20970855636ed9b2b3a4db0` |
| `docs/checkpoint-load-transaction-v1.md` | `bc3c938f488bdcbf002c788ce9c5ac493addfe81866f39875d583c4312842ccf` |
| `docs/production-rank-manifest-validation-v2.md` | `542c48d969ddebc40a14aefe269deff85656054ef937053034718650d8eb0f45` |
| `docs/target-program-projection-discriminator-v1.md` | `c8585f4790a33dc98af0246b30de62ea61d6a7a70150b661dc4d0499ea7f50fe` |
| `docs/target-layer-execution-v1.md` | `7f16667d55a50441dbc81b7fe18285be5a6f0fb06c68ef008a40aad8f406f478` |
| `docs/production-punchlist.md` | `239c0e83787e81842ca987477e7a4d04fc77287581a3b26fafd6479dbeebfdaa` |
| `docs/results-index.md` | `4344b2155159aeab808433a5325461efa6fb8d01eb80b34dcf438fddf5d22a42` |
| `crates/glm-format/src/container.rs` | `2fbe22c55d481e40699a02be9929f9a964f50bc08199853772dd314c85ade47f` |
| `crates/glm-format/src/native_reader.rs` | `953a56702ba1ee000f508fe24cbbae7c6137d6496d104720f658fede5572699c` |
| `crates/glm-format/src/nvfp4.rs` | `af9211c7df2c74b446d234ed215580614ce58c415963a1038eb86df48ad8b11a` |
| `crates/glm-format/src/rank_manifest.rs` | `cb57a81ad3643a86992c4fd9c2166e9ee238cc7e66063732355d14e485f73410` |
| `crates/glm-engine/src/checkpoint_load.rs` | `052198f4265ab2569eb19feb074c96b83daa34fa0566575e9517ec59f7ca5957` |
| `crates/glm-engine/src/memory.rs` | `2131c999b6762a9b7e505cfe542c957877d95af4ee04056affa9d677156e9491` |
| `crates/glm-engine/src/weight.rs` | `d658cefefc17757a28258bafd0e13f5309e8adcbf2b30c4d2bdc97be9899ca19` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-nvfp4-laboratory-manifest-v1-handoff.md
git diff --check 0e084967f6750253c584ca3b0221dbc6de382a30^ \
  0e084967f6750253c584ca3b0221dbc6de382a30
```

The handoff and queue metadata are added after the candidate and are not
candidate inputs. This is a source-only design review. Do not connect to a
host, inspect a checkpoint, read model bytes, or reproduce a proposed run.

## Review purpose

Determine whether this separate laboratory schema makes the progressive M4
checkpoint smoke implementable without weakening the strict production
manifest, falsifying file/device byte accounting, or creating a path from a
partial checkpoint to production health or service.

Independently attack:

1. format header membership;
2. source/output/conversion identity;
3. the exact 533-entry subset;
4. rank-invariant layout semantics;
5. file payload versus metadata versus device arenas;
6. laboratory memory-plan identity;
7. load-plan domain separation; and
8. handle/type-state separation from production.

## Review boundary

Acceptance covers only the proposed laboratory manifest, catalog, budget,
load-plan, and type-state design plus the diagnosed protected-header defect.

Acceptance does not implement or accept a format correction, manifest,
converter, budget, loader, checkpoint, M2/M3/M4 result, target program,
kernel, graph, cn4 run, quality result, capacity result, or performance
result. It does not accept the broader small-checkpoint design or pass H04.

## Required adversarial questions

1. Do all twenty candidate-input hashes match at review start and finish in
   a detached worktree?
2. Confirm that the production validator accepts only
   `glmaxx.rank-manifest.v0.2.2`, requires capacity EXL3, and rejects NVFP4
   descriptors and incomplete inventories.
3. Is a distinct `glmaxx.rank-manifest.nvfp4-laboratory.v1` schema necessary
   and sufficient to preserve that fail-closed production behavior?
4. Does format-v0 require bit 3 when any source-precision protected tensor is
   present, while current `derive_header_flags` never sets it?
5. Are M4's exact flags `DIRECT|NVFP4|PROTECTED = 11`, with EXL3 and hybrid
   clear?
6. Does the proposed correction reject both missing and extra protected bits
   across in-memory and streaming readers without accepting two spellings?
7. Do separate source, selected-source, conversion, native-payload, and
   result identities prevent a repack or requantization from masquerading as
   source bytes?
8. Are the five conversion routes closed and is 2D-to-1D correctly gated on
   separate conversion and per-position quality evidence?
9. Can the exact 533-tensor subset be derived from the operation manifest
   without names supplied by the laboratory file becoming authority?
10. Are the 19 layer-6 protected, 256 combined gate/up, 256 down, final norm,
    and sharded head records complete with no omitted executable dependency?
11. Do the FC1/FC2 codec, projection, shape, and layout rules match r3 while
    allowing only an explicitly accepted FC2 1D/2D variant?
12. Is the strict JSON field set canonical, bounded, secret-safe, and
    substitution-resistant?
13. Does every tensor field get compared with the descriptor, canonical
    codec metadata, source binding, subset contract, and conversion policy
    instead of defining physical truth?
14. Is the 192-byte semantic entry exact, versioned separately from the
    production 128-byte entry, and complete enough to bind projection, both
    layout IDs, layout source, and quantization policy?
15. Can all four ranks derive the same catalog while retaining all necessary
    rank-local global-scale, plane, file, source, and arena identities?
16. Re-derive the 1,982,245,376-byte payload and prove every plane has zero
    256-byte weight-arena slack.
17. Re-derive 65,536 file metadata bytes and 130,944 device metadata bytes
    from the retained per-record alignment algorithm.
18. Is it correct that the final device metadata arena is not rounded to
    131,072, and does one-byte-short validation fail before allocation?
19. Re-derive the 137,856-byte four-rank load-plan preimage.
20. Does the separate laboratory budget charge all HBM and host terms without
    inheriting 1M serving capacity or hiding measurement blockers?
21. Can the completed/allowed booleans or profile byte be mutated to create
    production health, serving, or a capacity claim?
22. Does the laboratory-specific plan hash domain prevent profile-byte or
    catalog-version substitution against capacity EXL3?
23. Can `LaboratoryWeightHandle` reach only the M4 executor, with no
    conversion to a production handle, HTTP model, prefix namespace, or
    `HEALTHY` state?
24. Does the CPU proof cover every inventory, identity, arithmetic,
    substitution, failure, cleanup, and schema-dispatch boundary before CUDA?
25. Are every no-format, no-implementation, no-checkpoint, no-cn4,
    no-quality, no-capacity, and no-performance nonclaim accurate?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately whether:

1. the protected-header defect is real and the proposed fail-closed correction
   is complete;
2. source, conversion, output, profile, and review identities are complete;
3. the 533-tensor inventory and per-tensor validation are exact;
4. the 192-byte semantic catalog binds every rank-common execution semantic;
5. payload, file metadata, device weight, device metadata, and plan bytes are
   exact and nonconflated;
6. the laboratory budget is bounded, measurable, and incapable of supporting
   a serving-capacity claim;
7. plan-domain and schema separation prevent capacity/laboratory
   substitution;
8. type state prevents partial-checkpoint production health or service;
9. CPU implementation remains correctly ordered behind review and before
   CUDA M4; and
10. no implementation, checkpoint, device, quality, capacity, or performance
   evidence is implied.

Only if all twenty-five questions and all ten statements are unqualified
`YES`, end with exactly one bare line containing the requested acceptance
token shown above.

Withhold for stale provenance, dual header spellings, source/output identity
conflation, an incomplete subset, caller-controlled descriptor semantics,
catalog drift, incorrect alignment arithmetic, a hidden memory term,
production/laboratory schema or plan collision, a handle/health escape,
incomplete CPU gates, or any implementation/hardware/performance
overstatement.
