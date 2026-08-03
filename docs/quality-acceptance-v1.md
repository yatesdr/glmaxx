# GLM-5.2 quality acceptance contract v1, revision 2

Date: 2026-08-03

Status: corrective design candidate; adversarial review required before
evaluator implementation or promotion

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
QualityRun.v2 {
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
    control_selection_manifest_sha256: [u8; 32]
    quality_control_policy_digest: [u8; 32]
    matched_benchmark_plan_sha256: [u8; 32]
    tp: 4
    dcp: 4
    vocabulary_rows: 154_880
    logical_vocabulary: 154_856
}
```

`String.v1` is a `u32` little-endian byte count followed by exactly that many
canonical UTF-8 bytes. It rejects NUL, non-shortest UTF-8, control characters,
leading/trailing whitespace, and Unicode strings not in NFC. All integers use
little-endian encoding; fixed arrays have no length prefix; enum values are
their declared `u8` discriminants. No field is JSON text or a host path.

Each `GpuIdentity.v1` is rank-ordered and contains the GPU UUID as 16 raw
bytes, PCI domain/bus/device/function as `u16/u8/u8/u8`, VBIOS as
`String.v1`, board power limit in milliwatts as `u32`, locked SM and memory
clock ceilings in MHz as two `u32` values, and the MIG/posture discriminant
as `u8` (`0` means the required non-MIG posture). The sampling tuple hashes
the canonical distributed-sampling parameter record, including greedy rows;
the seed manifest lists every stochastic auxiliary case and seed in case-ID
order. MPFR identity is explicit rather than inferred from the container.

The content-derived UUID is the first 16 bytes of SHA-256 over domain
`glmaxx.quality-run.v2\0` followed by every subsequent field in the listed
order and exact encoding above. Every rank and the evaluator verify the same
manifest before work starts. A missing, unknown, noncanonical, or divergent
field aborts the run.

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
They are copied rank-shard by rank-shard only after the measured production
step has completed and device timing has stopped, then assembled by the
offline evaluator. Production sampling never gathers a full vocabulary and
offline evaluation copies are excluded from latency and throughput samples.
Each file records:

- dtype and byte order;
- exact row and vocabulary dimensions;
- prompt and continuation token IDs;
- physical-row mask;
- source rank-shard hashes;
- assembled-file SHA-256; and
- the run manifest digest.

One `PositionQuality.v2` record is retained for every next-token position:

```text
quality_run_sha256
row_identity_sha256
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

`quality_run_sha256` hashes the complete canonical `QualityRun.v2` bytes.
`row_identity_sha256` binds the case/window/position, identical teacher-forced
token history where applicable, reference and candidate raw-row file hashes,
and row ordinals. Both fields are required before any metric is computed.

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
  weight-policy bytes permits zero `STABLE_MISMATCH`; its 2,047-row
  single-window gate permits at most two `TIE_ADJACENT` rows, and the
  one-sided 95% Wilson upper bound is at most `0.003`; and
- a compressed policy against BF16 reports all three classes as a quality
  metric. It is governed by the absolute and capacity-feasible-control gates
  below, not falsely required to produce the BF16 top token at every
  teacher-forced position.

The same-policy runtime comparison may not substitute BF16 weights, change
precision membership, or use a different cache/attention posture. It is an
implementation-correctness gate, not a quantization-quality gate.

For MTPK versus the same compressed profile at MTP0, the numerical comparison
uses exactly the 64 frozen 2,048-token windows and their identical
teacher-forced histories: 131,008 paired next-token rows per depth. Every
depth permits zero stable mismatches, at most 131 tie-adjacent rows, and a
one-sided 95% Wilson upper bound at most `0.0012`. Generated task behavior is
tested separately; after a differing emitted token, later logits from the two
different histories are never mislabeled as paired numerical rows.

