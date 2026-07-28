# Fable v0.2 adversarial re-review handoff

Date: 2026-07-28

Review target: GLM-5.2-only, SM120-only, TP4 Rust inference engine contract

Implementation status: none

GPU work performed or authorized by these documents: none

## Provenance

The original review is [fable-adversarial.md](../fable-adversarial.md), with
SHA-256
`4a6e6e36d2f226f8fadc6135b910281db1aa9e5d11cded3b6bd43e287e975137`.
The line-by-line response is
[fable-review-disposition.md](fable-review-disposition.md).

The original review pinned:

| File | Reviewed SHA-256 |
|---|---|
| `spec/engine-v0.md` | `fe89ebb87f0b10558630481d863c36542e9ac4b5420aee302a50db70dcf98205` |
| `spec/format-v0.md` | `8edfda4a31ac4bc677814f5e8d78e21892622745b2d040c5eb860201a7406275` |
| `docs/native-engine-plan.md` | `111c2956b5117f102e95fceb64fd1eaadc57ba9669e8b54d5f3fc4e53e0bedf1` |

Re-review these revised files as one contract:

| File | Revision | Candidate SHA-256 |
|---|---:|---|
| [Engine specification](../spec/engine-v0.md) | 0.2.0 | `15c8a9033420828307ebc10d472c70ed739e611f4ed0308b88c42972be1666a3` |
| [Rank/checkpoint and cache format](../spec/format-v0.md) | 0.2.0 | `0e56b4b0cb1c4a64b666022a9d43c4ebc48a4609503d72e25b7526032a19fac6` |
| [Supporting engine plan](native-engine-plan.md) | post-review draft | `91950cea58ca2ff4f64f3588897b7f130288828b87cd34789f5e0bdb825787d1` |

Recompute these hashes at both the start and finish of re-review. The two
specifications are normative. Report a conflict with the supporting plan
rather than choosing an interpretation.

## Conditions incorporated

The revision makes these contract-level changes:

1. The all-NVFP4 serving profile is deleted. Direct block-16 NVFP4 requires
   379.6875 GiB for routed experts alone; even a hypothetical compact 2D
   lower bound reaches 398.813243 GiB after protected tensors, target and
   draft 1M KV, and minimum escrow. NVFP4 remains the M1–M4 laboratory
   backend; EXL3 is mandatory before M5; `hybrid-serve` is the only
   NVFP4-bearing serving profile.
2. MTP verification uses verifier-pass logits as authoritative. Stable
   positions require exact token agreement with MTP0; tie-adjacent
   divergence has an explicit bounded/reporting gate. Version zero does not
   silently assume batch-invariant kernels.
3. Rank-file and conversion identities are content-derived, the in-file
   timestamp is zero, and nondeterministic provenance lives outside canonical
   bytes.
4. The attention-bearing draft layer has a separate sealed KV sidecar,
   totaling 0.359375 GiB at 1M tokens, with atomic target/draft publication,
   restore, commit, and rollback.
5. DCP4 decode transports queries and merges deterministic sparse candidates
   and owner-local FP32 log-sum-exp partials. Packed KV record gather is
   forbidden in production decode. DCP1 and DCP2 are out of v0.
6. Sampling is vocabulary-sharded. Distributed argmax, bounded top-k merge,
   rank-mass/CDF categorical sampling, and residual sampling replace
   full-vocabulary logits gathers.
7. Cache content identity is separated from the HBM attachment ABI so a DCP
   posture change does not discard posture-neutral tier bytes.
8. Graph residency, physical memory budgets, escrow, strong tier integrity,
   volatile DRAM behavior, and NVMe write endurance now have explicit gates.

## Requested decision

Return findings ordered as `BLOCKER`, `MAJOR`, `MINOR`, or `QUESTION`, each
citing a file and section, then answer:

1. Are B1, B2, and B3 resolved as specification-contract blockers?
2. Are all eight major findings resolved or safely moved behind a named gate?
3. May the project begin M1–M2 NVFP4 CPU-reference and codec-proof work?
4. What minimum edits, if any, remain before that CPU-only phase?
5. Which concerns may remain deferred until EXL3, GPU microbenchmarking,
   concurrent serving, or NVMe tiering?

Pay particular attention to the corrected NVFP4 byte arithmetic, the
verifier-authoritative MTP equivalence rule, the draft-sidecar lifecycle,
the DCP4 fixed-order merge, the bounded top-p rule, and fail-closed separation
between cache content identity and attachment ABI.

This re-review must not authorize cn4 GPU work. GPU activity remains an
explicit operator decision after the CPU gate.
