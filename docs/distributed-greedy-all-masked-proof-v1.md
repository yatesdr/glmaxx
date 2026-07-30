# Distributed greedy all-masked rejection CPU proof v1

Date: 2026-07-29

Implementation commit:
`b235dc854f1b43da232c056ce417c243c8a1897f`

Status: CPU reference correction passed; independent review pending

GPU claim: none

## Defect and correction

The distributed sampling contract uses negative infinity for padded and
forbidden vocabulary IDs. `validate_logit_shards` therefore correctly permits
`-∞` while rejecting NaN and positive infinity.

`distributed_greedy` previously selected one local candidate per rank,
performed the deterministic global merge, and returned the resulting token
without checking whether the global winning logit was finite. If every token
was masked, all four local winners and the global winner were `-∞`; the
function returned the lowest token ID as if it were a valid model decision.
That contradicted the contract's engine-fatal all-masked rule and differed
from the existing top-k and mass routes, which already reject a nonfinite
global maximum or invalid mass.

The correction retains `-∞` as a legal rank-local mask but requires the final
global greedy winner to be finite. A globally all-masked row now returns
`SamplingError::Logit`. A rank whose entire local partition is masked remains
legal when another rank owns a finite global winner.

## Distinguishing CPU proof

`greedy_rejects_an_all_masked_row_but_accepts_masked_ranks` starts with all 32
synthetic vocabulary positions masked and then makes only global token 17
finite. Three ranks are fully masked, yet the four-rank merge must select
token 17. The test then masks token 17 and requires exact
`Err(SamplingError::Logit)`.

The first assertion prevents an overcorrection that would reject any
rank-local empty/masked partition. The second assertion distinguishes the
former implementation, which returned token 0 for the same all-masked input.
The existing cross-rank tie test remains green and still proves lower-token
selection for equal winning logits.

## Gate result and exclusions

The focused sampling suite passed all seven sampling tests, and
`glm-reference` passed Clippy with warnings denied. The full local gate passed
266 Rust tests with zero failures, workspace formatting, workspace Clippy,
CUDA FFI type checks, every deterministic CPU proof command, and all 60
then-present review-handoff provenance proofs.

Commands:

```text
cargo test -p glm-reference \
  sampling::tests::greedy_rejects_an_all_masked_row_but_accepts_masked_ranks \
  -- --exact
cargo test -p glm-reference sampling::tests
cargo clippy -p glm-reference --all-targets -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-reference/src/sampling.rs
3205f2b11d5253c51176434337be8a3e4738a1cc84a4f2d16975248d816edfb5

docs/distributed-sampling-abi-v1.md
d717508e4d90f6ef378d486c0bd3e93e7dad522e6529b8504ccb687a0280fdce

docs/fable-distributed-sampling-abi-v1-handoff.md
4d4bb431eabc9c48435d8ed19cfdd2532a780224d0fb79c7e69ba2bbae7058c6

docs/quality-acceptance-v1.md
3f87cd128b633d6812dce31fb6f3bfbd700debae587a32350e0cb46e24a6e1e9

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, model, or HTTP
execution was used.

This correction does not accept or promote the pending distributed sampling
ABI, stochastic API parameters, counter schedule, MTP proposal/acceptance,
residual or bonus sampling, padding-mask kernel, collective route, or
production sampling performance. It proves only that the existing CPU greedy
reference cannot turn a globally all-masked vocabulary row into a token.
