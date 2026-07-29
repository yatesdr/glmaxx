# Deterministic generated quality corpus contract v1

Date: 2026-07-29

Status: design candidate; adversarial review required before generator or
evaluator implementation

GPU evidence: none

## Scope

This contract makes the five generated portions of
`manifests/quality-corpus-sources-v1.json` implementable:

- 500 JSON/schema cases;
- 500 long-generation/repetition cases;
- 1,000 frozen retrieval cases;
- 1,000 randomized retrieval cases; and
- 500 termination/parser cases.

It specifies deterministic derivation, exact counts, prompt/target
construction, token-length materialization, leakage checks, and retained
records. It does not generate a corpus, accept the parent quality contract,
implement an evaluator, run a model, authorize cn4, or establish quality.

The generated corpus is qualification-only. No prompt, target, seed, or
derived field may enter quantization calibration, protected-tensor selection,
kernel tuning, threshold selection, few-shot examples, or model training.

## Prerequisites and identity

Materialization requires all of:

1. `quality-acceptance-v1-accepted`;
2. `quality-corpus-sources-v1-accepted`;
3. acceptance of this contract;
4. the exact tokenizer/template bundle in
   `fixtures/tokenizer-contract-proof-v1.json`;
5. an immutable generator commit and container digest; and
6. an external content-addressed output directory that does not exist before
   the run.

The materializer refuses an existing destination. It writes to a sibling
temporary directory, fsyncs every file and the directory, then renames the
complete corpus atomically. Crash remnants are never resumed or interpreted
as a corpus.

One `GeneratedCorpusIdentity.v1` binds:

```text
source_recipe_sha256
generated_contract_sha256
generator_commit
generator_binary_sha256
generator_container_digest
model_revision
tokenizer_bundle_sha256
chat_template_sha256
token_output_table_sha256
public_seed_set_sha256
frozen_seed_commitment
randomized_seed_commitment
case_index_sha256
prompt_index_sha256
target_index_sha256
```

The corpus UUID is the first 16 bytes of:

```text
SHA256("glmaxx.generated-corpus-identity.v1" || 0x00 || all fields above)
```

Fixed-width digests are raw bytes in listed order. Variable strings are
encoded as `u32 little-endian byte length || UTF-8 bytes`. There is no
timestamp, random UUID, host path, or map iteration in the identity.

`public_seed_set_sha256` is:

```text
SHA256(
  "glmaxx.generated-public-seeds.v1" || 0x00
  || len32("json_schema") || "json_schema" || 32-byte expanded key
  || len32("long_generation_repetition")
     || "long_generation_repetition" || 32-byte expanded key
  || len32("termination_parser")
     || "termination_parser" || 32-byte expanded key
)
```

where `len32` is the little-endian UTF-8 byte length and the order above is
fixed.

## Common derivation primitive

Generated values use RFC 2104 HMAC-SHA256. Public 64-bit seed strings in the
source recipe are parsed as unsigned hexadecimal, encoded little-endian into
the first eight bytes of a 32-byte key, and followed by 24 zero bytes.
Retrieval uses a full 32-byte external key.

One derivation message is:

```text
"glmaxx.generated-field.v1" || 0x00
|| u32le(len(stratum)) || stratum UTF-8
|| u32le(len(family))  || family UTF-8
|| u32le(case_ordinal)
|| u32le(len(label))   || label UTF-8
|| u32le(attempt)
```

`case_ordinal` is zero-based within a stratum. `attempt` starts at zero.
Stratum, family, and label are nonempty ASCII and contain neither NUL nor
control bytes.

The 32 HMAC bytes are used directly for digests and secrets. When an unsigned
integer in `[0,n)` is required, interpret bytes `0..8` as little-endian `x`.
Let:

```text
limit = floor(2^64 / n) * n
```

Accept and return `x mod n` only when `x < limit`; otherwise increment
`attempt` and derive again. `n=0`, attempt overflow, or failure to accept
within 1,024 attempts is fatal. The implementation computes `limit` in an
integer type wider than 64 bits; it may not approximate `2^64` with
`u64::MAX`.

Uppercase hexadecimal is `0123456789ABCDEF`, two characters per byte. No
prefix or separators are added unless a field definition says so.

