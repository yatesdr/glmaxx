# Deterministic generated quality corpus contract v1, revision 2

Date: 2026-08-03

Status: corrective design candidate; adversarial review required before
generator or evaluator implementation

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

The materializer resolves the configured output root from a trusted directory
file descriptor with `openat2(RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS)` and opens
the output parent as a directory file descriptor. It rejects a symlinked
parent, destination, or temporary component. File creation is relative to
that descriptor with `openat2`, the same resolution flags,
`O_CREAT|O_EXCL|O_NOFOLLOW`, and mode `0600`; directory creation uses
exclusive `mkdirat` with mode `0700`. Every opened object is checked for the
expected type and owner UID; every regular file must also have link count one.
The sibling temporary directory name is
`.glmaxx-generated-<identity-hex>-<128-bit-os-csprng-hex>` and is created with
`mkdirat` exclusive; the nonce is operational only and never enters content.
The destination and temporary directory must share the opened parent and
filesystem device. After fsyncing every file bottom-up, every temporary
directory, and the parent, the materializer publishes with Linux
`renameat2(RENAME_NOREPLACE)` and fsyncs the parent again before reporting
success. Existing destinations and temp-name collisions fail without
overwrite. Crash remnants are never resumed or interpreted as a corpus.

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
lexeme_table_sha256
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

`lexeme_table_sha256` is:

```text
SHA256(
  "glmaxx.generated-safe-lexeme-table.v1" || 0x00
  || u32le(entry_count)
  || for each table index in ascending order:
       u32le(lexeme_byte_count) || exact ASCII lexeme bytes
       || u32le(token_count) || each token ID as u32le
)
```

The entry count is at least 256 and indices are exactly the contiguous range
`0..entry_count-1`. Duplicate lexeme bytes or indices, a missing index, an
empty token list, a non-ASCII byte, a token/decode mismatch, or a table digest
mismatch is fatal. The tokenizer bundle and token-output table are separate
identity fields; none substitutes for this solver table.

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
  `\b`, `\t`, `\n`, `\f`, and `\r`, encode every other U+0000..U+001F
  scalar as the literal lowercase prefix `\u00` followed by two uppercase
  hexadecimal digits, and
  encode every other valid scalar as UTF-8 without `\u` escaping;
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
present. Array-schema key order is `type`, `prefixItems`, `items`, `minItems`,
`maxItems`, then `uniqueItems` when present. A union schema contains only
`oneOf`; branch order is email, sms, webhook, none. Enum array order is
exactly the value order printed for its family. Keywords absent from a schema
are omitted, not encoded as null. String lengths use Unicode code-point
counts exactly as JSON Schema Draft 2020-12 requires; a UTF-16 code-unit
validator is rejected.

### Exact schema byte templates

The following single-line templates are normative. The bytes inside each code
block, excluding the code-block delimiter and its LF, are the complete schema
bytes. `<L>`, `<H>`, `<T>`, `<N0>`, `<N1>`, and `<N2>` are metavariables, not
literal output: before hashing or prompt construction the generator replaces
each with shortest unsigned base-10, where `L` is the arrays-family length,
`H=L-1`, `T=scalar_count(atom0)`, and `N0..N2` are the scalar counts of notes
`atom1..atom3`. Replacement introduces no quotes, spaces, or leading zero.

`flat`:

```json
{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"record_id":{"type":"string","pattern":"^GX-[0-9A-F]{16}$"},"enabled":{"type":"boolean"},"count":{"type":"integer","minimum":-1000000,"maximum":1000000},"priority":{"type":"integer","minimum":0,"maximum":9},"status":{"type":"string","enum":["queued","running","paused","done"]}},"required":["record_id","enabled","count","priority","status"],"additionalProperties":false}
```

`nested`:

```json
{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"user":{"type":"object","properties":{"id":{"type":"string","pattern":"^U-[0-9A-F]{12}$"},"display":{"type":"string"},"active":{"type":"boolean"}},"required":["id","display","active"],"additionalProperties":false},"limits":{"type":"object","properties":{"requests":{"type":"integer","minimum":1,"maximum":10000},"tokens":{"type":"integer","minimum":256,"maximum":1048576}},"required":["requests","tokens"],"additionalProperties":false},"routing":{"type":"object","properties":{"region":{"type":"string","enum":["us-east","us-west","eu-central","ap-south"]},"shard":{"type":"integer","minimum":0,"maximum":63}},"required":["region","shard"],"additionalProperties":false}},"required":["user","limits","routing"],"additionalProperties":false}
```

`arrays`:

```json
{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"batch_id":{"type":"string","pattern":"^B-[0-9A-F]{16}$"},"items":{"type":"array","items":{"type":"object","properties":{"slot":{"type":"integer","minimum":0,"maximum":<H>},"key":{"type":"string","pattern":"^K-[0-9A-F]{10}$"},"weight":{"type":"integer","minimum":1,"maximum":1000}},"required":["slot","key","weight"],"additionalProperties":false},"minItems":<L>,"maxItems":<L>},"selected_slots":{"type":"array","items":{"type":"integer","minimum":0,"maximum":<H>},"minItems":1,"maxItems":<L>,"uniqueItems":true}},"required":["batch_id","items","selected_slots"],"additionalProperties":false}
```

`tagged_union`:

```json
{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"delivery":{"oneOf":[{"type":"object","properties":{"kind":{"type":"string","const":"email"},"address":{"type":"string","pattern":"^local-[0-9A-F]{8}@example\\.test$"}},"required":["kind","address"],"additionalProperties":false},{"type":"object","properties":{"kind":{"type":"string","const":"sms"},"country_code":{"type":"integer","minimum":1,"maximum":999},"number":{"type":"string","pattern":"^[0-9A-F]{12}$"}},"required":["kind","country_code","number"],"additionalProperties":false},{"type":"object","properties":{"kind":{"type":"string","const":"webhook"},"url":{"type":"string","pattern":"^https://example\\.test/h/[0-9A-F]{16}$"},"retries":{"type":"integer","minimum":0,"maximum":8}},"required":["kind","url","retries"],"additionalProperties":false},{"type":"object","properties":{"kind":{"type":"string","const":"none"},"reason":{"type":"null","const":null}},"required":["kind","reason"],"additionalProperties":false}]},"trace":{"type":"string","pattern":"^T-[0-9A-F]{16}$"}},"required":["delivery","trace"],"additionalProperties":false}
```

`unicode_escape`:

```json
{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"title":{"type":"string","minLength":<T>,"maxLength":<T>},"notes":{"type":"array","prefixItems":[{"type":"string","minLength":<N0>,"maxLength":<N0>},{"type":"string","minLength":<N1>,"maxLength":<N1>},{"type":"string","minLength":<N2>,"maxLength":<N2>}],"items":false,"minItems":3,"maxItems":3},"exact_length":{"type":"integer","const":<T>}},"required":["title","notes","exact_length"],"additionalProperties":false}
```

No other schema keyword or byte is permitted. The materializer parses each
completed template with the pinned Draft 2020-12 validator, validates its
independently derived expected value, and requires serialize(parse(bytes)) to
reproduce the original bytes before it hashes or embeds the schema.

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

Materialization starts with one record at `k=1`. At each step it constructs
and tokenizes the complete target for exactly `k+1` records plus the family
terminator. It appends that next record only when this complete target has at
most `maximum_output_tokens - 2` tokens. It stops permanently at the first
failure; it does not test any later record count even if BPE token length would
be non-monotonic. The retained record count is the resulting `k`.
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
The two distractor labels are exactly `records/<i>/key` and
`records/<i>/value`, where `<i>` is replaced by the zero-based distractor
index in shortest unsigned base-10 with no leading zero.

Frozen master-key bytes are generated once by an OS CSPRNG outside Git. Before
any prompt materialization or candidate tuning, publish:

```text
HMAC-SHA256(
  frozen_master_key,
  "glmaxx.frozen-retrieval-seed-commitment.v1"
)
```

Randomized master-key bytes are generated after the serving policy is frozen
but before either control or candidate inference. Publish exactly:

```text
HMAC-SHA256(
  randomized_master_key,
  "glmaxx.randomized-retrieval-seed-commitment.v1"
)
```

The same key-derived prompts are used for every member of the run family. Raw
master keys and prompts stay in the access-controlled result bundle and are
disclosed only after the immutable result record is sealed.

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

`maximum_output_tokens` is exactly 64 for every retrieval case.

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
candidate total; it does not greedily assume token additivity. Enumeration
stops after 4,096 distinct candidate sequences or 1,000,000 solver states,
whichever comes first. Reaching either cap without an accepted candidate is
fatal.

Each gap has a safepoint at the byte boundary immediately after the most
recent complete distractor record, or after the rendered empty-record
envelope when no distractor precedes it. The materializer tokenizes these
fixed bytes once and selects a rollback token boundary at or before the
safepoint. The committed-prefix receipt covers only tokens strictly before
the rollback boundary and contains their byte count, byte SHA-256, token
count, and SHA-256 of canonical `u32le` token IDs. The bytes from the rollback
boundary through the safepoint remain part of the re-tokenization window.