Wilson bounds use `z = 1.6448536269514722` encoded as binary64 bits
`0x3ffa515209676abb`, the standard score interval without continuity
correction. Promote integer `k` and `n` exactly to binary64 first. With
binary64 round-to-nearest-even after every displayed operation, `p=k/n`,
`z2=z*z`, `center=p+z2/(2*n)`,
`radius=z*sqrt((p*(1-p))/n + z2/(4*n*n))`, and
`upper=(center+radius)/(1+z2/n)`. Evaluation follows those assignments from
left to right; no contraction or reassociation is permitted. The count,
denominator, raw rate, and bound are retained.

## KLD construction

KLD is `D_KL(P_reference || P_candidate)` over all 154,856 logical tokens.
Neither top-k truncation nor sampling filters are applied.

Every MPFR destination has precision exactly 256 bits and every operation
uses `MPFR_RNDN`. Before evaluation the process sets `emin=-1_000_000` and
`emax=+1_000_000`, reads both values back, clears all MPFR exception flags,
and records the exact MPFR version and build flags in `QualityRun.v2`.

For ascending logical token ID `i`, let stored reference and candidate logits
be `r_i` and `c_i`. The normative evaluator performs exactly:

1. promote every finite FP32 input exactly; reject NaN or infinity;
2. find `r_max` and `c_max` under the declared top-token order;
3. compute `xr_i = RN256(r_i-r_max)` and `xc_i = RN256(c_i-c_max)`;
4. compute `er_i = RN256(exp(xr_i))` and `ec_i = RN256(exp(xc_i))`;
5. initialize `Zr=+0` and `Zc=+0`, then for each `i` assign
   `Zr=RN256(Zr+er_i)` and `Zc=RN256(Zc+ec_i)`;
6. compute `Lr=RN256(log(Zr))` and `Lc=RN256(log(Zc))`;
7. initialize `D=+0` and, for each `i`, assign in order
   `p_i=RN256(er_i/Zr)`, `lr_i=RN256(xr_i-Lr)`,
   `lc_i=RN256(xc_i-Lc)`, `d_i=RN256(lr_i-lc_i)`,
   `t_i=RN256(p_i*d_i)`, and `D=RN256(D+t_i)`; and
8. round `D` once to IEEE binary64, round-to-nearest-even.

No FMA, fused dot, tree sum, pairwise sum, reordered sum, alternative
log-softmax identity, or algebraic regrouping is equivalent evidence. A
separate ascending-ID sum of the stored `p_i` values must differ from one by
at most `2^-50`.

The 154,856 logical tokens are never masked in this corpus. The 24 physical
padding rows are checked as negative infinity and excluded before step 1. If
an admitted future corpus has logical masking, the identical mask must be
bound into both row identities and masked tokens are excluded from every
maximum, sum, and KLD operation. An exponential of a finite admitted logit
must be positive: zero, MPFR underflow, overflow, NaN, range, or divide-by-zero
flags, zero total mass, nonfinite intermediate, or final negative KLD is
fatal. The MPFR inexact flag is expected for transcendental operations; it is
cleared before each row, recorded after the row, and is not a failure by
itself.

The evaluator pins the MPFR version, build flags, exponent range, and
container digest. An independent implementation must reproduce every
reported binary64 bit.

### Prior-cn4 compatibility diagnostic

The first NVFP4 and TR3 smoke runs also report a non-gating diagnostic that
matches the earlier cn4 procedure. This preserves historical comparability
without treating its implementation-dependent FP32 reduction as the revision-2
acceptance definition.

The compatibility cell fixes:

- the same 2,048 token IDs and 2,047 next-token rows;
- BF16 reference-logit payload SHA-256
  `87f992a689c054a0548a4b3863da6c809f9239beacd5786d0401e45904fec063`
  and reference manifest SHA-256
  `985120136741037918bcd4dc8da9813c1f6268b35a730302f99cf6b3eebb7606`;
- TP4, DCP1, MTP0, eager execution, and 32-row KLD chunks;
- historical evaluator image ID, as provenance only,
  `sha256:a5608e0b4a2fcdaec476de79fbe5cf2f6e9ce2ecf30bf2dfe0c1314d97c6666e`;
- both physical `[2047,154880]` FP32 logit arrays, including the historical
  physical padding treatment; and
