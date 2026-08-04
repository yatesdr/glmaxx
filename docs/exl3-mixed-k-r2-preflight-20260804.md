# EXL3 mixed-K r2 review preflight and descriptor cross-check

Date: 2026-08-04

Status: review-readiness and read-only metadata evidence; no design token,
parser/kernel implementation, tensor payload read, CUDA launch, checkpoint
admission, layer replay, quality, capacity, or performance result

## Exact design-candidate preflight

The corrective handoff was evaluated at its exact candidate:

```text
23e6e26c172e370b63608a07aa2f781a24faef24
```

All seventeen handoff inputs matched at that detached commit. The complete
`scripts/local-checks.sh` gate passed 413 tests, formatting, Clippy with
warnings denied, deterministic CPU proofs, and 137 review handoffs with
39/119 configured results accepted and none withheld. The tokenizer proof was
skipped because its external bundle was not configured; CUDA compilation was
skipped because the local host has no `nvcc`. Neither skip is mixed-K or GPU
evidence.

The arithmetic independently rederived at preflight was:

```text
target sparse layers                         75
draft sparse layers                           1
target K3 descriptors   75 * 192 * 3 * 4 = 172,800
target K4 descriptors    75 * 64 * 3 * 4 =  57,600
draft K3 descriptors      1 * 256 * 3 * 4 =   3,072
total descriptors                              233,472
K4 delta/rank       393,216 * 75 * 64 * 3 = 5,662,310,400
routed bytes/rank 1,192,964 * 76 * 256 * 3
                    + 5,662,310,400       = 75,293,233,152
```

This confirms handoff reproducibility; it is not adversarial acceptance.
`exl3-mixed-k-source-and-kernel-v1-r2-design-accepted` remains absent.

## Stronger real-source diagnostic

The historical header probe proved aggregate layer/projection/rank counts but
did not join each trellis descriptor back to the tier-map expert ID. A new
read-only diagnostic now performs that exact join for all 233,472 tuples:

```text
scripts/tr3-tier-descriptor-crosscheck.py
SHA-256 15c682a0cb27d70d6e9ea55b373ec5e14c8204452f5cfbc3c3aee0f2f2e0f8da
```

It ran from the clean detached cn4 worktree at:

```text
source commit  e75dc8b69c3d02caa36c01fd0f19e85651704342
worktree       /home/derek/glmaxx/worktrees/integration-e75dc8b-20260804T090400Z
checkpoint     /home/derek/models/GLM-5.2-EXL3-TR3-3.25bpw
index SHA-256  f5dcd976a64ca70808dd4d8bd3ad07e9610c8ca6c30e3a6ed77ddefdac4c1d21
tier SHA-256   a287ffe816de5998fbc35a56a1ec05f69eb71087d5bbdfe631242c6b296b2a3d
Python         3.12.3
start/end UTC  2026-08-04T09:05:33Z / 2026-08-04T09:05:35Z
```

The diagnostic reads only the tier bytes plus each layer shard's eight-byte
prefix and JSON header. It requires every expert `0..255`, each of
gate/up/down, and each rank `0..3` exactly once, derives K from the validated
trellis third dimension, and compares that value with the exact tier entry.

The canonical result is:

```json
{"claim":"read-only diagnostic over tier bytes plus safetensors prefixes/headers; no payload or publisher authentication","counts":{"draft_k3":3072,"draft_k4":0,"target_k3":172800,"target_k4":57600},"expected_tuples":233472,"first_mismatches":[],"layers":76,"mismatch_count":0,"observed_valid_tuples":233472,"schema":"glmaxx.tr3-tier-descriptor-crosscheck.diagnostic.v1","tier_sha256":"a287ffe816de5998fbc35a56a1ec05f69eb71087d5bbdfe631242c6b296b2a3d"}
```

Its canonical no-newline SHA-256 is
`fff10f7ac8675ede3a38b9610f69a4d3ba779c75191c2cc1b62f68275607f50d`;
the newline-terminated `summary.json` hashes to
`36584c924ded6f00996a493ca3fa5b38eed048236ec76c0c5a8ad8fc21db1126`.

The successful sealed evidence is:

```text
/home/derek/glmaxx/evidence/20260804T090600Z-tr3-tier-descriptor-crosscheck-e75dc8b-r2
evidence-sha256.txt SHA-256 ef9fa61623f36d1966a61e6c30575c0bb15e123c41b85440d4ece6b1be6bf19c
verdict COMPLETE_METADATA_ONLY_NO_PAYLOAD_NO_CUDA
```

The artifact manifest was verified after publication. Both compute-process
files were empty. All four GPUs remained at 0% utilization and
2/2/2/10 MiB before and after.

## Failed attempt disposition

The first wrapper run completed the diagnostic successfully but compared the
newline-terminated file hash with the diagnostic's canonical no-newline hash.
It stopped before terminal publication and was sealed as FAILED without
overwriting or reuse:

```text
/home/derek/glmaxx/evidence/20260804T090400Z-tr3-tier-descriptor-crosscheck-e75dc8b-r1
evidence-sha256.txt SHA-256 e9e3424ecff25a52f911b5367348e14c5e771c74a59a619dce85f910da62c2da
verdict FAILED_WRAPPER_SUMMARY_FILE_NEWLINE_HASH_MISMATCH
```

It is excluded from successful evidence.

## Consequence and boundary

The real metadata now corroborates the r2 target/draft partition at exact
per-expert granularity, not only aggregate counts. This still does not parse
duplicate JSON keys, authenticate the publisher manifest or payloads, admit
the checkpoint, implement K4 CPU decoding, construct a mixed-K plan, compile
a K4 specialization, launch CUDA, or prove any model result. The required
order remains: adversarial design token, Rust CPU parser/proof, implementation
review, SM120 K3/K4 controls, mixed target/draft replay, then checkpoint smoke.
