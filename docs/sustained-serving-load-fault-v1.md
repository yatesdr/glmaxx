# Sustained serving load and fault qualification v1

Date: 2026-07-29

Status: design candidate; adversarial review required before implementation

GPU claim: none

## Purpose

Single-request unit tests do not establish a multi-user serving engine. This
contract defines one Rust-owned, black-box load/fault driver and immutable
result bundle for:

- continuous batching and weighted tenant fairness;
- bounded admission and backpressure;
- streaming and slow/disconnected clients;
- collective-safe cancellation;
- cold and warm prefix behavior;
- KV tier pressure and restart;
- rank/process failure; and
- sustained TTFT, ITL, throughput, capacity, and failure-rate evidence.

The same driver targets the retained CPU backend during proof and the
production OpenAI-compatible HTTP endpoint during qualification. CPU results
prove harness semantics and control-plane invariants only; they are never
reported as model throughput.

## Process and trust boundary

The driver runs in a separate process from the server. It uses only:

- `GET /health`;
- `GET /metrics`;
- `POST /v1/chat/completions`;
- the authenticated cancellation route; and
- a test-only fault-control socket enabled by an explicit non-production
  server flag.

The driver cannot read coordinator memory, scheduler queues, prefix indexes,
or output-token fixtures. Server metrics are evidence inputs, not substitutes
for client-observed clocks.

Fault controls are compiled out of the production binary by default. A
qualification binary exposes only named, generation-bound injections and
records every accepted injection. It cannot execute arbitrary commands,
alter model weights, or bypass authentication.

## Immutable run manifest

Every run begins from a canonical manifest:

```text
schema                         glmaxx.sustained-serving-run.v1
run_uuid                       content-derived
source_commit                  exact 40-hex commit
server_binary_sha256           SHA-256
driver_binary_sha256           SHA-256
model_revision                 exact
rank_manifest_sha256[4]        exact
weight_policy_sha256           exact
tokenizer/template hashes      exact
graph_profile_sha256           exact
cache_namespace_sha256         exact
hardware/topology record       exact
container/toolchain record     exact
scenario                       closed enum
arrival_schedule_sha256        exact
request_corpus_sha256          exact
tenant_policy_sha256           exact
fault_schedule_sha256          exact
duration/seed                  exact
resource ceilings             exact
```

The UUID hashes every field except itself. Wall-clock time belongs in an
external provenance envelope and cannot change manifest bytes.

The driver validates health identity before the first arrival and again
after the last completion. A health identity change makes the run
inconclusive unless restart is the named scenario.

## Deterministic request corpus

The corpus stores request descriptors, not model answers:

- tenant ID and request ordinal;
- exact tokenized prompt hash and token count;
- maximum output tokens;
- MTP depth;
- sampling tuple and seed;
- stop strings;
- prefix cohort and cold/warm intent;
- slow-reader/disconnect behavior; and
- expected terminal class, not expected generated text.

Prompt bytes and tokenized IDs remain outside Git when derived from a model
bundle or licensed dataset. Git stores only the generator, public-source
pins, hashes, and compact non-sensitive fixtures.

Every stochastic request has a unique explicit seed derived from the run
seed, tenant, and request ordinal. Replays use the identical request seed;
arrival jitter never changes model RNG.

## Arrival models

The harness implements both:

1. **open loop** — absolute monotonic arrival deadlines are generated before
   the run and never delayed by prior completions; and
2. **closed loop** — a fixed number of virtual users submits the next request
   after its prior terminal event.

Open-loop overload is the primary capacity test. The driver records scheduled
arrival, actual socket dispatch, header completion, first token, every token,
terminal event, and connection close using one monotonic clock.

If the driver cannot dispatch by the scheduled deadline, that delay is
client-side load-generator lag and is reported separately. A run with
driver-lag p99 above 1 ms or CPU saturation above the declared ceiling is
invalid for server latency claims.

## Required scenario matrix

### CPU harness proof

Against the deterministic CPU backend:

| Scenario | Minimum shape |
|---|---|
| open-loop continuous batch | 4 tenants, C64, 50,000 requests |
| closed-loop lifecycle | 64 users, 10,000 requests |
| weighted fairness | weights 1:2:4:8, 20,000 requests |
| overload/backpressure | 2× measured sustainable arrival rate, 120 s |
| cancellation | 10% before admission, during prefill, and during decode |
| slow consumers | 25% readers stalled at three deterministic boundaries |
| disconnect | 10% before headers, after headers, and mid-token |
| prefix | matched cold miss then warm reuse for 1,000 cohorts |
| rank fatal | each rank and each step phase, one fault per clean restart |
| event/command saturation | every bounded queue at capacity and capacity+1 |

The CPU proof verifies exact counts and state transitions; its wall times are
diagnostic only.

### SM120 qualification

Against the real checkpoint and four SM120 ranks:

- concurrency `C1, C4, C16, C64`;
- prompt bands `128, 2K, 32K, 256K`, plus the separately admitted 1M case;
- output bands `32, 128, 1K`;
- MTP depths `0, 1, 3, 6`;
- greedy and each accepted probabilistic route;
- cold miss and warm prefix reuse;
- no-pressure, DRAM-pressure, and NVMe-pressure cache postures;
- at least three open-loop arrival rates bracketing saturation;
- one 15-minute warmup, one 60-minute measured steady state, and one
  six-hour soak at the selected production operating point; and
- cancellation, slow-client, disconnect, cache-thrash, rank-fatal,
  storage-fault, and clean-restart schedules.

