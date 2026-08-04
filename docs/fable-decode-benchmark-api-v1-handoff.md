# Fable handoff: decode-benchmark API contract v1

Date: 2026-08-04

Status: adversarial design review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4, launch CUDA, start an HTTP server, or modify an external
runtime. This is a CPU/design review. A loopback mock or an isolated copy of
the pinned benchmark source is allowed; network mutation and its update path
must remain disabled.

Review candidate commit:
`d9515c648739454a58223a6531e230ddd4a5eded`

Required result path:
`docs/reviews/fable-decode-benchmark-api-v1.md`

Requested acceptance token, only if every blocker and major is resolved:
`decode-benchmark-api-v1-design-accepted`

## Required provenance

Review the exact candidate in a detached worktree. Hash every candidate input
at review start and finish. Any mismatch withholds the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `goal.md` | `fe1c6b73163b201f0685c8889d3c824ed92e98f3a3acbaf325753d9eeecef742` |
| `docs/decode-benchmark-api-v1.md` | `6ba0f38c8ce2707cdd5b39790be397191104e3619f929dc59d4654b0ea4b9811` |
| `docs/local-inference-lab-decode-bench-20260803.md` | `b0b43ee158ce665efc196a2b27b59ec572f0989abbd4f7858708e665941f02ea` |
| `docs/local-inference-lab-decode-pin-20260804.md` | `023f7d281857c6ba56e1073c8ef5a2a88d0208a934ce521bcabf8ec07b63e680` |
| `docs/resident-weight-runtime-generation-v1.md` | `ec76be8698ab53480ede07044bdfa73c8ccd9bbf391771bc728569c3023ef8b1` |
| `docs/matched-runtime-control-v1.md` | `446e25396e7eabd2fce85aa848c70318f964b1a9a7cf02a4945acc9917c02bf8` |
| `crates/glm-serving/src/http.rs` | `c4a1700cef43ecfdc3578155670ae31d92d4a1e31bbfc8bdf82782c3bada4887` |
| `crates/glm-serving/src/backend.rs` | `07a4d53de6755ed8180ff90aed82f9efc71b8461070a909a10891c783a8fcf78` |
| `crates/glm-serving/src/lib.rs` | `d5fe73c061e282a2e777d3b6faf0d2a25c00706e641ac2448f8405357afefff8` |
| `crates/glm-serving/src/metrics.rs` | `378b7e441f8e2759ab562d61d2df05591fa40523b0effb1401b66b88ac644499` |
| `crates/glm-tokenizer/src/decode.rs` | `b77943f8f9b3bdf7bdac13d0f7f5b8e36d1a738287a15353e38561a900a0f772` |
| `crates/glm-tokenizer/src/lib.rs` | `aa7a738c58df6618880c8311a8c1fa4b7f9cae46ef2b6988fe51e06ca3358b84` |
| `scripts/local-checks.sh` | `1675ca5bac9bda032ab6db206629bc0052ad6afc5cb6f59a7b6697e4e5c779d0` |

The external benchmark authority is repository
`https://github.com/local-inference-lab/llm-inference-bench.git`, commit
`86cf05c2f42f4d21b909b6e684424ca1aab89fd5`, file
`llm_decode_bench.py`, 846,324 bytes, SHA-256
`fa227030012a8b55545af6b6a50fa4adcbdff8d003bb16469dc8e2de024ed0c0`.
Obtain that exact blob without running it, or use an existing byte-identical
read-only copy. Hash it at review start and finish and attest the external
commit and blob separately. A missing or mismatched blob withholds the token.

Run and record the complete CPU-only gate:

```text
./scripts/local-checks.sh
```

The current source does not implement this design. A green gate is baseline
evidence only and cannot accept the contract or the historical feature branch.

## Required independent work

1. Trace the pinned driver's model discovery, vendor probes, scout request,
   sustained request, authentication headers, manual KV budget, stream parser,
   and aggregate-source selection. Confirm every request field and prove that
   it never sends `mtp_depth`.
