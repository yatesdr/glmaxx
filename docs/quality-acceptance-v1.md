# GLM-5.2 quality acceptance contract v1

Date: 2026-07-29

Status: design candidate; adversarial review required before evaluator
implementation or promotion

GPU evidence: none

## Scope

This contract closes two blocking specification choices:

1. the fixed stable-position/tie-adjacent vocabulary for comparing greedy
   execution across batch shapes and MTP depths; and
2. the per-position logit, KLD, task, retrieval, repetition, parser, and
   termination gates required before a compressed profile can serve.

It applies only to the pinned GLM-5.2 model, vocabulary, tokenizer, chat
template, and TP4 engine in this repository. It does not accept any current
weight profile, enable MTP, permit full-checkpoint conversion, or claim a
model result.

Quality is evaluated before throughput. A faster profile cannot pass by
changing the reference, prompts, context, cache posture, sampling tuple,
precision membership, or evaluator.

## Gate ordering

The mandatory order is:

```text
evaluator CPU proof
→ target-only MTP0 smoke
→ target-only MTP0 multi-window logits
→ target-only task/retrieval qualification
→ freeze serving profile
→ MTP1 comparison
→ MTP2 ... MTP6 comparisons
→ matched performance enablement
```

Failure at an earlier stage blocks every later stage. Each MTP depth is
accepted independently; passing MTP3 does not accept MTP4–6.

The calibration set used to select EXL3/NVFP4/protected membership is
disjoint from every qualification window and task item. Qualification data
cannot be used to retune membership, clipping, scales, kernels, tie
thresholds, or task extraction. Any such change creates a new candidate and
reruns the full gate.

## Immutable run identity

Before generating candidate outputs, one run manifest freezes:

```text
QualityRun.v1 {
    run_uuid: content-derived 128 bits
    model_revision: 40-byte lowercase Git identity
    model_config_sha256: [u8; 32]
    tokenizer_bundle_sha256: [u8; 32]
    chat_template_sha256: [u8; 32]
    engine_commit: 20 bytes
    evaluator_commit: 20 bytes
    evaluator_container_digest: [u8; 32]
    weight_policy_digest: [u8; 32]
    rank_container_digests: [[u8; 32]; 4]
    cuda_driver_and_runtime: fixed strings
    corpus_manifest_sha256: [u8; 32]
    task_manifest_sha256: [u8; 32]
    sampling_abi_sha256: [u8; 32]
    cache_abi_sha256: [u8; 32]
    tp: 4
    dcp: 4
    vocabulary_rows: 154_880
    logical_vocabulary: 154_856
}
```

The content-derived UUID hashes every following field in the listed order
under domain `glmaxx.quality-run.v1\0`. Every rank and the evaluator verify
the same manifest before work starts. A missing or divergent field aborts the
run.

The BF16 reference and each compressed control use separate immutable
`weight_policy_digest` values but otherwise share the same run family. A
comparison across different run-family fields is invalid, not degraded
evidence.

## Retained logits and per-position record

The evaluator retains all 154,856 logical-vocabulary logits for every scored
position. The 24 padded physical rows are retained separately and must be
negative infinity before sampling or KLD construction. Any finite padded row
is fatal.

Raw logit arrays remain outside Git in content-addressed compressed files.
Each file records:

- dtype and byte order;
- exact row and vocabulary dimensions;
- prompt and continuation token IDs;
- physical-row mask;
- source rank-shard hashes;
- assembled-file SHA-256; and
- the run manifest digest.

One `PositionQuality.v1` record is retained for every next-token position:

```text
case_id
window_id
logical_position
context_length
reference_top1_id, reference_top2_id
candidate_top1_id, candidate_top2_id
reference_top1_logit, reference_top2_logit
candidate logits at both reference top IDs
candidate_top1_logit, candidate_top2_logit
reference_margin, candidate_margin
maximum_centered_logit_error
rms_centered_logit_error
kld_reference_to_candidate
classification: MATCH | TIE_ADJACENT | STABLE_MISMATCH
mtp_depth
target_row_bucket
verifier_row_bucket
cache_posture
```

