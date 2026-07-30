# Matched runtime control and comparison contract v1

Date: 2026-07-30

Status: design candidate; adversarial review required before implementation

GPU claim: none

## Purpose

The production gate requires an honest end-to-end comparison against pinned
general-purpose runtimes. A shared prompt set and the same number of GPUs are
not enough. Weight membership, tokenizer behavior, cache residency, offered
load, MTP posture, resource ceilings, and measurement boundaries can each
reverse a result.

This contract defines:

- immutable identities for glmaxx and every control runtime;
- a Rust-owned connector boundary for target and control endpoints;
- one fail-closed comparison key;
- exact request, streaming, cache, MTP, and scheduler rules;
- common raw records and derived metrics;
- paired-run and statistical verdict rules; and
- explicit treatment of unsupported or physically impossible controls.

It extends `docs/benchmark-contract.md` and is intended to feed the black-box
driver specified by `docs/sustained-serving-load-fault-v1.md`. Neither design
is implementation authority until its own adversarial review passes.

## Nonclaims and authority

This document does not:

- select or certify a current vLLM, SGLang, llama.cpp, or ExLlama revision;
- claim that every control can load GLM-5.2 or every weight policy;
- claim that a full NVFP4, FP8, or BF16 checkpoint fits four 96 GiB GPUs;
- permit a format, tokenizer, template, precision, MTP, or cache mismatch;
- authorize downloading, converting, or deleting model data;
- authorize starting or stopping a process, container, or GPU workload;
- authorize connecting to cn4;
- implement a driver, connector, launcher, tokenizer, or result comparator;
- provide CPU, CUDA, checkpoint, capacity, quality, or performance evidence;
  or
- make one runtime's missing feature look like zero latency or zero
  throughput.

A separately authorized operator starts each runtime from a reviewed runbook.
The benchmark process never launches, stops, signals, reconfigures, or clears
the target or a control.

## Comparison classes

Every published row has exactly one comparison class.

### `MATCHED_END_TO_END`

All fields in the comparison key are equal and every required preflight,
runtime observation, cache assertion, request record, and health check
passes. Only this class can support a faster, lower-latency, higher-capacity,
or noninferior claim.

### `PRODUCT_POSTURE`

The row is useful operational evidence, but one or more deliberate product
choices differ. Examples include glmaxx MTP3 versus control MTP0, EXL3 versus
NVFP4, HBM-only versus CPU offload, or different admitted token ceilings.
Such rows are reported in a separate table and never enter a matched ratio.

### `UNAVAILABLE`

The runtime cannot express the requested model, weight policy, context,
cache tier, MTP depth, sampling route, or API semantics on the target
resources. The record retains the exact failed capability and evidence.
Unavailable is not a performance value.

### `INVALID`

The intended matched run suffered identity drift, an unproven tokenizer or
streaming boundary, an unexpected cache hit, a dropped row, a restart,
thermal invalidation, load-generator lag, a fallback, or another contract
violation. Invalid rows remain in the bundle and are excluded from claims
with a machine-readable reason. They are never silently retried away.

## Runtime families

The initial control inventory names:

- vLLM;
- SGLang;
- llama.cpp;
- ExLlama/ExLlamaV3;
- the retained existing-runtime checkpoint path used by the production
  evidence repository; and
- BF16, FP8, and direct decoded-weight controls where a complete matched cell
  is physically and semantically possible.

The family name is not a revision pin and does not imply support. Each
family/cell receives an independent capability receipt. A runtime may be an
eligible control for one policy and unavailable for another.

Full-model BF16, FP8, and standalone NVFP4 rows are never assumed to fit.
They may be admitted only when the same four target devices plus explicitly
matched DRAM/NVMe resources can load the complete model and retain the
required KV escrow. Otherwise those precision controls remain kernel,
one-layer, or product-posture evidence and cannot be placed in the full-model
matched table.

## Immutable runtime identity

Each endpoint is bound to a canonical `RuntimeIdentity.v1` before any warmup:

