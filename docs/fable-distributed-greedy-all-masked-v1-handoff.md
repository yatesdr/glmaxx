# Fable handoff: distributed greedy all-masked rejection v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`7867ed2e3839d74aad83f8b504bf5000247838b6`

Required result path:
`fable-distributed-greedy-all-masked-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`distributed-greedy-all-masked-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-reference/src/sampling.rs` | `3205f2b11d5253c51176434337be8a3e4738a1cc84a4f2d16975248d816edfb5` |
| `docs/distributed-sampling-abi-v1.md` | `d717508e4d90f6ef378d486c0bd3e93e7dad522e6529b8504ccb687a0280fdce` |
| `docs/fable-distributed-sampling-abi-v1-handoff.md` | `4d4bb431eabc9c48435d8ed19cfdd2532a780224d0fb79c7e69ba2bbae7058c6` |
| `docs/quality-acceptance-v1.md` | `3f87cd128b633d6812dce31fb6f3bfbd700debae587a32350e0cb46e24a6e1e9` |
| `docs/distributed-greedy-all-masked-proof-v1.md` | `c6fd7efa937a1cee875621a190b8886153eddc9d323ee65bf7d406a5a8df9b46` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-distributed-greedy-all-masked-v1-handoff.md
cargo test --offline -p glm-reference \
  sampling::tests::greedy_rejects_an_all_masked_row_but_accepts_masked_ranks \
  -- --exact
cargo test --offline -p glm-reference sampling::tests
cargo clippy --offline -p glm-reference --all-targets -- -D warnings
```

## Review boundary

This review covers only the existing CPU distributed-greedy reference's
handling of negative-infinity masks after the four-rank global merge. It does
not accept or promote the broader pending distributed-sampling ABI,
stochastic serving, counter scheduling, MTP acceptance/residual/bonus routes,
CUDA sampling, checkpoint execution, quality evidence, or performance.

## Required adversarial questions

1. Does the vocabulary contract use `-∞` for padded and forbidden IDs while
   requiring a globally all-masked row to fail the entire step?
2. Did `validate_logit_shards` intentionally permit negative infinity while
   rejecting NaN and positive infinity?
3. Did the former greedy route return the merged token without checking that
   the global winning logit was finite?
4. With every input logit `-∞`, did deterministic ordering therefore return
   token 0 instead of an error?
5. Is checking finiteness only after the global merge necessary to preserve a
   legal rank whose local partition is fully masked?
6. Does the correction return exactly `SamplingError::Logit` only when the
   global winner is nonfinite?
7. Does the first regression assertion prove three fully masked ranks remain
   legal when rank 2 owns finite token 17?
8. Does the second assertion distinguish the prior behavior by requiring the
   same row, once fully masked, to fail rather than return token 0?
9. Do the unchanged greedy tie tests still prove deterministic lower-token
   selection for equal finite winners?
10. Do top-k and mass routes already fail all-masked input through invalid
    weight/global-maximum or mass checks, making this correction consistent
    rather than a new sampling policy?
11. Does this correction avoid implementing or prejudicing the pending
    stochastic/MTP wire and transaction ABI?
12. Are the 266-test, 60-handoff, CPU-only boundary, and all exclusions
    accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- globally all-masked greedy rows now fail closed;
- rank-local masked partitions remain legal;
- the regression distinguishes the former token-0 behavior;
- finite cross-rank tie behavior remains unchanged;
- no pending stochastic or MTP sampling gate is opened; and
- the CPU proof and all exclusions are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, rank-local overrejection,
surviving all-masked token selection, nondistinguishing test, changed tie
order, or overstated production claim.

The token accepts only this CPU greedy fail-closed correction. It does not
open cn4, authorize CUDA work, or accept distributed sampling for production.