Unless a field explicitly supplies a different label, its derivation label is
its slash-separated path as printed in this contract, for example
`user/id`, `items/3/key`, or `records/17/value`. A boolean is
`bounded(2) != 0`. An inclusive integer range `[lo,hi]` is
`lo + bounded(hi - lo + 1)` with checked signed arithmetic. Separate fields
never consume a shared state or depend on evaluation order.

## Common case record

Every generated case has one external canonical record:

```text
GeneratedQualityCase.v1 {
    case_id: 32 lowercase hex characters
    stratum: fixed enum
    family: fixed ASCII string
    ordinal: u32
    context_band: u32 | 0
    position_bin: u8 | 0xff
    public_seed_id: fixed string | null
    secret_seed_commitment: [u8; 32] | null
    prompt_utf8_sha256: [u8; 32]
    prompt_bytes: u64
    prompt_token_ids_sha256: [u8; 32]
    prompt_tokens: u32
    maximum_output_tokens: u32
    expected_kind: fixed enum
    expected_sha256: [u8; 32]
    checker_config_sha256: [u8; 32]
    source_recipe_sha256: [u8; 32]
}
```

`case_id` is the first 16 bytes of:

```text
SHA256(
  "glmaxx.generated-quality-case.v1" || 0x00
  || length-prefixed stratum
  || length-prefixed family
  || u32le(ordinal)
  || prompt_utf8_sha256
  || prompt_token_ids_sha256
  || expected_sha256
)
```

The ordered case index is stratum enum order, family order declared here,
then ordinal. Its binary digest stream starts with
`"glmaxx.generated-case-index.v1\0"` and appends each 16-byte case ID.
Duplicate case ID, prompt digest, or target digest is fatal unless this
contract explicitly permits a shared empty target.

Stratum enum order is exactly:

```text
json_schema
long_generation_repetition
frozen_retrieval
randomized_retrieval
termination_parser
```

The corpus has exactly 3,500 generated cases.

`expected_sha256` hashes the strict typed expectation record, not necessarily
literal target text. Control-relative cases encode the paired rule and
absolute invariants in that record; they never fill it with a control output
after the fact.

Prompt text is UTF-8 with LF line endings, no BOM, no NUL, and no Unicode
normalization. Templates below are ASCII. Derived Unicode values are inserted
as their exact listed scalars. No locale, host newline, Python object
rendering, or unordered map affects bytes.

Unless a family says otherwise, each request is a single user message,
thinking enabled at reasoning effort `max`, no system message, no tools,
greedy sampling, MTP0, and no stop string.

## Canonical structural encoding

Generator-owned JSON uses this restricted canonical form:

- object keys are emitted in the exact schema order, not sorted;
- no insignificant whitespace appears;
- strings use JSON double quotes, escape `"` and `\`, use the short escapes
  `\b`, `\t`, `\n`, `\f`, and `\r`, and encode every other valid scalar as
  UTF-8 without `\u` escaping;
- integers use shortest base-10 with no leading zero and no negative zero;
- only integers, strings, booleans, null, arrays, and objects are generated;
  and
- duplicate keys and nonfinite numbers are impossible.

The evaluator accepts arbitrary legal member order from the model. It parses
one RFC 8259 JSON value, rejects duplicate keys at every depth, rejects any
non-whitespace prefix or suffix, validates Draft 2020-12 schema semantics, and
then compares typed semantic values. Generator canonicalization is for stable
targets and hashes, not a demand that model whitespace or member order match.

## JSON/schema stratum

There are exactly 500 cases. Families are in this order, 100 cases each:

```text
flat
nested
arrays
tagged_union
unicode_escape
```

The public key is source-recipe seed `0x004a534f4e5f7631`.
Ordinal is `family_index * 100 + family_ordinal`.
Maximum output is 512 tokens. Success requires EOS after the single JSON
value; a length or stop-string finish fails the case.

Every prompt has these exact sections and separators:

```text
Return exactly one JSON value. Do not use markdown or commentary.
The value must satisfy the JSON Schema and preserve every source value with
the required JSON type.

SOURCE
{source lines}