- FP32 `log_softmax(reference)` and `log_softmax(candidate)`, followed by
  PyTorch `kl_div(log_candidate, log_reference, reduction="none",
  log_target=true).sum(dim=-1)` in 32-row chunks.

At immutable `glm52-opt` commit
`38cba1091c043bdecd426a0d4625f58211f94e0c`, the historical wrapper
`harness/run_glm52_tr3_dynamic_kld.sh` has SHA-256
`63c02bd1156ef8db49f9c0fc7d3d80fdbf46f9331f9499d7c334f37cca9ac55a`;
the runner it admits has SHA-256
`d1dc1a63b9889e881f3bd899638d0ec65a1a1079132f6a207a600d9cba845405`.
The read-only comparison records are
`experiments/2026-07-28-v20-nvfp4-scaling-kld-n3/README.md` at SHA-256
`0bd879ea07d8b6be00271c2736b2c15b20cac9cf2ea27b5a3261f19beff56524`
and `experiments/2026-07-28-cn4-tr3-qualification/README.md` at SHA-256
`c54bf499b8edb5d5886daa372ad34025ebc652f6545c45b353e1b389bdd09fff`
in the separately maintained `glm52-opt` evidence repository.

GLMAXX does not execute from, write to, mount writable, or reuse a vLLM
worktree, cache, container, image, volume, service, or result path for this
diagnostic. A dedicated GLMAXX evaluator image is built from independently
pinned CUDA/PyTorch package bytes after their versions and library hashes are
inventoried read-only. Before a compatibility claim, it must reproduce a
frozen synthetic row-set's historical per-position binary64 digest exactly;
otherwise the result is labeled `LEGACY_PROCEDURE_UNVALIDATED` and no legacy
mean is published. Admitted reference bytes are copied read-only into a
content-addressed GLMAXX fixture below
`/home/derek/glmaxx/cache/quality-fixtures/`; all new raw logits and results
belong to the unique GLMAXX evidence run. The evidence retains per-position
legacy binary64 values and their hash, not only the mean.

The published field is named `legacy_cn4_kld_mean`; it is never compared to
the MPFR thresholds, selected as `kld_reference_to_candidate`, or mixed into
control ranking. Any difference between the two metrics is reported, not
"corrected" by a fitted offset.

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

No historical mean is an input to this gate. Every control is rerun under the
same revision-2 evaluator and immutable run family.

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

The multilingual stratum additionally retains the same metrics separately
for every language. These rows are diagnostic in revision 2, but a missing
language report invalidates the combined stratum and the diagnostic cannot be
used to discard a language after results are visible.

## Capacity-feasible control and noninferiority

The comparison set contains:

- pinned BF16 reference logits;
- every preregistered compressed control that fits the same four-card target
  with the same cache posture, context capacity, and MTP0 state; and
- the candidate serving policy.

An all-NVFP4 profile that cannot meet the capacity contract is a kernel
control, not a quality noninferiority control.

Controls may have different EXL3, NVFP4, FP8, 6-bit, or BF16 membership
because choosing that membership is the purpose of this gate. Every
difference is explicit in the immutable weight-policy digest and physical
budget; a result may not relabel a precision change as a kernel-only speedup.

The `quality_control` is selected before qualification on a separate frozen
control-selection corpus that is disjoint from calibration and qualification.
It is the capacity-feasible control with the lowest mean KLD on that corpus.
Ties within `1e-6` are resolved by p99 KLD, then the frozen task aggregate
from a disjoint control-selection task subset, then lexicographic policy
digest. The evaluator writes the complete candidate
set, selection-corpus hash, metrics, tie breaks, selected policy digest, and
selection code hash into `control_selection_manifest_sha256`; the selected
digest is also stored directly in `QualityRun.v2`. It is never reselected on
qualification rows or inside bootstrap replicates.

For the logit corpus, one bootstrap block is exactly one frozen 2,048-token
window. All positions from a sampled window remain together. Each stratum is
resampled independently with its original window count, and the combined
replicate concatenates those resampled strata. The 8-window strata are
reported as low-block-count estimates and receive no asymptotic substitution.

