# Local Inference Lab decode-bench pin

Date: 2026-08-03

Status: benchmark input pinned; no engine benchmark run

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

This pin does not claim API compatibility, successful execution, or any
throughput result.
