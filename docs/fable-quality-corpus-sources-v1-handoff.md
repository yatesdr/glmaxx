# Fable handoff: quality corpus sources v1

Date: 2026-07-29

Status: adversarial source-provenance and selection review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`83fb3747ae5d5ae996edb4784f775836e7c1a3e6`

Required result path:
`fable-quality-corpus-sources-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`quality-corpus-sources-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof` on this
handoff, hash every input at review start and finish, and report a stale
candidate without the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/quality-corpus-manifest-v1.md` | `93be881749fee8faab8d1d3191a8c12041d4fb5f7a41c0acc7fa450f996d8df5` |
| `manifests/quality-corpus-sources-v1.json` | `40c5068c38f792b28b28b7ffdf2d52f7df866a0ecb25a50fb5b3bb8f390bb8d5` |
| `docs/quality-acceptance-v1.md` | `3f87cd128b633d6812dce31fb6f3bfbd700debae587a32350e0cb46e24a6e1e9` |
| `fixtures/tokenizer-contract-proof-v1.json` | `bb0a29719ffc69e6676ac3edf156ea47ff6dc6e1424a0d866fbd5d2d76db5223` |
| `docs/tokenizer-serving-contract.md` | `9e9c82d2375d3f759b10568fdb33bc75d102f8a2f609ddbc6f980036c81b7527` |
| `docs/production-punchlist.md` | `1a51a0dddadcf3f7ff6de255be48b5d3a184b259ba264127d6814785c16ae12d` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-quality-corpus-sources-v1-handoff.md
```

## Independent reproduction inputs

Do not trust the preparation prose or query `main`. Fetch only the exact
revisions and paths in the source manifest. Recompute every listed content
SHA-256. The total ungated payload is approximately 7.5 MB; no model weights
or GPU are involved.

The expected independently parsed facts are:

```text
MMLU-Pro test rows: 12032, unique question_id: true
MMLU-Pro validation rows: 70
MMLU-Pro selected reasoning IDs: 1000
MBPP rows: 974
MBPP task IDs 11..510 inclusive: 500
HumanEval IDs HumanEval/0..HumanEval/163: 164
BFCL simple_python question/answer IDs: 400/400, equal sets
BFCL multiple question/answer IDs: 200/200, equal sets
BFCL parallel question/answer IDs: 200/200, equal sets
BFCL parallel_multiple question/answer IDs: 200/200, equal sets
BFCL selected offline AST IDs: 500
WikiText raw test rows/UTF-8 bytes: 4358/1287656
WikiText raw validation rows/UTF-8 bytes: 3760/1144248
```

Reimplement both domain-separated ranking rules without copying Sol's audit
script. Reproduce these ordered selected-ID stream digests:

```text
reasoning:
e6be6b3307b9f953ebe837852fe8fee7c7962c524e430023abd7af7d424f2c7a

MBPP primary coding:
e7d3b431215d4931cccdd2092a8e37dbc07203253c9041a3df70719b6d48d467

HumanEval diagnostic:
0dbbca61baa0b9b486debc99ea894681688e1950f93f1a0171ccd2a7adea114e

