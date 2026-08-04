# GLM-5.2 quality acceptance contract v1, revision 3 amendment

Date: 2026-08-04

Status: corrective design candidate; adversarial acceptance required before
evaluator implementation or model execution

GPU evidence: none

## Scope and precedence

This amendment consumes `docs/quality-acceptance-v1.md` revision 2 and closes
four implementation-blocking ambiguities found during independent preflight:

1. revision 2 says distinct weight policies share a run family but derives
   the only run identity from the policy itself;
2. `PositionQuality.v2` lists field names without types, encodings, enum
   discriminants, or deterministic centered-error arithmetic;
3. revision 2 permits an open-ended logit corpus while requiring exactly
   64 windows and 131,008 paired MTP rows; and
4. the repetition-incidence confidence bound has no exact statistic or
   endpoint algorithm.

This amendment changes no quality threshold or result. In a conflict it takes
precedence over revision 2. Revision-2 acceptance alone cannot open evaluator
implementation after this amendment exists; the exact revision-3 token is
also required.

## Canonical run family and execution run

Revision 2's `QualityRun.v2` is replaced by `QualityFamily.v1` and
`QualityRun.v3`. This separates comparison-invariant identity from the two
fields that legitimately vary between reference and candidate executions.

`QualityFamily.v1` contains the following fields in this exact order:

```text
model_revision: [u8; 40]
model_config_sha256: [u8; 32]
tokenizer_bundle_sha256: [u8; 32]
chat_template_sha256: [u8; 32]
engine_commit: [u8; 20]
evaluator_commit: [u8; 20]
evaluator_container_digest: [u8; 32]
rank_container_digests: [[u8; 32]; 4]
cuda_driver: String.v1
cuda_runtime: String.v1
gpu_identity: [GpuIdentity.v1; 4]
mpfr_version: String.v1
mpfr_build_flags: String.v1
corpus_manifest_sha256: [u8; 32]
task_manifest_sha256: [u8; 32]
sampling_abi_sha256: [u8; 32]
sampling_tuple_sha256: [u8; 32]
stochastic_seed_manifest_sha256: [u8; 32]
logit_bootstrap_seed: u64
task_bootstrap_seed: u64
cache_abi_sha256: [u8; 32]
cache_posture_sha256: [u8; 32]
control_selection_manifest_sha256: [u8; 32]
quality_control_policy_digest: [u8; 32]
matched_benchmark_plan_sha256: [u8; 32]
tp: u8 = 4
dcp: u8 = 4
vocabulary_rows: u32 = 154_880
logical_vocabulary: u32 = 154_856
```

The model revision is exactly 40 lowercase ASCII hexadecimal bytes, not raw
20-byte Git bytes. Revision 2's `String.v1` and `GpuIdentity.v1` encodings are
retained unchanged. Every integer is little-endian and no implicit padding is
serialized.

`quality_family_sha256` is SHA-256 over literal domain bytes
`glmaxx.quality-family.v1`, one NUL byte, then the complete canonical family
bytes above. It is independent of weight membership and MTP execution depth.

One physical execution has:

```text
QualityRun.v3 {
    run_uuid: [u8; 16]
    quality_family_sha256: [u8; 32]
    weight_policy_digest: [u8; 32]
    mtp_depth: u8
    family: QualityFamily.v1
}
```

`mtp_depth` is `0..=6`. The family is encoded in full after the three leading
fields; its recomputed digest must equal `quality_family_sha256` before the
run can be admitted. `run_uuid` is the first 16 bytes of SHA-256 over literal
domain `glmaxx.quality-run.v3`, one NUL byte,
`quality_family_sha256 || weight_policy_digest || mtp_depth`. The canonical
run SHA-256 is over all `QualityRun.v3` bytes including the UUID.

The sampling tuple remains the canonical `SamplingConfig.v1`; MTP depth is an
execution posture and exists only in the explicit run field. A missing,
duplicated, noncanonical, or divergent family/run field fails before logits
are produced.

## Typed comparison identity

Every comparison is admitted by:

```text
QualityComparison.v1 {
    quality_family_sha256: [u8; 32]
    reference_run_sha256: [u8; 32]
    candidate_run_sha256: [u8; 32]
    reference_weight_policy_digest: [u8; 32]
    candidate_weight_policy_digest: [u8; 32]
    reference_mtp_depth: u8
    candidate_mtp_depth: u8
    kind: u8
}
```

Fields are encoded in listed order with no padding. The comparison SHA-256 is
over domain `glmaxx.quality-comparison.v1`, NUL, then those bytes. Discriminants
and admission rules are:

| Kind | Value | Required relation |
|---|---:|---|
| `SAME_POLICY_RUNTIME` | 0 | policies equal; both depths zero |
| `COMPRESSED_VS_BF16` | 1 | policies differ; both depths zero |
| `MTP_VS_MTP0` | 2 | policies equal; reference depth zero; candidate depth `1..=6` |

Both runs must recompute to the same `quality_family_sha256`. The run hashes,
policies, and depths in the comparison must equal the referenced run
manifests. Any other difference is invalid rather than a degraded match.

## Fixed `PositionQuality.v3` ABI

Revision 2's prose-only record is replaced by one fixed 320-byte,
little-endian, padding-free record:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 32 | `quality_family_sha256` |
| 32 | 32 | `comparison_sha256` |
| 64 | 32 | `reference_run_sha256` |
| 96 | 32 | `candidate_run_sha256` |
| 128 | 32 | `row_identity_sha256` |
| 160 | 32 | `case_id_sha256` |
| 192 | 2 | `window_ordinal: u16` |
| 194 | 4 | `logical_position: u32` |
| 198 | 4 | `context_length: u32` |
| 202 | 4 | `reference_top1_id: u32` |
| 206 | 4 | `reference_top2_id: u32` |
| 210 | 4 | `candidate_top1_id: u32` |
| 214 | 4 | `candidate_top2_id: u32` |
| 218 | 4 | `reference_top1_logit_bits: u32` |
| 222 | 4 | `reference_top2_logit_bits: u32` |
| 226 | 4 | `candidate_at_reference_top1_bits: u32` |
| 230 | 4 | `candidate_at_reference_top2_bits: u32` |
| 234 | 4 | `candidate_top1_logit_bits: u32` |
| 238 | 4 | `candidate_top2_logit_bits: u32` |
| 242 | 8 | `reference_margin_bits: u64` |
| 250 | 8 | `candidate_margin_bits: u64` |
| 258 | 8 | `maximum_centered_logit_error_bits: u64` |
| 266 | 8 | `rms_centered_logit_error_bits: u64` |
| 274 | 8 | `kld_reference_to_candidate_bits: u64` |
| 282 | 1 | `classification: u8` |
| 283 | 1 | `mtp_depth: u8` |
| 284 | 2 | `target_row_bucket: u16` |
| 286 | 2 | `verifier_row_bucket: u16` |
| 288 | 32 | `cache_posture_sha256` |

Logit fields contain raw IEEE binary32 bits. Margin and metric fields contain
raw IEEE binary64 bits. Classification discriminants are `MATCH=0`,
`TIE_ADJACENT=1`, and `STABLE_MISMATCH=2`. Token IDs must be below 154,856,
`window_ordinal` is `0..63`, depths and buckets must match both run and row
execution receipts, and every digest must match its referenced manifest.

`case_id_sha256` hashes domain `glmaxx.quality-case-id.v1`, NUL, then the
canonical task/corpus case-ID bytes. It is never a pathname or locale-derived
string.

`row_identity_sha256` hashes domain `glmaxx.quality-row.v3`, NUL, then in
order: comparison SHA-256, case-ID SHA-256, window ordinal, logical position,
context length, teacher-forced token-history SHA-256, reference raw-row file
SHA-256 and row ordinal, candidate raw-row file SHA-256 and row ordinal, and
cache-posture SHA-256. Ordinals are little-endian `u32`. The referenced raw
array manifest must expose every preimage field; a digest without those
records is invalid.