Logit error is measured after subtracting each row's logical-vocabulary
maximum. This removes the softmax-invariant constant shift without hiding a
change in relative logits.

Aggregate reports never replace these records or the raw arrays.

## Deterministic top-token order

All quality tooling orders candidates by:

1. descending finite FP32 logit under IEEE total ordering; then
2. ascending global token ID.

NaN or positive infinity is fatal. Negative infinity is permitted only for a
masked token. Exact ties choose the smaller global token ID.

The same rule is used for BF16 reference rows, compressed MTP0 rows,
verifier rows, and draft rows. A framework's unspecified `topk` tie order is
not evidence.

## Stable and tie-adjacent classification

The fixed logit tolerance is:

```text
TIE_LOGIT = 2^-8 = 0.00390625
```

For comparison row `A` against authoritative row `R`:

- `MATCH` means both rows choose the same top-1 token.
- `TIE_ADJACENT` requires all of:
  - the selected tokens differ;
  - the two selected IDs are exactly the top-two ID set in both rows;
  - `R`'s top-two margin is at most `TIE_LOGIT`;
  - `A`'s top-two margin is at most `TIE_LOGIT`; and
  - the absolute centered-logit error at each of those two IDs is at most
    `TIE_LOGIT`.
- every other differing selection is `STABLE_MISMATCH`.

Equality at the threshold is tie-adjacent. The threshold is evaluated from
stored FP32 bit patterns promoted exactly to FP64. It cannot be enlarged from
observed candidate error, batch size, quantization tier, MTP depth, or task
outcome.

Runtime correctness and weight quality are distinct comparisons:

- target-only MTP0 against the pinned reference runtime using the exact same
  weight-policy bytes permits zero `STABLE_MISMATCH`; its single-window
  tie-adjacent rate is at most `0.001`, with a one-sided 95% Wilson upper
  bound at most `0.002`; and
- a compressed policy against BF16 reports all three classes as a quality
  metric. It is governed by the absolute and capacity-feasible-control gates
  below, not falsely required to produce the BF16 top token at every
  teacher-forced position.

The same-policy runtime comparison may not substitute BF16 weights, change
precision membership, or use a different cache/attention posture. It is an
implementation-correctness gate, not a quantization-quality gate.

For MTPK versus the same compressed profile at MTP0, every depth permits zero
stable mismatches, a tie-adjacent rate no greater than `0.001`, and a
one-sided 95% Wilson upper bound no greater than `0.002`.

## KLD construction

KLD is `D_KL(P_reference || P_candidate)` over all 154,856 logical tokens.
Neither top-k truncation nor sampling filters are applied.

The normative evaluator:

1. promotes each stored FP32 logit exactly to a 256-bit MPFR value;
2. subtracts the row maximum;
3. evaluates exponentials with MPFR round-to-nearest;
4. sums in ascending global token order at 256-bit precision;
5. computes log-normalizers and the KLD at 256-bit precision; and
6. rounds only the reported value to IEEE binary64, round-to-nearest-even.

The evaluator pins the MPFR version, build flags, and container digest.
An independent implementation must reproduce every reported binary64 bit.
A negative KLD, nonfinite value, zero mass, masked-token mass, or
normalization error greater than `2^-50` is fatal.

## Target-only logit gates

### Smoke cell

The first gate uses the already specified 2,048-token window and scores all
2,047 next-token positions with MTP disabled. The candidate must satisfy:

| Metric | Maximum |
|---|---:|
| arithmetic mean KLD | `0.130000` |
| median KLD | `0.080000` |
| p95 KLD, nearest-rank | `0.500000` |
| p99 KLD, nearest-rank | `1.250000` |
| maximum per-position KLD | `5.000000` |
| stable-mismatch rate versus BF16 | `0.100000` |
| tie-adjacent rate versus BF16 | `0.020000` |

The approximately `0.1195253311` historical EXL3 mean is context for choosing
the precommitted smoke ceiling, not an accepted result. It must be rerun
under this contract.

### Multi-window qualification

The final logit corpus contains at least:

