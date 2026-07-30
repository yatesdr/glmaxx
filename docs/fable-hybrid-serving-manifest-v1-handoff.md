# Fable handoff: hybrid serving weight policy and rank manifest v1

Date: 2026-07-30

Status: adversarial design review requested

Review candidate commit:
`67db1a33de774762f724ca8157fccb1a0d689e4d`

Required result path:
`docs/reviews/fable-hybrid-serving-manifest-v1.md`

Requested acceptance token, only if every blocker and major is resolved:
`hybrid-serving-manifest-v1-design-accepted`

GPU, host, process, container, network, model, checkpoint, conversion, or
storage authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4, query its state, start or stop a process,
build, test, create a CUDA context, access a checkpoint, convert weights, or
launch work for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if needed. Hash every candidate input at review start and finish.
Any mismatch must withhold the token as stale.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `spec/engine-v0.md` | `277a97c675c5021b1e310146bdf04896ccec9dea312a73a188379e633423e6d8` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `docs/hybrid-serving-manifest-v1.md` | `934787ea37a5dbd9b6778844adbeb0b40fd365d4653991fc7cbfe77df3c685cf` |
| `docs/nvfp4-fused-routed-moe-v1-r3.md` | `f60d3adb777321ed715ae465abcf20895dbbf470c20970855636ed9b2b3a4db0` |
| `docs/nvfp4-laboratory-manifest-v1.md` | `8a0adb54dedfab1dba0afcf09579614ce567da92fda43134b0c404af5aafb0ee` |
| `docs/target-program-projection-discriminator-v1.md` | `c8585f4790a33dc98af0246b30de62ea61d6a7a70150b661dc4d0499ea7f50fe` |
| `docs/target-layer-execution-v1.md` | `7f16667d55a50441dbc81b7fe18285be5a6f0fb06c68ef008a40aad8f406f478` |
| `docs/quality-acceptance-v1.md` | `3f87cd128b633d6812dce31fb6f3bfbd700debae587a32350e0cb46e24a6e1e9` |
| `docs/checkpoint-load-transaction-v1.md` | `bc3c938f488bdcbf002c788ce9c5ac493addfe81866f39875d583c4312842ccf` |
| `docs/production-rank-manifest-validation-v2.md` | `542c48d969ddebc40a14aefe269deff85656054ef937053034718650d8eb0f45` |
| `docs/native-rank-load-plan-proof-v1.md` | `19558f87ef912c8ead99c31cf0f1a1867dcc384ab79d8efbbee96f66abfe0e63` |
| `docs/sm120-rank-executor-v1-r2.md` | `4f40ea7652858b4cebbe4093dc81149cb30aa26bedc69edef72fa627c987df89` |
| `docs/step-execution-abi-v3.md` | `1cde3bcabba0a0d861691b06ddb140cb64dfbefaab1129c8a04bc302c0ce609e` |
| `docs/production-punchlist.md` | `77c7af8a34425008d35c3b81baf09617e95a6ef472b9f68018b0cbfc797dcc6b` |
| `docs/results-index.md` | `d8a2448bd53df880a6b3fdc8e6c7fd525c995a723f1538ec99ac4b0aa9e1354e` |
| `crates/glm-format/src/container.rs` | `2fbe22c55d481e40699a02be9929f9a964f50bc08199853772dd314c85ade47f` |
| `crates/glm-format/src/checkpoint.rs` | `08450f0cb33e592ec76dfbe655b06580ba1743e60f3109a65218d052dbea406c` |
| `crates/glm-format/src/native_reader.rs` | `953a56702ba1ee000f508fe24cbbae7c6137d6496d104720f658fede5572699c` |
| `crates/glm-format/src/nvfp4.rs` | `af9211c7df2c74b446d234ed215580614ce58c415963a1038eb86df48ad8b11a` |
| `crates/glm-format/src/exl3.rs` | `f6fa1b25311d78e13e22a0c7c908da7abca636948218fef1987c89850e974edb` |
| `crates/glm-format/src/rank_manifest.rs` | `cb57a81ad3643a86992c4fd9c2166e9ee238cc7e66063732355d14e485f73410` |
| `crates/glm-engine/src/weight.rs` | `d658cefefc17757a28258bafd0e13f5309e8adcbf2b30c4d2bdc97be9899ca19` |
| `crates/glm-engine/src/memory.rs` | `2131c999b6762a9b7e505cfe542c957877d95af4ee04056affa9d677156e9491` |
| `crates/glm-engine/src/checkpoint_load.rs` | `052198f4265ab2569eb19feb074c96b83daa34fa0566575e9517ec59f7ca5957` |
| `crates/glm-engine/src/startup.rs` | `1a5f1ac8aae94e6eb2aaf2cf4701dfc290604013103eacc1423046211609a5fc` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-hybrid-serving-manifest-v1-handoff.md
git diff --check 67db1a33de774762f724ca8157fccb1a0d689e4d^ \
  67db1a33de774762f724ca8157fccb1a0d689e4d
