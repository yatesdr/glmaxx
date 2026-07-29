# Serving observability v1

Date: 2026-07-29

Status: CPU-tested host observability candidate; device timing and cache-tier
telemetry remain unqualified

GPU evidence: none

## Purpose

The serving runtime now records enough bounded host evidence to distinguish
queueing, prefix admission, graph selection, scheduler padding, committed
output, MTP acceptance, collectives, and client backpressure. These metrics
describe the CPU/control-plane candidate only. They are not substitutes for
CUDA events, CUPTI counters, NVML samples, or a retained per-step benchmark
record.

`ServingCoordinator::tick_observed` returns one
`ServingStepObservation` for every successfully committed scheduler step. It
contains:

- step mode and ID;
- graph ID;
- real and bucketed sequence/query-row counts;
- scheduled prompt tokens and MTP depth;
- collective count and schedule hash;
- exclusive TP, DCP packed-CKV, query, candidate, partial-state, and sampling
  bytes with their route IDs; and
- worker round-trip, coordinator overhead, and total host time.

The total is measured around scheduler selection, compilation, rank
consensus, scheduler commit, and event creation. Worker round-trip is measured
around bounded rank submission and four-rank consensus. Coordinator overhead
is their saturating difference. A production rank executor must add
CUDA-event kernel, TP, DCP, transfer, and graph-launch timing; host
round-trip cannot be relabeled as kernel time.

## Request metrics

The coordinator API backend records:

- chat rendering/tokenization time;
- bounded-command queue time;
- prefix-resolution time;
- target and draft prompt tokens restored or computed;
- admission-to-first-token and total TTFT;
- inter-token latency;
- request lifetime;
- useful committed output and accepted draft tokens;
- selected MTP depth, verify steps by depth, and accepted drafts by depth and
  draft ordinal;
- stop/length termination, cancellation, failure, and slow receivers.

Queue time begins immediately before bounded command enqueue and ends when
the runtime thread begins admission. Prefix time begins at runtime admission
and ends when the coordinator emits `Admitted`. TTFT begins before chat
rendering/tokenization so host preprocessing is not hidden.

The accepted-draft ordinal is carried in `RequestEvent::Token` as
`draft_ordinal=0..5`. The backend rejects an event whose speculative flag,
ordinal, or selected MTP depth disagree. Later correction/bonus tokens have no
draft ordinal and are not counted as accepted drafts.

## Bounded implementation

Latency histograms use sixteen fixed microsecond bounds from 100 us through
10 s plus infinity. Counters, sums, and maxima saturate at `u64::MAX`; they
never wrap. Recording performs no allocation. Graph selection uses three
fixed arrays for prefill, decode, and verify graph IDs `0..4095`; larger IDs
continue executing but increment `glmaxx_graph_selection_overflow_total`.
Rendering allocates only when `/metrics` is requested.

The histogram buckets are cumulative in Prometheus text exposition. Metrics
include:

```text
glmaxx_tokenization_time_us
glmaxx_queue_time_us
glmaxx_prefix_resolution_time_us
glmaxx_admission_to_first_token_us
glmaxx_ttft_us
glmaxx_itl_us
glmaxx_request_time_us
glmaxx_step_worker_round_trip_us_{prefill,decode,verify}
glmaxx_step_coordinator_overhead_us_{prefill,decode,verify}
glmaxx_step_total_time_us_{prefill,decode,verify}
glmaxx_graph_selections_total{graph_id,mode}
glmaxx_collective_{tp,dcp_ckv,dcp_query,dcp_candidate,dcp_partial,sampling}_bytes_total
glmaxx_scheduler_{real,bucket}_{sequence,query}_rows_total
glmaxx_{prefix_cached,prompt_computed,draft_prompt_restored,draft_prompt_computed}_tokens_total
glmaxx_output_tokens_total
glmaxx_accepted_draft_tokens_total
glmaxx_admitted_requests_total{mtp_depth}
glmaxx_verify_steps_total{mtp_depth}
glmaxx_accepted_draft_tokens_by_depth_total{mtp_depth}
glmaxx_accepted_draft_tokens_by_ordinal_total{draft_ordinal}
```

## CPU evidence

Unit tests prove:

- exact real/bucketed rows, routes, hashes, and exclusive collective bytes for
  prefill and decode observations;
- worker plus coordinator time reconstructs total step time;
- cumulative histogram boundaries;
- saturating counters;
- fixed graph-ID overflow accounting;
- exact request, token, prompt, graph, collective, TTFT, ITL, and step counts
  through the bounded API backend; and
- stop termination and slow-receiver isolation remain visible.

The complete local gate is `scripts/local-checks.sh`.

## Remaining observability gates

- per-tier restore source, bytes, wait, eviction, and write amplification;
- HBM allocation and page-table deltas after the page-transaction review;
- CUDA kernel, graph launch, TP, DCP query/candidate/partial-LSE, and transfer
  times;
- device clocks, power, temperature, throttling, and hardware counters;
- retained per-request and per-step evidence records with prompt/policy hashes;
- sliding-window useful tokens/s, fairness, and p50/p95/p99 report generation;
- sustained concurrency and fault-run result bundles.

Until those exist, `/metrics` is service telemetry rather than performance or
quality qualification.