```text
schema                         glmaxx.runtime-identity.v1
runtime_family                 closed enum
runtime_source_url             canonical URL
runtime_source_commit          exact 40-hex commit
runtime_source_tree_sha256     canonical tracked-tree digest
runtime_dirty                  false
patch_bundle_sha256            exact or explicit none
container_manifest_digest      immutable sha256 digest
container_image_id             local immutable image identity
container_rootfs_sha256        exact when locally built
launcher_binary_sha256         exact
server_binary_sha256           exact
dependency_lock_sha256         exact
compiler/toolchain identities  exact
CUDA/driver/NCCL identities    exact
kernel-library identities      exact
argv_sha256                    canonical ordered argv
safe_environment_sha256        canonical allowlisted environment
configuration_sha256           canonical runtime configuration
model_revision                 exact immutable revision
model_manifest_sha256          exact
rank_payload_sha256[4]         exact
weight_policy_sha256           exact
precision_membership_sha256    exact
tokenizer_sha256               exact
chat_template_sha256           exact or explicit none
draft_model_revision           exact or explicit none
draft_manifest_sha256          exact or explicit none
topology_record_sha256         exact
resource_contract_sha256       exact
endpoint_origin                exact scheme/host/port/path
```

The identity UUID is the SHA-256 of the domain string
`glmaxx.runtime-identity.v1\0` followed by canonical bytes for every field
except the UUID. Wall-clock timestamps, hostnames, usernames, credentials,
tokens, and mutable tags are excluded from the identity and retained only
where safe in an external provenance envelope.

An endpoint does not self-attest these fields. The operator supplies a signed
or content-addressed launch receipt, and a read-only preflight observes the
process, executable, container, device binding, loaded model identity, and
health identity. A disagreement is `INVALID`; an omitted required field is
not inferred from a runtime family default.

Secrets are never hashed as a substitute for omission. Authorization headers
and credential-bearing environment entries are supplied to the connector
through an external secret channel and are neither logged nor incorporated
into evidence.

## Precision membership and logical-weight identity

The weight comparison has two independent identities:

1. **precision membership** maps every logical tensor and protected slice to
   codec, bit width, scale mode, rounding mode, and accumulation policy; and
2. **logical dequantized weight identity** hashes the canonical CPU decode of
   every quantized tensor using the accepted format oracle.

`MATCHED_END_TO_END` requires identical precision membership. A control may
repack physical bytes only when:

- the source rank manifests and logical tensor intervals are identical;
- both packers are pinned;
- every tensor passes the accepted CPU pack/dequant equivalence gate;
- the complete ordered logical-dequant digest matches;
- padding is excluded from the logical digest and separately proven
  canonical; and
- no tensor silently falls back to a different precision policy.

Equivalent average bits, file size, KLD, or a sample of decoded rows does not
establish matched membership. A fallback counter greater than zero makes the
run invalid.

The comparison key also binds accumulator, activation, attention, embedding,
LM-head, and KV precision. A weight-only match cannot hide a different KV or
activation policy.

## Resource contract

One canonical `ResourceContract.v1` binds:

- exactly four target GPUs by UUID and PCIe BDF;
- rank-to-device and CPU/NUMA affinity;
- TP and DCP posture;
- GPU power limit, application clocks where supported, and persistence
  posture;
- per-rank HBM model, graph, workspace, KV, and escrow ceilings;
- host pinned and pageable DRAM ceilings;
- NVMe device identity, namespace, filesystem, direct-I/O posture, and byte
  ceiling;
- maximum queued requests and queued tokens;
- maximum running sequences and batched tokens;
- maximum context, chunked-prefill size, and decode rows;
- prefix-cache enablement and tier budgets;
- MTP target/draft budgets; and
- network transport class, driver placement, and CPU ceilings.

An internal scheduler is free to form different batches; scheduler quality is
part of the comparison. The exposed ceilings, offered arrivals, request
corpus, and resources must remain equal. A control that cannot enforce a
ceiling is unavailable for that cell unless an external, measured limiter
enforces the same bound without changing the timed path.

GPU memory fractions are not accepted as equivalent to bytes. Every runtime
records observed idle, post-load, warmup-peak, measured-peak, and terminal
memory per rank. Host and NVMe usage receive the same start/peak/end
accounting.