```

The handoff and queue metadata are added after the candidate and are not
candidate inputs. This is a source-only design review. Do not connect to a
host, inspect a checkpoint, fetch or read model bytes, run a converter, or
reproduce a proposed measurement.

## Review purpose

Determine whether the candidate supplies a fit-capable full-serving
EXL3/NVFP4 boundary without reinterpreting the capacity-EXL3 manifest,
allowing physically impossible gate/up combinations, conflating source
planes with aligned file/device bytes, forming a policy/budget hash cycle, or
letting an unmeasured policy reach conversion or production health.

Independently attack:

1. expert-atomic policy and physical naming;
2. source, conversion, quality, selection, and output identity;
3. variable full-checkpoint inventory and byte arithmetic;
4. strict schema and header flags;
5. rank-common catalog versus rank-local physical identity;
6. 1M/MTP6 budget closure;
7. load-plan domain and type-state separation; and
8. all-rank startup, cache, graph, and request immutability.

## Review boundary

Acceptance covers only the proposed WeightPolicy.v2, hybrid rank-manifest,
semantic-catalog, finalized-budget, load-plan, and production type-state
design plus the hybrid amendment to engine-v0.

Acceptance does not implement or accept a policy selector, format
correction, converter, manifest, catalog, budget, builder, loader, kernel,
graph, checkpoint, model execution, source-equivalence result, quality
result, fit result, 1M result, capacity result, performance result, serving
health, or cn4 use. It does not pass M1 through M7 or any H04–H07 gate.

## Required adversarial questions

1. Do all twenty-eight candidate-input hashes match at review start and
   finish in a detached worktree?
2. Is `WeightPolicy.v1` genuinely unusable as the hybrid physical contract
   because it independently selects gate/up/down, aliases NVFP4 1D and 2D,
   and charges two records for combined FC1?
3. Does v2 make gate/up expert-atomic while still retaining separate gate and
   up quality evidence, and does it allow exactly the three stated down
   choices?
4. Are all offsets and lengths in the 256-byte
   `ExpertPhysicalPolicy.v2` exact, nonoverlapping, and canonical?
5. Does the physical-realization digest cover exactly bytes 0 through 47
   without self-reference, while the manifest record digest covers the
   complete 256-byte record?
6. Are all offsets and lengths in the 256-byte WeightPolicy header exact,
   and is the canonical body exactly 4,980,992 bytes before the domain
   prefix?
7. Are 75 target sparse layers plus recurrent draft layer 78, 256 experts,
   `N=19,456`, 58,368 capacity routed records, 1,217 protected records, and
   59,585 total capacity records exact?
8. Do `F`, `D1`, and `D2` have sufficient bounds, and does the membership
   rule require both EXL3 and NVFP4 without pretending that this weak rule
   proves fit?
9. Are the canonical split gate/up, combined gate_up, and down physical names
   unique, deterministic, source-independent, and sufficient to derive
   profile-local tensor IDs?
10. Can the compiled operation manifest plus policy derive every physical
    tensor without trusting names or counts supplied by the rank file?
11. Does the sorted source set identify exactly one protected authority and
    require accepted logical-base provenance/equivalence for every other
    source checkpoint form?
12. Are source slices, source logical components, conversion routes, native
    planes, selected policy, and output tensor hashes distinct and
    substitution-resistant?
13. Are the six closed conversion routes sufficient, including distinct
    retained 1D/2D down routes and a separately reviewed 2D-to-1D
    requantization route?
14. Does deterministic selection consume per-position full-vocabulary
    evidence for every logical role, actual-shape inclusive SM120 timing,
    the pinned hot-expert distribution, exact physical bytes, and immutable
    objective/tie-break inputs?
15. Is the selection receipt acyclic because it commits its inputs and
    selected record stream but excludes the final header and policy digest?
16. Is the policy/budget/manifest chain acyclic: selection consumes a
    constraint envelope, manifests bind that envelope, the finalized budget
    binds the manifests and policy, and the load plan binds the finalized
    budget?
17. Does the strict hybrid schema remain distinct from capacity-EXL3 and M4
    laboratory schemas, rejecting unknown fields, alternate spellings,
    nondeterministic identity, and profile substitution?
18. Are the exact hybrid header flags
    `NVFP4|EXL3|PROTECTED|HYBRID = 30`, with direct clear because source EXL3
    is present, and is the protected-header correction correctly a
    prerequisite rather than an accepted implementation?
19. Does every tensor record bind and cross-check projection, codec, both
    layout IDs, source/conversion/policy identities, exact plane lengths,
    hashes, and descriptor semantics instead of defining physical truth?
20. Re-derive `EXL3=3N-2F-D`, `NVFP4=F+D`, routed descriptors `3N-F`, and
    `T=59,585-F`.
21. Re-derive protected tensor-plane bytes 11,959,396,352 and the rank
    tensor-plane formula
    `81,590,319,104 + 1,153,016F + 576,508D`.
22. Re-derive file codec metadata
    `5,603,328 - 64F + 32D`.
23. Re-derive the aligned native payload-region/device-weight formula
    `81,605,027,840 + 1,152,512F + 576,256D`, including the 252-byte EXL3
    successor gap and unrounded final protected tensor.
24. Does the schema keep tensor-plane bytes, aligned file-payload-region
    bytes, file metadata bytes, device-weight bytes, and device-metadata
    bytes distinct even where two exact totals are equal?
25. Re-derive device metadata `(R-1)*256+L`, including the authenticated
    final metadata-bearing descriptor and the 96/128-byte last-record cases.
26. Is the 224-byte hybrid semantic entry exact at offsets 0 through 223 and
    complete enough to distinguish projection, value/scale layout,
    representation source, quantization policy, and expert policy?
27. Can all four ranks derive byte-identical catalogs while retaining every
    necessary rank-local source range, plane/global-scale hash, file offset,
    arena offset, manifest, payload, and device identity?
28. Does the completed budget measure every weight, metadata, context,
    module, graph, workspace, collective, staging, target/draft KV/indexer,
    page-table, journal, allocator, fragmentation, and emergency-escrow term
    independently per rank?
29. Can host DRAM, pinned-host memory, NVMe, aggregate HBM, or live free-memory
    observations hide or compensate for a failing 1,048,576-position MTP6
    rank budget?
30. Re-derive the load-plan preimage
    `1,408 + 256T = 15,255,168 - 256F` and confirm the hybrid-specific domain
    prevents capacity/laboratory collision.
31. Does the design accurately state that current Rust builders reject
    hybrid and that a separate implementation is still required?
32. Can `ProductionWeightHandle::Hybrid` exist only after four-rank
    verification and adoption, while production `HEALTHY` remains after all
    program/cache/page-table/graph/collective/known-answer consensus gates?
33. Do graph, step, prefix, cache, namespace, result, and serving identities
    bind the exact policy/catalog/plan so a request cannot mutate membership
    or trigger rank-local fallback?
34. Does any validation, adoption, graph, collective, cache, or startup
    disagreement abort all ranks without capacity/laboratory downgrade or
    partial-model continuation?
35. Does the required CPU proof cover every encoding, inventory, source,
    selection, arithmetic, catalog, schema, budget, transaction, health, and
    namespace mutation before conversion or CUDA?
36. Are every no-policy, no-conversion, no-checkpoint, no-device, no-quality,
    no-fit, no-capacity, no-speed, and no-serving-health nonclaim accurate?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately whether:

1. WeightPolicy.v2 is expert-atomic, byte-exact, and non-self-referential;
2. source, conversion, quality, selection, policy, and native-output
   identities are complete and acyclic;
3. the variable physical inventory and canonical names are exact;
4. tensor-plane, aligned file-region, file-metadata, device-weight,
   device-metadata, and load-plan arithmetic are exact and nonconflated;
5. the strict manifest and exact flags fail closed without weakening
   capacity-EXL3 or laboratory schemas;
6. the 224-byte catalog binds all rank-common execution semantics while
   retaining every rank-local identity;
7. the finalized four-rank budget truthfully requires 1M/MTP6 capacity and
   cannot be rescued by another memory tier or aggregate free HBM;
8. plan-domain, schema, and handle types prevent cross-profile substitution;
9. adoption cannot publish production health before every later startup
   receipt;
10. requests, graphs, caches, and prefixes cannot change policy membership;
11. all-rank failure and no-fallback rules are complete;
12. the CPU implementation gate is complete and correctly precedes
    conversion and CUDA; and
13. no implementation, checkpoint, device, quality, fit, capacity,
    performance, or serving evidence is implied.

Only if all thirty-six questions and all thirteen statements are
unqualified `YES`, end with exactly one bare line containing the requested
acceptance token shown above.

Withhold for stale provenance, mixed gate/up backends, 1D/2D aliasing,
source/output conflation, policy or budget hash cycles, caller-authoritative
inventory, incorrect flags, plane/file/device byte conflation, alignment or
final-record error, rank-common/rank-local identity loss, an omitted 1M/MTP6
budget term, cross-profile plan or type escape, premature health, rank-local
fallback, incomplete CPU gates, or any conversion/hardware/quality/capacity/
performance overstatement.
