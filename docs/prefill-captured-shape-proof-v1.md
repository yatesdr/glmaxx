# Captured-shape prefill scheduling CPU proof v1

Date: 2026-07-29

Implementation commit:
`4c421615120de9e73edfdf36b3ebedf60c28a693`

Status: CPU/reference correction passed; independent review pending

GPU claim: none

## Existing design and defect

The accepted engine posture requires chunked prefill to execute only through
captured graph families. `Scheduler::build_prefill_batch` previously filled
one batch up to the configured `maximum_prefill_tokens` and
`maximum_batch_sequences`, then asked for a matching graph. If the
configuration limits were wider than every captured prefill entry, the
scheduler returned `UncapturedShape` even when a smaller captured graph could
make progress. A 65-token cold prompt, a 64-token scheduler limit, and a
32-row captured graph stalled before executing the first 32-token chunk.

The design inputs remain:

- `spec/engine-v0.md`;
- the captured prefill families in `docs/native-engine-plan.md`; and
- the bounded continuous-batching posture in
  `docs/offline-serving-foundation.md`.

This correction does not create or qualify a new graph family.

The audit also found an adjacent profile-ABI limitation. Prefill
`GraphKey.verifier_row_bucket` is required to be zero, while graph keys must
be unique. The current profile therefore cannot represent two prefill chunk
sizes with the same sequence bucket and attention transport; the second
regression uses distinct sequence buckets to exercise two legal entries.
This scheduler correction does not change that ABI. A reviewed prompt-row
bucket extension is required before the intended SM120 prefill graph family
can contain multiple chunk captures for the same concurrency/transport key.

## Corrected selection

For each validated prefill `GraphEntry`, the scheduler now constructs the
largest fair-order prefix that simultaneously fits:

```text
active sequences <= min(config sequence limit, graph active limit)
query rows       <= min(config token limit,
                        graph prompt-token limit,
                        graph query-row limit)
```

It selects the candidate with the lexicographically greatest
`(query_rows, active_rows)` and calls `finalize_batch` exactly once. Because
the rows were constructed against a concrete profile entry, at least that
entry can accept the resulting shape. `finalize_batch` still performs the
ordinary graph lookup and increments the step ID only for the selected
batch.

The request order remains the existing deterministic weighted-fair order.
Ties retain validated profile order, which is canonical graph-ID order.
This is a correctness and progress policy for the CPU reference, not a claim
that maximum query rows is the measured latency-optimal production choice.
The SM120 graph sweep and PCIe route cost table remain later hardware gates.

## CPU proof

Two new regressions prove:

1. A 65-token request with a 64-token scheduler limit and only a 32-row
   prefill capture makes progress as `32, 32, 1`, instead of failing on the
   first uncaptured 64-row attempt.
2. With incompatible profile tradeoffs—one sequence at 64 rows versus four
   sequences at 32 rows—the scheduler evaluates both legal candidates and
   chooses the 32-row/two-request batch when the first request has only one
   token remaining. The selected rows are exactly `1 + 31`, and the chosen
   graph is the four-sequence entry.

Both tests fail on the prior single-shape implementation. The full local
gate passed 236 Rust tests with zero failures, workspace formatting,
workspace Clippy with warnings denied, CUDA FFI type checks, every
deterministic CPU proof command, and all 35 then-present handoff provenance
proofs.

Commands:

```text
cargo fmt --check
cargo test -p glm-scheduler
cargo test -p glm-serving
cargo clippy -p glm-scheduler --all-targets -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-scheduler/src/lib.rs
98259570e137bad517e19e46ab68f604e1aeba35e1535ab82fc179a04fda5a0e

crates/glm-engine/src/graph.rs
c85ca1aa52ba42294fc6a43524e8f70357523977d343e8c2f212787e7754cd22

docs/native-engine-plan.md
33552cd81e3d79b8b484856a99620420f3e2eddfdfa529a23b191353a702ed80

docs/offline-serving-foundation.md
9a722fdcc77522ac361493ca8fc02fea1e4692a28d1c22bd2e96607568cd4ce0

docs/offline-serving-spine.md
27b24d4cbafc8203937d3620e7bcd85d47fcb86cc4d8b89e237025e5d40a62f9

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, CUDA graph, real collective, checkpoint, or
model execution was used. This proof does not authorize cn4 or establish
prefill speed, graph-capture correctness, route optimality, model quality,
or serving performance. It also does not resolve the prefill graph-key
limitation described above.