## Deterministic centered-error arithmetic

For each logical token, promote both raw binary32 logits exactly to binary64.
Reference and candidate maxima use revision 2's total order. Compute each
centered value and their difference with a separate binary64
round-to-nearest-even operation in ascending token-ID order. No contraction
or reassociation is allowed.

Margins are the separately rounded binary64 difference between the promoted
top-1 and top-2 logits. Maximum centered error is the largest absolute
binary64 difference, with the lowest token ID winning an exact error tie.

RMS uses fresh 256-bit MPFR destinations and `MPFR_RNDN`. In ascending token
ID, promote each already-rounded binary64 centered error exactly, square it,
and add it to an initially positive-zero accumulator with one rounded
operation each. Divide by exact integer 154,856, take one rounded square root,
then round once to binary64 RNE. Clear and check flags separately from KLD;
ordinary inexact is retained, while underflow, overflow, NaN, range,
divide-by-zero, nonfinite output, or negative output is fatal.

These rules make every `PositionQuality.v3` byte reproducible. They do not
change revision 2's top-token, tie, KLD, or gate thresholds.

## Exact logit and MTP corpus

The runnable `corpus_manifest_sha256` must bind an accepted materialization of
`docs/quality-corpus-manifest-v1.md`. It contains exactly 64 windows, not at
least 64:

```text
natural text       32 windows    65,504 rows
reasoning/code      8 windows    16,376 rows
tool/JSON           8 windows    16,376 rows
multilingual        8 windows    16,376 rows
long context        8 windows    16,376 rows
total              64 windows   131,008 rows
```

Each window is exactly 2,048 token IDs and scores positions 1 through 2,047.
The same ordered 64-window manifest is used for target MTP0 and each MTP-depth
teacher-forced comparison. An extra, missing, shortened, reordered, or
qualification/calibration-overlapping window invalidates the run.

The task manifest also binds exactly 1,000 frozen and 1,000 randomized
retrieval cases: 200 cases at each of 16,384, 65,536, 131,072, 491,520, and
1,048,576 total positions, with 40 cases in each of five position bins per
band. Per-band and per-bin results are retained. These counts replace any
interpretation in which the minimum can be distributed arbitrarily across
bands.

## Exact repetition confidence bound

Revision 2's absolute repetition-incidence difference remains at most 0.005.
Its one-sided 95% confidence upper bound is now the same paired-item bootstrap
stream and block construction used for task inference:

1. each item contributes binary event `candidate_repetition -
   control_repetition` in `{-1,0,1}`;
2. for each of exactly 10,000 replicates, resample paired items within every
   declared internal sub-stratum using the task SplitMix64 stream and retain
   original sub-stratum counts;
3. sum each replicate in ascending sampled-item ordinal using binary64 RNE and
   divide by the exact item count;
4. sort by replicate value then replicate ordinal; and
5. select nearest-rank element `ceil(0.95 * 10,000) = 9,500` using one-based
   rank.

The selected upper endpoint must be at most 0.010. No Wilson substitution,
unpaired resampling, continuity correction, dropped replicate, or normal
approximation is permitted.

## Required revision-3 CPU proof

After adversarial acceptance, the revision-2 CPU matrix is extended to prove:

1. byte-exact `QualityFamily.v1`, `QualityRun.v3`,
   `QualityComparison.v1`, and 320-byte `PositionQuality.v3` fixtures;
2. mutation rejection for every family/run/comparison field and all three
   comparison kinds;
3. exact raw-float encodings, row-identity preimages, classification
   discriminants, centered-error maximum, and MPFR RMS;
4. exact 64-window, stratum, row, retrieval-band, and position-bin counts;
5. repetition upper-bound fixtures immediately below, at, and above 0.010;
   and
6. failure on the revision-2 ambiguous-family and untyped-record encodings.

The quality evaluator, corpus materializer, CUDA, checkpoint, KLD result,
task result, retrieval, MTP, capacity, cold-start, latency, throughput, and
serving nonclaims remain unchanged.