For candidate screening, it re-tokenizes at most 4,096 token IDs of suffix
context beginning at the rollback boundary, followed by
the candidate and the already fixed bytes through the next complete-record or
query boundary. A screen that would require more suffix context is rejected,
not truncated. Local screening is only an ordering optimization and cannot
accept a candidate. The first four screened candidates that meet the local
length/position predicate are full-validation finalists in enumeration order.
The materializer tokenizes the complete rendered prefix or prompt for each
finalist, at most four full tokenizations per gap, and accepts the first whose
full token sequence begins with the exact committed-prefix receipt and meets the
actual predicate. Any context-sensitive merge that changes that receipt or
the predicted boundary invalidates the finalist.

For the pre-target invocation, the actual predicate is that the target start
is within 64 tokens of the desired start. For the final invocation, it is that
the complete rendered prompt has exactly `prompt_tokens` tokens. No token
additivity or bounded-merge locality is used as a correctness assumption. No
accepted finalist, more than 4,096 screened candidates, more than 1,000,000
solver states, more than 4,096 locally re-tokenized token IDs per candidate,
more than four full tokenizations per gap, or more than a 64-token difference
between actual and desired target-record start is fatal.

Per gap retain the safepoint and rollback byte/token offsets, committed-prefix
receipt, solver-state count, candidate count,
locally re-tokenized token count for every candidate, finalist ordinals,
full-tokenization count, every rejection reason, and the accepted sequence of
lexeme table indices. These counters are checked against the hard caps before
publication.

The final target start is recorded. It must lie in the declared 20%-wide bin:

```text
[bin * 20%, (bin + 1) * 20%)
```

with the upper endpoint inclusive only for bin four. Filler may occur only
between complete distractor records, never inside the target record or query.

### Retrieval leakage rejection

The positive occurrence-scan universe is exactly all generated prompt bytes
across both retrieval strata, including templates, target records,
distractors, query suffixes, and filler; all checker-visible case metadata;
and every calibration-source byte stream. Each file or field is scanned
independently, so concatenation boundaries cannot create an occurrence. The
universe deliberately excludes target files, typed expectation records, and
model-result evidence because those artifacts intentionally contain the
answer and are never model input. Those excluded artifacts remain
access-controlled and are covered by their ordinary content hashes.

For every byte stream in the positive occurrence-scan universe, scan:

- raw UTF-8 bytes;
- Unicode NFC and NFKC forms;
- case-folded Unicode;
- tokenizer token IDs;
- all token n-grams of answer length; and
- every other generated case and every calibration source.

The answer and every contiguous substring of its hexadecimal payload of at
least 16 characters must occur exactly once in the entire universe: inside
that case's target record. The lookup key must occur exactly twice in its
case's prompt—the target record and query—and zero times elsewhere. Neither
may occur in a source URL, checker-visible metadata, template, distractor,
filler lexeme, other prompt, or calibration record.

The generator also constructs a negative prompt by removing the complete
target record and its one separating LF without changing any other byte. A
separate capability-isolated negative checker receives only:

```text
negative prompt bytes and token IDs
lookup key
context band
position bin
stratum ordinal
case_id
negative-prompt byte and token hashes
```

It parses complete `<record>` elements and returns the fixed enum
`NotRecoverable` with no value bytes when no exact key is present. Success
requires that enum, an empty value, and zero occurrences of the answer and
its protected substrings under every positive scan transform. The checker
process has no filesystem or network capability and no inherited descriptor
except its allowlisted input and output pipes.

The negative checker is explicitly denied answer bytes, expected hashes,
target files or paths, typed expectation records, target offsets or content,
positive-prompt bytes or token IDs, retrieval master keys, key-bearing
derivation inputs, result outputs, and any cache/page identity known to
contain the answer. Its binary SHA-256, container digest, canonical allowlist
record, negative prompt hashes, scan evidence, terminal enum, and empty output
are retained. Any denied field visible in its process map or descriptor table
is a corpus-generation failure. The negative prompt is never sent to the
model.

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

The typed expectation is the fixed rule
`CONTROL_RELATIVE_TOKEN_LIMIT.v1`, containing the selected maximum, exact
prompt and code-stream hashes, and the absolute invariants below. It is
serialized and hashed into `expected_sha256` before any model executes; it
does not contain an observed terminal class, count, or prefix.

