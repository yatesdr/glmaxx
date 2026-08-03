# Fable handoff: quality corpus sources v1 r2

Date: 2026-08-03

Status: bounded corrective adversarial review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4, launch CUDA, access gated FLORES+ content, accept
dataset terms, or download model/checkpoint payloads. This is a CPU/source
recipe review only.

Review candidate commit:
`a2fc47afb8557fb0b8a3396865fb951064380dad`

Required result path:
`fable-quality-corpus-sources-v1-r2.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`quality-corpus-sources-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Withhold the token for any mismatch or incomplete input.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/quality-corpus-manifest-v1.md` | `c74288b346dbb8d5f1c8bc87037207a8b6aa4d713efa1c134ec97f625b703c29` |
| `manifests/quality-corpus-sources-v1.json` | `10d29a0271e4264e449bff4a3fbc8d447b142f628282ef42282708a04947919d` |
| `docs/quality-acceptance-v1.md` | `705bb0611464bd5d76a08943b3122ecb8a78506e78f9c20a46d4e1ce24fc7be6` |
| `fixtures/tokenizer-contract-proof-v1.json` | `bb0a29719ffc69e6676ac3edf156ea47ff6dc6e1424a0d866fbd5d2d76db5223` |
| `docs/tokenizer-serving-contract.md` | `9e9c82d2375d3f759b10568fdb33bc75d102f8a2f609ddbc6f980036c81b7527` |
| `crates/glm-cli/src/main.rs` | `04537c79fe4bcac67627483e96fcedc783702d08a16db8c10f3894964fe99afc` |
| `crates/glm-format/src/rank_manifest.rs` | `cb57a81ad3643a86992c4fd9c2166e9ee238cc7e66063732355d14e485f73410` |
| `docs/production-punchlist.md` | `d6c082356718bb6ef9a9243f0fffbe6b598e725cb60c21b9a70f8534504894de` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

Run the complete CPU-only gate and record its exit status:

```text
./scripts/local-checks.sh
```

## Exact external source check

Fetch only this small official file at exact lm-evaluation-harness commit
`f4d4b3de3ee6741a7151a9fe74945ee515262f4c`:

```text
https://raw.githubusercontent.com/EleutherAI/lm-evaluation-harness/f4d4b3de3ee6741a7151a9fe74945ee515262f4c/lm_eval/tasks/mmlu_pro/README.md
```

It must contain exactly 4,776 bytes, hash to
`7a46bdda17a62d7ae4752a9c12e990ac43ede208e1a44a7a5a5b426bb8a1ceea`,
and explicitly record the version 3 to 3.1 change. Do not use `main`.

## Required independent work

1. Parse the manifest and independently emit the exact HumanEval stream:
   domain UTF-8 plus NUL, then 164 source-ordered records
   `human-eval<TAB>HumanEval/<i><LF>` for `i=0..163`. It must contain 4,034
   bytes and hash to
   `0dbbca61baa0b9b486debc99ea894681688e1950f93f1a0171ccd2a7adea114e`.
2. Mutate the domain, prefix, tab, slash, decimal rendering, order, newline,
   or count and prove every mutation changes the digest or fails validation.
3. Recompute the tokenizer bundle from the three exact component digest
   fields, raw digest bytes, declared name/NUL ordering, and domain. It must
   equal `31eb3c003ad8b1545e29144e3f161ad706883e0ccc927beebeddf950b36a6abb`.
4. Compare the prose/manifest bundle definition independently with both Rust
   implementations and prove the chat-template digest is separate.
5. Fetch/hash the exact MMLU-Pro README and verify the 3.1 claim is source
   backed while lm-evaluation-harness remains a non-normative reference.
6. Recheck all findings, minor notes, question, seven required statements,
   and nonclaims from `fable-quality-corpus-sources-v1.md` against the new
   bytes; do not redo gated content access or infer absent materialization.

## Required decisions

Answer each with an unqualified `YES` or `NO`:

1. Is the HumanEval stream preimage complete, independently reproducible,
   mutation-sensitive, and consistent between manifest and prose?
2. Does the recomputed HumanEval digest match the unchanged selected set and
   preserve its diagnostic-only separation from the 500 MBPP primary cases?
3. Is the tokenizer-bundle construction exact, source-linked, independently
   reproducible, and consistent with both Rust implementations?
4. Does the exact official MMLU-Pro file at the pinned commit establish task
   version 3.1 without turning the harness into a normative evaluator?
5. Do all previously accepted public source identities, selection rules,
   counts, digests, gated-file boundaries, and materialization nonclaims remain
   unchanged and valid?
6. Is the recipe mechanically distinct from a materialized
   `glmaxx.quality-corpus.v1` and unusable as `QualityRun.v2.corpus_manifest`?
7. Does revision 2 resolve the sole major, both minors, and the question in
   the first review without weakening any source or license gate?
8. Does acceptance open only a later CPU verifier/materializer after the
   parent quality and generated-corpus designs are also accepted?
9. Are all evaluator, corpus, gated-content, model, task, retrieval, quality,
   GPU, cn4, latency, throughput, capacity, and serving nonclaims accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, followed
by the independent derivations and all nine decisions. Only if every decision
is `YES`, attest the candidate and all nine input hashes, then end with the
requested token as the only bare acceptance line.

Acceptance does not implement or materialize a corpus, authorize gated data,
accept the parent quality or generated-corpus contracts, authorize cn4/CUDA,
execute a model, or establish any quality or performance evidence.