Over exactly 10,000 stratified window-bootstrap replicates with the pinned
SplitMix64 seed, the candidate must satisfy these one-sided 95% upper
confidence bounds. Replicate results are sorted by value then replicate
ordinal; the upper endpoint is nearest-rank element `ceil(0.95*10_000)`.

The logit bootstrap uses `QualityRun.v2.logit_bootstrap_seed`; the task
bootstrap uses `QualityRun.v2.task_bootstrap_seed`. Each has an independent
counter that starts at zero. Output `x` for each counter is exactly the
wrapping-u64 mapping in `docs/distributed-sampling-abi-v1.md`: add
`(counter+1)` times
`0x9e3779b97f4a7c15`, then the xor/shift/multiply constants
`0xbf58476d1ce4e5b9` and `0x94d049bb133111eb`, then the final xor-shift.
The counter increments once per generated `x`. Logit-stratum discriminants
are natural text `0`, reasoning/code `1`, tool/JSON `2`, multilingual `3`,
and long-context `4`. Task-stratum discriminants are the seven inferential
rows below in table order, `0..6`. Within each independent stream, iteration
order is replicate ordinal, stratum discriminant, then sample ordinal.
For a stratum of `n` blocks, bounded selection computes
`threshold=(0u64.wrapping_sub(n)) % n`, advances SplitMix64 until output
`x>=threshold`, and selects `x%n`. Rejected draws still advance the stream.
This rule is also used for task-item bootstrap with the task-stratum order
declared below.

| Ratio, candidate / quality control | Maximum upper bound |
|---|---:|
| mean KLD | `1.02` |
| p95 KLD | `1.05` |
| p99 KLD | `1.10` |
| stable-mismatch rate | `1.02` |
| tie-adjacent rate | `1.25` |

For KLD metrics, a replicate with control denominator zero has ratio `1` when
the candidate numerator is also zero and positive infinity otherwise. For
stable/tie rates only, the ratio statistic uses the fixed Jeffreys transform
`(events+0.5)/(rows+1)` separately for candidate and control before division;
absolute rate gates continue to use unsmoothed counts. No replicate is
dropped, clamped, retried, or replaced. Positive infinity sorts after every
finite value and therefore fails any finite upper bound when it reaches the
95th percentile.

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
Any hosted judge is pinned by an immutable provider/model revision digest and
an exact request/response schema digest; a model name or API alias is
insufficient.
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

One task bootstrap block is one paired item. No prompt, conversation, test,
language, context band, or generated response is split across blocks. The
manifest declares any internal sub-strata; resampling preserves each
sub-stratum's original item count. Candidate and control outcomes for the
same item are always selected together.

The item score is the pinned scalar in `[0,1]`: exact correctness/pass is
`0` or `1`; a declared semantic checker may use another exactly serialized
binary64 score. The long-generation success score is `1` only when the
response reaches its pinned valid completion/EOS state within limit with no
repetition, parser, UTF-8, JSON, or tool violation listed below; otherwise it
is `0`. For each of the seven inferential primary strata below, the observed
statistic and every bootstrap statistic are the arithmetic mean of
`candidate_score-control_score`, summed in ascending item ID with binary64
round-to-nearest-even. Over exactly 10,000 paired, stratified replicates, the
uncorrected one-sided 95% lower endpoint (nearest-rank element
`ceil(0.05*10_000)` after ordering by value then replicate ordinal) must be
at least:

| Inferential stratum | Lower bound |
|---|---:|
| reasoning | `-0.025` |
| coding with executable tests | `-0.030` |
| tool selection/arguments | `-0.020` |
| JSON/schema constrained output | `-0.020` |
| long generation/repetition success | `-0.015` |
| randomized long-context retrieval | `-0.010` |
| termination/parser behavior | `-0.015` |

These are deliberately tight quality-noninferiority margins, not an
expectation of near-zero paired discordance. Frozen retrieval is a separate
deterministic hard gate: the candidate must answer every one of its 1,000
items exactly at every assigned band, so no zero-margin bootstrap CI is
constructed for it.

