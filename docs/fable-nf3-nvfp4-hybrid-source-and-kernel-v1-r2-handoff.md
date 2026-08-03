# Fable handoff: NF3/ModelOpt-NVFP4 source and kernel v1 r2

Date: 2026-08-03

Status: adversarial corrective-design review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4 or read checkpoint payloads. The checked-in audit and
inventory are the external-evidence boundary for this review.

Review candidate commit:
`2b8785907c11d2b58d8c5fa7f782845fae03e3ad`

Required result path:
`fable-nf3-nvfp4-hybrid-source-and-kernel-v1-r2.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`nf3-modelopt-nvfp4-source-kernel-v1-r2-design-accepted`

## Provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Withhold the token for any mismatch or incomplete input.

| Candidate input | SHA-256 |
|---|---|
| `docs/nf3-nvfp4-hybrid-source-and-kernel-v1-r2.md` | `80a055c354971e7ff82b0eeeb54413cd6acafb7ee31611426f3154accc35b2aa` |
| `docs/nf3-nvfp4-hybrid-source-and-kernel-v1.md` | `9b7937f78f400986c50fde236702c361e5a4249641f2de359e3f0e6dcc444e12` |
| `docs/cn4-hybrid-r2-contract-audit-20260803.md` | `2e80f773468ffc89972ea8dbb6dee82b51fec6c0b3f49b319cc2ebf913698573` |
| `docs/cn4-hybrid-source-inventory-20260803.md` | `caa270096611f2acfdfdef5f8cafd743b49d3b675e9779813dc5aeb7c400e247` |
| `docs/nvfp4-fused-routed-moe-v1-r3.md` | `f60d3adb777321ed715ae465abcf20895dbbf470c20970855636ed9b2b3a4db0` |
| `crates/glm-format/src/float.rs` | `e2f547b3ec5efae0d9fdb975136164f557e24a93770a5791c4ca7d7359e7e1de` |
| `crates/glm-format/src/nvfp4.rs` | `af9211c7df2c74b446d234ed215580614ce58c415963a1038eb86df48ad8b11a` |
| `crates/glm-format/src/safetensors.rs` | `4a7d8d4a2121a2257a5e8b7ec531c98b4b83bddb6ea140ade697088a05009594` |
| `kernels/include/glmaxx_kernel.h` | `c5f5ceed453c901a63dfeecea0ec83a53b6485e98c32763650c708c699b56406` |
| `kernels/sm120/nvfp4_routed_fc1.cu` | `67d954f2ba1bf28f0eca30c42ab18c014b19353b4102e89edd7089a1ad9770c5` |
| `kernels/sm120/cutlass_nvfp4_dense_control.cu` | `1ec4abf4fc307f709aa5d2576ac80c84825d75a7eb9db7b11ae036fd67a34541` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

Run `./scripts/local-checks.sh` in that detached candidate and retain the exit
status. Do not use the caller's dirty worktree as evidence.

## Required independent work

1. Re-derive the target/draft tier counts, source component families, shapes,
   TP4 axes, and every rank-local plane length from the pinned metadata.
2. Independently prove NF3 source extraction, BF16 codebook bits, canonical
   E4M3 byte set, and rounding path. Attack signed zero and both NaN spellings.
3. Implement an independent coordinate enumerator for both actual rank shapes.
   Prove every NF3 logical value appears exactly once, no coordinate is out of
   bounds, every three-word pack is invertible, and both scale maps are
   bijections. Do not reuse a proposed inverse as the forward authority.
4. Re-derive ModelOpt low/high nibble order, block scales, outer scalars, and
   W4A16 accumulation/epilogue semantics. Determine whether input scale is
   authentically retained while unused rather than silently selecting W4A4.
5. Inspect the existing 128-byte codec and kernel ABI. Prove it cannot retain
   two fused outer scalars and that codec `0x0102` plus 192-byte metadata closes
   the alias without reinterpreting codec `0x0100`.
6. Independently classify the current CUTLASS control as W4A4 or W4A16 from
   operand types and activation preparation. Attack any path by which its
   evidence could qualify the new profile.
7. Recompute all NF3 and ModelOpt payload/metadata totals and inspect the
   streaming converter, source-route domains, scalar arity, graph binding,
   rank consensus, and nonclaims for a hidden expansion or fallback.

## Decisions

Answer every decision with an unqualified `YES` or `NO`:

1. Is source admission exact and fail-closed for all 75 target layers plus the
   uniform ModelOpt draft layer?
2. Are NF3 source arithmetic, invalid encodings, and the BF16 authority exact?
3. Is the complete NF3 r2 value mapping bijective and bit-unambiguous at both
   actual rank shapes?
4. Are the NF3 and ModelOpt scale address maps exact and bijective?
5. Are ModelOpt source arithmetic, separate gate/up outer scalars, and unused
   but authenticated input scalars precise?
6. Does distinct codec `0x0102` and its 192-byte record prevent alias with the
   one-scale canonical codecs and preserve both FC1 scalars?
7. Are both metadata formats checksum-complete, domain-separated, strict on
   reserved bytes, and sufficient to bind source, tier, geometry, TP, layout,
   plane bytes, and scalar arity?
8. Is conversion bounded, deterministic, and free of a hidden dense/int32
   checkpoint materialization or numerical re-quantization?
9. Are W4A16 production semantics separated from the W4A4 diagnostic in
   numerical policy, ABI, graph identity, quality evidence, and claims?
10. Do route scratch, ordered reduction, rank consensus, and collective
    binding forbid shared-output atomics or rank-local fallback as correctness
    paths?
11. Are every per-expert, target, draft, and rank arithmetic value exact?
12. Does the gate sequence authorize only CPU proof after design acceptance?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, followed
by independent derivations and all twelve decisions. Only if every decision is
`YES`, attest the candidate and all twelve hashes, then end with the requested
token as the only bare acceptance line.