JSON_SCHEMA
{canonical schema JSON}
```

The final byte is `}` from the schema; there is no trailing LF. Source lines
are family-defined, ordered, and separated by LF. A source string is encoded
as canonical JSON even when it appears after `=`.

Every schema has:

```text
"$schema":"https://json-schema.org/draft/2020-12/schema"
"type":"object"
"additionalProperties":false
```

and explicitly lists `required`. The expected semantic object is derived
before the prompt; the schema and source lines are independently derived from
that typed object. Re-parsing source lines is not the target oracle.

Root schema key order is exactly `$schema`, `type`, `properties`, `required`,
`additionalProperties`. `properties` follows expected-object field order.
Nested object-schema key order is `type`, `properties`, `required`,
`additionalProperties`. Primitive-schema key order is `type`, then `const`,
`enum`, `pattern`, `minimum`, `maximum`, `minLength`, and `maxLength` when
present. Array-schema key order is `type`, `items`, `minItems`, `maxItems`,
then `uniqueItems` when present. Keywords absent from a schema are omitted,
not encoded as null.

### `flat`

Fields, schema order, and source order are:

```text
record_id : string "GX-" + hex(8 derived bytes)
enabled   : boolean from bounded(2)
count     : integer in [-1_000_000, 1_000_000]
priority  : integer in [0, 9]
status    : one of "queued", "running", "paused", "done"
```

The schema enforces string pattern `^GX-[0-9A-F]{16}$`, exact integer bounds,
and the four-value enum. Source line form is `field=canonical_value`.

### `nested`

Expected object order and shape:

```text
{
  "user": {
    "id": "U-" + hex(6 bytes),
    "display": selected Unicode atom,
    "active": boolean
  },
  "limits": {
    "requests": integer 1..10000,
    "tokens": integer 256..1048576
  },
  "routing": {
    "region": enum "us-east"|"us-west"|"eu-central"|"ap-south",
    "shard": integer 0..63
  }
}
```

Every nested object has `additionalProperties:false` and exact required
membership. Source lines use dotted paths in this order:
`user.id`, `user.display`, `user.active`, `limits.requests`,
`limits.tokens`, `routing.region`, `routing.shard`.

### `arrays`

Derive `length = 3 + bounded(5)`. Expected shape:

```text
{
  "batch_id": "B-" + hex(8 bytes),
  "items": [
    {"slot":0,"key":"K-"+hex(5 bytes),"weight":integer 1..1000},
    ...
  ],
  "selected_slots": [distinct slot integers in ascending order]
}
```

Each item field is derived with label suffix `/{slot}`. Select at least one
slot by label `items/{slot}/selected` and one derived bit per item; if none are
selected, select `bounded(length)` with label `selected_slots/fallback`. The
schema fixes `minItems=maxItems=length`, requires unique selected slots, and
bounds every slot. The semantic checker requires each item slot to equal its
array index and selected slots to match the source selection exactly.

### `tagged_union`

Variant is `bounded(4)`:

```text
email:   {"kind":"email","address":"local-"+hex(4)+"@example.test"}
sms:     {"kind":"sms","country_code":integer 1..999,"number":hex(6)}
webhook: {"kind":"webhook","url":"https://example.test/h/"+hex(8),
          "retries":integer 0..8}
none:    {"kind":"none","reason":null}
```

The root is `{"delivery":variant_object,"trace":"T-"+hex(8)}`. The schema
uses `oneOf` with a required constant `kind` and
`additionalProperties:false` in every branch. The checker rejects a value
that validates more or fewer than one branch.

### `unicode_escape`

Choose four distinct atoms without replacement from this fixed ordered table:

```text
plain ASCII
quote " slash \ backspace \u0008 tab \u0009 newline \u000A
北
مرحبا
हिन्दी
日本語
🙂
e\u0301
é
line\u2028separator
zero-width\u200Djoin
```

The `\uNNNN` notation above denotes the named scalar, not six literal bytes;
the quote and backslash are literal characters. The expected object is:

```text
{"title":atom0,"notes":[atom1,atom2,atom3],"exact_length":scalar_count(atom0)}
```

`scalar_count` counts Unicode scalar values, not bytes or graphemes. The
schema bounds strings by scalar count and requires exactly three notes.
Source values are canonical JSON strings, so control characters are escaped
and all other scalars remain UTF-8.

Selection uses a mutable ordered list initialized to the table above. For
`atom/0` through `atom/3`, choose `bounded(remaining_length)` and remove that
entry before the next choice. The selected order is title, then notes array
order.

### JSON gate records

Per case retain:

```text
parse_success
duplicate_key
schema_success
semantic_success
first_json_error_byte
first_schema_error_path
first_semantic_error_path
output_utf8_valid
terminal_reason
```

The stratum success bit is
`UTF8 && parse && !duplicate && schema && semantic`. An evaluator exception,
unsupported schema keyword, or ambiguous `oneOf` is an invalid run, not a
failed model item.

## Long-generation and repetition stratum

There are five families, five maximum-output bands, and 20 replicas per cell:

```text
families:
  indexed_copy
  reverse_ledger
  grouped_totals
  sectioned_extract
  periodic_decoy