No target and control share a GPU concurrently. Runs are sequential, begin
from the same reviewed idle threshold, and use a newly observed target
environment record. An unrelated process, MIG partition, changed clock,
thermal throttle, ECC/Xid event, or topology drift invalidates the block.

## Canonical comparison key

`ComparisonKey.v1` contains only fields that must be equal between the target
and control. Runtime identity, endpoint origin, connector kind, internal
kernel implementation, and internal batch formation intentionally differ and
are referenced beside the key rather than hashed into it.

The canonical key contains:

```text
schema                              glmaxx.runtime-comparison-key.v1
model_revision                      exact
operation_manifest_sha256           exact
target_logical_dequant_sha256        exact
precision_membership_sha256         exact
draft_revision                      exact or none
draft_logical_dequant_sha256         exact or none
draft_precision_membership_sha256    exact or none
tokenizer_sha256                     exact
template_sha256                      exact or none
rope/context policy                  exact
weight/activation/accumulator/KV     exact precision tuple
TP/DCP and collective semantics      exact
sampling/MTP/termination semantics   exact
resource_contract_common_sha256      exact
cache posture/budgets/start state    exact
corpus and token-vector SHA-256      exact
arrival/tenant/fault schedule hashes exact
concurrency/context/length bands     exact
warmup/measured durations            exact
driver/measurement ABI SHA-256       exact
environment common-hardware SHA-256  exact
```

The key uses a length-delimited canonical encoding, fixed field order, closed
enums, exact integer values, and IEEE bit patterns for accepted floating
values. Optional values have distinct absent/present tags. Lists have counts;
maps are forbidden. Its digest is SHA-256 over
`glmaxx.runtime-comparison-key.v1\0 || canonical_bytes`.

The common resource and environment projections omit runtime-specific
binary/container/endpoint identities but include every physical resource and
operating condition that can affect a comparison. Projection is a checked
conversion from the complete records, not a caller-supplied second
description. Every omitted field is enumerated in source, and adding a field
to either complete schema requires an explicit projection-version change.

Equality is byte equality of canonical key bytes, not field-by-field best
effort. A comparison report always retains both complete runtime identities,
both complete observed resource/environment records, their common
projections, and the shared key.

## Capability preflight

Before warmup, each connector produces a canonical
`CapabilityReceipt.v1`. It proves or rejects:

- exact model architecture and revision;
- complete tensor and draft-tensor load;
- rank count and rank/device binding;
- maximum context and rope/scaling posture;
- weight, activation, accumulator, attention, and KV precision;
- TP/DCP and collective routes;
- continuous batching and chunked prefill;
- prefix cache plus each requested HBM/DRAM/NVMe tier;
- MTP depths and draft identity;
- greedy and requested probabilistic sampling;
- deterministic seed handling;
- usage fields and cached-token reporting;
- one-token-per-event streaming or another auditable token clock;
- cancellation and terminal error observation;
- metrics needed by the cell; and
- absence of fallback.

Claims are checked through independent configuration, logs, model inventory,
and deterministic probes wherever the runtime exposes them. Self-reported
capability alone is insufficient.

The receipt includes one disposition per requested cell:
`ELIGIBLE`, `UNAVAILABLE`, or `INVALID`. It never substitutes a nearby
configuration.

## Connector boundary

The future Rust driver owns a closed connector enum. Dynamic plugins,
arbitrary commands, shell fragments, and runtime-supplied parsers are
forbidden.

Every connector may:

- perform bounded HTTP requests to the exact configured origin;
- attach a secret authorization value without logging it;
- parse health, metrics, JSON, and server-sent-event responses;
- submit a pre-rendered prompt or exact token IDs through a reviewed API;
- request cancellation through a fixed connector-specific route; and
- convert raw events into the common record below.

It may not:

- start, stop, signal, or inspect an unrelated process;
- run a shell;
- change runtime configuration or cache state;
- execute a server-provided command;
- read model/checkpoint/cache files;
- scrape unbounded logs;
- synthesize a missing metric; or
- reinterpret an error as a successful empty response.

Connector-specific behavior is fixed in source and selected by enum plus
canonical configuration. Unknown fields, duplicate JSON keys, invalid UTF-8,
oversized headers/events/bodies, response truncation, malformed SSE framing,
and protocol downgrade fail closed.