The matrix may be partitioned into immutable runs, but no omitted cell can be
silently averaged into another posture.

## Prefix posture

Cold and warm runs are distinct:

- cold prompts randomize the first complete 64-token block per request and
  require server-reported `cached_tokens = 0`;
- warm requests use an exact previously sealed prefix and require the
  reported matched page count and namespace;
- the warm request starts only after the publishing request's durable and
  active leases have completed; and
- a DCP posture change cannot invalidate posture-neutral durable bytes but
  may require a measured HBM restore.

The driver retains the exact prefix cohort and server cache deltas for each
request. A throughput row without these fields is invalid.

## Fault schedule and restart

Faults use absolute request/step predicates, never wall-clock guesses:

- cancel request ordinal before admission, during prefill chunk N, or after
  generated token N;
- stop reading after SSE frame N and resume after a fixed monotonic interval;
- close the connection after headers or SSE frame N;
- fail rank R before reservation upload, graph launch, collective K, output
  receipt, or commit upload;
- fail one DRAM/NVMe operation before submission, after submission, or after
  uncertain completion;
- corrupt one unpublished, published, or journal-tail extent in a copied
  test store; and
- terminate and restart the server after a named durable transaction.

Rank or collective failure must fail the complete worker generation and every
affected request. It must never retry one rank, continue a partial
collective, or report a successful cleanup receipt.

Restart scenarios verify catalog replay, orphan invisibility, prefix
identity, quarantine/arena reset, request failure semantics, and new
generation health before admitting new work.

## Bounded driver architecture

The Rust driver owns:

- a fixed connection slab with generation counters;
- bounded submission, parser, event, and result queues;
- one preallocated timestamp vector per admitted request, bounded by maximum
  output tokens;
- fixed-cardinality histogram buckets plus raw per-request records;
- a bounded error-string dictionary; and
- a streaming evidence writer outside Git.

Connection, request, timer, and fault-completion records bind slab index plus
generation. A late event for a closed/reused slot is rejected and counted;
it cannot complete another request.

The evidence writer applies backpressure before the run starts by reserving
the declared maximum bytes. During a measured interval it cannot drop,
sample, or aggregate away per-request timestamps.

## Required measurements

Per request:

- scheduled arrival and dispatch lag;
- queue/admission duration;
- prefix lookup/restore and cached-token count;
- prefill start/end and TTFT;
- timestamp for every emitted token and ITL sequence;
- MTP proposals, accepted draft tokens, rejected work, and useful tokens;
- terminal reason and structured error;
- cancellation request/ack/terminal timing;
- bytes sent/received; and
- cache/graph/context/sampling posture.

Per run:

- offered, accepted, completed, cancelled, rejected, and failed counts;
- request and useful-token throughput;
- TTFT and ITL p50/p95/p99 plus raw distributions;
- queue/admission/prefill/decode/collective/cache-tier time;
- weighted-fair service and starvation windows per tenant;
- graph selection and eager fallback counts;
- HBM/DRAM/NVMe high-water marks and transfer bytes;
- prefix hit/miss/upgrade counts;
- rank/collective failure and restart counts;
- driver lag and resource use;
- server RSS, pinned memory, HBM, file descriptors, queue depths, and error
  cardinality at fixed intervals; and
- power, clocks, temperature, and throttling for SM120 runs.

Useful-token throughput counts only committed response tokens. Rejected MTP
work is reported separately and never credited as throughput.

## Acceptance rules

A CPU proof passes only when:

- submitted requests equal terminal successes plus structured rejections,
  cancellations, and fatal failures;
- every accepted request has exactly one terminal event;
- no tenant/request ownership crosses;
- queue and connection high-water marks stay within configured capacities;
- all cancellation/fault schedules hit their intended phase;
- rank faults fail the full generation;
- slow/disconnected clients do not stall unrelated completions;
- weighted tenants receive service proportional to configured weights within
  the declared finite-window tolerance;
- memory, descriptor, and queue usage reaches a plateau; and
- two identical runs produce identical request/fault classifications and
  output tokens for deterministic routes.

Production performance passes only against thresholds frozen after a clean
baseline run and an independent review. The threshold artifact may reject a
candidate; it cannot edit scenario membership, precision, context, prefix
posture, batching, or cache posture.

Any driver lag violation, health identity drift, missing raw request record,
counter mismatch, unplanned server restart, rank-local recovery, unbounded
resource growth, or changed comparison posture makes the run invalid or
failed as specified. It cannot be discarded as an outlier.

## Result bundle

Raw bundles remain outside Git and contain:

```text
manifest.json
provenance.json
requests.ndjson
server-metrics.ndjson
hardware.ndjson
faults.ndjson
server.log
driver.log
summary.json
sha256sums.txt
```

`sha256sums.txt` covers every file and is itself named by the compact Git
record. The Git record contains input hashes, commands, summary statistics,
raw-bundle root hash, explicit PASS/FAIL/INCONCLUSIVE, and limitations.

## Gate order and non-claims

Implementation order is:

1. adversarial design verdict;
2. deterministic schedule/corpus CPU references;
3. bounded driver and fake-server parser proof;
4. retained CPU backend sustained/fault proof;
5. real-server no-fault smoke;
6. short fault matrix;
7. one-hour measured matrix; and
8. six-hour production soak.

This design does not implement the driver, accept current HTTP transport or
observability, authorize cn4, prove model correctness, establish throughput,
or set performance thresholds.
