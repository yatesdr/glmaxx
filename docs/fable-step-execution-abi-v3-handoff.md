# Fable handoff: complete GLM-5.2 step execution ABI v3

Date: 2026-07-30

Status: adversarial cross-contract design review requested

Review candidate commit:
`bab7866b6bd494d3e70ba28463043555f5b583c8`

Required result path:
`docs/reviews/fable-step-execution-abi-v3.md`

Requested acceptance token, only for an unqualified pass:
`step-execution-abi-v3-design-accepted`

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch CUDA for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if necessary, run `review-proof`, and hash every input at review
start and finish. A mismatch is a stale candidate and must withhold the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/step-execution-abi-v3.md` | `1cde3bcabba0a0d861691b06ddb140cb64dfbefaab1129c8a04bc302c0ce609e` |
| `docs/step-execution-io-v1.md` | `055412c022cfcf9299e95e3ad3f7b888a2d472835388c35c2a8443be71a7422c` |
| `docs/prefill-graph-profile-abi-v2.md` | `37154c9e31109acdf35a382c6be87b3a865e2b7f6ae8f801969526789dd41f91` |
| `docs/mtp-layer-execution-v1.md` | `5ad5bf01cdbd5e183b5e50aa0940344b5aabc09bf05a90c57d58e3e5b28dd3a7` |
| `docs/distributed-sampling-abi-v1.md` | `d717508e4d90f6ef378d486c0bd3e93e7dad522e6529b8504ccb687a0280fdce` |
| `docs/serving-page-transaction-v1.md` | `31983cce95ee01a5968213d5daf12c7a855f75f8735314700f2b4a9e55625d1a` |
| `docs/fixed-page-transaction-v1.md` | `c03dd66f78b8e81ce5b0743d34091449d84c43d08e620a694a0c66b318a5d6fc` |
| `docs/sm120-rank-executor-v1-r2.md` | `4f40ea7652858b4cebbe4093dc81149cb30aa26bedc69edef72fa627c987df89` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `crates/glm-engine/src/step.rs` | `4963a58da7c9c6bbed0fb57fb7ef56d90d1e0f09fe54da8cc02c35891f743359` |
| `crates/glm-engine/src/input.rs` | `c3d090429015030416f6c03ddb6fef2dfd569859ff6e0fcc05bcb2d6a163ffa2` |
| `crates/glm-engine/src/output.rs` | `1a82e9990af4e2892831e950f5cd3db256032ab5e43642faeb08229cbf2f1c2c` |
| `crates/glm-cache/src/delta.rs` | `71ac2da15e869a6f2470c3551a7cd6ec4ff387850a23240e9a44ad96a538ff16` |
| `crates/glm-cache/src/mtp.rs` | `1134213f9786eafab9dcb3dd0410f708e5b9addf083140676a523a586968a4b0` |
| `crates/glm-cache/src/sequence.rs` | `8c0491d4f2d3e50da12e15961c8ac65a2fe5449a3527d40a38cdaa5ef27d644e` |
| `crates/glm-engine/src/worker.rs` | `b8498639bb05ef84c2d06eb1e4650d8f7915eb1e3b306abdfd2cc0fb93b104fa` |
| `crates/glm-scheduler/src/compile.rs` | `220cf549c0b5882d109ebce4ebd646e9b28ebbab80a83fa579ef5a2c591a070a` |
| `crates/glm-serving/src/lib.rs` | `bc7eff0297e14b73df7eec5ade3352ad0f75ceabeaca1862c4866a51efb948e3` |
| `scripts/local-checks.sh` | `ac185a784489fe7e85e8d1c13956f8ba2a35b740cb7a7f2c076dad6e25530d8a` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-step-execution-abi-v3-handoff.md
```

## Review boundary

This review must judge whether the proposed ABI is internally complete and
implements the already pinned target/MTP semantics without hidden choices. It
must not infer that the current v1 Rust types implement v3. It may require a
bounded correction but should not demand compatibility with unpromoted v1/v2
records.

The executor r2, target-layer, MTP, sampling, graph-profile, and page
transaction designs still have their own independent gates. Acceptance here
does not automatically accept those artifacts.

## Required adversarial questions

1. Independently add every StepPlan field width. Is the hash input exactly 95
   bytes and the record exactly 127, with no missing or overlapping offset?
2. Independently add every SequenceInput field. Is it exactly 480 bytes, with
   all holes explicit and zero?
3. Independently add every rank output field. Is it exactly 240 bytes, with
   bytes `0..136` common and `136..240` local?
4. Do all six phases represent the pinned target/MTP pipeline without
   reprocessing a token, skipping target materialization, or committing
   recurrent scratch as teacher state?
5. Trace materialized and emitted ends through target decode, bootstrap,
   zero-depth pipeline, flush, every verify rejection position, all accepted,
   accepted EOS, residual/bonus EOS, output clamp, and context clamp.
