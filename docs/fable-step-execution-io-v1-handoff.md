# Fable handoff: step execution I/O v1

Date: 2026-07-29

Status: review request; token withheld by Sol

GPU authorization conveyed by this handoff: none

Review candidate commit:
`a5ef0764a4c36c01553006dd4041fb23233bd559`

## Provenance

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/step-execution-io-v1.md` | `e8681e9278034b25fe6928c059ad58730818ce014fb3e0251549f678aa1621d5` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-engine/src/step.rs` | `4963a58da7c9c6bbed0fb57fb7ef56d90d1e0f09fe54da8cc02c35891f743359` |
| `crates/glm-engine/src/output.rs` | `1a82e9990af4e2892831e950f5cd3db256032ab5e43642faeb08229cbf2f1c2c` |
| `crates/glm-engine/src/worker.rs` | `400c7c22f2c74d3f386fefe2d144da3437f27e6c99ce9f2d4bbf87ffe98fe437` |
| `crates/glm-scheduler/src/lib.rs` | `5651a507ad240f19755d50336f09eb3ca97e32f8be51f90e0fe49ef304350f38` |
| `crates/glm-serving/src/lib.rs` | `3fdaad1d005db1cb5cb887e77609c28062b7c885b5ec69f9d86c7dc09cd9b9b1` |
| `crates/glm-serving/src/http.rs` | `238444c448b8af56a4df5f33ac7e48281d30300f611b511c8f85f0388c7c9828` |
| `crates/glm-reference/src/sampling.rs` | `6d90f43dbf0d2865ef63c001601c9084d6370c168056f883f49ffe7c732d9d13` |

## Candidate

Review [the step execution I/O candidate](step-execution-io-v1.md) against:

- `spec/engine-v0.md` sections 9, 10, 15.3–15.6;
- `crates/glm-engine/src/step.rs`;
- `crates/glm-engine/src/output.rs`;
- `crates/glm-engine/src/worker.rs`;
- `crates/glm-scheduler/src/lib.rs`;
- `crates/glm-serving/src/lib.rs`;
- `crates/glm-serving/src/http.rs`; and
- `crates/glm-reference/src/sampling.rs`.

Hash every input at review start and finish. Review the exact candidate commit
in a worktree if `main` advances. Do not infer GPU authorization or device
qualification.

## Requested adversarial questions

1. Does the pipelined prefill/decode convention emit the first and every
   subsequent token exactly once without reprocessing the final prompt token?
2. Is the proposed input sufficient for an SM120 rank executor to reproduce
   prompt positions, sampling, MTP posture, output limits, and RNG state, or
   is any rank-local choice still possible?
3. Can prefix restoration, chunked prefill, cancellation, or a failed step
   make the retained prompt slice disagree with scheduler progress?
4. Is generation plus canonical input-hash consensus sufficient without
   changing `StepPlan.v1`, and what device-upload acknowledgment is required?
5. Is accepted draft EOS represented correctly by accepted-draft count plus
   an absent target token?
6. Which exact RNG counter deltas can be frozen now for greedy, categorical,
   proposal, acceptance, rejection, residual, bonus, and accepted-EOS paths?
7. Are the one-million-token host bounds and all integer/float validations
   fail-closed?
8. Does this contract preserve distributed sampling without a full-vocabulary
   gather?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Withhold
any acceptance token unless every blocker and major is resolved. State
separately whether the candidate may proceed to CPU implementation and whether
it requires a `StepPlan` ABI revision.
