# Fable handoff: quality acceptance v1 r2

Date: 2026-08-03

Status: corrective adversarial design review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4, launch CUDA, read model/checkpoint payloads, or execute
any vLLM resource. This is an immutable-source, numerical, statistical, and
CPU-design review only.

Review candidate commit:
`eb62b3d138880e7bfcacec74f975de5a017cd977`

Required result path:
`fable-quality-acceptance-v1-r2.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`quality-acceptance-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Withhold the token for any mismatch or incomplete input.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/quality-acceptance-v1.md` | `705bb0611464bd5d76a08943b3122ecb8a78506e78f9c20a46d4e1ce24fc7be6` |
| `spec/engine-v0.md` | `52497e022bde5278372a9bce168e87a602fca9341c4cb4e019b4a3c7ce63179b` |
| `docs/benchmark-contract.md` | `024eda56d7bb7632c5023d4d0e8f095bd7b32a05ed241df1f8a13369ce5a3ebe` |
| `docs/native-engine-plan.md` | `493c0d218d93a3a8d7cf83da45a934fc44570fc190e85340c5eaba74edd50bdd` |
| `docs/distributed-sampling-abi-v1.md` | `383e328a527cc780ed553af0b78382cf200ad60f97afb26d96a2a1494b57c89b` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `docs/tokenizer-serving-contract.md` | `9e9c82d2375d3f759b10568fdb33bc75d102f8a2f609ddbc6f980036c81b7527` |
| `profiles/profile-budget-v0.json` | `cdbe4eaad9465181b2ba60b3656fe5207eee54467abfbf8d9bc398c3e68c23e0` |
| `crates/glm-cli/src/review.rs` | `82afcab11d2bc19978c39ed561204368ffb27cc2f9679bd24da7aa4de14a17d5` |
| `docs/review-provenance-verifier-v1.md` | `c4be2415ad0b13cea7fc154ce10c7aea839bd47b57af3e42fa6f329b92f3cb4e` |
| `docs/cn4-experiment-isolation-v1.md` | `aab1dc4860fd2dde21e19b067b211f842387436d3d92a48b2fb31037a945d735` |
| `docs/matched-runtime-control-v1.md` | `446e25396e7eabd2fce85aa848c70318f964b1a9a7cf02a4945acc9917c02bf8` |
| `docs/local-inference-lab-decode-bench-20260803.md` | `cd1dfed287c04ea93af399b56fc74a67213850110ca2f5d9bdb09e17ffd36c77` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

Run the complete CPU-only gate and record its exit status:

```text
./scripts/local-checks.sh
```

## Exact prior-procedure source check

The separately maintained `../glm52-opt` working tree may be dirty. Do not
review its working-tree bytes. Create a detached worktree at commit
`38cba1091c043bdecd426a0d4625f58211f94e0c` and verify these exact files:

| External input at exact commit | SHA-256 |
|---|---|
| `harness/run_glm52_tr3_dynamic_kld.sh` | `63c02bd1156ef8db49f9c0fc7d3d80fdbf46f9331f9499d7c334f37cca9ac55a` |
| `harness/prefill_kld_fallback_integrity.patch` | `19b183d48322ceb6d680e9234a1bf14a98343413d970853acf4f96347fe7e9f0` |
| `harness/prove_kld_repeat_determinism.py` | `690225fa94334ac830c8a1ae8b7d5789137ae439921792e76c12682b7786e39d` |
| `experiments/2026-07-28-v20-nvfp4-scaling-kld-n3/README.md` | `0bd879ea07d8b6be00271c2736b2c15b20cac9cf2ea27b5a3261f19beff56524` |
| `experiments/2026-07-28-cn4-tr3-qualification/README.md` | `c54bf499b8edb5d5886daa372ad34025ebc652f6545c45b353e1b389bdd09fff` |

Verify that the wrapper itself pins runner
`d1dc1a63b9889e881f3bd899638d0ec65a1a1079132f6a207a600d9cba845405`,
reference logits
`87f992a689c054a0548a4b3863da6c809f9239beacd5786d0401e45904fec063`,
reference manifest
`985120136741037918bcd4dc8da9813c1f6268b35a730302f99cf6b3eebb7606`,
and historical image
`sha256:a5608e0b4a2fcdaec476de79fbe5cf2f6e9ce2ecf30bf2dfe0c1314d97c6666e`.
Do not execute that image or access cn4.

## Required independent work

Do not accept the correction ledger by prose inspection alone. Independently:

1. serialize `QualityRun.v2`, `String.v1`, `GpuIdentity.v1`, and
   `PositionQuality.v2`; mutation-test ordering, strings, NFC, hardware,
   sampling/seeds, MPFR identity, control selection, and benchmark-plan fields;
2. recompute the Wilson bounds using the exact binary64 sequence and verify
   that 2/2,047 is below `0.003` and 131/131,008 is below `0.0012`, while the
   immediately larger disallowed counts fail the raw-count gates;
3. implement an independent small-vocabulary MPFR oracle using precision 256,
   `emin/emax=+/-1,000,000`, exact operation order, ascending accumulation,
   and flag handling; try regrouping/reordering and extreme-logit mutations;
4. prove that inexact transcendental flags are legal while zero exponential,
   underflow, overflow, NaN, range, divide-by-zero, nonfinite, zero mass, and
   final negative KLD fail closed;
5. enumerate the exact 64-window/131,008-row teacher-forced MTP corpus and
   prove a generated-history divergence cannot enter a paired numerical row;
6. reproduce both independent SplitMix64 streams, rejection sampling, window
   and paired-item block construction, nearest-rank endpoints, and all 10,000
   replicate orderings;
7. exercise KLD zero denominators and event-rate zero counts, including the
   Jeffreys transform and positive-infinity ordering, with no dropped sample;
8. independently derive every task margin's equal-quality discordance
   tolerance at its minimum item count and decide whether each is strict but
   statistically usable rather than a false-negative machine;
9. derive the centered-bootstrap p-values, fixed tie ordering, seven-member
   Holm step-down decisions, and adjusted p-values from synthetic fixtures;
10. trace control selection to prove calibration, selection, and qualification
    are disjoint and the selected digest cannot change inside bootstrap;
11. verify frozen and randomized retrieval, per-band 1M work, behavior
    classes, per-language diagnostics, restart semantics, and all 20 CPU
    proof fixtures are unambiguous;
12. reconstruct the prior-cn4 operator sequence from the five external files
    and distinguish the non-gating legacy field from the normative MPFR field;
13. prove the dedicated GLMAXX evaluator requirement cannot reuse or mutate a
    vLLM worktree, image, container, cache, volume, service, or result; and
14. trace engine, sampling, benchmark, native-plan, MTP, tokenizer, capacity,
    and performance pointers for a remaining contradictory quality definition.

## Required decisions

Answer every decision with an unqualified `YES` or `NO`:

1. Is runtime correctness still distinct from compressed-weight quality and
   impossible to pass by changing weights, cache posture, prompts, or sampling?
2. Are `QualityRun.v2` and `PositionQuality.v2` complete, byte-deterministic,
   hardware/sampling/seed/MPFR bound, and safe from path or string ambiguity?
3. Are top-token ordering, `TIE_LOGIT`, top-two-set membership, and third-token
   rejection exact and consistent across the engine and sampling contracts?
4. Are the two Wilson gates coherent with their exact denominators, counts,
   formula, binary64 constant, and acceptance ceilings?
5. Does the teacher-forced MTP corpus contain exactly 131,008 valid paired rows
   per depth without comparing logits after generated histories diverge?
6. Is the MPFR KLD sequence complete and bit-reproducible, including every
   intermediate, rounding, exponent setting, accumulation, and final round?
7. Are mask, padding, zero-mass, extreme-logit, and MPFR flag semantics total
   without rejecting ordinary inexact transcendental results?
8. Are absolute smoke/multi-window gates and every per-position/raw retention
   rule still discriminating, with no mean-only or language-omission escape?
9. Is quality-control preregistration deterministic, disjoint, capacity-aware,
   and free of qualification-corpus selection or bootstrap reselection bias?
10. Are both bootstrap streams, unbiased bounded draws, block units, stratum
    ordering, quantiles, zero denominators, and infinite values exact?
11. Are the seven task margins feasible at minimum counts while retaining the
    requested quality-first posture, and is frozen retrieval correctly a hard
    exact gate rather than an undefined zero-margin CI?
12. Are the centered-bootstrap p-values and Holm correction independently
    implementable with no undefined ratio, interval, ordering, or edge case?
13. Are task, retrieval, repetition, parser, EOS, hosted-judge, and 1M-band
    rules complete enough to prevent a quality regression from averaging out?
14. Are the MTP1-6 quality gates complete and blocked behind target-only MTP0
    plus the separately reviewed sampling mechanics?
15. Is the performance-enable band frozen before sampling and unable to trade
    quality, KV capacity, or latency for aggregate throughput?
16. Does offline full-vocabulary assembly preserve the production no-gather
    rule and remain excluded from latency/throughput measurements?
17. Does the legacy diagnostic faithfully preserve prior-cn4 comparability
    while remaining non-gating and visibly distinct from normative MPFR KLD?
18. Does the cn4 isolation rule prevent any GLMAXX review or future diagnostic
    from executing or reusing ongoing vLLM resources or result paths?
19. Does revision 2 resolve every M1-M5, minor, and question from
    `fable-quality-acceptance-v1.md` without weakening provenance or silently
    changing a prior result?
20. Are all evaluator, CUDA, model, KLD-result, task, retrieval, capacity,
    cold-start, latency, throughput, and serving nonclaims accurate?
21. Does acceptance open only CPU evaluator implementation/proof and preserve
    the required gate order before model execution or performance claims?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, followed
by the independent derivations and all twenty-one decisions. Only if every
decision is `YES`, attest the candidate commit, all fourteen repository input
hashes, the external commit, and all five external hashes, then end with the
requested token as the only bare acceptance line.

Acceptance opens only a CPU `QualityRun.v2`/`PositionQuality.v2`, numerical,
bootstrap, task-statistics, and legacy-diagnostic validation proof. It does
not accept an implementation, authorize cn4 or CUDA, execute a model, enable
MTP, permit conversion, or establish KLD, task, retrieval, capacity,
cold-start, latency, throughput, or serving evidence.