maximum_output_tokens:
  256, 512, 1024, 2048, 4096
```

Family-major, then band-major, then replica-major order gives 500 cases.
The public key is source-recipe seed `0x004c4f4e475f7631`.
Global ordinal is
`((family_index * 5 + band_index) * 20) + replica_index`.

Each family produces an ordered stream of source records and an exact target
record stream. Record payloads are uppercase hex derived independently for
each case and record index. Source and target use LF separators and no
trailing LF.

Materialization starts with one record and appends records until appending the
next record plus the family terminator would make the tokenized target exceed
`maximum_output_tokens - 2`. It chooses the largest fitting record count.
The final target, excluding EOS, must have at least:

```text
floor(maximum_output_tokens * 3 / 4)
```

tokens. Failure to reach that floor, a target above `max-2`, or a record count
above 100,000 is fatal. The two-token reserve is for a terminator boundary and
EOS; it is not filled with arbitrary padding.

The prompt is:

```text
Perform the deterministic transformation below. Output only transformed
records, one per line, followed by the exact terminator {terminator}.

RULE
{family rule}

INPUT
{source records}
```

The terminator is `END-` plus 12 uppercase hex characters derived for the
case. It appears once in the target, nowhere in input records, and is not a
runtime stop string.

### `indexed_copy`

Source record `R{index:06}|{hex(12 bytes)}` becomes
`{index:06}:{payload}`. Target order equals source order.

### `reverse_ledger`

Source record
`R{index:06}|C{bounded(16):02}|V{bounded(1_000_001):07}` becomes
`{index:06}:C{channel}:V{value}`. Target order is reverse source order.
The transform performs no arithmetic.

### `grouped_totals`

Each source record is
`R{index:06}|G{bounded(8)}|V{1+bounded(9999)}`. The target contains eight
lines in group order:

```text
G{group}|COUNT{count:06}|SUM{sum:012}
```

followed by an `ITEMS` line containing source indices in ascending order as
six-digit values separated by commas. To scale target length, each generated
source record also contributes one exact detail line
`D{index:06}|G{group}|V{value:04}` after the group summary. Integer sums use
checked `u64`.

### `sectioned_extract`

Each source record has a section in `A..H`, a six-digit ordinal, a key, one
decoy, and one fact:

```text
S{section}|R{index:06}|K{hex(4)}|D{hex(8)}|F{hex(12)}
```

The target emits headings `[A]` through `[H]`, then for each source record in
that section emits `K{key}=F{fact}` in source-index order. Decoy bytes may
not appear in the target.

### `periodic_decoy`

Input alternates a repeated 16-record decoy cycle with one unique exception
record every 17th record:

```text
P{cycle_slot:02}|IGNORE-{cycle_slot:02}
X{index:06}|KEEP-{hex(12)}
```

The target emits only exception lines as
`{index:06}:KEEP-{payload}`. The target itself contains no repeated record.
This family tests whether a highly periodic context induces an output loop.

### Completion and repetition accounting

Retain exact-target token IDs, output token IDs, first divergence, matched
prefix length, matched target-record count, terminator presence/count,
terminal reason, and post-terminal token attempts.

An exact completion has all target bytes, one terminator, no other bytes,
then an EOS token. A target-compatible completion may vary only LF versus
CRLF if the renderer was configured to normalize it before the run; version
one configures no such normalization, so only LF is compatible.

Unexpected repetition begins at the first output-token position that differs
from the exact target, or at target length if extra tokens follow an exact
target. For each period `p=1..32`, find maximal runs in the unexpected suffix
whose token at `i` equals token at `i-p`. A repetition episode requires a run
of at least `max(64, 4p)` tokens. Report:

```text
has_unexpected_repetition
first_episode_position
episode_period
episode_tokens
longest_periodic_suffix
```

Choose the earliest episode, then smaller period. Expected target repetition
before divergence cannot trigger the detector. Invalid UTF-8, missing
terminator, multiple terminators, output beyond the request limit, or any
post-EOS publication is a behavior failure independently of exact-match
quality.

## Retrieval strata

Frozen and randomized retrieval use the same generator and separate 32-byte
master keys. They contain 200 cases at each total-context band:

```text
16,384
65,536
131,072
491,520
1,048,576
```

Within each band, ordinals `0..199` map to position bin
`floor(ordinal / 40)`, giving 40 cases in each of five bins. Global stratum
ordinal is `band_index * 200 + ordinal`.

For derivation, retrieval family is `ctx-` followed by the canonical
base-10 context band, and case ordinal is the global stratum ordinal.
Distractor labels append their zero-based index in canonical base-10.

Frozen master-key bytes are generated once by an OS CSPRNG outside Git. Before
any prompt materialization or candidate tuning, publish:

```text
HMAC-SHA256(
  frozen_master_key,
  "glmaxx.frozen-retrieval-seed-commitment.v1"
)
```

Randomized master-key bytes are generated after the serving policy is frozen
but before either control or candidate inference. Publish the analogous
domain-separated commitment. The same key-derived prompts are used for every
member of the run family. Raw master keys and prompts stay in the access-
controlled result bundle and are disclosed only after the immutable result
record is sealed.

### Retrieval fields

For each case derive:

```text
lookup_key = "K-" + hex(12 bytes)
answer     = "GX-" + hex(20 bytes)
record_key = "D-" + hex(12 bytes) for each distractor index
record_val = "DV-" + hex(24 bytes) for each distractor index
```

All labels include the band, case ordinal, and distractor index. Derivations
for frozen and randomized strata use different stratum strings. Duplicate
lookup key, answer, distractor key, or distractor value anywhere in either
corpus is fatal.

The target record is exactly:

```text
<record key="{lookup_key}">
value={answer}
</record>
```

One distractor record is:

```text
<record key="{record_key}">
value={record_val}
</record>
```

Records are LF-separated. The query suffix is exactly:

```text

