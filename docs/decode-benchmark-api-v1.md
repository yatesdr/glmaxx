# Local Inference Lab decode-benchmark API contract v1

Date: 2026-08-04

Status: design candidate; adversarial acceptance required before integrating
the existing API branch

## Purpose and diagnosed gap

The pinned Local Inference Lab `llm_decode_bench.py` at commit
`86cf05c2f42f4d21b909b6e684424ca1aab89fd5` and SHA-256
`fa227030012a8b55545af6b6a50fa4adcbdff8d003bb16469dc8e2de024ed0c0`
sends `stream_options` and `ignore_eos`, but it sends no `mtp_depth` field.

The unreviewed branch `feature/decode-bench-api-v1` adds the required model
discovery and streaming controls, but its request parser maps an absent
`mtp_depth` to zero. Running the pinned unmodified driver against a nominal
MTP3 server would therefore execute MTP0 while the surrounding run was labeled
MTP3. That branch must not be integrated unchanged.

This contract makes speculative depth immutable serving-generation state and
binds it to discovery, health, every request, every completion, metrics, and
benchmark evidence. It preserves one OpenAI-compatible endpoint and does not
modify the pinned benchmark script.

## Immutable serving generation

Before health publication, all four ranks and the coordinator accept one
`ServingGeneration.v1` record containing exactly:

```text
model ID and model revision
generation serial
resident-weight generation SHA-256
target-program and MTP-program SHA-256 values
graph-profile and backend-policy SHA-256 values
cache-policy and capacity-ledger SHA-256 values
fixed configured MTP depth, 0 through 6
maximum model positions, exactly 1,048,576
```

Every digest field is exactly 32 bytes and nonzero. The resident-weight digest
is the common ordered digest of all four rank-arena manifests, not a rank-local
pointer or local payload hash. A depth-zero generation uses
`cd198f0992622179eed0a678ed9c645a36268ebd7b276f2c46678a2f48812ec5`,
the SHA-256 of the 25-byte ASCII domain `glmaxx-no-mtp-program-v1\0`; it does
not use zero bytes or omit the field.

The generation digest is not one of its own inputs. Its preimage is the ASCII
domain `glmaxx-serving-generation-v1\0`, followed by the nonzero generation
serial as little-endian `u64`, fixed depth as `u8`, maximum positions as
little-endian `u32`, model ID and revision as separate little-endian `u16`
byte lengths plus their nonempty UTF-8 bytes, and the seven digest fields above
in their listed order. Length overflow, an unknown field, zero serial, zero
digest, invalid UTF-8, or trailing input fails validation. The record stores
the resulting SHA-256 separately as `generation_sha256`; all participants
recompute it from the canonical fields before voting to publish. The serial is
monotonic and never reused during a coordinator lifetime.

The fixed depth is included in the generation digest and is identical across
the coordinator and four ranks. The matched benchmark uses two different
immutable generations: one depth 0 and one depth 3. A generation cannot change
depth, program, graph, cache posture, weights, or capacity while healthy or
while a request is active. Compatible hot reload constructs and atomically
publishes a new generation under its separate reviewed transaction.

## Request depth semantics

`mtp_depth` remains an optional GLMAXX request extension for explicit clients:

- absent: use the healthy generation's fixed depth;
- present and equal to the fixed depth: accept;
- present outside 0 through 6: reject first with HTTP 400, code
  `INVALID_MTP_DEPTH`, and parameter `mtp_depth`; and
- present, in range, and different: reject before tokenization/admission with HTTP 409,
  code `MTP_GENERATION_MISMATCH`, and parameter `mtp_depth`; and
  no request state may be allocated.

The validated request stores both the requested optional value and the
generation-derived effective depth. Only the effective value reaches prefix
capability, KV admission, graph selection, scheduling, rank input, metrics,
and completion receipts. No request-local fallback can change it.

The exact pinned benchmark payload omits `mtp_depth`; tests must prove it runs
effective depth 0 against the MTP0 generation and effective depth 3 against
the MTP3 generation. Adding a proxy, editing the script, or relabeling a
request-default depth is not an accepted workaround.

## Required OpenAI-compatible surface

The authenticated server exposes:

- `GET /v1/models` with exactly one model card for `glm-5.2`, object `model`,
  owner `glmaxx`, maximum model length 1,048,576, model revision, fixed MTP
  depth, and generation SHA-256;
- `POST /v1/chat/completions` with the existing bounded schema plus exact
  `stream_options.include_usage`,
  `stream_options.continuous_usage_stats`, `ignore_eos`, and optional
  `mtp_depth` handling;
- `GET /health` reporting healthy only when all four production rank workers,
  weights, programs, graphs, cache ledger, fixed MTP depth, and serving
  generation are mutually bound; and
- `GET /metrics` using GLMAXX names only, including one generation-info record,
  fixed/effective MTP depth, useful committed tokens, physical target steps,
  proposed/accepted/rejected draft tokens, bonus tokens, and acceptance rate.

