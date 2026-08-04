# Local Inference Lab decode benchmark pin

Date: 2026-08-04

Status: source and invocation contract pinned; no GLMAXX serving run yet

## Source identity

The benchmark source already present on cn4 is:

| Field | Identity |
|---|---|
| repository | `https://github.com/local-inference-lab/llm-inference-bench.git` |
| cn4 checkout | `/home/derek/llm-inference-bench` |
| commit | `86cf05c2f42f4d21b909b6e684424ca1aab89fd5` |
| script | `llm_decode_bench.py` |
| script version | `0.4.29` |
| bytes | `846324` |
| SHA-256 | `fa227030012a8b55545af6b6a50fa4adcbdff8d003bb16469dc8e2de024ed0c0` |

The working script matches the Git blob at that commit. The checkout has
only untracked `.venv/` and `results/` paths; no tracked change affects the
script. These two prior-run copies are byte-identical to the Git blob:

```text
/home/derek/dsv4/evidence/first-boot-lucifer-native-sys-20260731/repro/llm_decode_bench.py
/home/derek/glm52-tr3-336-r15-qualification/bench-tools/llm_decode_bench.py
```

A CPU-only `--help` invocation through the checkout's pinned virtual
environment, with the dead-proxy posture below, returned success and exposed
all required flags. The script SHA-256 was identical before and after.

Do not follow the script's update path during a GLMAXX run. Copy this exact
blob into each immutable GLMAXX evidence directory and hash it before and
after execution.

## Required GLMAXX sustained-decode invocation

Bind `<host>`, `<port>`, `<model>`, `<kv_tokens>`, and `<output>` to the exact
healthy GLMAXX runtime and evidence directory. The fixed workload arguments
are:

```text
export HTTPS_PROXY=http://127.0.0.1:9
export HTTP_PROXY=http://127.0.0.1:9
export ALL_PROXY=http://127.0.0.1:9
export NO_PROXY=127.0.0.1,localhost,<host>
python3 llm_decode_bench.py
  --host <host>
  --port <port>
  --model <model>
  --concurrency 1,2,4,8
  --contexts 0,16k,32k,64k,128k
  --duration 10
  --decode-warmup-seconds 30
  --skip-prefill
  --kv-budget <kv_tokens>
  --temperature 0
  --display-mode plain
  --output <output>
```

The deliberately dead proxy prevents the script's unconditional five-second
GitHub update check from downloading or replacing its own bytes while
`NO_PROXY` preserves access to the bound GLMAXX endpoint. Mount the copied
script read-only as a second fail-closed control. The 30-second argument is
the hidden C1 decode warmup; each cell additionally performs its built-in
readiness warmup, whose observed duration and timeout state remain in JSON.

Run MTP0 and MTP3 as distinct immutable engine generations. Repeat the
C1/context-zero cell at least five times with a fresh output file each time.
Record the exact endpoint/model strings, effective MTP depth, prefix-cache
posture, resident-weight generation, graph/profile identity, KV page count,
and engine evidence digest alongside every invocation.

The decode driver issues a scout request before sustained concurrency, so
these cells are warm-prefix decode measurements. Cold prefill remains a
separate unique-prompt run and must not be inferred from `--skip-prefill`.
The default estimated token targeting remains part of this pin unless a
separately reviewed exact-token endpoint or fixed token-vector ingress is
used consistently for every matched runtime.

## Result interpretation

For sustained cells, `aggregate_tps` prefers continuous OpenAI stream usage
tokens divided by their measured interval, then observed stream chunks, then
server metrics. Preserve `aggregate_source`; do not compare cells backed by
different token-count semantics without a matched control.

Retain the full JSON, including:

- requested and effective concurrency, queue samples, underfill, warmup
  timeout, and capacity-limited state;
- measurement and wall durations, client/server token totals, errors, and
  completed requests;
- request-level TTFT, second-token, ITL, latency, and per-user throughput;
- speculative acceptance metrics for MTP3; and
- sampled utilization, memory, power, temperature, and PCIe traffic.

GLMAXX useful-token throughput is the driver token rate only after the engine
receipt proves committed target-visible tokens and excludes rejected draft
tokens. Physical MTP steps, accepted draft tokens, bonus tokens, and target
verification work must be retained separately; the benchmark JSON alone
cannot prove those counters.

This pin establishes no throughput, latency, quality, capacity, cold-start,
or serving result.