BFCL offline tools:
2d5633bf2c912ce0d98185a019071f8b2c56d8b804e68d80113a38e6e494b25b
```

FLORES+ is license-gated. Do not bypass access control, accept terms on
another person's behalf, guess file content hashes, or treat Git blob SHA-1
values as SHA-256. Metadata-only validation of its exact repository revision,
paths, Git blobs, sizes, declared license, and gated posture is sufficient for
this review.

## Review boundary

This gate covers only source identity, licensing/access posture, source
arithmetic, deterministic reasoning/coding/tool selection, and the
fail-closed distinction between a source recipe and a materialized corpus.

It does not accept:

- the still-pending parent quality acceptance contract;
- generated JSON, repetition, retrieval, or termination corpora;
- authorized FLORES+ content bytes;
- tokenizer-dependent logit windows;
- any prompt renderer, sandbox, checker, evaluator, or container;
- a `glmaxx.quality-corpus.v1` materialized manifest;
- any model result, quality threshold, checkpoint conversion, or MTP depth;
  or
- cn4 access, CUDA work, or GPU evidence.

## Required adversarial questions

1. Are every repository URL, exact revision, file path, size, content
   SHA-256, Git blob, license, and gated flag internally coherent? Identify
   any source identity that still follows mutable "latest" state.
2. Does the source recipe correctly treat lm-evaluation-harness as a pinned
   prompt/config reference rather than a normative or transitive unpinned
   evaluator dependency?
3. Independently parse MMLU-Pro. Are all 12,032 test `question_id` values
   unique, do the category counts and quotas make 1,000 exactly, and does an
   independent implementation reproduce the reasoning stream digest?
4. Is the hash-ranked, near-equal category selection defensible against
   source ordering and category imbalance? Could category strings, integer
   rendering, digest ordering, tie breaking, or emission ordering diverge
   between implementations?
5. Do first-five validation few-shot rows per category remain disjoint from
   scored test IDs? Does the contract pin enough now to prevent the later
   materializer from changing prompt policy while retaining the same item
   selection?
6. Independently parse MBPP. Do IDs 11 through 510 exist exactly once and
   produce exactly 500 primary cases and the pinned stream digest? Is keeping
   HumanEval separate as a 164-case diagnostic necessary to avoid changing
   the primary paired coding metric?
7. Does the coding source contract sufficiently fail closed on untrusted
   model-generated code, or does any source-selection decision depend on a
   future sandbox detail that belongs in this gate?
8. Independently parse all eight BFCL files. Are question/answer ID sets
   equal, are category counts exact, and does taking 125 hash-ranked IDs per
   category reproduce the tool stream digest?
9. Are the four selected BFCL categories deterministic and network-free? Can
   excluded live, web, memory, multi-turn, or executable behavior leak into
   the selected files or checker through transitive metadata?
10. Does the source contract pin enough BFCL parser/checker identity to
    prevent semantic drift, while correctly leaving the eventual normative
    GLM renderer and evaluator outside this source-only token?
11. Are the WikiText files and row/byte counts sufficient source material for
    32 nonoverlapping 2,048-token natural windows without claiming tokenizer
    output that has not been materialized?
12. Are the exact tokenizer/config/template/bundle identities derived
    consistently with the existing tokenizer proof, and is the bundle digest
    construction sufficiently referenced rather than redefined
    ambiguously?
13. Is the FLORES+ handling fail-closed? Specifically, can any path promote
    metadata, a Git SHA-1, size, source-repository README, or unauthorized
    bytes into a materialized multilingual content identity?
14. Does the 64-window arithmetic equal 131,008 scored positions and give
    every named non-natural stratum 16,376 positions? Do all eight
    long-context windows actually permit scored positions strictly after
    131,072?
15. Can this source recipe ever be mistaken for the
    `corpus_manifest_sha256` in `QualityRun.v1`? Are all missing generated,
    gated, tokenizer, prompt, deduplication, evaluator, and checker artifacts
    explicit and fail-closed?
16. Does any claim in the results index or production punchlist overstate
    this source audit as evaluator, task, retrieval, logit, model-quality, or
    GPU evidence?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then state
separately whether:

- ungated source identities and byte hashes are accepted;
- reasoning selection and its 1,000-item digest are accepted;
- MBPP primary and HumanEval diagnostic selections are accepted;
- BFCL offline tool selection and question/answer pairing are accepted;
- WikiText and FLORES+ source postures are accepted;
- tokenizer identity and source-versus-materialized boundaries are accepted;
  and
- the source recipe may be used as input to a separately reviewed CPU
  materializer after the parent quality contract is accepted.

Only if all seven answers are unqualified `YES`, end with the requested token.
Do not emit it for a conditional pass, stale input, omitted source hash,
selection mismatch, unauthorized gated access, or a source recipe that can be
mistaken for a runnable corpus.

The token accepts only source identities, selection arithmetic, and the
source/materialized boundary. It does not accept a generator, materializer,
evaluator, task score, model output, GPU result, cn4 access, or checkpoint
conversion.