Target and control use the same driver process, network stack, monotonic
clock, request corpus, and arrival schedule. Connector CPU time, bytes,
retries, and parser stalls are retained. Automatic HTTP or model retries are
disabled.

## Prompt and tokenizer equivalence

The corpus stores for every request:

- canonical pre-rendered UTF-8 prompt bytes;
- exact token IDs;
- prompt byte, token-vector, tokenizer, and template digests;
- special-token policy;
- maximum output tokens, stop policy, and sampling tuple;
- prefix cohort and intended cache posture; and
- expected terminal class.

Chat APIs are not compared by sending messages through runtime-specific chat
templates. The template is rendered once by the accepted corpus tool.
Connectors then use either:

1. exact token-ID ingress; or
2. raw text completion with chat templating and implicit special tokens
   disabled.

Before a cell is eligible, the runtime must reproduce the complete expected
token vector for every distinct corpus prompt or provide a tokenizer endpoint
whose complete results are independently checked. Prompt-token count alone
does not prove equality. If the runtime cannot expose exact tokenization, the
cell is unavailable.

No lossy text round trip is used for prompts whose token vector does not
round-trip uniquely. NUL, invalid UTF-8, normalization-sensitive, and special
token cases are explicit preflight fixtures.

## Output and streaming equivalence

For greedy MTP0 correctness cells, connectors retain exact output token IDs.
If an endpoint returns only text, the output must round-trip to one unique
token sequence under the pinned tokenizer and match a non-streaming
token-logprob or token-ID audit. Otherwise correctness is unavailable.

An inter-token-latency sample requires one visible output token per timestamped
event. A connector may split an event only when the event supplies exact
per-token timestamps. It may not assign the receive time of a multi-token
chunk to invented token boundaries.

For runtimes that batch stream output:

- throughput and terminal latency may remain eligible if exact output-token
  counts are proven;
- ITL is marked unavailable; and
- the missing ITL cannot be replaced by bytes, characters, or SSE chunk
  intervals.

The record distinguishes visible target tokens, proposed draft tokens,
accepted draft tokens, rejected draft tokens, and bonus tokens. Useful output
throughput counts only visible committed target tokens. Draft proposals and
retokenized text pieces never inflate it.

EOS, stop-string, length, cancellation, and error termination are distinct.
A connector must preserve the server's raw terminal cause and the normalized
cause.

## Sampling and MTP equivalence

The primary runtime comparison is greedy MTP0. It binds:

- temperature `0`;
- top-p `1`;
- no top-k truncation;
- no penalties;
- exact maximum output length;
- identical stop behavior; and
- the same distributed tie policy.

Probabilistic rows require the separately accepted distributed sampling ABI,
identical parameters, seed, initial RNG counter, and draw-ticket semantics.
An API that accepts a seed but cannot establish counter behavior is
unavailable for a matched probabilistic row.

MTP-K rows are matched only when both runtimes use:

- the same target weights;
- the same ordered draft-layer weights and precision;
- identical K;
- the same verification and bonus-token policy;
- the same maximum generation budget; and
- distribution-preserving acceptance accepted by the quality gate.

A control without MTP-K remains a valid MTP0 control. Comparing glmaxx MTP-K
to that control is `PRODUCT_POSTURE`, not `MATCHED_END_TO_END`.

Target-only quality gates always precede performance enablement. A faster MTP
row that has not passed `docs/quality-acceptance-v1.md` cannot be published
as a win.

## Cache posture equivalence

Every row names one cache posture:

- `PREFIX_DISABLED`;
- `COLD_MISS`;
- `WARM_HBM`;
- `WARM_DRAM`;
- `WARM_NVME`; or
- `PRESSURE_MIX`.

`COLD_MISS` randomizes the first complete 64-token page, requires no prior
namespace entry, and requires observed `cached_tokens = 0`. Restarting a
runtime is neither necessary nor sufficient to prove a miss.

Warm rows use identical sealed prefix token IDs and begin only after the
publisher is terminal and the required tier transition is observed. The
matched prefix length, source tier, restored bytes, and HBM residency before
decode are required. A runtime that cannot report or independently prove
those fields is unavailable for that warm-tier cell.

