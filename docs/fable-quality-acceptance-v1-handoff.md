# Fable handoff: GLM-5.2 quality acceptance v1

Date: 2026-07-29

Status: adversarial design review; evaluator and implementation tokens
withheld by Sol

GPU authorization conveyed by this handoff: none

Review candidate commit:
`70222ab17ea5c10bdb5a68e98a1a5839a040eec9`

Requested acceptance token, only if every blocker and major is resolved:
`quality-acceptance-v1-accepted`

## Provenance

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/quality-acceptance-v1.md` | `3f87cd128b633d6812dce31fb6f3bfbd700debae587a32350e0cb46e24a6e1e9` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/benchmark-contract.md` | `cd51d22a8faf2baacfb4682ff5e1dcb5986edc27d8aa3af188105842bb49a507` |
| `docs/native-engine-plan.md` | `33552cd81e3d79b8b484856a99620420f3e2eddfdfa529a23b191353a702ed80` |
| `docs/distributed-sampling-abi-v1.md` | `d717508e4d90f6ef378d486c0bd3e93e7dad522e6529b8504ccb687a0280fdce` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `docs/tokenizer-serving-contract.md` | `9e9c82d2375d3f759b10568fdb33bc75d102f8a2f609ddbc6f980036c81b7527` |
| `docs/production-punchlist.md` | `67d2bbbf5b6a17631dcd2ec19d7513374c53c13e29124086d35b6a4d361a0964` |
| `profiles/profile-budget-v0.json` | `cdbe4eaad9465181b2ba60b3656fe5207eee54467abfbf8d9bc398c3e68c23e0` |
| `crates/glm-cli/src/review.rs` | `d2c2d2756b94df8fb5555f578e7c907bef7c09b7b10fb3f310f45566f73c1c45` |
| `docs/review-provenance-verifier-v1.md` | `c4be2415ad0b13cea7fc154ce10c7aea839bd47b57af3e42fa6f329b92f3cb4e` |