2. Inspect historical commits `1cb3b614f0d8d16ef1de0b1f603d2adaa6bcc1f3`,
   `0a611b3bba8bb59f2d1966a96db0a761a3918d53`, and
   `6b4a3cf6f725ac86be79f36d5eaeb6745be09433`. Confirm the absent-depth default
   would run MTP0 and that those bytes cannot be accepted unchanged.
3. Independently serialize `ServingGeneration.v1`, rederive its field count,
   byte order, length boundaries, depth-zero digest
   `cd198f0992622179eed0a678ed9c645a36268ebd7b276f2c46678a2f48812ec5`,
   and generation SHA. Attack self-reference, zero sentinels, rank-local
   identity, truncation, field reordering, trailing bytes, and serial reuse.
4. Build the complete request-depth validation table for absent, equal,
   unequal, and out-of-range values against every configured depth. Confirm
   range validation precedes mismatch and no backend request is allocated on
   failure.
5. Feed an independent SSE consumer with visible, empty-text, EOS, stop,
   accepted-draft, rejected-draft, bonus, terminal, cancellation, and overflow
   sequences. Prove cumulative usage and `[DONE]` behavior match the pinned
   driver's accounting without equating chunks, target steps, or draft tokens
   with useful committed tokens.
6. Model C1/C2/C4/C8 admission and a serving-generation transition across the
   coordinator and four ranks. Search for mixed generation/depth batches,
   health/discovery races, old-fingerprint completions, and rank-local
   fallback.
7. Verify endpoint authentication and generic-engine detection against the
   exact driver. Confirm GLMAXX metrics cannot silently become the driver's
   headline source, while retained GLMAXX counters and generation labels are
   sufficient for independent evidence reconciliation.
8. Specify the smallest corrected Rust CPU/mock implementation and loopback
   test that can execute the unmodified pinned driver without claiming CUDA,
   checkpoint output, quality, capacity, or performance.

## Required decisions

Answer each decision separately with an unqualified `YES` or `NO`:

1. Is the diagnosed silent MTP3-to-MTP0 benchmark failure real in the pinned
   driver plus historical API branch?
2. Is the fixed-depth serving generation deterministic, non-self-referential,
   common across TP4, and sufficiently bound to weights, programs, graphs,
   backend policy, cache policy, and capacity?
3. Are absent, explicit, mismatched, and invalid request depths total,
   unambiguous, and fail-closed before backend admission?
4. Do discovery, health, metrics, completion fingerprints, and terminal
   receipts identify the same immutable generation without a publication race?
5. Are the streaming-option constraints and cumulative useful-token semantics
   exact for both continuous and terminal-only usage?
6. Does the contract preserve EOS, stop-string, empty-decoding, cancellation,
   and slow-consumer correctness without permitting a falsely successful
   incomplete stream?
7. Can concurrent batches and generation transitions avoid mixed depth,
   generation, sampling collective, graph route, and cache capability on all
   ranks?
8. Will the exact pinned driver select the generic OpenAI route, authenticate
   correctly, use the explicit KV budget, and derive sustained headline
   throughput from continuous OpenAI usage?
9. Are the GLMAXX metrics generation-safe and sufficient to reconcile useful
   tokens, physical target steps, drafts, bonus tokens, and acceptance without
   contaminating the driver's portable headline?
10. Is the eight-part CPU/API proof executable and broad enough to precede a
    separate production-backend and benchmark-evidence review?
11. Does the contract preserve the repository gate order and keep every CUDA,
    checkpoint, quality, KV-capacity, reload, latency, and throughput claim
    explicitly unopened?
12. Is the combined contract accepted for a corrected Rust API/CPU mock
    implementation?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, followed
by the independent derivations and every decision. Only if all twelve answers
are `YES`, attest the candidate commit, all fourteen candidate input hashes,
the external commit/blob/byte count, and end with the requested token as the
only bare acceptance line.

Acceptance opens only the fixed-generation API model, strict request parser,
stream/usage accounting, generation-safe CPU coordinator mock, metrics, and
unmodified-driver loopback proof. It does not accept the resulting
implementation, production rank execution, any CUDA launch, checkpoint
output, KLD, KV capacity, cold start, concurrency throughput, or a benchmark
number.
