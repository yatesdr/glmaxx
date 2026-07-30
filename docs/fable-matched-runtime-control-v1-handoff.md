# Fable handoff: matched runtime control and comparison v1

Date: 2026-07-30

Status: adversarial design review requested

Review candidate commit:
`660a0707bb4b0a67f3c3983b4cef1dc18a38b6b1`

Required result path:
`docs/reviews/fable-matched-runtime-control-v1.md`

Requested acceptance token, only if every blocker and major is resolved:
`matched-runtime-control-v1-design-accepted`

GPU, host, process, container, network, model, or storage authorization
conveyed by this handoff: none

cn4 posture: do not connect to cn4, query its current state, start or stop a
process/container, build, test, create a CUDA context, access a checkpoint,
or launch work for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if needed. Hash every candidate input at review start and finish.
Any mismatch must withhold the token as stale.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/benchmark-contract.md` | `cd51d22a8faf2baacfb4682ff5e1dcb5986edc27d8aa3af188105842bb49a507` |
| `docs/sustained-serving-load-fault-v1.md` | `3c80abd792455cbd00fb769702784c97c676bfec6e19ccba97c7c4bbe6e8bc38` |
| `docs/matched-runtime-control-v1.md` | `446e25396e7eabd2fce85aa848c70318f964b1a9a7cf02a4945acc9917c02bf8` |
| `docs/quality-acceptance-v1.md` | `3f87cd128b633d6812dce31fb6f3bfbd700debae587a32350e0cb46e24a6e1e9` |
| `docs/native-engine-plan.md` | `33552cd81e3d79b8b484856a99620420f3e2eddfdfa529a23b191353a702ed80` |
| `docs/production-punchlist.md` | `4756ca97cc41b6df2655d17760eac06a4d99c6a95b701e2fd606c4a7b22fc52d` |
| `docs/results-index.md` | `b3f99ff5e5ee93ffc59cce289c92d007c55d251b15ae9cd59f66e776c5fa857a` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-matched-runtime-control-v1-handoff.md
git diff --check 660a0707bb4b0a67f3c3983b4cef1dc18a38b6b1^ \
  660a0707bb4b0a67f3c3983b4cef1dc18a38b6b1
```

The handoff and queue metadata are added after the candidate and are not
candidate inputs. This is a source-only design review. Do not contact an
endpoint, inspect a live runtime, download a control, or reproduce a proposed
run.

## Review purpose

Determine whether the design makes a target/control comparison honest,
implementable, bounded, and fail closed without making general-purpose
runtime support or full-model physical fit an assumption.

Attack the boundaries between:

1. runtime-specific identity and common comparison-key identity;
2. logical weight equivalence and physical repacking;
3. exact tokenizer/token-stream semantics and text-only APIs;
4. identical offered work and intentionally different internal scheduling;
5. external resource/cache posture and internal policy;
6. MTP performance and target-only quality;
7. unavailable, invalid, product-posture, and matched rows; and
8. a valid design/control pin and separately authorized target execution.

## Review boundary

Acceptance covers only the proposed identities, projections, capability
receipt, connector restrictions, request records, metric boundaries,
comparison classes, paired statistics, invalidation precedence, CPU-proof
plan, and gate order.

Acceptance does not cover an implementation, selected runtime revision,
container, connector, tokenizer audit, model conversion, endpoint, process,
cn4 connection, CPU run, GPU run, kernel, checkpoint, cache tier, capacity,
quality, latency, throughput, reliability, or production-health result.

## Required adversarial questions

1. Do all ten candidate-input hashes match at review start and finish in a
   detached worktree?
2. Is implementation explicitly gated on review while every live endpoint,
   process, container, checkpoint, and GPU action remains separately
   authorized?
3. Is `RuntimeIdentity.v1` acyclic and complete enough to prevent source,
   patch, binary, container, dependency, model, rank, tokenizer, draft,
   topology, configuration, argv, and executable substitution?
4. Does secret omission avoid both plaintext leakage and credential hashes
   while retaining enough safe environment identity to reproduce a run?
5. Does the common projection omit only runtime-specific facts, include every
   performance-relevant physical/semantic fact, and fail on schema growth
   rather than silently ignore a new field?
6. Is byte equality of `ComparisonKey.v1` both necessary and sufficient for
   a matched cell without accidentally requiring identical runtime/kernel
   implementations?
7. Can a control repack weights only after identical tensor membership and a
   complete accepted logical-dequant proof? Can padding, protected slices,
   accumulator/KV precision, or a fallback escape the identity?
