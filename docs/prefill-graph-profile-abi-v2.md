# Prefill row-bucket and graph-profile ABI v2

Date: 2026-07-29

Status: design candidate; adversarial review required before implementation

GPU authorization: none

## Decision

Replace the compute-plan field named `verifier_row_bucket` with the
mode-neutral `row_bucket`, give prefill and decode/verify separate row
ceilings, and bump the semantic plan and graph-profile ABIs to v2.

The canonical plan hash input remains 85 bytes. Offset 29 remains one
little-endian `u32`; its v2 meaning is the captured padded query-row capacity
for every compute mode.

Compatibility with v1 is intentionally not preserved. GLMAXX targets one
engine, one model, and one hardware generation, and no production v1
checkpoint or server exists. A v1 plan, graph profile, or hash domain must
fail closed in a v2 process.

## Blocking contradictions in v1

The current implementation cannot express the intended SM120 prefill graph
family for two independent reasons.

### One key for every prompt chunk

`GraphKey` currently contains:

```text
(mode, sequence_bucket, verifier_row_bucket, mtp_depth,
 attention_transport)
```

For prefill, validation requires `verifier_row_bucket == 0`. Graph keys must
be unique. Two prefill captures with the same sequence bucket and attention
transport therefore collide even if one captures 1,536 query rows and the
other captures 3,072.

Varying the sequence bucket or attention transport merely to distinguish
prompt chunk sizes would encode a false hardware distinction and would not
cover all intended combinations.

### Prefill is incorrectly capped at the verifier limit

`validate_request` applies `MAX_VERIFIER_ROWS == 448` to `query_rows` before
dispatching on mode. The planned and already source-bounded prefill control
uses up to 3,072 rows. A correct 3,072-row prefill plan is therefore rejected
as `PlanError::Shape`.

The 448 ceiling is correct for:

```text
64 active sequences * (MTP6 depth + 1) = 448 verifier rows
```

It is not a prefill-token ceiling.

## v2 identities and constants

Implementation must introduce:

```text
STEP_PLAN_ABI             = "glmaxx.step-plan.v2"
PLAN_HASH_DOMAIN          = "glmaxx.step-plan.v2\0"
GRAPH_PROFILE_SCHEMA      = "glmaxx.graph-profile.v2"
GRAPH_PROFILE_HASH_DOMAIN = "glmaxx.graph-profile.v2\0"

MAX_PREFILL_QUERY_ROWS    = 3072
MAX_DECODE_VERIFY_ROWS    = 448
```

`STEP_PLAN_HASH_INPUT_BYTES` remains 85 and the appended hash record remains
117 bytes. Retaining the byte count does not make v1 and v2 compatible; the
ABI string, field semantic, validation, graph schema, and hash domains all
change.

The first reviewed SM120 sweep starts at the measured 3,072-row control.
Increasing the maximum later requires a reviewed ABI amendment backed by
kernel and memory evidence. Smaller captured buckets are profile data, not
new constants.

## v2 `StepPlan`

The field at byte offset 29 is:

| Field | Type | Meaning |
|---|---|---|
| `row_bucket` | `u32` | captured padded query-row capacity |

Mode invariants are:

### `PREFILL`

```text
1 <= scheduled_prompt_tokens
scheduled_prompt_tokens == query_rows
query_rows <= row_bucket <= 3072
mtp_depth == 0
sampling_route_id == 0
attention_transport in {PREFILL_CKV, PREFILL_QUERY}
```

### `DECODE`

```text
scheduled_prompt_tokens == 0
query_rows == active_sequences
query_rows <= row_bucket <= 448
mtp_depth == 0
sampling_route_id != 0
attention_transport == DECODE_QUERY_LSE
```

### `VERIFY`

```text
scheduled_prompt_tokens == 0
query_rows == active_sequences * (mtp_depth + 1)
query_rows <= row_bucket <= 448
1 <= mtp_depth <= 6
sampling_route_id != 0
attention_transport == DECODE_QUERY_LSE
```

### `CACHE_ONLY`

`row_bucket` is zero with every other compute field. `MIXED` remains rejected
until its dual-transport contract is reviewed.

All arithmetic remains checked. `row_bucket == 0`, `query_rows == 0`, a real
row count above its mode ceiling, or a bucket smaller than the real row count
fails before hashing or worker dispatch.

## v2 graph key and entry

`GraphKey.v2` is:

```text
(mode, sequence_bucket, row_bucket, mtp_depth, attention_transport)
```

Every entry must satisfy:

```text
maximum_query_rows == key.row_bucket
maximum_active_sequences <= key.sequence_bucket
```

For prefill:

```text
maximum_prompt_tokens == key.row_bucket
1 <= key.row_bucket <= 3072
```

For decode/verify:

```text
maximum_prompt_tokens == 0
1 <= key.row_bucket <= 448
```

The exact equality prevents two different claimed capacities from sharing
one captured-shape key and avoids a profile whose bucket has unused,
unexplained rows. Real work may be smaller than the bucket; graph metadata
and row work are masked to `query_rows`.

The graph-profile hash serializes `row_bucket` in the same position formerly
occupied by `verifier_row_bucket`, under the v2 hash domain. Unique keys then
permit, for example:

```text
(PREFILL, C4, 1536, MTP0, PREFILL_CKV)
(PREFILL, C4, 3072, MTP0, PREFILL_CKV)
```

without inventing a different sequence bucket or transport.

Context band, PCIe topology class, and immutable weight policy do not become
graph-key fields. Context/topology select a globally identical qualified
route table when kernel structure is unchanged. If measurements show a
different captured DAG is required, that DAG needs a distinct reviewed
graph identity rather than a rank-local decision.

## Scheduler and compiler binding

The scheduler's captured-shape selection continues to construct candidates
against each concrete `GraphEntry`. Under v2 it can evaluate actual prefill
row-bucket families with identical sequence/transport keys.

`best_graph` must compare `row_bucket` as part of its deterministic key. The
compiler copies `entry.key.row_bucket` into the immutable `StepPlan`; it must
never derive the bucket independently from current batch size. The
`GraphProfile::admit` reconstruction must require exact equality between the
plan key and entry key, then separately require:

```text
active_sequences <= maximum_active_sequences
scheduled_prompt_tokens <= maximum_prompt_tokens
query_rows <= maximum_query_rows
```

All four ranks consume the same plan hash and graph ID. No rank chooses its
own prompt bucket, attention transport, or route.

The CPU reference continues to maximize `(query_rows, active_rows)` among
legal candidates. That policy proves bounded progress; it is not promoted as
the measured production latency policy. Authorized SM120 evidence must
populate the profile's admission/SLO classes and the global prefill route
cost table before performance claims.

## Required implementation sweep

After design acceptance, one atomic implementation candidate must update:

- `glm-engine::step`: field, constants, v2 hash domain, mode validation, and
  byte-stability tests;
- `glm-engine::graph`: key field, v2 schema/domain, entry validation,
  admission, and profile-hash tests;
- `glm-scheduler`: graph selection, compiler binding, helpers, and tests;
- `glm-serving` and `glm-cli`: graph fixtures and proof constructors;
- offline contract prose and deterministic fixtures that name v1 or the
  448-row global limit;
- every checked manifest/profile whose hash or ABI identity changes; and
- current-tree acceptance and Phase-B/Phase-C pins after the implementation
  receives its own adversarial token.

Historical review candidates remain immutable evidence of their pinned
commits. They are not silently rewritten to claim v2 acceptance.

## CPU exit gate

The implementation review must include tests proving:

1. a 3,072-row prefill plan and matching graph entry pass;
2. prefill row 3,073 fails;
3. decode/verify row 449 fails while prefill row 449 passes;
4. two prefill entries with identical sequence bucket and transport but
   distinct row buckets coexist and hash distinctly;
5. an exact duplicate v2 key fails;
6. `maximum_query_rows != row_bucket` and prefill
   `maximum_prompt_tokens != row_bucket` fail;
7. `query_rows > row_bucket` fails for every compute mode;
8. cache-only nonzero `row_bucket` fails;
9. compiler output copies the selected entry bucket and passes graph
   admission on all four rank views;
10. scheduler chunking selects only reachable v2 captures;
11. v1 plan/profile identities and hash domains are rejected; and
12. all canonical record sizes remain exact despite the semantic bump.

Then run the full local gate and pin exact source, fixture, and result hashes.

## Hardware and performance boundary

This design opens no CUDA or cn4 gate. After CPU implementation review:

1. compile and capture each required prefill bucket on SM120;
2. compare eager and captured output for every bucket/transport;
3. sweep around the 3,072-row control;
4. measure CKV versus query transport by prompt chunk, context band, batch
   shape, and PCIe topology;
5. freeze one rank-invariant cost table and graph profile; and
6. rerun cold/warm concurrent prefill and decode-interference gates.

Until then there is no graph-capture correctness, prefill throughput, route
optimality, model quality, or serving-performance claim.