LOOKUP
Return only the value for key {lookup_key}.
```

The answer target is exactly the ASCII answer followed by EOS.

### Exact token length and placement

The total-context band includes prompt and the 64-token generation
reservation. Therefore:

```text
prompt_tokens = context_band - 64
```

The chat renderer and generation prompt are included in `prompt_tokens`.
Materialization first tokenizes the rendered empty-record envelope, target
record, query suffix, and one distractor. If they cannot fit, it aborts.

The desired target-record start is:

```text
floor(prompt_tokens * (2 * position_bin + 1) / 10)
```

corresponding to 10%, 30%, 50%, 70%, or 90%. Generate independent distractor
records before the target until another full record would cross the desired
start. Generate independent records after it until another full record would
exceed `prompt_tokens`.

Before inserting the target, fill toward the desired start with a
deterministic safe-lexeme solver. After inserting the target and as many
post-target distractors as fit, invoke the same solver for the final prompt
gap. The accepted generator freezes an ordered table of at least 256 ASCII
lexemes. At startup it:

1. tokenizes each lexeme independently with no special tokens;
2. requires exact decode-to-original bytes;
3. rejects any lexeme containing `<record`, `</record>`, `LOOKUP`, `K-`,
   `GX-`, or `DV-`;
4. requires at least one lexeme of token length one; and
5. hashes the complete `lexeme bytes -> token IDs` table into the corpus
   identity.

For a gap of `g` tokens, enumerate lexeme sequences by increasing absolute
difference between their independently tokenized length and `g`, then by
shorter total, then lexicographically by the tuple of table indices. No index
may occur twice consecutively. Dynamic programming works backward from the
candidate total; it does not greedily assume token additivity.

For the pre-target invocation, re-tokenize the complete prefix after each
candidate insertion and choose the first sequence whose actual target start
is within 64 tokens of the desired start. For the final invocation, only a
sequence making the complete rendered prompt exactly `prompt_tokens` is
eligible. A context-sensitive BPE merge invalidates a candidate rather than
changing its recorded length. More than one million solver states, no exact
final solution, or more than a 64-token difference between actual and desired
target-record start is fatal.

The final target start is recorded. It must lie in the declared 20%-wide bin:

```text
[bin * 20%, (bin + 1) * 20%)
```

with the upper endpoint inclusive only for bin four. Filler may occur only
between complete distractor records, never inside the target record or query.

### Retrieval leakage rejection

For every materialized case, scan:

- raw UTF-8 bytes;
- Unicode NFC and NFKC forms;
- case-folded Unicode;
- tokenizer token IDs;
- all token n-grams of answer length; and
- every other generated case and every calibration source.

The answer and every substring of it of at least 16 hex characters must occur
exactly once: in the target record. The lookup key must occur exactly twice:
the target record and query. Neither may occur in a source URL, metadata,
case ID, template, distractor, filler lexeme, other prompt, or calibration
record.

The generator also constructs a negative prompt with the target record
removed and proves that the deterministic checker cannot recover the answer
from any retained field. The negative prompt is not sent to the model but its
hash and zero-occurrence proof are retained.

Any collision, answer occurrence outside the permitted record, calibration
overlap, normalization ambiguity, token-length mismatch, or position-bin
failure aborts the whole corpus.

### Retrieval result

Output is trimmed only of ASCII space, HTAB, CR, and LF at both ends. It then
must equal the ASCII answer byte-for-byte and terminate by EOS before the
64-token reservation. Markdown, explanation, JSON quoting, multiple answers,
stop-string termination, length termination, or sourcing from another
message fails the case.

Retain exact prompt token IDs, target record byte/token offsets, position bin,
answer bytes, output bytes, output token IDs, terminal reason, prefix-cache
posture, page IDs/generations, and whether any answer-containing page came
from HBM, DRAM, or NVMe.

Frozen retrieval requires exact success for every case. Randomized retrieval
uses the parent statistical gate, with results reported separately for each
band and position bin.

## Termination and parser stratum

There are five ordered families, 100 cases each:

```text
ordinary_eos
token_limit
tool_terminal
reasoning_terminal
incremental_utf8
```

The public key is source-recipe seed `0x005445524d5f7631`.
Ordinal is `family_index * 100 + family_ordinal`.

All cases retain every generated token ID, every incremental decoder delta,
parser state before and after each token, buffered UTF-8 bytes, held stop
prefix, finish event, output byte count, and any token offered after finish.

### `ordinary_eos`

Prompt asks for one derived ASCII code:

```text
Reply with exactly GX-{hex(8 bytes)} and then finish.
```

Maximum output is 32. Success requires exact visible text, one of the three
pinned EOS IDs, no stop-string or length finish, empty UTF-8 and stop buffers,
and rejection of every post-EOS token.

### `token_limit`

Prompt asks for an unbroken sequence of derived 16-character codes numbered
from zero through 999, one per line. Maximum output is selected from
`16, 32, 64, 128` by `bounded(4)`. The expected finish class is the engine
token-limit boundary unless the authoritative control emits EOS earlier.

Absolute invariants apply regardless of control: no more than the maximum
number of committed output tokens, no output token after terminal
publication, complete UTF-8 only, and exact committed-token accounting.
Control and candidate terminal class, token count, and visible prefix are
paired fields; a candidate-only early EOS, late EOS, or extra token is a new
failure.

### `tool_terminal`

Provide one ordered tool schema named `lookup_{hex(4 bytes)}` with required
string argument `key` and integer argument `limit`. The user supplies exact
derived values. The expected response is one GLM tool call and no visible
assistant prose.

Success requires the exact tool name, exact typed arguments, one closed
`</tool_call>`, terminal tool-parser state, and EOS. Missing, duplicated,
reordered semantic arguments are handled by the tool checker; XML tag order
and incremental parser state are retained separately. A complete tool call
must never be published before its closing tag.

### `reasoning_terminal`

Prompt supplies a two-integer addition and requests a short explanation and
the exact decimal sum. Thinking is enabled. Success requires:

- at most one `<think>` and one `</think>` transition in parser state;
- no visible partial reasoning tag;
- a closed reasoning state before final answer publication;
- exact decimal answer after the reasoning segment;
- EOS and no post-EOS token; and
- valid incremental UTF-8 throughout.

The explanation text is not judged semantically. Raw reasoning bytes remain
in protected quality evidence and are not exposed by the ordinary public
response when the serving contract suppresses them.

### `incremental_utf8`

Choose one item from the fixed table:

```text
北極
مرحبا🙂
हिन्दी
日本語
e\u0301/é
```

Prompt asks for that string exactly three times separated by `|`. Maximum
output is 64. Success requires exact text and EOS. Additionally, the retained
token-output bytes must include at least one UTF-8 scalar split across token
boundaries. If the pinned tokenizer does not produce such a split for a
selected table item, derive a deterministic concatenation of two table items;
if none of the finite table pairs split, materialization fails rather than
weakening the case.

Decoder output deltas must each be valid UTF-8. A final replacement scalar,
invalid byte, leaked partial stop prefix, incomplete scalar at EOS, or
different output under token-by-token versus whole-sequence decode is a
failure.

## Determinism and negative proofs

Before model execution, two isolated materializer processes with:

- different temporary paths;
- different process IDs;
- different thread counts;
- reversed worker scheduling; and
- randomized input-map insertion order

must produce byte-identical case records, prompt files, target files, indexes,
and final manifest. Public-seed corpora must match across hosts. Retrieval
corpora match when given the same external master keys.

The CPU proof includes at least:

1. every HMAC and bounded-integer known-answer vector;
2. attempt/rejection and `2^64` boundary cases;
3. each JSON family and every parser/schema failure class;
4. duplicate JSON keys and ambiguous `oneOf`;
5. each long-output family at all five token bands;
6. exact-fit, one-token-short, and one-token-over target construction;
7. expected-target repetition that must not false-positive;
8. candidate-only loops for every period `1..32`;
9. all retrieval bands, bins, first/last ordinals, and exact prompt length;
10. no lexeme solution and context-sensitive BPE merge rejection;
11. answer/key collision and every leakage-normalization failure;
12. frozen versus randomized domain separation;
13. all three EOS IDs, early EOS, length stop, and post-terminal rejection;
14. incomplete tool/reasoning parser states;
15. split and incomplete UTF-8;
16. duplicate/missing/reordered cases and index corruption;
17. changed tokenizer/template/lexeme-table identity; and
18. crash before rename, existing destination, and nondeterministic rerun.

An independent implementation reproduces every public-seed case digest and a
review-only retrieval fixture with a published non-secret 32-byte key. The
qualification retrieval master keys are never used as the public fixture.

## Materialized output

The output tree is external and content-addressed:

```text
manifest.json
case-index.bin
prompt-index.bin
target-index.bin
cases/<case_id>.json
prompts/<prompt_sha256>.utf8
tokens/<prompt_token_ids_sha256>.u32le
targets/<expected_sha256>.bin
schemas/<checker_config_sha256>.json
leakage/<case_id>.json
```

Every index is sorted, duplicate-free, length-delimited, and strongly hashed.
Token arrays are canonical little-endian `u32` with no count header; their
length comes from the case record and file size must be exactly
`4 * prompt_tokens`.

`manifest.json` is strict canonical JSON under the repository's accepted
manifest canonicalizer. It contains no raw secret key. The external encrypted
evidence envelope retains seed bytes and hashes the ciphertext, key-management
identity, and access policy.

No dataset, prompt, target, seed, schema bundle, generated code, or raw model
output is committed to Git.

## Gate and exclusions

Acceptance of this design opens only CPU generator implementation after its
parent quality and source tokens exist. The implementation then requires its
own adversarial review and independent deterministic proof before a model
sees any generated prompt.

This contract does not:

- accept thresholds in the parent quality contract;
- accept any public source or tokenizer implementation;
- select a BF16 or compressed quality control;
- permit tuning on qualification cases;
- accept an evaluator, sandbox, or checker implementation;
- establish long-context correctness, cache correctness, model quality, or
  performance;
- enable MTP1–6; or
- authorize cn4, CUDA, checkpoint conversion, or serving.