Evicting through an admin API, clearing a cache, or restarting a runtime is a
setup mutation performed only by a separately authorized operator between
runs. It is recorded in the provenance envelope and is never performed by
the driver.

Prefix capacity, eviction policy, DRAM/NVMe ceilings, direct/buffered I/O,
and write durability are all recorded. Capacity ceilings, starting residency,
tier semantics, I/O posture, and durability belong to the comparison key.
The internal eviction algorithm may differ: its behavior under the identical
pressure schedule is part of what the run measures. A runtime with only an
HBM prefix cache cannot serve as the matched control for DRAM or NVMe reuse.

## Workload and scheduler equivalence

The shared run manifest binds:

- every request descriptor and its order-independent content hash;
- open-loop absolute arrival deadlines or closed-loop user transitions;
- tenant weights and quotas;
- concurrency and context band;
- prompt/output length bands;
- sampling and MTP depth;
- cache posture and prefix cohorts;
- cancellation and slow-reader schedule;
- warmup and measured intervals; and
- all random seeds.

Open-loop deadlines never shift when an earlier request completes. A
connector backpressure delay is server-path latency; driver inability to
dispatch on time is separately measured load-generator lag.

The target and control need not produce the same internal batch shapes.
They must receive the same offered work and enforce the same external
ceilings. Queue rejection, overload, admission delay, and cancellation are
outcomes, not rows to discard.

Throughput cells suppress EOS and stop strings and require exactly the pinned
number of visible output tokens unless the request is rejected or fails.
This prevents one runtime's different greedy text from doing less decode
work. A runtime that cannot disable early termination is unavailable for
fixed-length throughput cells; its natural-termination behavior may be
reported as `PRODUCT_POSTURE`. Quality and termination cells retain normal
EOS/stop semantics and are never substituted for fixed-work performance.

The first qualification matrix contains:

- concurrency `C1, C2, C4, C8, C16, C32, C64` where admitted;
- starting context `0, 16K, 64K, 128K, 480K`, and near 1M;
- cold prefill `8K, 64K, 128K, 480K`, and near 1M;
- output bands `32, 128, 1K`;
- MTP depths `0, 1, 2, 3, 4, 5, 6`;
- cold miss and each supported warm tier;
- short-decode, long-decode, and chunked-prefill mixes; and
- each accessible PCIe layout.

An omitted cell remains omitted with a reason. Capacity-limited cells are
not extrapolated.

`near 1M` means an exact total sequence budget of 1,048,576 positions:
prompt tokens plus maximum visible target output tokens. Draft sidecars,
tentative target/draft pages, page slack, and failure escrow do not increase
logical context, but their complete memory cost remains charged.

## Common raw request record

Every request produces one canonical `RuntimeRequestRecord.v1`:

```text
run_uuid / block_uuid / request_uuid
runtime_identity_sha256
comparison_key_sha256
corpus_request_sha256
tenant / sequence / prefix cohort
scheduled / dispatch / request-commit monotonic ns
headers / first-token / token-event[] / terminal monotonic ns
raw-event byte ranges and hashes
prompt / cached / computed / visible-output token counts
proposed / accepted / rejected / bonus draft counts
queue / prefill / decode observations where exposed
cache tier and byte deltas
sampling seed / initial / final counter
normalized and raw terminal cause
HTTP status and bounded error identity
connector counters and parser disposition
server request ID and health generation
```

Raw response bytes are stored outside Git in bounded chunk files. The request
record refers to byte intervals and SHA-256 digests. Publication is
transactional: chunks and request records are durable before the canonical
run manifest is published last with no-replace semantics.

Monotonic timestamps are process-local observations. Wall-clock correlation
is retained separately and never used to derive latency.

## Metric definitions

All distributions retain raw samples.

- **Dispatch lag** =
  `actual_dispatch - scheduled_arrival`.
- **Queue-included TTFT** =
  `first_visible_token_byte - request_commit`.
- **Terminal latency** =
  `terminal_event - request_commit`.