Run this fail-closed check before reviewing:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-quality-acceptance-v1-handoff.md
```

Hash every input at review start and finish. Review the exact candidate
commit in a separate worktree if `main` advances. The candidate deliberately
contains no evaluator implementation, dataset bundle, checkpoint result,
quality result, or GPU evidence.

## Requested adversarial questions

1. Does the contract correctly separate same-weight runtime correctness from
   compressed-policy quality against BF16, or can either path hide a
   precision or kernel change?
2. Is the quality-first lexicographic profile selection consistent with the
   user's priorities while retaining a later minimum performance gate?
3. Can the run-family identity accidentally compare different model,
   tokenizer, chat-template, sampling, cache, TP/DCP, or evaluator bytes?
4. Is the content-derived UUID field order complete and unambiguous? Identify
   any missing model/runtime identity that must be hashed.
5. Are 154,880 physical rows, 154,856 logical tokens, and 24 masked rows
   exact? Can a finite padded logit reach any quality or sampling operation?
6. Does retaining full-vocabulary raw arrays, rank-shard hashes, token
   histories, and every per-position record prevent mean-only or
   reassembly-biased evidence?
7. Is centered-logit error correctly invariant only to a constant shift, or
   does it hide another quality-relevant transformation?
8. Is descending FP32 total-order logit plus ascending token ID a complete
   deterministic top-token rule at signed zero, infinities, and exact ties?
9. Re-derive the `2^-8` tie threshold and every
   `MATCH/TIE_ADJACENT/STABLE_MISMATCH` predicate. Can a third token or a
   large candidate error be mislabeled tie-adjacent?
10. Are equality-at-threshold and exact FP32-to-FP64 promotion sufficiently
    unambiguous?
11. Is zero same-policy stable mismatch plus the stated tie rate/Wilson bound
    appropriate for runtime correctness without demanding batch-invariant
    kernels?
12. Does the BF16 comparison correctly treat stable/tie rates as measured
    compression quality rather than implementation correctness?
13. Is 256-bit MPFR, fixed token order, round-to-nearest, and one final
    binary64 rounding a reproducible full-vocabulary KLD definition?
14. Can an independent implementation truly reproduce every KLD binary64
    bit? If not, specify the minimum defensible cross-check tolerance and
    provenance.
15. Does the KLD definition handle `-inf` masks, extreme logits, underflow,
    normalization, and a mathematically tiny negative result without
    silently clamping corruption?
16. Are the smoke ceilings precommitted but realistically discriminating?
    Identify any threshold that is either impossible for a useful compressed
    model or permissive enough to admit obvious degradation.
17. Recompute `64 × 2,047 = 131,008` scored multi-window positions and verify
    every stratum minimum and long-context placement.
18. Are combined and per-stratum KLD/stable/tie ceilings sufficient to
    prevent a good mean from hiding a language, tool, reasoning, or
    long-context tail failure?
19. Can prompt-block bootstrap resampling with 10,000 fixed-seed replicates
    provide valid one-sided confidence for mean, p95, p99, and rare
    divergence ratios? Should windows or corpus sources be the block unit?
20. Are the `1.02/1.05/1.10` KLD and stable/tie noninferiority ratios
    coherent at zero and near-zero denominators?
21. Does choosing the lowest-mean capacity-feasible control create selection
    bias when the same qualification corpus chooses and compares the control?
    If so, require a preregistered or multiplicity-corrected alternative.
22. Is excluding an all-NVFP4 profile that cannot meet the capacity contract
    correct while retaining it as a kernel/operator control?
23. May controls vary EXL3/NVFP4/FP8/6-bit/BF16 membership as stated, and are
    immutable policy digests plus byte budgets enough to prevent a precision
    change being sold as a kernel-only speedup?
24. Are the task item minimums and paired noninferiority margins statistically
    meaningful for reasoning, coding, tools, JSON, retrieval, repetition,
    and termination?
25. Does the contract sufficiently pin judge prompts/models, deterministic
    checkers, few-shot policy, answer extraction, and hidden data while
    preserving reproducibility?
26. Is Holm correction over the eight primary strata the correct family, and
    are both the bootstrap confidence construction and correction order fully
    specified?
27. Is exact frozen retrieval plus 99% randomized retrieval at every 16k,
    64k, 128k, 480k, and 1M band feasible and strong enough?
28. Can prefix reuse, data leakage, prompt memorization, or a retrieval answer
    outside the permitted context falsely satisfy the long-context gate?
29. Are the no-new-failure classes for UTF-8, JSON, tools, parser state, EOS,
    repetition, token limits, and retrieval exhaustive?
30. For MTP1–6, are zero stable mismatch, the tie bound, KLD ceilings, and
    centered top-two error thresholds sufficient to allow different verifier
    row buckets without accepting material output drift?
31. Must each MTP depth run the full task suite, or can a statistically valid
    staged subset precede the full enablement gate?
32. Does the MTP record retain enough target/draft state to distinguish a
    verifier numerical difference from proposal, acceptance, residual,
    bonus, EOS, RNG, or rollback error?
33. Is keeping probabilistic MTP blocked on the distributed-sampling review
    sufficient, or does this contract need additional distributional
    thresholds?
34. Are 5% useful-throughput improvement and 5% regression ceilings
    appropriate for default MTP enablement, and must every depth/context cell
    pass independently?
35. Can invalid rows, retries, cache hits, rank restarts, or fallbacks bias a
    run under the fail-whole-run rule? Identify any event that should be
    recoverable rather than invalidating.
36. Does the 16-item CPU proof matrix cover every arithmetic, statistics,
    manifest, and MTP boundary needed before model execution?
37. Which thresholds or record schemas must version atomically with the
    distributed-sampling ABI, tokenizer contract, benchmark contract, and
    serving observability?
38. Does any rule conflict with the normative engine specification or require
    an explicit engine-v0 amendment before CPU proof?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Withhold
the token unless every blocker and major is resolved. State separately
whether:

- the runtime-versus-quality separation is accepted;
- the tie-adjacent classification and threshold are accepted;
- the MPFR KLD definition is accepted;
- the smoke and multi-window numerical thresholds are accepted;
- the control-selection and noninferiority procedure are accepted;
- the task/retrieval/behavior gates are accepted;
- the MTP1–6 quality and performance-enable gates are accepted;
- a CPU evaluator implementation may begin;
- full-checkpoint conversion and MTP enablement remain blocked; and
- no cn4 access or GPU launch is authorized by the verdict.
