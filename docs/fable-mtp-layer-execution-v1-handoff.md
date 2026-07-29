# Fable handoff: recurrent MTP execution v1

Date: 2026-07-29

Status: adversarial design review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released and occupied by another four-rank workload; do not
connect to cn4 or launch CUDA for this review

Review candidate commit:
`fd80e16d88434fdf7bf55778977044c64dd1a366`

Required result path:
`fable-mtp-layer-execution-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`mtp-layer-execution-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof` on this
handoff, then hash every input at review start and finish. If either set
differs, report a stale candidate and do not emit the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/mtp-layer-execution-v1.md` | `5ad5bf01cdbd5e183b5e50aa0940344b5aabc09bf05a90c57d58e3e5b28dd3a7` |
| `docs/manifest-source-audit-20260729.md` | `480bc583315b071f6af6aba2372400db6007e96c17ee1f49767b650a51290095` |
| `docs/target-layer-execution-v1.md` | `7f16667d55a50441dbc81b7fe18285be5a6f0fb06c68ef008a40aad8f406f478` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `docs/step-execution-io-v1.md` | `e8681e9278034b25fe6928c059ad58730818ce014fb3e0251549f678aa1621d5` |
| `docs/distributed-sampling-abi-v1.md` | `d717508e4d90f6ef378d486c0bd3e93e7dad522e6529b8504ccb687a0280fdce` |
| `docs/serving-page-transaction-v1.md` | `e3a9a1d9f2eb26dc5312d7c42297fa3d832e444f7e3f269094746a85fb3deac2` |
| `docs/online-prefix-publication-v1.md` | `67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873` |
| `crates/glm-cache/src/mtp.rs` | `1134213f9786eafab9dcb3dd0410f708e5b9addf083140676a523a586968a4b0` |
| `crates/glm-engine/src/output.rs` | `1a82e9990af4e2892831e950f5cd3db256032ab5e43642faeb08229cbf2f1c2c` |
| `crates/glm-format/src/checkpoint.rs` | `08450f0cb33e592ec76dfbe655b06580ba1743e60f3109a65218d052dbea406c` |
| `docs/cn4-release-20260729.md` | `bb73391108c321b8384662a63ed5f00af84cd18f9ac20c1e635891f9c107ab9f` |
| `scripts/local-checks.sh` | `378a717bc9d592bac68ac999c6a91d4655b633177a9005529288096a3d2a029b` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-mtp-layer-execution-v1-handoff.md
```

## Exact external source check

Do not fetch model weights. Verify these read-only files if the sibling
checkout is available:

```text
../glm52-opt/workspace/vllm-v20-indexer-wht-prototype/vllm/model_executor/models/deepseek_mtp.py
../glm52-opt/workspace/vllm-v20-indexer-wht-prototype/vllm/models/deepseek_v32/nvidia/mtp.py
../glm52-opt/workspace/vllm-v20-indexer-wht-prototype/vllm/v1/spec_decode/llm_base_proposer.py
```

The first two must hash respectively to:

```text
3a8a0b30e5dc5eb8c1f0ddb2ce317c375dc094de5b5ba8ba78f71d5481deae6d
8e09e33823d4a6feb5071eb4ef3a5822bf79c1fab7ab59b9e5220be67b5571ca
```

The official Transformers modeling/configuration files at commit
`5204b4fe36956e9214b9279f1e1be2fd5dd1d9f3` must hash to the identities in
the source audit. Do not substitute the public `v5.12.0` tag.

## Review focus

The critical new claim is not merely that layer 78 is recurrent. The candidate
states that the pinned proposer teacher-forces `token[s]` with target hidden
`hT[s-1]` at draft logical position `s-1`, while later recurrence rows use
draft hidden and cannot become durable merely because their tokens are
accepted.

The proposed HBM/tier representation stores that transition in successor slot
`s`; slot zero is a sentinel. This keeps a page's draft bytes dependent on
the same token prefix as its target page. A private pending target token may
prepare the draft sidecar one position ahead of materialized target KV, but
only equal valid ranges may seal or publish.

## Required adversarial questions

1. Independently trace the pinned proposer's shift, positions, hidden rows,
   first pass, tuple handling, top-k compaction, and recurrence loop. Is
   `token[s] + hT[s-1] + position[s-1]` the exact teacher lineage? Identify
   any pre/post-final-norm or off-by-one alternative that the source permits.
2. Does successor storage slot `s=x+1`, sentinel slot zero, and RoPE position
   `x=s-1` preserve the exact layer-78 attention sequence? Re-derive slots
   0,1,63,64,65 and 1,048,575, page ownership, capacity, causal reads, and
   cross-page writes.
3. Is the same chained target page key sufficient for the proposed sidecar?
   Prove or disprove that every sidecar byte in page `j` depends only on that
   page's token prefix and parent, and compare this with predecessor-slot
   storage whose last record depends on the next page.
4. Does every accepted recurrent row require teacher synchronization with an
   authoritative target hidden? Can any source property make recurrent KV or
   indexer bytes commit-equivalent? Is treating `skip_topk` rows as scratch
   consistent with the missing durable key production?
5. Is the pipelined state with exactly one authoritative
   emitted-but-not-materialized target token correct? Trace bootstrap,
   rejection at every `i`, all accepted, bonus, pending/accepted EOS, stop,
   cancellation, retry, suspension, and worker failure without duplicating,
   omitting, or retracting a client-visible token.
6. Recompute target, teacher, scratch, and reservation row counts for MTP0–6.
   Is `R+1` target verifier rows, up to seven teacher-sync rows, and an
   optional `R+2` successor-slot span sufficient at page boundaries and the
   model/output limits? Can a draft-only private next page become an orphan?
7. Is a target boundary replay sufficient after shared-prefix restoration to
   recover `hT[last]` and pending logits without reading a cached current-row
   KV as an extra self key or overwriting shared bytes? Must the cache ABI
   instead persist a boundary hidden/logit record?
8. Does `TEACHER_SYNC` correctly execute the shared embedding, norms,
   replicated `eh_proj`, full index group 21, absorbed MLA/DCP route,
   routed/shared MoE, two TP sums, one final norm, and sharded head? Are any
   layer-78 weight, dtype, tensor-parallel, or collective bindings missing?
9. Can teacher rows independently select top-2,048 winners while the final
   row's winner list is compacted and reused only by its proposal generation?
   Test mixed C64 acceptance lengths, early EOS, stable compaction, generation
   ABA, and masked graph rows.
10. For probabilistic MTP, is the retained local q representation sufficient
    for exact later residual sampling without a full-vocabulary gather?
    Recompute the 929,280-byte depth-six local bound and determine the exact
    proposal/acceptance ticket continuation needed across physical steps.
11. Does pipelining preserve the sampling ABI's logical distributions and
    counter formulas? Identify the exact `SamplingCounter.v2`,
    `StepInput/Output`, trace, retry, and transaction fields required before
    probabilistic CPU implementation.
12. Can the collective and graph schedule express every teacher, recurrent,
    verifier, and sampling phase without rank-local fallback, hidden
    collectives, full-logits gather, or scratch exposure? Recompute the
    maximum graph row/buffer lifetimes.
13. Are the eight named contract amendments complete and correctly versioned?
    Identify every contradiction with current engine/format/manifest,
    page-transaction, prefix-publication, output, scheduler, quality, and
    cache-budget semantics. Do not accept two contradictory live ABIs.
14. Does the CPU gate establish shifted-source equivalence, prefix byte
    determinism, teacher/scratch non-equivalence, all state transitions, q/RNG
    continuation, and failure atomicity before any CUDA work? Are the later
    layer-78 SM120 gates appropriately scoped?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then state
separately whether:

- the exact shifted source lineage and one-layer row program are accepted;
- the successor-slot HBM/tier ABI and prefix-key determinism are accepted;
- teacher synchronization, recurrent scratch, and top-k reuse are accepted;
- the pipelined proposal/verification/commit state machine is accepted;
- probabilistic q retention and cross-step RNG ownership are accepted; and
- the proposed contract amendments plus CPU/SM120 gate order are accepted.

Only if all six answers are unqualified `YES`, end with the requested token.
Do not emit it for a conditional pass, stale input, source mismatch,
underspecified state, or a contract that depends on a later invention.

The token accepts only this design and opens coordinated ABI amendments plus
CPU proof. It does not accept an implementation, authorize cn4, authorize a
CUDA launch, accept model quality, permit conversion, or establish speed,
capacity, concurrency, prefix, tiering, or production-serving claims.

