# Fable handoff: SM120 W4A16/NF3 fused-MoE execution v1 r2

Date: 2026-08-03

Status: corrective adversarial design review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4, read a checkpoint, compile CUDA, launch a GPU, or modify
any runtime resource. This is a design, ABI, and arithmetic review only.

Review candidate commit:
`10f83f6862c6c345573ebef5bece69d95f4c58fc`

Required result path:
`fable-sm120-w4a16-nf3-fused-moe-v1-r2.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`sm120-w4a16-nf3-fused-moe-v1-r2-design-accepted`

The v1 handoff is superseded and must not issue its token.

## Provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Withhold the token for any mismatch or incomplete input.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/sm120-w4a16-nf3-fused-moe-v1-r2.md` | `311d1214ad57e97c7bab45069fae5507602c0e21922b1fde677ba129e734f265` |
| `docs/sm120-w4a16-nf3-integration-audit-20260803.md` | `cf3c768977c40b8a73e0c1acc5ad7d6d424cc7d235802e5215776957e4531861` |
| `docs/sm120-w4a16-nf3-fused-moe-v1.md` | `4a1f0b4af0977e659f3382795f4c98f50f6664fe648f92ff9fc3688db95e09e3` |
| `docs/nf3-nvfp4-hybrid-source-and-kernel-v1-r2.md` | `80a055c354971e7ff82b0eeeb54413cd6acafb7ee31611426f3154accc35b2aa` |
| `docs/nf3-nvfp4-native-rank-manifest-v1-r2.md` | `ee81b6fc50a9a948af48aaf60d992fceb4c1bcb0c1687ac8cbe6f133a15baf9a` |
| `docs/target-layer-execution-v1-r2.md` | `808da35c2e54eb5692512996650839fb6f127cb91658603eb2fb5ce049c56ed2` |
| `docs/mtp-layer-execution-v1.md` | `5ad5bf01cdbd5e183b5e50aa0940344b5aabc09bf05a90c57d58e3e5b28dd3a7` |
| `docs/sm120-rank-executor-v1-r2.md` | `4f40ea7652858b4cebbe4093dc81149cb30aa26bedc69edef72fa627c987df89` |
| `docs/sm120-rank-executor-native-abi-v1.h` | `0d0f0357a17eba4e678d5c82da4dbff552e292fb7948931496a4382289ae4d6e` |
| `docs/resident-weight-runtime-generation-v1.md` | `ec76be8698ab53480ede07044bdfa73c8ccd9bbf391771bc728569c3023ef8b1` |
| `docs/resident-tensor-device-binding-proof-v1.md` | `15a21ae2ff24758d2f115540a895191f7aeb2acf13c31f2972dcb0700adbab6d` |
| `docs/hybrid-mtp3-capacity-ledger-v1-r2.md` | `6efeee90addafb4f8d645610bf617f1f4dd9b1bd630096f570193f407c49c9c6` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

Run the complete CPU-only local gate and record its exit status:

```text
./scripts/local-checks.sh
```

## Required independent work

Do not accept r2 by prose inspection alone. Independently:

1. encode every 16-, 88-, 256-, 320-, 192-, and 160-byte record and verify all
   offsets, sizes, alignments, reserved bytes, and digest self-exclusions;
2. enumerate every legal and illegal projection/codec/value-layout/scale-layout
   combination, including ModelOpt scalar arity, NF3 zero scalar slots, and
   protected zero layouts;
3. rebuild target-program v3 and MTP-program v2 preimages and prove old
   layout-free domains cannot collide or enter a hybrid graph;
4. independently derive all 19,456 semantic records, tier-local IDs, locators,
   table bytes, and target/draft program assignments;
5. model four rank-local arena layouts and CUDA addresses, proving common
   semantic/plan/step bytes match while local materialization digests differ
   and remain ordered receipts rather than consensus values;
6. trace every production pointer from an adopted executor span through the
   rank owner and graph node; find any path for a request/coordinator pointer,
   direct launch, allocation, codec selection, or fallback to bypass it;
7. generate skewed mixed-tier routes and prove canonical compacted bytes remain
   global-expert/token/slot while the active-expert execution view may be
   codec-partitioned without changing ranges or reductions;
8. rederive the M8 and M3072 live charges, map each subspan to classes 19, 21,
   22, 27 or graph status, and search graph phases for overlap, early reuse,
   double charging, or an uncharged opaque workspace;
9. compare FP32 routed reduction, protected shared output, the one BF16 rank
   partial, the single MLP TP4 sum, and the residual boundary against the
   target-layer precision contract; and
10. model ten compatible runtime generations and prove persistent table bytes,
    all weight pointers, and every model-open/read/staging/H2D counter remain
    unchanged while module/graph materializations change.

## Required decisions

Answer each with an unqualified `YES` or `NO`:

1. Does r2 close all seven audit findings without weakening the W4A16/NF3
   numerical or quality policy?
2. Is the 16-byte target/MTP binding sufficient to bind every physical hybrid
   representation and reject every alias?
3. Are target-program v3, MTP-program v2, and HybridResidentWeightSetIdentity
   v2 domain-separated, complete, and noncircular?
4. Is the 88-byte semantic stream rank-invariant while still sufficient to
   derive and validate every local pointer?
5. Is the persistent table independent of replaceable module/runtime
   generations and exactly 2,573,312 bytes per rank?
6. Are common plan and step receipts free of rank, handle, allocation-base,
   and CUDA-address bytes?
7. Are rank materializations owner-only, span-derived, graph-profile-bound,
   and correctly excluded from TP4 equality claims?
8. Does the production path remain entirely behind the native executor graph
   node and forbid direct diagnostic entry points from `execute_bound`?
9. Does compacted execution preserve the target layer's exact logical order
   while still enabling codec-specialized launches?
10. Are direct and tiled workspace arithmetic, status lifetime, graph classes,
    and no-double-charge rules exact for every bucket?
11. Is the routed/shared BF16 boundary consistent with one and only one MLP
    TP4 reduction and the later residual?
12. Can a local validation failure retain fixed graph and collective posture
    while preventing any bad output publication?
13. Can compatible hot reload change modules/graphs without changing or
    reuploading binding-table or weight bytes?
14. Are MTP0 and MTP3 both bound to the resident layer-78 identity without
    claiming MTP execution before its separate gates?
15. Do the CPU and later SM120 gates cover all new identities, record bytes,
    route views, live ranges, precision boundaries, and failure paths?
16. Does the gate sequence obey `AGENTS.md`, and are all authorization and
    nonclaim statements accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, followed
by all independent encodings/models/derivations and the sixteen decisions.
Only if every decision is `YES`, attest the candidate commit and all thirteen
input hashes, then end with the requested token as the only bare acceptance
line.

Acceptance opens only the r2 CPU record, identity, routing, workspace,
materialization, and numerical-boundary proof after all prerequisite design
tokens also exist. It does not accept CUDA, cn4 use, a checkpoint, resident
weights, target/MTP execution, quality, capacity, reload, cold start, latency,
throughput, or serving.
