# Fable handoff: target-layer execution v1

Date: 2026-07-29

Status: adversarial design review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another developer; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`83f5005a7e6dd3f45422df6cb091c4e743727bbd`

Required result path:
`fable-target-layer-execution-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`target-layer-execution-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof` on this
handoff, then hash every input at review start and finish. If either set
differs, report a stale candidate and do not emit the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/target-layer-execution-v1.md` | `7f16667d55a50441dbc81b7fe18285be5a6f0fb06c68ef008a40aad8f406f478` |
| `docs/manifest-source-audit-20260729.md` | `480bc583315b071f6af6aba2372400db6007e96c17ee1f49767b650a51290095` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `docs/step-execution-io-v1.md` | `e8681e9278034b25fe6928c059ad58730818ce014fb3e0251549f678aa1621d5` |
| `docs/distributed-sampling-abi-v1.md` | `d717508e4d90f6ef378d486c0bd3e93e7dad522e6529b8504ccb687a0280fdce` |
| `docs/serving-page-transaction-v1.md` | `e3a9a1d9f2eb26dc5312d7c42297fa3d832e444f7e3f269094746a85fb3deac2` |
| `docs/sm120-rank-runtime.md` | `19638590ee3b42da32bfab7673986c26488da064649c635df895700838da5624` |
| `crates/glm-engine/src/step.rs` | `4963a58da7c9c6bbed0fb57fb7ef56d90d1e0f09fe54da8cc02c35891f743359` |
| `crates/glm-engine/src/graph.rs` | `c85ca1aa52ba42294fc6a43524e8f70357523977d343e8c2f212787e7754cd22` |
| `crates/glm-engine/src/output.rs` | `1a82e9990af4e2892831e950f5cd3db256032ab5e43642faeb08229cbf2f1c2c` |
| `crates/glm-engine/src/worker.rs` | `400c7c22f2c74d3f386fefe2d144da3437f27e6c99ce9f2d4bbf87ffe98fe437` |
| `crates/glm-cache/src/attention.rs` | `662965eb0c7e9e22768ee7c95c849b403a0a0004a1c061fb98c996fdd9c4e89f` |
| `crates/glm-format/src/checkpoint.rs` | `08450f0cb33e592ec76dfbe655b06580ba1743e60f3109a65218d052dbea406c` |
| `scripts/local-checks.sh` | `378a717bc9d592bac68ac999c6a91d4655b633177a9005529288096a3d2a029b` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-target-layer-execution-v1-handoff.md
```

## Exact external source check

Fetch no model weights. From the official Transformers repository, fetch only
these two files at commit
`5204b4fe36956e9214b9279f1e1be2fd5dd1d9f3`:

```text
src/transformers/models/glm_moe_dsa/modeling_glm_moe_dsa.py
src/transformers/models/glm_moe_dsa/configuration_glm_moe_dsa.py
```

They must hash respectively to:

```text
adb8317a21716b01273046e46c807f14f0dbaf035af59b60d52bd6bc3007cf72
5a81164be746307431ad998f789b6b0bca20eb4c14a726552eb3730268413997
```

Do not substitute the public `v5.12.0` tag: its modeling file has different
bytes and a different indexer RoPE implementation.

## Required adversarial questions

1. Do the exact upstream bytes, model config, operation manifest, and strict
   tensor contracts prove all stated shapes, layer modes, full/shared indexer
   groups, source dtypes, and operator order? Identify any claim derived from
   a moving branch or an unpinned implementation.
2. Does the program correctly enter through the vocabulary-row-sharded
   embedding with one TP sum, execute layers 0–77 with two TP sums per layer,
   and exit through final RMSNorm plus a row-sharded LM head without a
   full-logits gather? Are production prefill/decode/verify head rows and
   pending-logit pipeline order exact?
3. Independently derive absorbed MLA from the pinned KV-B layout. Are
   `W_K[192,512]`, `W_V[256,512]`, the 512+64 query, the `256^-0.5` scale,
   the 512-value latent numerator, and the post-merge V expansion correct?
   Does the proposed BF16/FP32 rounding contract need any additional field or
   matched decoded-expand control before CPU implementation?
4. Recompute every wire size and offset: 368-byte KV, 132-byte indexer,
   1,152-byte absorbed query, 2,064-byte partial state, 16-byte candidate,
   32,776-byte candidate batch, 16,392-byte winner list, and 40-byte
   collective record. Do empty states, padding, nonfinite values, counts,
   position zero, and malformed records fail closed?
5. Does the exact indexer path use epsilon-`1e-6` LayerNorm, the pinned
   interleaved 64-value RoPE path, ReLU after `128^-0.5` FP32 head scores,
   FP32 head weights scaled by `32^-0.5`, causal selection, deterministic
   score/position ties, and the post-RoPE durable key? Is layer 6 the first
   sparse-MoE/full-indexer replay and does layer 7 reuse precisely its
   winner set?
6. For `PREFILL_CKV`, can canonical generation-bound physical-record unions
   safely deduplicate shared prefixes, gather full-layer indexer keys once,
   compute identical winners on every rank, and gather only the referenced KV
   union? Is the `ALL_POSITIONS_ASC` shortcut correct at context lengths at
   most 2,048, and can any causal, generation, owner, or union-index mismatch
   silently select a different set?
7. For `PREFILL_QUERY`, decode, and verify, are query gather, local index
   selection, fixed candidate exchange, owner-local attention, partial return,
   fixed `0,1,2,3` LSE merge, V expansion, O projection, and TP reduction
   ordered without a full KV gather or rank-local fallback?
8. Does sparse MLP exactly preserve FP32 router arithmetic, corrected-score
   selection, deterministic ties, original-score weights with `1e-20`,
   stable expert/token/route compaction, route weighting after down
   projection, routed-plus-shared combination before one TP sum, and residual
   placement? Does this remain valid for mixed immutable EXL3/NVFP4 expert
   membership and empty/skewed experts?
9. Is `TargetProgram.v1` common across ranks while rank-specific descriptor
   offsets and hashes remain authenticated? Are its entry hashes,
   `target_program_sha256` consensus bindings, and required amendments to
   startup, graph profile, step input, and pending-logit state sufficiently
   specified to implement without inventing an ABI?
10. Is `CollectiveOp.v2` byte-stable, overflow-safe, layer/phase addressed,
    route-manifest-accounted, dependency-complete, and capable of expressing
    every PREFILL, DECODE, VERIFY, MTP0–6 sampling, embedding, DCP, and TP
    ordinal? Can graph/eager, fixed/variable capacity, participant masks, or
    zero-count records diverge by rank?
11. Can any graph slot alias while live, can a captured node observe a changed
    codec/weight/program pointer, or can a tentative KV/indexer/pending-logit
    record become visible to another step, prefix lookup, eviction, or commit
    after failure? Is worker-generation termination limited to genuine
    collective uncertainty without allowing split-rank continuation?
12. Does the required CPU proof precede all CUDA work and cover independent
    source math, all C64/MTP0–6 shapes, payload arithmetic, fault injection,
    route consensus, buffer lifetimes, and decoded-expand controls? Does the
    layer-6/layer-7 replay plus downstream full-vocabulary reference
    continuation establish an adequate first SM120 layer gate without
    overstating full-model service?
13. Identify every conflict with the current engine/format specs or pending
    step, transaction, sampling, rank-manifest, graph, worker, and quality
    contracts. Distinguish required contract amendments from defects in this
    target program; do not accept two contradictory ABIs.

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then state
separately whether:

- the exact source-derived target operator program is accepted;
- absorbed MLA plus both DCP prefill/decode routes are accepted as a CPU-proof
  contract;
- candidate/winner/partial/collective wire ABIs and arithmetic are accepted;
- embedding, pending-logit, transaction, graph, and program-hash integration
  are accepted; and
- the proposed CPU/reference and later layer-6/layer-7 gates are accepted.

Only if all five answers are unqualified `YES`, end with the requested token.
Do not emit it for a conditional pass, stale input, source mismatch, or an
underspecified ABI.

This token accepts only the target execution design and opens its CPU ABI
amendments/reference proof. It does not accept any implementation, authorize
cn4, authorize CUDA, accept a kernel, permit full-checkpoint conversion, or
establish checkpoint startup, quality, speed, capacity, or serving.
