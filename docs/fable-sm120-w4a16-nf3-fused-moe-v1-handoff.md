# Fable handoff: SM120 W4A16/NF3 fused-MoE execution v1

Date: 2026-08-03

Status: superseded by the v1 r2 handoff; do not review or issue the v1 token

Use `docs/fable-sm120-w4a16-nf3-fused-moe-v1-r2-handoff.md`.

GPU authorization conveyed by this handoff: none

Do not connect to cn4, read a checkpoint, launch CUDA, or modify any runtime
resource. This is a design and arithmetic review only.

Review candidate commit:
`fc5786dde5f88bc1f99efa8dd4c883f35b750c7e`

Required result path:
`fable-sm120-w4a16-nf3-fused-moe-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`sm120-w4a16-nf3-fused-moe-v1-design-accepted`

## Provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Withhold the token for a mismatch or incomplete input.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/sm120-w4a16-nf3-fused-moe-v1.md` | `4a1f0b4af0977e659f3382795f4c98f50f6664fe648f92ff9fc3688db95e09e3` |
| `docs/nf3-nvfp4-hybrid-source-and-kernel-v1-r2.md` | `80a055c354971e7ff82b0eeeb54413cd6acafb7ee31611426f3154accc35b2aa` |
| `docs/nf3-nvfp4-native-rank-manifest-v1-r2.md` | `ee81b6fc50a9a948af48aaf60d992fceb4c1bcb0c1687ac8cbe6f133a15baf9a` |
| `docs/target-layer-execution-v1-r2.md` | `808da35c2e54eb5692512996650839fb6f127cb91658603eb2fb5ce049c56ed2` |
| `docs/resident-weight-runtime-generation-v1.md` | `ec76be8698ab53480ede07044bdfa73c8ccd9bbf391771bc728569c3023ef8b1` |
| `docs/cn4-w4a16-nf3-kernel-source-audit-20260803.md` | `d2ea066383bc17f69847714a1641d7343c3860287b25cbf2aa37574b1fef7b76` |
| `docs/hybrid-mtp3-capacity-ledger-v1-r2.md` | `6efeee90addafb4f8d645610bf617f1f4dd9b1bd630096f570193f407c49c9c6` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

Run the complete CPU-only local gate and record its exit status:

```text
./scripts/local-checks.sh
```

## Required independent work

Do not accept the design by prose inspection alone. Independently:

1. build a byte-level encoder for the 256-byte table header, 64-byte layer
   directory, 4-byte locator, 128-byte binding, 256-byte plan, 192-byte step,
   and 16-byte work entry; verify every offset, size, alignment, reserved byte,
   and digest self-exclusion rule;
2. rederive the 76-layer/19,456-expert binding-table counts and exact
   2,573,312-byte rank charge;
3. simulate tensor-ID-interleaved resident payloads and prove every FC1/FC2
   value, scale, and metadata pointer can be materialized without moving a
   weight or assuming tier-contiguous payloads;
4. generate adversarial target/draft tier maps and routes, then prove direct
   token/slot order, compacted codec/expert/token/slot order, locator bounds,
   and slot-ordered reduction remain deterministic and rank invariant;
5. independently rederive the direct M8 and tiled prefill M3072 workspace
   ledgers, including separate 256-byte plane rounding, and search every graph
   phase for a live-range alias or premature tile reuse;
6. compare the W4A16 arithmetic with the official PTX ISA valid-combination
   table and the pinned source audit; determine whether any native BF16 by
   E2M1 block-scaled instruction was incorrectly assumed;
7. trace ModelOpt gate/up/down scalar arity from authenticated metadata through
   binding, FC1/FC2 epilogues, and ordered route reduction;
8. model one rank failure at validation, FC1, FC2, reducer, generation prepare,
   quiesce, and publication, looking for rank-local fallback or mixed
   generation; and
9. charge the binding table and maximum workspace into the pending hybrid
   capacity ledger without treating arithmetic as physical fit evidence.

## Required decisions

Answer each with an unqualified `YES` or `NO`:

1. Does the design consume exactly the r2 W4A16/NF3 numerical policies without
   borrowing W4A4 evidence or inventing an NF3 global scalar?
2. Are the resident binding table and all four record layouts complete,
   byte-exact, deterministic, and safe for actual tensor-ID arena order?
3. Are address-free logical receipts correctly separated from rank-local CUDA
   address materialization and its digest?
4. Do target and draft tier counts, locator widths, canonical ordering, and all
   table-byte arithmetic hold at every boundary?
5. Are ModelOpt gate and up outer scalars provably distinct through the ABI and
   applied before SwiGLU, with down applied before route weighting?
6. Can direct and compacted work streams validate every route before pointer
   use and remain identical on all ranks without a rank-local fallback?
7. Do the separate graph phases avoid every illegal inter-CTA synchronization,
   output race, floating atomic, and changed reduction boundary?
8. Is FC2 tiling numerically identical to the canonical slot-ordered FMA while
   bounding prefill scratch and forbidding premature tile reuse?
9. Are both workspace totals and the forbidden untiled slot-plane total exact,
   overflow-safe, and independently charged?
10. Are plan and step descriptors sufficient to reject shape, byte-length,
    pointer, generation, digest, workspace, and reserved-byte drift before
    launch?
11. Is compatible runtime replacement possible without rereading, moving, or
    retransferring weights, while mixed-generation execution remains
    impossible?
12. Do the CPU and SM120 gates test actual GLM-5.2 shapes, M1/M4 priorities,
    mixed tiers, spill/resource evidence, determinism, and phase-separated
    timing before checkpoint smoke?
13. Does the design preserve the MTP3 KV-capacity gate by charging its new
    metadata/workspace terms rather than claiming unmeasured fit?
14. Does the gate sequence obey `AGENTS.md`, and are every authorization and
    nonclaim accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, followed
by the independent encodings/models/derivations and all fourteen decisions.
Only if every decision is `YES`, attest the candidate commit and all eight
input hashes, then end with the requested token as the only bare acceptance
line.

Acceptance opens only implementation of the reviewed CPU binding, work-stream,
workspace, and ABI proofs after all prerequisite design tokens also exist. It
does not accept CUDA, cn4 use, a checkpoint, resident weights, quality, KV
capacity, hot reload, cold start, latency, throughput, or serving.