The minimum Prometheus set is
`glmaxx_serving_generation_info{generation_sha256,model_revision,mtp_depth} 1`,
`glmaxx_requests_active{generation_sha256,mtp_depth}` as a gauge, and monotonic
per-generation counters `glmaxx_useful_tokens_total`,
`glmaxx_target_steps_total`, `glmaxx_mtp_proposed_tokens_total`,
`glmaxx_mtp_accepted_tokens_total`, `glmaxx_mtp_rejected_tokens_total`, and
`glmaxx_mtp_bonus_tokens_total`, each with the same generation and depth
labels. `glmaxx_mtp_acceptance_ratio` is a gauge equal to accepted divided by
proposed for the current generation, or zero when proposed is zero. Counters
start at zero only after the new generation is atomically published and are
never carried across generation identities.

All four endpoints use the configured bearer-key/tenant authority. An
operator may expose a separate unauthenticated deployment probe outside this
server, but it cannot change the core `/health` contract. The benchmark path
uses the same bearer key for model discovery, metrics probes, and completions.
SGLang/vLLM probe endpoints and metric prefixes remain absent so the driver
selects its generic OpenAI-compatible route honestly. Because the generic
route has no engine-specific KV discovery, matched runs pass the retained
capacity-ledger value explicitly through the driver's `--kv-budget` option.

## Streaming and usage semantics

The request accepts `stream_options` only when `stream=true`. Unknown option
keys fail. `continuous_usage_stats=true` requires `include_usage=true`.

Every committed target-visible token produces one progress event even when it
decodes to an empty text fragment. With both `include_usage=true` and
`continuous_usage_stats=true`, every such event contains cumulative
`prompt_tokens`, `completion_tokens`, and checked `total_tokens`. With
`include_usage=true` and continuous usage false, only the terminal event has
usage. With `include_usage=false`, no event has usage. Rejected draft tokens
and physical target verification steps never increment completion usage. The
terminal event repeats the terminal cumulative totals when usage is enabled,
without adding a token, and precedes exactly one `data: [DONE]` record.
Backpressure/cancellation behavior remains bounded and cannot drop a
usage-bearing committed-token event while accepting the request as complete.

`ignore_eos=true` counts EOS as one committed token but does not terminate on
it; custom stop strings still terminate. The default preserves ordinary EOS
termination. Both policies retain incremental UTF-8 and stop-prefix state
across arbitrary token boundaries.

Every streaming JSON event and buffered response includes the stable
`system_fingerprint` string `glmaxx-` followed by the 64 lowercase hexadecimal
digits of the serving-generation SHA-256. The backend
request/terminal evidence separately retains the complete digest, effective
MTP depth, program/graph/cache identities, and useful/physical/speculative
counters. A model name alone cannot identify a run.

## Generation-safe concurrency

The backend snapshots one healthy `Arc<ServingGeneration>` before tokenization
and retains it through terminal cleanup. Commands and active requests carry
that generation digest and effective depth. Continuous batches may combine
only requests with identical generation, effective depth, sampling collective,
graph route, and cache capability.

A generation transition first stops new admission, drains or collectively
cancels the old generation, commits the new rank set atomically, and only then
publishes new health/model discovery. Old completions keep the old fingerprint;
new completions keep the new one. Mixed-generation batches, rank-local depth,
and a health/model race are fatal consistency failures.

## CPU/API proof

After adversarial design acceptance, the corrected implementation must add:

1. the exact pinned scout and sustained-decode payloads as fixtures, proving
   omitted depth selects fixed MTP0 and fixed MTP3 correctly;
2. explicit equal/mismatched/out-of-range depth tests before tokenization;
3. authenticated model discovery with exact model length, depth, and
   generation identity plus unhealthy/transition rejection;
4. stream-option unknown/missing dependency cases and cumulative usage for
   visible text, empty text, EOS, stop strings, MTP accepted drafts, rejected
   drafts, bonus tokens, and checked overflow;
5. buffered and SSE `system_fingerprint` stability and exact terminal receipt
   binding;
6. concurrent C1/C2/C4/C8 requests with no mixed generation/depth and bounded
   slow-consumer cancellation;
7. a transition race covering model discovery, health, queued requests,
   inflight requests, and new admission; and
8. a driver compatibility test using the exact unmodified pinned script in a
   loopback CPU/mock environment, retaining its raw JSON and proving the
   reported aggregate source is continuous usage.

The existing branch commits `1cb3b61`, `0a611b3`, and `6b4a3cf` are useful
implementation material but predate this design and omit fixed generation
depth. They require a corrected implementation candidate and separate review;
their tests or green gate cannot accept this contract.

## Claim boundary

Acceptance authorizes only a corrected Rust API/CPU mock implementation. It
does not accept a production backend, target/MTP execution, useful-token
counter correctness on CUDA, checkpoint output, KLD, KV capacity, cold start,
concurrency throughput, or any benchmark number. Those claims require the
healthy real generation and the matched retained benchmark matrix.