Randomized retrieval must additionally have absolute accuracy at least
`0.99`. Frozen and randomized retrieval pass separately at 16k, 64k, 128k,
480k, and 1,048,576 positions. The full retrieval gate is required for MTP0
and every depth proposed for production; its compute cost does not weaken or
sample the 1M band.

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

For each of the seven inferential strata with noninferiority margin `m_i`,
let observed paired difference be `d_i` and replicate difference be
`d_i,b`. Its one-sided centered-bootstrap p-value for null
`d_i <= m_i` is:

```text
p_i = (1 + count_b((d_i,b - d_i) <= (m_i - d_i))) / 10001
```

Holm step-down correction uses family-wise `alpha=0.05`. Sort by
`(p_i, fixed_stratum_discriminant)` ascending. At sorted ordinal `j=1..7`,
reject the null only when `p_(j) <= 0.05/(7-j+1)` and every earlier null was
rejected; stop at the first failure. The adjusted p-value is
`min(1, max_{k<=j}((7-k+1)*p_(k)))`, mapped back to its original stratum.
Each stratum must pass both its uncorrected lower-bound gate and Holm. Retain
all replicates, uncorrected endpoints, raw p-values, adjusted p-values,
ordering, comparisons, and decisions. There is no undefined or mislabeled
"Holm-corrected confidence interval."

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
| tie-adjacent rows | `131 / 131,008`, Wilson upper `0.0012` |
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
The target concurrency band is frozen in `QualityRun.v2` through the matched
benchmark-plan digest before the first performance sample is taken.

## Failure, retry, and publication

A run is invalid if it has a missing row, duplicate row, changed prompt,
changed token history, unexpected cache hit, rank restart, model retry,
unrecorded fallback, nonfinite logit, evaluator warning, or hash mismatch.
Invalid rows are not silently dropped or imputed.

The rank-restart rule is intentional even before the first scored row: a
restart creates a new run UUID and attempt rather than a continuation. This
keeps failure handling independent of when scoring happened and retains the
failed attempt in the append-only ledger.

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
10. bootstrap seed plus stratified window and paired-item resampling
    stability;
11. control selection and zero denominators;
12. Holm correction;
13. missing, duplicate, reordered, and cross-run rows;
14. all task noninferiority boundaries;
15. every MTP depth and row-bucket pairing; and
16. byte-stable manifests and output digests.

It additionally covers:

17. extreme finite logits at and beyond the pinned MPFR exponent range,
    including mandatory underflow/overflow/flag rejection;
18. zero-control-event bootstrap replicates for both KLD and Jeffreys-smoothed
    event-rate ratios;
19. nearest-rank combined and per-stratum quantiles with exactly eight
    windows; and
20. a third-token intrusion that must classify `STABLE_MISMATCH` rather than
    `TIE_ADJACENT`.

CPU proof does not begin until adversarial review accepts this contract.

## Revision-2 correction ledger

This revision changes no measured result and claims no evaluator
implementation. It resolves the first adversarial review as follows:

- M1: the 2,047-row runtime gate now honestly permits two tie rows with a
  `0.003` Wilson ceiling; every MTP depth uses exactly 131,008 paired
  teacher-forced rows, at most 131 ties, and a `0.0012` ceiling;
- M2: task margins are resized by stratum, frozen retrieval is an exact hard
  gate rather than a zero-margin CI, and the full 1M retrieval matrix remains
  mandatory;
- M3: window/item resampling, unbiased bounded draws, zero denominators,
  Jeffreys event ratios, centered-bootstrap p-values, Holm ordering, adjusted
  p-values, and the absence of a fictitious corrected interval are exact;
- M4: MPFR precision, rounding, exponent range, exception handling, every KLD
  intermediate, and ascending accumulation order are normative; and
- M5: the engine, sampling ABI, benchmark contract, and native plan now point
  to one quality definition and explicitly distinguish offline logit assembly
  from production sampling.

Run identity now includes exact strings, rank-ordered hardware, sampling and
seed manifests, MPFR identity, preregistered control selection, and the
matched benchmark plan. Bootstrap units, control selection, language reports,
hosted-judge identity, restart policy, and the four requested CPU fixtures are
explicit. The historical unpinned KLD mean was removed.
