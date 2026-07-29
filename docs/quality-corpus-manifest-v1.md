# GLM-5.2 quality corpus source and materialization contract v1

Date: 2026-07-29

Status: design candidate; public source bytes and selection arithmetic audited,
but the qualification corpus is not materialized

GPU evidence: none

## Outcome

`manifests/quality-corpus-sources-v1.json` freezes the public source revisions,
content hashes, item-selection rules, tokenizer identities, minimum generated
strata, and fail-closed materialization requirements for the quality suite.
It closes the previous ambiguity about what "1,000 reasoning cases", "500
coding cases", and "500 tool cases" mean.

It is deliberately not the `corpus_manifest_sha256` required by
`QualityRun.v1`. Generated prompt bytes, gated multilingual content hashes,
tokenized windows, and the evaluator do not exist yet. Treating this source
recipe as a runnable corpus is a contract violation.

The CPU evaluator remains blocked by the adversarial gate in
`docs/quality-acceptance-v1.md`. This work pins inputs and generation rules; it
does not begin evaluator implementation.

## Audited public sources

The audit downloaded only the listed small public files to an isolated
temporary directory, hashed the received bytes, parsed their identifiers, and
removed no production data. No dataset bytes enter Git.

The checked facts are:

- MMLU-Pro contains 12,032 test rows with unique integer `question_id` values
  and 70 validation rows. The 14 category counts sum to 12,032.
- The balanced, domain-separated rank rule selects exactly 1,000 reasoning
  items: 72 from each of the first six lexicographic categories and 71 from
  each of the remaining eight.
- MBPP has 974 unique integer tasks. IDs 11 through 510 inclusive exist and
  select exactly 500 executable qualification tasks.
- HumanEval has all 164 IDs `HumanEval/0` through `HumanEval/163`. It is an
  additional diagnostic and cannot replace the 500-case MBPP primary stratum.
- The pinned BFCL question and possible-answer files contain respectively
  400, 200, 200, and 200 unique IDs for simple Python, multiple, parallel, and
  parallel-multiple calls. Each question ID set exactly equals its answer ID
  set. Taking 125 independently hash-ranked items from each category selects
  exactly 500 offline AST cases.
- WikiText raw test and validation contain 4,358 and 3,760 rows and
  1,287,656 and 1,144,248 UTF-8 source bytes. Their exact tokenizer-dependent
  2,048-token windows are not yet claimed.

All item ranking uses the explicitly listed SHA-256 domain and byte
concatenation. The ordered selected-ID stream is itself domain-separated and
hashed. A materializer must reproduce the checked digest before it may access
the corresponding prompt or answer.

## Reasoning task

The primary reasoning set is the pinned MMLU-Pro test split. MMLU-Pro is used
instead of making qualification depend on gated GPQA bytes.

Within each category, rank a row by:

```text
SHA256(
  "glmaxx.quality.reasoning.mmlu-pro.selection.v1" UTF-8
  || 0x00
  || category UTF-8
  || 0x00
  || question_id in canonical unsigned base-10 ASCII
)
```

Sort by digest bytes ascending, then numeric `question_id`; take the declared
category quota. Emit categories in manifest order and preserve the rank order
within a category. The 1,000 selected `category<TAB>question_id<LF>` records,
after the separate stream-domain prefix and NUL, hash to the manifest value.

Five-shot examples are the first five validation rows per category in pinned
source order. They are prompt context, not scored items. A scored ID appearing
in a few-shot row is fatal. Prompt rendering initially follows the pinned
lm-evaluation-harness MMLU-Pro 3.1 task configuration, but the eventual
repository-local evaluator, rendered prompt bytes, and container digest are
normative. The upstream harness revision is a reference, not an unpinned
runtime dependency.

## Executable coding task

MBPP IDs 11 through 510 inclusive are the 500 primary cases, ordered
numerically. All visible and challenge tests, setup code, prompt construction,
language version, time limit, memory limit, process limit, syscall policy, and
output normalization must be frozen in the materialized task manifest.

Generated code runs offline under an unprivileged, single-use sandbox with:

- no network namespace access;
- a read-only evaluator image and empty writable scratch;
- fixed CPU, wall-clock, memory, file, and process limits;
- no host paths, credentials, model files, or inherited descriptors; and
- fail-closed classification of sandbox setup failure, signal, timeout,
  output overflow, or evaluator warning.

HumanEval's 164 cases are an additional separately reported diagnostic. They
do not increase or dilute the paired MBPP primary pass rate. Candidate code is
untrusted even when it came from the pinned model.

## Tool task

The primary tool task is 500 offline BFCL AST cases: 125 from each of simple
Python, multiple, parallel, and parallel-multiple. Rank within a category by:

```text
SHA256(
  "glmaxx.quality.tools.bfcl.selection.v1" UTF-8
  || 0x00
  || category UTF-8
  || 0x00
  || item_id UTF-8
)
```

Sort by digest bytes and then item-ID bytes. The exact question and
possible-answer file hashes are paired in the source manifest. Live, web
search, stateful memory, stateful multi-turn, and network-executable cases are
excluded because their truth can change or depend on external services.

The materialized task must freeze the GLM tool chat-template rendering,
assistant extraction, accepted call ordering, number and string
normalization, schema validation, and semantic checker. BFCL's pinned AST
checker is a cross-check. It is not silently imported as "latest", and a
candidate cannot receive a different parser from its control.

## Deterministically generated behavior strata

Generator implementation begins only after the quality contract is accepted.
Its design requirements are already fixed:

### JSON/schema, 500 cases

Generate 100 cases in each family:

1. flat scalars, enums, numeric bounds, and required fields;
2. nested objects with exact required/optional membership and forbidden extra
   fields;
3. arrays with element schemas, length bounds, and nested records;
4. tagged unions, nullable fields, and mutually exclusive variants; and
5. escaped UTF-8, control characters, patterns, and string-length boundaries.

Each prompt carries source facts whose exact values must appear in the
response. Success requires one complete JSON value, no non-whitespace suffix,
valid UTF-8, schema validity, and exact semantic fields. Parse-only success is
insufficient.

### Long generation and repetition, 500 cases

Generate 100 cases in each family: numbered reconstruction, bounded
multi-section synthesis, long structured transformation, required terminal
sentinel, and adversarial periodic-prefix continuation. The five fixed maximum
output bands are 256, 512, 1,024, 2,048, and 4,096 tokens.

Retain generated token IDs, terminal reason, repeated n-gram episodes,
longest periodic suffix, required-section coverage, and the first divergence
from the deterministic checker. An output truncated by its own declared
maximum is not silently counted as complete.

### Retrieval, 1,000 frozen plus 1,000 randomized

The exact total-context bands are:

```text
16,384
65,536
131,072
491,520
1,048,576
```

There are 200 cases per band in each retrieval stratum. Sixty-four tokens are
reserved for answer and EOS, so prompt materialization must fit within
`band - 64` tokens. Answer records occupy five equal position bins, 40 cases
per bin per band.

Each case contains high-entropy key/value records, distractors, and one
permitted answer record. The materializer scans normalized UTF-8 and token
windows and aborts if the answer occurs in the question, distractors,
template, another case, calibration material, or anywhere outside its one
permitted record.

Frozen canaries derive from an external secret seed whose digest is committed
before any candidate run; the seed and raw prompts remain content-addressed
outside Git. Randomized canaries use fresh run-family seeds committed before
control or candidate inference. The same run-family seeds and prompts go to
the control and candidate. Seeds are retained after the run for authorized
reproduction. Public repository constants alone can never be the answer
source.

### Termination/parser, 500 cases

Generate 100 cases in each family: ordinary EOS, exact token-limit stop,
complete tool-call JSON, complete reasoning-tag state, and incremental UTF-8
split boundaries. The expected terminal reason, parser state, emitted byte
stream, stop sequence, and maximum output are part of each case. Success
requires the exact terminal class and no post-terminal token publication.

## Full-vocabulary logit windows

The 64 windows are exactly 2,048 tokens and score exactly 2,047 positions:

| Stratum | Windows | Positions |
|---|---:|---:|
| natural text | 32 | 65,504 |
| reasoning/code | 8 | 16,376 |
| tool/JSON | 8 | 16,376 |
| multilingual | 8 | 16,376 |
| long context | 8 | 16,376 |
| total | 64 | 131,008 |

Natural text uses 16 nonoverlapping windows from each pinned WikiText
evaluation split. Reasoning/code uses four MMLU-Pro and four MBPP record
streams. Tool/JSON uses four BFCL and four generated-schema streams.
Multilingual uses one window each from Arabic, Simplified Chinese, Hindi,
Japanese, Russian, Spanish, Swahili, and Thai FLORES+ devtest files.

Long-context windows use four frozen-retrieval prompts at total band 491,520
and four at 1,048,576. Every scored row starts after logical position 131,072.
The materialized manifest records prompt token IDs, window start, source item
IDs, and SHA-256 for every 2,048-token array.

Record streams use source row order only as input. The accepted materializer
must define separators, prompt rendering, packing, block selection, and
padding before window bytes are frozen. No token window is accepted merely
from this prose.

FLORES+ is license-gated. This contract pins repository revision, Git blob,
size, language, split, and license, but not a content SHA-256. An operator
must accept the dataset terms and provide authorized access. Materialization
then records the received SHA-256 for all eight files. A Git blob identity is
not promoted to a content hash.

## Materialized corpus boundary

The future materializer emits a separate
`glmaxx.quality-corpus.v1` manifest containing:

- this source-recipe SHA-256 and accepted review identity;
- every source revision, size, content hash, license, and local
  content-addressed path;
- every selected item ID and selected-ID stream hash;
- exact rendered prompt bytes and token IDs;
- expected output/checker data and maximum-output values;
- generator commit, algorithm version, seeds, and output digests;
- all 64 logit windows and source offsets;
- calibration-deduplication evidence;
- tokenizer, chat-template, evaluator, and container identities; and
- a content-derived corpus UUID.

Raw datasets, generated prompts, answers, code completions, and model outputs
stay outside Git. The checked-in materialized manifest may contain only
identities, item IDs, counts, and hashes.

Materialization fails as a whole on a source mismatch, missing license,
gated-file hash omission, duplicate/missing ID, prompt drift, tokenization
drift, selection-digest mismatch, calibration overlap, answer leakage,
generator warning, or nondeterministic output.

## Current gate

Proved now:

- exact revisions and byte hashes for every listed ungated source file;
- exact public row and ID counts;
- exact reasoning, coding, and tool selection counts and stream digests; and
- enough pinned WikiText evaluation bytes to source the natural windows.

Still absent:

- acceptance of the quality contract and this source recipe;
- the generated-corpus implementation and its CPU proof;
- authorized FLORES+ content hashes;
- exact tokenized prompt/window manifests;
- the repository-local evaluator and sandbox image;
- calibration-deduplication evidence;
- any BF16, compressed, MTP, retrieval, task, or model-quality result; and
- any GPU evidence or cn4 authorization.

The next permitted implementation after adversarial acceptance is a
CPU-only, fail-closed source verifier and deterministic corpus materializer.
Model execution remains after that CPU proof.