6. Is `target_rows=sum(R+1)` for verify correct for mixed actual proposal
   counts, and do all bucket formulas cover but not understate C1/C64 work?
7. Is prefill teacher capacity sufficient for the slot-zero sentinel,
   continued prompt chunks, and restored-prefix boundary replay without
   falsely treating every target row as a real teacher row?
8. Can a target-only row or a pending depth-zero MTP row masquerade as the
   other posture under the configured depth, bundle generation, retained
   digest, and phase rules?
9. Is one process-common next bundle generation selectable before launch even
   when EOS later prevents installation? Can any discarded generation be
   reused or confused with a committed bundle?
10. For greedy/TOP_K/MASS, are proposal tokens, state lengths, four rank-local
    digests, retained-state digests, program identity, and trace sufficient to
    bind every byte the later verifier consumes?
11. Does retained state also bind pending target logits and authoritative
    hidden across first/continued/restored prefill and every decode phase?
12. Re-derive physical SamplingCounter.v2 order across adjacent steps. Are
    installed DRAFT tickets counted exactly once, and do ACCEPTANCE,
    RESIDUAL/BONUS, TARGET, and replacement DRAFT tickets have one
    unambiguous sequence?
13. Re-derive every per-phase maximum counter advance. Find any control flow
    that exceeds its preflight bound or consumes a ticket after terminal
    clamping.
14. Can failed/prelaunch-rejected/native-launched work reuse or incorrectly
    advance a seed, counter, ticket range, or bundle generation?
15. Does PageTableDelta.v2 represent separate target validity and one-ahead
    draft preparation at the same logical page, including page 0 sentinel and
    a cross-page pending token?
16. Are global delta, rank projection, and full post-apply device-table hashes
    acyclic and sufficient against wrong-offset upload, partial upload, stale
    restart, rank substitution, and re-signed host-only state?
17. Independently re-derive every reservation maximum. Is `R+2 <= 8` required
    for verify and does eight still imply no more than one existing-tail plus
    one new-page edit per row?
18. At position 1,048,575, can bootstrap/verify/pipeline/flush reserve or emit
    an impossible successor slot? Check the missing successor slot
    1,048,576 explicitly.
19. Are scratch proposal KV/q state structurally absent from the active page
    table and all durable/prefix/session publication?
20. Can four byte-identical common output prefixes still hide divergent
    rank-local proposal/page state? Does ordered local-suffix validation close
    that without demanding identical sharded bytes?
21. Reconcile every phase's emitted/materialized counts, token array, pending
    token, target kind, terminal flag, counter, and next-state fields. Is any
    invalid combination still canonical?
22. Does host stop filtering after model output admit a bounded atomic prefix
    commit without publishing later target/teacher writes or a next bundle?
    Identify any tokenizer-stop case requiring an unstated rewind.
23. Is the proposal-state arena bound sufficient for 64 concurrent MASS MTP6
    rows on each rank, and is it charged before admission rather than allocated
    in a step?
24. Can rows with different sampling tuples/proposal counts/depths share one
    graph only when phase, bucket, route class, and collective counts remain
    identical?
25. Does removing CACHE_ONLY/MIXED from this ABI compose exactly with
    `RankTierCommand.v1` and avoid two monotonic step-ID allocators?
26. Are all v1/v2 schema/hash identities rejected, and is there any source or
    handoff that would silently accept a mixed predecessor object?
27. Is the CPU proof matrix sufficient before implementation? Add any missing
    corruption, boundary, transaction, counter, or allocation probe.
28. Did the design introduce a quality-changing numerical rule, rank-local
    fallback, hidden full-vocabulary gather, or unbounded hot-path object?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer each statement separately:

1. `StepPlan.v3` serialization, phase shapes, graph keys, and row buckets are
   accepted.
2. `StepInput.v2` serialization and target/MTP retained-state bindings are
   accepted.
3. `SamplingCounter.v2` physical-step ticket ownership and bounds are
   accepted.
4. `PageTableDelta.v2`, device-table attestation, and one-ahead reservation
   semantics are accepted.
5. Rank/process `StepOutput.v2`, stop finalization, and four-rank atomic
   transaction semantics are accepted.
6. Fixed-capacity/concurrency arithmetic and the CPU/reference gate are
   accepted.

Only if all six are unqualified `YES`, end with:

```text
step-execution-abi-v3-design-accepted
```

Do not emit the token for a conditional pass, stale candidate, byte-layout
mistake, missing counter path, page-slot under-reservation, digest cycle,
rank-local ambiguity, unbounded state, or an implementation choice left open.

The token opens only coordinated CPU/reference implementation of these exact
v3/v2 ABIs after their prerequisite design tokens exist. It does not accept
the current v1 Rust worker, authorize cn4/CUDA, accept model execution,
approve checkpoint conversion, or establish quality, capacity, concurrency,
or speed.