- **ITL[i]** =
  `visible_token_event[i] - visible_token_event[i-1]`, only with proven
  one-token event boundaries.
- **Visible decode throughput** =
  committed visible output tokens divided by the common measured interval.
- **Useful-token throughput** =
  the same visible committed target tokens divided by the interval; draft
  proposals are excluded.
- **Prefill throughput** =
  newly computed prompt tokens divided by the prefill interval; cached tokens
  are excluded.
- **Acceptance rate** =
  accepted draft tokens divided by proposed draft tokens, with zero-proposal
  rows reported as not applicable.
- **Goodput** =
  successful visible tokens for requests meeting the declared SLO divided by
  the measured interval.
- **Fairness** =
  per-tenant achieved weighted service plus Jain's index over normalized
  tenant service; both raw tenant series are retained.
- **Capacity** =
  the largest independently repeated admitted posture that preserves model,
  graph, KV, workspace, and failure escrow without OOM, fallback, or
  admission-contract violation.

TTFT, ITL, and terminal latency report p50, p95, p99, maximum, count, and
bootstrap intervals. Throughput reports every fixed-duration block, not just
the run aggregate. Errors, retries, cancellations, disconnects, restarts,
fallbacks, cache transitions, and dropped observations are counts, never
percentages without denominators.

Kernel, framework, scheduler, TP, DCP, tier I/O, and end-to-end time are
separate ledgers. Exclusive phase totals must reproduce measured wall time
within the tolerance pinned by the run manifest.

## Paired run design

One comparison block contains one target run and one control run with:

- identical comparison key and arrival/corpus hashes;
- a fresh environment and idle receipt before each run;
- the same warmup and measured duration;
- no concurrent GPU workload;
- health and resource observations before and after; and
- randomized target/control order derived from the block seed.

At least five valid blocks are required for a performance claim. Runs are
not repeated merely because their result is unfavorable. A failed or invalid
member invalidates the pair but remains published.

The primary ratio is computed per pair before aggregation. Throughput ratios
are `target / control`; latency ratios are `target / control`, so higher is
better only for throughput. The result bundle reports the median paired
ratio and a deterministic 95% percentile-bootstrap interval over paired
ratios. The comparator performs exactly 100,000 resamples of `N` pairs with
replacement, computes one median per resample, sorts IEEE total-order finite
values, and selects zero-based nearest-rank indices 2,499 and 97,499. It uses
SplitMix64 with wrapping `u64` arithmetic:

```text
state += 0x9e3779b97f4a7c15
z = state
z = (z ^ (z >> 30)) * 0xbf58476d1ce4e5b9
z = (z ^ (z >> 27)) * 0x94d049bb133111eb
next = z ^ (z >> 31)
```

The initial state is the first eight little-endian bytes of
`SHA-256("glmaxx.paired-bootstrap.v1\0" || comparison_key_sha256)`.
To sample `[0, N)`, interpret `next` as `u128`, reject it when it is at least
`floor(2^64 / N) * N`, and otherwise use `next mod N`. An odd-sample median
is its middle value; an even-sample median is
`lower + (upper - lower) / 2` in binary64 round-to-nearest-even. Any
nonfinite or nonpositive ratio is invalid. Unpaired aggregates are
descriptive only.

A material throughput win requires:

- a median useful-token-throughput ratio of at least `1.05`;
- a lower 95% paired-bootstrap bound greater than `1.00`;
- no target matched cell with a median useful-throughput ratio below `0.95`;
- no target matched cell with a p99 ITL ratio above `1.05`; and
- no reduction in admitted context/KV capacity.

If the interval crosses the required boundary, the verdict is
`INCONCLUSIVE`, not PASS. The complete matrix is reported even when only one
priority regime supplies the material win required by M7.

## Invalidation and verdict precedence

The following precedence applies before statistics:

1. identity, source, model, precision, tokenizer, draft, or topology mismatch
   → `INVALID_IDENTITY`;
2. unexpressible requested capability → `UNAVAILABLE`;
3. fallback, restart, rank loss, OOM, nonfinite output, or fatal server event
   → `INVALID_RUNTIME`;