8. Does physical impossibility become `UNAVAILABLE` instead of a fabricated
   BF16/FP8/NVFP4 full-model throughput or a comparison on extra unrecorded
   hardware?
9. Can a runtime self-report an unsupported model, context, cache tier, MTP
   depth, precision, or deterministic sampling route and still pass the
   independent capability preflight?
10. Is the closed Rust connector boundary implementable without dynamic
    plugins, shells, process mutation, cache clearing, unbounded parsing,
    automatic retries, or server-defined commands?
11. Can credentials, malformed HTTP/JSON/SSE, duplicate keys, truncation,
    protocol downgrade, or connector-specific parsing create a valid record
    or leak a secret?
12. Do exact token-ID/raw-prompt ingress and complete tokenizer proof prevent
    runtime-specific templates, special tokens, normalization, or
    non-round-tripping text from changing the work?
13. Can text-only or multi-token streaming invent token IDs or ITL
    timestamps? Are throughput-only and correctness/ITL availability
    separated correctly?
14. Do fixed-length throughput cells prevent EOS/text divergence from making
    one runtime perform less work while leaving normal termination behavior
    to separate quality/product rows?
15. Are greedy, probabilistic, and MTP-K rows matched on every distribution-
    relevant input without requiring the internal execution schedule to be
    identical?
16. Do MTP performance rows remain blocked on target-only quality, and can
    proposed/rejected/bonus draft tokens ever inflate useful throughput?
17. Are cold miss, warm HBM/DRAM/NVMe, pressure, tier bytes, and starting
    residency independently observable? Can different internal eviction
    algorithms still be measured under identical external bounds?
18. Do identical arrivals, tenants, limits, context/output bands, fault
    schedules, and driver placement preserve offered work while allowing
    scheduling quality itself to differ?
19. Is the 1,048,576-position definition correct without counting draft
    sidecars as logical context or omitting their physical memory/escrow?
20. Are raw record publication, monotonic clocks, metric denominators,
    visible/useful token counts, phase ledgers, and missing observations
    sufficient to recompute every published aggregate?
21. Does the paired design randomize order, retain failed/unfavorable runs,
    require enough independent blocks, define ratio direction, and implement
    its SplitMix64/bootstrap/quantile/median arithmetic deterministically?
22. Can the 5% win/nonregression gates pass through a reversed latency ratio,
    an unpaired aggregate, cherry-picked cell, missing capacity row, noisy
    interval, or invalid pair?
23. Is invalidation precedence deterministic, and can an unsupported feature
    be confused with a runtime failure or an invalid run be silently retried
    into a pass?
24. Does the CPU proof cover at least two connectors, every mismatch class,
    bounded protocol parsing, cache/MTP/token semantics, evidence recovery,
    statistics, and absence of process/GPU/cn4 action?
25. Does the staged order require reviewed designs, CPU proofs, immutable
    operator-selected pins, completed correctness/quality/capacity gates,
    new execution authorization, and final evidence review before any speed
    claim?
26. Are all no-implementation, no-pin, no-endpoint, no-cn4, no-GPU,
    no-model, no-capacity, no-quality, and no-performance nonclaims exact?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately whether:

1. runtime identity and common comparison identity are acyclic, complete, and
   substitution-resistant;
2. precision membership and logical-dequant equivalence permit honest
   repacking without admitting a different numerical policy;
3. capability and connector boundaries fail closed without mutation, secret
   leakage, or invented observations;
4. prompt, tokenizer, output-token, streaming, sampling, and MTP semantics
   preserve the same model work;
5. resources, cache tiers, workload, and starting state are matched while
   runtime scheduling/eviction implementations remain legitimate subjects of
   comparison;
6. unavailable, product-posture, invalid, inconclusive, PASS, and FAIL
   classes cannot be conflated;
7. raw evidence, metrics, pairs, deterministic statistics, and thresholds
   prevent cherry-picking or reversed claims;
8. full-model fit is measured rather than assumed;
9. CPU implementation and live qualification are ordered behind all required
   reviews and authorizations; and
10. no runtime selection, implementation, endpoint, cn4, GPU, checkpoint,
    model, capacity, quality, or performance evidence is implied.

Only if all twenty-six questions and all ten statements are unqualified
`YES`, end with exactly one bare line containing the requested acceptance
token shown above.

Withhold for stale provenance, cyclic/incomplete identity, an under-specified
comparison projection, precision/token/cache/workload drift, invented timing,
mutable or overpowered connector behavior, a physically impossible numeric
control, cherry-picked or statistically reversed verdicts, authorization
leakage, or any implementation/hardware/performance overstatement.
