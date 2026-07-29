# Fable Phase-A implementation and engine-contract handoff

Date: 2026-07-28

Review target: post-v2 Phase-A artifacts plus the CPU-only execution-contract
candidate

GPU authorization conveyed by this handoff: none

## Provenance

Hash every input again at the start and finish of review. Review the exact
bytes below; report a stale hash instead of inferring which version was
intended.

| Input | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `spec/format-v0.md` | `fbe147ebe5c8a88b2fe81dfdb871bd32d43395f1b5ccf5c7162779a3f8cf7b77` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `docs/nvfp4-physical-abi.md` | `8936c8a60a1d6a7a2038fcd7f3f4a352b80477c359a6f3f2f89ea3903d2a9e99` |
| `docs/fable-v2-disposition.md` | `fd60c89ec188fc6467507ad054f114a379625b0eec40b863cb61c5ace5b1783b` |
| `crates/glm-engine/src/lib.rs` | `d204f12365abcc2c78a2388b527b8398f56b81a5fa884cecae906aac8e30dd9c` |
| `crates/glm-engine/src/step.rs` | `4963a58da7c9c6bbed0fb57fb7ef56d90d1e0f09fe54da8cc02c35891f743359` |
| `crates/glm-engine/src/graph.rs` | `c85ca1aa52ba42294fc6a43524e8f70357523977d343e8c2f212787e7754cd22` |
| `crates/glm-engine/src/memory.rs` | `cbd14ace3eac793dc46678071ac28b6f5a66ed67f07732ef3b8b66d7185ee381` |
| `fixtures/engine-contract-proof-v1.json` | `08a06a9a07b74de27c96a5b58c65d9ca1b72b758f3cc1acefb4159aab42a1cd5` |

Read [the engine-contract candidate](offline-engine-contract.md) for the
implementation boundary and review questions. That explanatory file is not
normative; the specifications remain authoritative.

## Requested review

First close the review point left by Fable v2:

1. Verify the generated model operation manifest against the pinned GLM-5.2
   graph, including TP4 slicing, reduction boundaries, the 21 IndexShare
   groups, and the recurrent layer-78 draft step.
2. Verify the v0.2.2 combined draft-KV/draft-indexer sidecar arithmetic,
   attachment, tiering, and rollback contract.
3. Verify that the implemented NVFP4 physical ABI matches the reviewed
   format contract rather than silently adding a second layout.

Then adversarially review the engine-contract candidate:

1. Is the canonical 85-byte plan hash input unambiguous and complete?
2. Can two ranks accept different collective behavior while retaining the
   same plan and collective hashes?
3. Are mode-specific zero fields strict enough to prevent stale graph,
   route, sampling, or sequence-table state?
4. Should collective entries carry explicit layer and IndexShare group IDs,
   or is a contiguous ordinal tied to the graph profile sufficient?
5. Is rejecting `MIXED` correct until the single attention-transport field is
   replaced by a reviewed dual/compound representation?
6. Are graph-profile keys sufficient to reject every unreachable or
   unqualified serving shape?
7. Does the memory planner name every required term, take the correct
   mutually exclusive workspace maximum, preserve committed floors, account
   for slack/tentative slots, and reject a single overcommitted rank?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. State
separately whether:

- the manifest/v0.2.2 review gate is closed for authorized M2 execution;
- the `StepPlan` candidate may be promoted into the normative ABI;
- separate prefill and decode steps may proceed while `MIXED` remains
  disabled;
- any finding blocks continued CPU-only EXL3 extraction.

Do not infer cn4 authorization from a review pass.