4. unexpected cache state or cache-accounting gap → `INVALID_CACHE`;
5. tokenization, output-token, or stream-boundary ambiguity
   → `INVALID_TOKEN_BOUNDARY`;
6. corpus, arrival, resource, or scheduler-ceiling drift
   → `INVALID_WORKLOAD`;
7. excessive driver lag, driver CPU saturation, parser loss, or evidence gap
   → `INVALID_DRIVER`;
8. clock, power, thermal, PCIe, process, or environment drift
   → `INVALID_ENVIRONMENT`;
9. insufficient valid pairs or a confidence interval crossing a gate
   → `INCONCLUSIVE`; otherwise
10. evaluate the pinned PASS/FAIL thresholds.

The earliest applicable reason is canonical; all additional reasons are
retained. No automated retry can erase a reason or replace a run UUID.

## Result bundle

The external bundle contains:

- runtime identities and launch receipts;
- environment, topology, and resource contracts;
- capability receipts;
- corpus, arrival, tenant, cache, and fault manifests;
- exact connector and comparator binaries;
- raw request records and response chunks;
- health, metrics, device, host, thermal, and storage snapshots;
- per-block phase ledgers;
- every derived sample and aggregate;
- paired comparison tables;
- unavailable, invalid, and omitted cell tables;
- exact commands and safe environment;
- all content hashes;
- known limitations; and
- one canonical `PASS`, `FAIL`, or `INCONCLUSIVE` verdict.

Model data, request corpora requiring external storage, raw responses, logs,
and benchmark evidence are not committed to Git.

## CPU proof required before endpoint use

After design acceptance, the first implementation remains CPU-only. It must
use deterministic local mock endpoints to prove:

1. canonical identity, capability, comparison-key, and UUID serialization;
2. equality and one-field mismatch for every comparison-key member;
3. physical repack acceptance only after full logical-dequant identity;
4. tokenizer equality plus normalization/special-token/round-trip failures;
5. bounded fragmented HTTP and SSE parsing with duplicate/oversized/truncated
   input rejection;
6. one-token events, multi-token ITL unavailability, and exact token counts;
7. cache cold/warm/tier assertions and an unexpected-hit rejection;
8. MTP0/MTP-K classification and draft-count arithmetic;
9. open-loop schedule preservation under connector backpressure;
10. request, raw-chunk, run-manifest, and no-replace publication recovery;
11. every verdict-precedence branch;
12. deterministic paired bootstrap and threshold boundary cases;
13. unavailable versus invalid versus product-posture separation;
14. secret omission from all records and errors; and
15. zero process launch, signal, shell, cache mutation, GPU, or cn4 action.

The proof must test at least two distinct connector implementations against
the same mock semantic transcript so connector normalization cannot define
the oracle by itself.

## Staged implementation and qualification

The required order is:

1. adversarial review of this design;
2. adversarial review of the sustained-serving driver design;
3. Rust schemas, canonical encoders, comparison-key checker, and CPU tests;
4. bounded mock HTTP/SSE connector proofs;
5. source-only connector reviews;
6. operator selection of exact control revisions and immutable containers;
7. source and artifact provenance review for those pins;
8. separately authorized loopback tests with non-model fixtures;
9. completed kernel, layer, checkpoint, quality, and capacity gates;
10. separately authorized target-hardware capability preflight;
11. separately authorized MTP0 matched smoke;
12. full paired matrix, profiler ledger, and reliability qualification; and
13. adversarial review of the immutable result bundle before a performance
    claim.

No later step authorizes an earlier missing gate. A valid control pin does not
authorize a GPU run, and a GPU window does not authorize an unreviewed
connector.

## Exit criteria

This design is ready for implementation only when adversarial review confirms
that:

- every material precision, model, tokenizer, workload, cache, MTP, resource,
  and environment difference enters the comparison key;
- missing control capabilities cannot become favorable numeric values;
- connectors cannot mutate runtimes or invent token timing;
- the target and controls receive the same offered work without requiring
  identical internal scheduling;
- cold/warm and tier claims are independently observable;
- paired statistics cannot cherry-pick runs or hide uncertainty;
- full-model physical impossibility remains an explicit unavailable result;
  and
- implementation and hardware work remain separately gated.