- 32 non-overlapping 2,048-token natural-text windows;
- 8 reasoning/code windows;
- 8 tool/JSON windows;
- 8 multilingual windows; and
- 8 long-context slices whose scored rows begin after 128k positions.

It scores at least 131,008 positions in total. Each content stratum contains
at least 8,192 positions. Window hashes, extraction offsets, licenses, and
deduplication against calibration data are frozen before the run.

Every candidate must satisfy:

| Multi-window KLD metric | Combined maximum | Per-stratum maximum |
|---|---:|---:|
| arithmetic mean | `0.130000` | `0.160000` |
| median | `0.080000` | `0.100000` |
| p95, nearest-rank | `0.500000` | `0.650000` |
| p99, nearest-rank | `1.250000` | `1.750000` |
| maximum position | `7.500000` | `7.500000` |
| stable-mismatch rate versus BF16 | `0.080000` | `0.120000` |
| tie-adjacent rate versus BF16 | `0.010000` | `0.020000` |

It must also satisfy the control-relative gates below.

## Capacity-feasible control and noninferiority

The comparison set contains:

- pinned BF16 reference logits;
- every compressed control that fits the same four-card target with the same
  protected tensors, cache posture, context capacity, and MTP0 state; and
- the candidate serving policy.

An all-NVFP4 profile that cannot meet the capacity contract is a kernel
control, not a quality noninferiority control.

The `quality_control` is the capacity-feasible control with the lowest mean
KLD. Ties within `1e-6` are resolved by p99 KLD, then task aggregate, then
lexicographic policy digest. The choice is made by the evaluator, not by the
candidate owner.

Over 10,000 prompt-block bootstrap replicates with a pinned SplitMix64 seed,
the candidate must satisfy these one-sided 95% upper confidence bounds:

| Ratio, candidate / quality control | Maximum upper bound |
|---|---:|
| mean KLD | `1.02` |
| p95 KLD | `1.05` |
| p99 KLD | `1.10` |
| stable-mismatch rate | `1.02` |
| tie-adjacent rate | `1.25` |

When a control denominator is zero, the candidate metric must also be zero.
No aggregate ratio can excuse an absolute-ceiling or stable-position failure.

If two capacity-feasible policies both pass, production selects
lexicographically by:

1. no task/retrieval regression;
2. lower mean KLD upper confidence bound;
3. lower p99 KLD upper confidence bound;
4. lower stable/tie-adjacent divergence;
5. higher measured useful throughput; then
6. lower physical weight bytes.

This ordering implements the stated quality-first priority.

## Task and behavior qualification

The task manifest pins dataset revisions, item IDs, few-shot policy, chat
template, tool schemas, decoding parameters, maximum output, answer
extraction, judge model or deterministic checker, and evaluator code. Hidden
judge prompts and raw model outputs remain content-addressed outside Git.
Primary target-quality comparisons use greedy MTP0. Any stochastic auxiliary
row has a fixed seed and is reported separately.

The minimum suite has:

| Stratum | Minimum scored items | Gate |
|---|---:|---|
| reasoning | 1,000 | paired accuracy noninferiority |
| coding with executable tests | 500 | paired pass-rate noninferiority |
| tool selection/arguments | 500 | exact schema and semantic score |
| JSON/schema constrained output | 500 | exact parse/schema rate |
| long generation/repetition | 500 | repetition and completion behavior |
| frozen long-context retrieval | 1,000 | exact answer at every context band |
| randomized long-context retrieval | 1,000 | exact answer and seed retention |
| termination/parser behavior | 500 | correct EOS/parser terminal state |

For accuracy/pass-rate metrics, the lower endpoint of a paired, stratified
10,000-replicate 95% bootstrap confidence interval for
`candidate - quality_control` must be at least:

- `-0.010` absolute for reasoning and coding;
- `-0.005` for tool and JSON/schema success;
- `-0.002` for termination/parser success; and
- `0.000` for frozen retrieval.

Randomized retrieval must be at least `0.99` and no more than `0.005` below
the control. Each required context band—16k, 64k, 128k, 480k, and
1,048,576—passes separately.

The candidate may add no new:

- invalid UTF-8 or invalid JSON class;
- tool-name or argument-schema violation;
- reasoning-parser state violation;
- EOS suppression or spurious EOS class;
- unbounded repetition episode;
- request that exceeds its pinned token limit; or
- retrieval answer sourced from outside the permitted context.

Repetition incidence may not exceed the control by more than `0.005`
absolute, and the one-sided 95% confidence upper bound on that difference
must be at most `0.010`.

Holm correction at family-wise `alpha=0.05` applies across the eight primary
strata. Both uncorrected and corrected intervals are retained.

## MTP batch-shape and depth gates

MTPK uses target verifier logits as authoritative for its own step, as
required by the engine specification. Quality comparison does not demand
bit-identical kernels across M buckets.

For every enabled depth `K=1..6`, compare the candidate policy's MTPK run with
its separately executed MTP0 run using identical prompts, prefix posture,
sampling parameters, and maximum output. Retain the verifier row shape and
MTP0 row shape at every position.

Greedy depth qualification requires:

| Metric, MTPK target versus MTP0 target | Maximum |
|---|---:|
| stable mismatches | `0` |
| tie-adjacent rate | `0.001` plus Wilson bound above |
| mean full-vocabulary KLD | `1e-5` |
| p99 full-vocabulary KLD | `1e-4` |
| maximum full-vocabulary KLD | `1e-3` |
| p99 centered top-two logit error | `0.01` |
| maximum centered top-two logit error | `0.03` |

The full task and behavior suite then uses the same noninferiority thresholds
against MTP0. Accepted length, rejection position, residual/bonus provenance,
and accepted EOS are retained per speculative cycle. Acceptance rate is
reported by context band and concurrency but cannot compensate for a quality
failure.

Probabilistic MTP remains blocked until the distributed sampling ABI is
accepted and its statistical proof passes. Seed, initial counter, final
counter, and every draw ticket are then part of the matched record.

## Performance enablement after quality

Passing quality makes a depth eligible, not automatically useful. A depth is
enabled by default only if a matched end-to-end matrix shows:

- at least 5% higher useful decoded tokens/second than MTP0 in one declared
  target concurrency band;
- no target context/concurrency cell with more than 5% useful-throughput
  regression;
- no p99 inter-token-latency regression greater than 5%; and
- no reduction in admitted context capacity or KV escrow.

Otherwise the implementation remains available only behind an explicit
experimental profile. MTP0 always remains available as the quality reference.

## Failure, retry, and publication

A run is invalid if it has a missing row, duplicate row, changed prompt,
changed token history, unexpected cache hit, rank restart, model retry,
unrecorded fallback, nonfinite logit, evaluator warning, or hash mismatch.
Invalid rows are not silently dropped or imputed.

Every published result bundle contains:

- immutable run and dataset manifests;
- exact commands and environment;
- raw-array hashes and per-position record hashes;
- all per-position records;
- aggregate and bootstrap outputs;
- task responses and checker/judge provenance;
- failure and retry logs;
- profile membership and capacity proof;
- explicit PASS, FAIL, or INCONCLUSIVE per gate; and
- an append-only disposition for every invalidated prior run.

No mean-only summary, cherry-picked prompt subset, task-only result, or
unhashed screenshot is acceptance evidence.

## Required CPU proof before model execution

The evaluator implementation must pass fixtures for:

1. exact ties and lower-token selection;
2. every boundary immediately below, at, and above `TIE_LOGIT`;
3. a constant logit shift;
4. one-token and full-vocabulary perturbations;
5. masked physical rows;
6. NaN, positive infinity, all-negative-infinity, and zero-mass rejection;
7. MPFR KLD against an independent rational/small-vocabulary oracle;
8. nearest-rank median/p95/p99 indexing;
9. Wilson bounds and zero-count cases;
10. bootstrap seed and prompt-block resampling stability;
11. control selection and zero denominators;
12. Holm correction;
13. missing, duplicate, reordered, and cross-run rows;
14. all task noninferiority boundaries;
15. every MTP depth and row-bucket pairing; and
16. byte-stable manifests and output digests.

CPU proof does not begin until adversarial review accepts this contract.
