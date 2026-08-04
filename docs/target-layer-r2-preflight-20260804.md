# Target-layer r2 review preflight

Date: 2026-08-04

Status: exact design candidate and handoff are review-ready; token absent

## Provenance

The exact candidate was inspected in a detached worktree at
`d4817ff9ff7eec09c74e98a99db5c27690286013`. All 21 candidate inputs in
`docs/fable-target-layer-execution-v1-r2-handoff.md` matched their pinned
SHA-256 values. The two operator-inbox prerequisites also matched:

```text
8738a8c2c4801a9d657a292e4702f500b947fbb1015ad9a62044e54731ae9469  docs/reviews/fable-target-layer-execution-v1.md
901481e5b1d6b26283a7c7e8eb1a1f7af1968df2b2e5d2ddda1c54d0075aa61c  docs/reviews/fable-distributed-sampling-abi-v1.md
```

The complete candidate `scripts/local-checks.sh` exited zero: 413 Rust tests,
formatting, Clippy with warnings denied, deterministic CPU proofs, and review
provenance all passed. The review suite verified 128 handoffs and 36 of 110
configured review results, all 36 present results accepted and none withheld.
The external tokenizer proof and CUDA compile were explicitly skipped because
this CPU-only host had neither configured.

## Exact external sources

The required immutable official bytes were fetched independently. Their
SHA-256 values matched the handoff exactly:

```text
185f93ee6d12548e16a847e279dc0c3c90b1524c970b0866b42fb545747d859a  GLM-5.2 config at b4734de
adb8317a21716b01273046e46c807f14f0dbaf035af59b60d52bd6bc3007cf72  modeling_glm_moe_dsa.py at 5204b4f
5a81164be746307431ad998f789b6b0bca20eb4c14a726552eb3730268413997  configuration_glm_moe_dsa.py at 5204b4f
```

The config independently yielded vocabulary 154,880, RMSNorm epsilon `1e-5`,
default RoPE with theta 8,000,000, FP32 router arithmetic, and Transformers
5.12.0. The modeling source independently confirmed Q-A/KV-A RMSNorm default
epsilon `1e-6`, indexer-K LayerNorm epsilon `1e-6`, FP32 index scoring/router
math, and deinterleaved-half RoPE output after interleaved-pair input.

## Independent arithmetic

The pinned operation manifest contains 75 sparse layers and 21 indexer groups:
3 dense full-indexer layers, 18 sparse full-indexer layers, and 57 sparse
shared-indexer layers. Therefore the exact binding count is:

```text
embedding + final + dense + full sparse + shared sparse
1 + 2 + 3*17 + 18*531 + 57*526 = 39,594 records
39,594 * 10 = 395,940 serialized binding bytes
```

The row/page/pending table ceilings rederive exactly:

```text
prefill: 3072*48 + 3072*40 + 64*48 = 273,408
C1:         1*48 +    1*40 +  1*48 =     136
verify:   448*48 +  448*40 + 64*48 =  42,496
```

The plan/input and table arithmetic also agrees: `95+32=127` plan-hash input
bytes, `127+32=159` plan-record bytes, a 430-byte StepInput prefix, 17 phase
operations occupying 204 bytes, and 32 lifetime records occupying 384 bytes.
The manifest's full-layer list and group consumers cover every target layer
exactly once.

No new blocker, false handoff premise, or arithmetic contradiction was found
in this preflight. This is not adversarial acceptance and does not open CPU
implementation. The exact token `target-layer-execution-v1-accepted` remains
required, and target-program construction additionally remains fail-closed on
an independently accepted distributed-sampling successor. No CUDA, checkpoint
payload, model execution, quality, capacity, cold-start, or speed claim is
made.
