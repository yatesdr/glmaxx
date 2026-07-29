# Fable handoff: deterministic generated quality corpus v1

Date: 2026-07-29

Status: adversarial generator-design review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`27fa48eab3d584920e8925abcbcc839be33a6485`

Required result path:
`fable-generated-quality-corpus-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`generated-quality-corpus-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and withhold the token on any stale or
missing input.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/generated-quality-corpus-v1.md` | `1d867dd19bad0dd2e3805bb0b82d97bb2de45849ead3e8f21377b559eccab06c` |
| `docs/quality-corpus-manifest-v1.md` | `93be881749fee8faab8d1d3191a8c12041d4fb5f7a41c0acc7fa450f996d8df5` |
| `manifests/quality-corpus-sources-v1.json` | `40c5068c38f792b28b28b7ffdf2d52f7df866a0ecb25a50fb5b3bb8f390bb8d5` |
| `docs/quality-acceptance-v1.md` | `3f87cd128b633d6812dce31fb6f3bfbd700debae587a32350e0cb46e24a6e1e9` |
| `fixtures/tokenizer-contract-proof-v1.json` | `bb0a29719ffc69e6676ac3edf156ea47ff6dc6e1424a0d866fbd5d2d76db5223` |
| `docs/tokenizer-serving-contract.md` | `9e9c82d2375d3f759b10568fdb33bc75d102f8a2f609ddbc6f980036c81b7527` |
| `docs/serving-observability-v1.md` | `4058d01d58c0d8f4d7222803e05577a9419cfa6f5d0f20a65c41e9e2779213e6` |
| `docs/production-punchlist.md` | `219097163ef8dc0c3589e98d8a436c1d6f651c2b8ef4e4371d2c8de64ff0c4bd` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-generated-quality-corpus-v1-handoff.md
```

## Review boundary

This is a design review only. There is no generator, materialized corpus,
evaluator, secret seed, model output, quality result, or GPU evidence.

The token, if earned, opens CPU generator implementation only after the
parent quality and source tokens are present. It does not accept that later
implementation or authorize any model to see qualification prompts.

Qualification retrieval seeds are intentionally absent. Do not create,
request, guess, or insert them during this review. A review-only non-secret
fixture key belongs to the later CPU proof.

## Required adversarial questions

1. Is `GeneratedCorpusIdentity.v1` complete, byte-unambiguous, deterministic,
   and independent of wall clock, host paths, random UUIDs, map order, and
   process scheduling? Can a different tokenizer, template, generator,
   source recipe, case index, or secret commitment retain the same identity?
2. Re-derive the public seed-set digest and every HMAC message field. Are
   little-endian seed expansion, length prefixes, labels, attempts, family,
   and ordinal sufficient to prevent cross-field or cross-stratum collisions?
3. Is bounded-integer rejection unbiased and implementable at `n=1`,
   non-power-of-two `n`, and near-`2^64` bounds? Does computing
   `floor(2^64/n)*n` in a wider type close the `u64::MAX` off-by-one?
4. Are case IDs, expected-record hashes, index ordering, and the exact 3,500
   count complete? Can a control-relative expectation be changed after
   seeing control output while retaining its hash?
5. Is the restricted canonical JSON definition byte-complete for generated
   sources, schemas, targets, and manifest inputs? Does the model checker
   correctly allow legal member order/whitespace while rejecting duplicate
   keys, suffixes, invalid UTF-8, ambiguous `oneOf`, and semantic drift?
6. Recompute every JSON family count and field range. Identify any schema
   that rejects its own expected value, accepts the wrong typed value,
   disagrees with Draft 2020-12 string length, or depends on Unicode
   normalization or unordered maps.
7. Are the fixed Unicode atoms and without-replacement algorithm
   unambiguous at control characters, combining marks, composed characters,
   UTF-8, scalar counts, and JSON escaping?
8. Does the long-generation cross product produce exactly 500 cases and an
   exact target between 75% of the limit and `limit-2` for every family and
   band? Can adding a record make a grouped target non-monotonic, exceed
   100,000 records, or make the "largest fitting" rule ambiguous?