The authoritative control is selected only by the immutable
`QualityRun.v2.control_selection_manifest_sha256` and
`quality_control_policy_digest`. All control cases execute before any
candidate case. For each case the evaluator seals a canonical
`TokenLimitControlReceipt.v1` containing `quality_run_sha256`, `case_id`,
control policy digest, raw output artifact SHA-256, terminal class, committed
token count, visible-byte SHA-256, parser-state SHA-256, and all absolute-
invariant bits. The case-ID-ordered receipt index is content-addressed; its
root digest and storage receipt are published before candidate admission.
Candidate workers receive this sealed root read-only and must match the
receipt selected by `case_id`. Missing, mutable, post-candidate, policy-
mismatched, or invariant-failing control evidence invalidates the run. Thus
the paired terminal source is a pre-candidate sealed control observation, not
a candidate-dependent expectation rewrite.

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

The fixed table contains exactly these five scalar sequences in index order:

```text
0: U+5317 U+6975
1: U+0645 U+0631 U+062D U+0628 U+0627 U+1F642
2: U+0939 U+093F U+0928 U+094D U+0926 U+0940
3: U+65E5 U+672C U+8A9E
4: U+0065 U+0301 U+002F U+00E9
```

No Unicode normalization is performed. Select `i=bounded(5)` with exact label
`incremental_utf8/item`. The initial string is table item `i`. Its exact target
is `string || "|" || string || "|" || string`.

A split exists only when a boundary between consecutive raw token-output-table
byte strings falls strictly inside the UTF-8 byte sequence of one Unicode
scalar in the exact target tokenization with no special tokens. A boundary
between scalars or a split visible only after whole-sequence decode does not
qualify.

If the initial target has no qualifying split, derive
`start=bounded(25)` with exact label
`incremental_utf8/fallback_pair_start`. For `q=0..24`, let
`p=(start+q) mod 25`, `left=floor(p/5)`, and `right=p mod 5`. The candidate
string is the exact scalar sequence `table[left] || table[right]` with no
separator; equal indices are allowed. Tokenize its three-copy target as above
and select the first candidate with a qualifying split. If all 25 fail,
materialization is fatal. Retain the initial index, fallback start or null,
tested pair indices in order, token IDs, raw token-output bytes, qualifying
scalar byte interval, and boundary offset.

The prompt asks for the selected string exactly three times separated by
`|`. Maximum output is 64. Success requires exact text and EOS.

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
3. independent byte serialization of every exact schema template for every
   possible derived metavariable, including C0 escaping and Unicode scalar
   counts;
4. each JSON family and every parser/schema failure class, including duplicate
   keys, ambiguous `oneOf`, mutated keyword order, and UTF-16 length rejection;
5. each long-output family at all five token bands, including a synthetic
   non-monotonic token-length fixture that proves first-failure stopping;
6. exact-fit, one-token-short, and one-token-over target construction;
7. expected-target repetition that must not false-positive;
8. candidate-only loops for every period `1..32`;
9. all retrieval bands, bins, first/last ordinals, and exact prompt length;
10. safe-lexeme table serialization, digest sensitivity, duplicate rejection,
    and decode mismatch;
11. solver state/candidate/window/finalist caps, safepoint rollback, no lexeme
    solution, and context-sensitive BPE receipt rejection;
12. answer/key collision and every leakage-normalization failure over the exact
    positive universe;
13. negative-checker capability allowlist and a fault-injection proof that each
    denied answer-bearing field makes publication fail;
14. literal frozen/randomized commitment domains and stratum separation;
15. all three EOS IDs, early EOS, length stop, post-terminal rejection, and a
    candidate blocked until its sealed control receipt exists;
16. mutation of a sealed control receipt, policy mismatch, and candidate-
    dependent expectation rewrite rejection;
17. incomplete tool/reasoning parser states;
18. every incremental UTF-8 initial item and all 25 fallback pairs, including
    raw-byte split acceptance, scalar-boundary rejection, and incomplete UTF-8;
19. duplicate/missing/reordered cases and index corruption;
20. changed tokenizer/template/lexeme-table identity; and
21. crash before and after rename, existing destination, symlink/hard-link
    substitution, cross-filesystem publication, concurrent temp collision, and
    nondeterministic rerun.

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

## Revision-2 correction ledger

This revision changes no case count, context band, quality threshold, or GPU
authorization. It closes every finding in the first adversarial review:

- exact one-line schemas now determine every JSON/schema prompt byte;
- the identity directly binds a canonical safe-lexeme table;
- the retrieval solver has explicit state, candidate, local-window, and full-
  tokenization caps with full-prefix acceptance;
- the positive leakage universe and negative checker's capability allowlist
  are disjoint and explicit;
- randomized commitment and distractor-label bytes are literal;
- incremental UTF-8 items, fallback derivation, pair order, and split predicate
  are exact;
- C0 escaping, Unicode length semantics, first-failure long-output stopping,
  and secure atomic publication are explicit; and
- token-limit control behavior comes from a sealed pre-candidate control
  receipt under the immutable parent run policy.

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
