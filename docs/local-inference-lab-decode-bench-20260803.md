# Local Inference Lab decode-bench pin

Date: 2026-08-03

Status: benchmark input pinned; no engine benchmark run

## Rust API compatibility candidate

Commit `1cb3b614f0d8d16ef1de0b1f603d2adaa6bcc1f3` closes the request and
streaming incompatibilities between the pinned script and GLMAXX without
claiming a model run. Follow-up commit
`0a611b3bba8bb59f2d1966a96db0a761a3918d53` also implements the driver's
mandatory authenticated `GET /v1/models` discovery call with exactly one
`glm-5.2` model and `max_model_len=1048576`. Its SGLang and vLLM probes remain
404 and GLMAXX metrics use neither vendor prefix, so the driver selects its
honest generic OpenAI-compatible route. The fail-closed Rust request schema
accepts exactly the script's `stream_options.include_usage`,
`stream_options.continuous_usage_stats`, and `ignore_eos` controls. Continuous
usage emits cumulative prompt/completion/total counts for every generated
token, including tokens that decode to no text, and repeats the final total
without double-counting. Invalid non-streamed options, unknown option keys, or
continuous usage without `include_usage` are rejected.

`ignore_eos=true` is implemented through both the incremental tokenizer and
the serving coordinator. EOS remains a counted token but cannot terminate the
request before its configured output length; custom stop strings remain
active. The coordinator retains this policy through asynchronous prefix
admission and removes it with the request's terminal page/prefix transaction.

The exact sustained-decode payload and discovery flow from script version
0.4.29 are Rust test vectors. Focused tests prove cumulative completion counts
`[1,2]`, two SSE usage observations for a one-token stream (progress plus
final), ordinary EOS handling through the requested length, preservation of
the default stop-on-EOS path, strict bearer authentication, and exact model
context discovery. No HTTP process using model outputs was started, and no
throughput number is claimed by this compatibility result. The exact
candidate passes 427 Rust tests, Clippy, deterministic CPU proofs, and the
review-provenance scan.

The benchmark authority is the public
`https://github.com/local-inference-lab/llm-inference-bench.git` repository at commit
`86cf05c2f42f4d21b909b6e684424ca1aab89fd5` (commit time
`2026-07-11T01:59:19Z`). The script reports version `0.4.29` and
`llm_decode_bench.py` hashes as
`fa227030012a8b55545af6b6a50fa4adcbdff8d003bb16469dc8e2de024ed0c0`.

It was cloned read-only into the isolated temporary path
`/tmp/glmaxx-llm-inference-bench-20260803`. No dependency was installed and no
API endpoint or benchmark was run. The invocation stopped at import because
the local proof host does not have `httpx`; this is not an engine failure.

The matched GLMAXX matrix is fixed by `goal.md`: MTP0 and MTP3, concurrency
`1,2,4,8`, contexts `0,16k,32k,64k,128k`, a separate pinned 30-second warmup,
10-second cells, `--skip-prefill`, the exact reported KV budget, and explicit
prefix-cache posture. C1/context-zero is repeated at least five times. Cold
prefill uses unique prompts and is reported separately.

Every run must copy the pinned script into its external evidence directory or
bind it read-only by content hash, disable auto-update/network mutation, record
the complete command and Python dependency lock, and preserve the produced
JSON without post-processing in place. Derived tables hash their raw JSON
inputs and report useful tokens, not accepted draft tokens or physical steps.

This pin and Rust proof do not claim successful model execution or any
throughput result.