9. Are all five long transforms independently checkable without a judge?
   Can source/target terminator collision, integer overflow, output
   normalization, or a periodic expected target make an invalid output pass?
10. Does repetition detection begin at the correct divergence boundary,
    avoid expected-target false positives, find every period 1–32
    deterministically, and retain enough evidence for paired control
    comparison?
11. Are frozen and randomized retrieval seed timing, commitments, access,
    disclosure, and domain separation sufficient to prevent tuning,
    memorization, seed reuse, or giving control and candidate different
    prompts? Is HMAC of a fixed domain an adequate key commitment?
12. Recompute retrieval counts: 200 cases × five bands × two strata. Are 40
    cases in each of five position bins and the global ordinal/family mapping
    exact?
13. Does `prompt_tokens = context_band - 64` correctly model the full
    1,048,576-token limit including chat-template and generation prompt?
    Identify any off-by-one involving BOS, EOS, answer tokens, or the
    64-token reserve.
14. Is target placement implementable and deterministic? Can whole-record
    insertion overshoot, pre-target lexemes, context-sensitive BPE merges, or
    solver ordering move a target outside its bin or make two
    implementations choose different prompts?
15. Is the safe-lexeme solver bounded in both remaining gap and search state
    at 1M context? Does independent token length plus full-prefix
    re-tokenization admit exponential behavior or a hidden assumption about
    token additivity? Propose a stricter algorithm if necessary.
16. Can filler or distractor construction accidentally expose the answer,
    lookup key, derivation labels, master seed, or another case? Are raw,
    normalized, case-folded, token, n-gram, cross-case, and calibration
    leakage scans both sufficient and implementable?
17. Does requiring every answer substring of 16 hex characters exactly once
    handle self-overlap and cross-answer collisions without a retry rule that
    changes the corpus silently?
18. Is the negative retrieval proof meaningful, or can its checker
    trivially "recover" an answer from oracle metadata? State exactly which
    fields must be excluded or amend the contract.
19. Are retrieval output trimming, exact-answer, terminal, prefix/page/tier,
    band, and position-bin records sufficient to catch answer leakage, cache
    posture drift, or a good aggregate hiding a failed 1M tail?
20. Do the five termination families make exactly 500 cases and exercise
    ordinary EOS, limit, tool, reasoning, and UTF-8 boundaries without
    relying on an unspecified parser? Can control-relative early EOS hide an
    absolute engine bug?
21. Are all three EOS IDs, stop/length distinction, incremental UTF-8
    buffering, post-terminal rejection, tool closure, reasoning closure, and
    hidden-reasoning publication covered consistently with the tokenizer and
    observability contracts?
22. Can two isolated implementations actually reproduce every public-seed
    byte and review-fixture retrieval byte from the prose alone? Identify any
    missing literal template byte, label, ordering, numeric rendering,
    schema keyword, tokenizer option, or target construction rule.
23. Does the external output tree and atomic publication prevent partial,
    stale, duplicate, path-traversal, symlink, hard-link, secret-in-Git, and
    crash-remnant corpora from entering a quality run?
24. Are the gate order and exclusions honest? Confirm that no generator,
    evaluator, corpus, model-quality result, GPU evidence, cn4 authorization,
    or checkpoint-conversion permission exists at the candidate.

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then state
separately whether:

- common identity, HMAC derivation, and bounded integers are accepted;
- JSON/schema construction and checking are accepted;
- long-generation construction and repetition accounting are accepted;
- frozen/randomized retrieval construction, secrecy, placement, and leakage
  rejection are accepted;
- termination/parser construction and accounting are accepted;
- deterministic publication, negative proofs, and external-secret handling
  are accepted; and
- the design may enter CPU generator implementation only after both parent
  tokens are present.

Only if all seven answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, an underspecified byte,
unbounded algorithm, answer leakage, secret-handling gap, or a generator that
cannot be implemented independently.

The token accepts only this design. It does not accept any implementation,
materialized prompt, evaluator, corpus result, model output, quality result,
GPU work, cn4 access, MTP depth, or checkpoint conversion.
