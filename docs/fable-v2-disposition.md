# Fable v2 re-review disposition

Date: 2026-07-28

Review: [fable-adversarial-v2.md](../fable-adversarial-v2.md)

Review SHA-256:
`f0019b96d5b35bdca6d026691629b56fbeb0c3c4528e1ae4ff9c1aa06817953e`

Reviewed revision: engine/format v0.2.0

Review-condition amendment: engine/format v0.2.1

Phase-A implementation amendment: engine/format v0.2.2 (independent review
pending)

GPU work authorized by the review: none

## Verdict accepted

Fable found no blocker and explicitly allowed M1 NVFP4 CPU-reference and
codec-proof work to begin. B1, B2, and B3 were re-derived and accepted. The
v0.2.1 amendment applies the one required pre-CPU edit and the recommended
riders. It does not treat the review as cn4 authorization.

## Finding dispositions

| Finding | Disposition |
|---|---|
| V2-M1 indexer key data has no home | Resolved as a mandatory cached 132-byte record: 128 E4M3 key bytes plus one FP32 power-of-two scale. The 21 full-indexer layers are manifest data. HBM uses target-page owner/generation; DRAM/NVMe use a mandatory paired sidecar. The 1M term is 2,906,652,672 bytes aggregate and is explicit in the budget. |
| V2-M2 DCP4 short-context collective chain | The coordinator may encode an owner subset when all step sequences have zero pages on omitted owners. Candidate exchange runs only on full-indexer layers and winners feed the IndexShare group. Sequential layer dependencies prevent cross-layer query/partial-return batching in v0. M3 has an exclusive collective-chain ledger line. |
| V2-m1 lower bound wording | Remaining parameters are labeled an estimate and priced at the 0.5625-byte NVFP4 1D floor. The new indexer store is included; the optimistic total remains above nominal HBM. |
| V2-m2 hidden budget terms | Model metadata, all page tables, and indexer key storage are separate terms in the per-rank inequality and Rust budget type. |
| V2-m3 top-p API | The API documents that `top_p < 1` requires `1 <= top_k <= 256`, the structured error, and the recommended explicit `top_k=256` substitution. |
| V2-m4 stale ABI-freeze wording | The frozen value layout, nibble order, SFB formula, and CUTLASS revision replace the stale OPEN wording. |
| V2-m5 MTP0 tolerance vocabulary | MTP0 and MTPK use the same stable-position/tie-adjacent vocabulary. Threshold values remain correctly blocked on the sampling ABI. |
| V2-Q1 cached versus recomputed indexer keys | Answered from the pinned local stack: production uses a 132-byte FP8 E4M3 plus FP32 scale cache with one scale per 128-element key and `ue8m0` power-of-two policy. |
| V2-Q2 28.224B provenance | It is labeled an estimate from the rounded 753B total until the complete generated tensor inventory replaces it. |

## Phase-A follow-through requiring review

Extracting the pinned layer-78 recurrence showed that the MTP draft attention
module is itself a full sparse-indexer layer. Its per-position key therefore
needs the same 132-byte cached record as a target full-indexer layer. Revision
0.2.2 combines that record with draft KV in a 500-byte committed-position
sidecar:

- 368 bytes of draft KV plus 132 bytes of draft indexer key;
- 32,000 payload bytes per 64-token page;
- a 36,864-byte sealed record including header and alignment;
- 524,288,000 bytes (0.48828125 GiB) aggregate at 1M positions.

This is a conservative capacity correction and closes V2's requested
draft-recurrence residency question. It changes a post-review cache ABI and
therefore must be independently reviewed before M2 execution.

## Read-only evidence used for V2-Q1

The neighboring `../glm52-opt` tree was not modified. The inspected HEAD was
`d213925ee6701072f117aec59ca94f1bf00d5e7f`. The relevant source pins are
recorded in the operation manifest:

- `deepseek_v2.py` SHA-256
  `5b14912dad2b006c7d1fb07eba6c706394e300a2d0e60529dba3557871649014`;
- indexer backend SHA-256
  `241bae6b76235fbaee3c10d690cd44708531d856d8dbce610555e2a0576ad074`;
- the cache kernel uses `fp8_e4m3`, writes a trailing FP32 scale, and rounds
  the scale upward to a power of two for `ue8m0`.

The independent next review point remains the generated operation manifest
and v0.2.1 physical ABI. No GPU evidence exists yet.
