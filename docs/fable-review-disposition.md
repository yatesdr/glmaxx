# Fable adversarial-review disposition

Date: 2026-07-28

Review: [fable-adversarial.md](../fable-adversarial.md)

Review SHA-256:
`4a6e6e36d2f226f8fadc6135b910281db1aa9e5d11cded3b6bd43e287e975137`

Status: all reported blockers and majors have a specification disposition;
revision 0.2.0 requires hash-pinned re-review before implementation acts on
it.

No GPU work is authorized by this disposition.

## Provenance

The review records the exact bytes it evaluated:

| File | Reviewed SHA-256 |
|---|---|
| `spec/engine-v0.md` | `fe89ebb87f0b10558630481d863c36542e9ac4b5420aee302a50db70dcf98205` |
| `spec/format-v0.md` | `8edfda4a31ac4bc677814f5e8d78e21892622745b2d040c5eb860201a7406275` |
| `docs/native-engine-plan.md` | `111c2956b5117f102e95fceb64fd1eaadc57ba9669e8b54d5f3fc4e53e0bedf1` |

The revised re-review candidates are:

| File | Revision | Candidate SHA-256 |
|---|---:|---|
| `spec/engine-v0.md` | 0.2.0 | `15c8a9033420828307ebc10d472c70ed739e611f4ed0308b88c42972be1666a3` |
| `spec/format-v0.md` | 0.2.0 | `0e56b4b0cb1c4a64b666022a9d43c4ebc48a4609503d72e25b7526032a19fac6` |
| `docs/native-engine-plan.md` | post-review draft | `91950cea58ca2ff4f64f3588897b7f130288828b87cd34789f5e0bdb825787d1` |

These hashes SHALL be recomputed at the start and finish of re-review.

## Blocker dispositions

### B1 — standalone NVFP4 serving fit

Accepted, with a corrected stronger calculation.

The review used 0.53125 byte per parameter for 1D NVFP4. The specified
direct layout uses 0.5 byte of E2M1 data plus one E4M3 byte per 16 values,
which is 0.5625 byte per parameter. The 724,775,731,200 target routed-expert
parameters therefore require 379.6875 GiB, not 358.6 GiB.

Resolution:

- `speed-nvfp4` was removed.
- `nvfp4-laboratory` is subset-only through M4 and cannot expose service.
- `hybrid-serve` is the only NVFP4-bearing serving profile.
- EXL3 is mandatory before M5–M7.
- Engine section 8 now carries a worked lower-bound table, a per-rank fit
  inequality, a required `profile-budget-v0.json`, and 1-GiB/rank escrow.

### B2 — impossible exact MTP0 equality

Accepted. Version zero does not mandate batch-invariant kernels.

Resolution:

- verifier-pass target logits are authoritative for each speculative step;
- the emitted sequence must exactly match those logits;
- MTPK versus MTP0 requires exact token agreement at reviewed numerically
  stable positions;
- tie-adjacent divergence is separately bounded and reported;
- all other mismatches fail;
- per-position margins, logit errors, KLD, task quality, and acceptance are
  retained;
- the stable/tie thresholds block MTP1, not the NVFP4 codec oracle.

### B3 — nondeterministic rank-file header

Accepted.

Resolution:

- rank `file_uuid` and four-rank `conversion_uuid` are content-derived;
- in-file `created_unix_seconds` is fixed to zero;
- build time and operator identity move to an unhashed sidecar;
- canonical manifests forbid random and wall-clock fields;
- deterministic conversion now means byte-identical complete rank files.

## Major dispositions

| Finding | Disposition |
|---|---|
| M1 draft KV absent | Confirmed the layer-78 module contains MLA attention and a full MoE. Added a 0.359375-GiB-at-1M draft-KV sidecar, HBM geometry, sealed record, prefix pairing, atomic publication, tentative commit, and rollback. |
| M2 decode DCP absent | Production decode now forbids record gather and specifies DCP4 query exchange, deterministic global sparse-candidate merge, owner-local partial softmax, and fixed-rank FP32 LSE combine before M3. |
| M3 DCP1/DCP2 unspecified | Cut from v0. DCP4 is process-immutable. |
| M4 graph set/budget absent | Added reviewed `graph-profile-v0.json`, admission rejection for unreachable keys, measured per-graph/resident bytes, maximum scratch, and 1-GiB/rank initial escrow. |
| M5 pre/post-filter ambiguity | Defined `p` and `q` as identically post-processed distributions and placed filter/RNG/residual/bonus semantics in the sampling ABI. |
| M6 posture invalidates cache | Split content namespace from HBM attachment ABI. Tier records are ownership-neutral and writer rank is advisory. |
| M7 full boot SHA cost | Added strong `FULL_SHA256` or pinned `FS_VERITY` verification modes; metadata receipts and noncryptographic checksums alone are insufficient. |
| M8 full logits gather | Added contiguous vocabulary sharding, distributed argmax, bounded candidate merge, distributed mass/CDF sampling, residual sampling, and a production prohibition on full-vocabulary gathers. |

## Minor and question dispositions

- The design-note page state machine now includes `HBM_TENTATIVE` and
  `INVALID`.
- DRAM is explicitly process-volatile rather than ambiguously recoverable.
- NVMe has a rolling bytes-per-day write cap.
- Unused `StepPlan` fields and `CACHE_ONLY` collective semantics are
  canonicalized.
- Current cn4 storage now requires a pre-conversion placement ledger.
- `cuMemGetInfo` validates but cannot expand the spec-owned physical budget.
- M5 KLD/task thresholds and serving detokenization remain explicit blocking
  OPEN items at their appropriate gates.
- The constants now state 75 target sparse layers and one attention-plus-MoE
  draft layer, for 79 checkpoint layer IDs.
- Activation NVFP4 uses a dynamic per-row global scale.
- Hybrid codec selection is explicitly keyed by
  `(layer_id, expert_id, tensor_role)`.

## Remaining re-review questions

1. Is the verifier-authoritative/stable-position MTP contract sufficiently
   strict without batch-invariant target kernels?
2. Is the separate draft sidecar preferable to a unified 79-layer tier
   record, and are target-only MTP0 attachments sufficiently explicit?
3. Is the DCP4 query/candidate/partial-LSE route complete enough to begin the
   CPU one-layer operation proof?
4. Is the bounded top-p rule (`top_p < 1` requires `1 <= top_k <= 256`)
   acceptable for v0?
5. Does the content/attachment namespace split preserve every required
   fail-closed boundary?
6. Are any v0.2 OPEN items incorrectly scoped before the NVFP4 CPU-proof
   phase?
