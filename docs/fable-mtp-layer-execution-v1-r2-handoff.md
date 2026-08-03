# Fable handoff: recurrent MTP execution v1 r2

Date: 2026-08-03

Status: bounded adversarial corrective-design re-review requested

Review candidate commit:
`83bd1ba4dd14aa60224a4483a80ed85dbcf74d14`

Required result path:
`docs/reviews/fable-mtp-layer-execution-v1-r2.md`

Requested acceptance token, only if every blocker and major is resolved:
`mtp-layer-execution-v1-accepted`

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch CUDA for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if needed, run `review-proof`, and hash every input at review start
and finish. Any mismatch is stale and must withhold the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/mtp-layer-execution-v1-r2.md` | `d75710b3b552f229cc3bef34a8977a7c30e5b03b4c4a268f27c0efb2a3d1f12c` |
| `docs/mtp-layer-execution-v1.md` | `5ad5bf01cdbd5e183b5e50aa0940344b5aabc09bf05a90c57d58e3e5b28dd3a7` |
| `docs/step-execution-abi-v3.md` | `1cde3bcabba0a0d861691b06ddb140cb64dfbefaab1129c8a04bc302c0ce609e` |
| `docs/target-layer-execution-v1-r2.md` | `808da35c2e54eb5692512996650839fb6f127cb91658603eb2fb5ce049c56ed2` |
| `docs/sm120-w4a16-nf3-fused-moe-v1-r2.md` | `311d1214ad57e97c7bab45069fae5507602c0e21922b1fde677ba129e734f265` |
| `docs/distributed-sampling-abi-v1.md` | `383e328a527cc780ed553af0b78382cf200ad60f97afb26d96a2a1494b57c89b` |
| `docs/step-execution-io-v1.md` | `055412c022cfcf9299e95e3ad3f7b888a2d472835388c35c2a8443be71a7422c` |
| `docs/serving-page-transaction-v1.md` | `31983cce95ee01a5968213d5daf12c7a855f75f8735314700f2b4a9e55625d1a` |
| `docs/online-prefix-publication-v1.md` | `67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873` |
| `docs/hybrid-mtp3-capacity-ledger-v1-r2.md` | `6efeee90addafb4f8d645610bf617f1f4dd9b1bd630096f570193f407c49c9c6` |
| `spec/engine-v0.md` | `52497e022bde5278372a9bce168e87a602fca9341c4cb4e019b4a3c7ce63179b` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `crates/glm-format/src/checkpoint.rs` | `08450f0cb33e592ec76dfbe655b06580ba1743e60f3109a65218d052dbea406c` |
| `crates/glm-engine/src/input.rs` | `c3d090429015030416f6c03ddb6fef2dfd569859ff6e0fcc05bcb2d6a163ffa2` |
| `crates/glm-engine/src/output.rs` | `1a82e9990af4e2892831e950f5cd3db256032ab5e43642faeb08229cbf2f1c2c` |
| `crates/glm-engine/src/memory.rs` | `2131c999b6762a9b7e505cfe542c957877d95af4ee04056affa9d677156e9491` |
| `crates/glm-cache/src/mtp.rs` | `1134213f9786eafab9dcb3dd0410f708e5b9addf083140676a523a586968a4b0` |
| `scripts/local-checks.sh` | `2d1882be9afd91f4a54c1d3ff9b9f02cd5087357eeb5668d4094c2114c3003ce` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-mtp-layer-execution-v1-r2-handoff.md
```

## Review boundary

The v1 review already independently accepted the source-proven teacher
lineage, successor-slot/RoPE mapping, page-key sufficiency, teacher/scratch
non-equivalence, pipelined state machine, row/reservation arithmetic,
boundary replay, teacher operator, winner reuse, q-state bound, and gate
order. Recheck those only for regression introduced by r2.

The corrective review must decide whether r2 completely closes v1's one
major, three minors, and two questions without leaving a later ABI invention.
Do not infer that the current Rust v1 types implement the design.

## Required adversarial questions

1. Does the exact 240-byte output distinguish materialized from emitted
   tokens, include `INITIAL`, bind current/next pending state, preserve the
   seven-token limit, and separate common from rank-local state?
2. Re-add every output offset. Are bytes `0..136` common and `136..240` local
   with exact reserved zeros and no overlap or hole?
3. Do the 48-byte `SamplingCounter.v2` and 48-byte `SamplingTicket.v2` records
   contain every field needed to bind bundle generation, purpose, logical
   position, before/after counters, and retry identity?
4. Trace TARGET, DRAFT, ACCEPTANCE, RESIDUAL, and BONUS tickets across
   bootstrap, verify, replacement bundle, early/accepted EOS, mismatch,
   bonus, context/output clamp, cancellation, and stop. Is every ticket
   consumed once and is every per-phase maximum sufficient?
5. Can a reserved-but-uninstalled bundle generation be reused after a
   launched terminal/clamped step? Does the burn/overflow rule close ABA?
6. Does the no-same-generation-retry-after-native-launch rule compose with
   immutable prelaunch retry and boundary replay without either double draw
   or client-output rewind?
7. Recompute all four new `SystemMemoryPlan.v3` terms at C64/MTP6. Are
   59,473,920 proposal bytes, 196,608 aligned scratch bytes, 786,432 boundary
   hidden bytes, and 46,080 sequence-row bytes exact and nonoverlapping?
8. Is the 16,640-byte aligned winner stride and 7,454,720-byte 448-row maximum
   correctly owned by class 11 and charged once as verifier workspace rather
   than retained state?
9. Are the one-ahead teacher record, target table prefix, pending logits, q
   state, bundle rows, and recurrent key-zero region each given one and only
   one budget/arena home? Can any digest substitute for allocated bytes?
10. Does operation-manifest v2 completely extend routed/router membership to
    layer 78, bind index group 21, and declare the four protected BF16
    replicated roles with the exact shapes and IDs? Is codec selection
    correctly delegated to the immutable physical MTP program?
11. Can target-only decode alias pipelined MTP0? Check `INITIAL` versus
    `MTP0`, configured/effective depth, pending state, nonzero bundle identity,
    zero proposal/q/ticket state, graph identity, and cache capability.
12. Does chunked prefill retain exactly one authoritative BF16 boundary
    hidden per MTP-capable sequence, and can restored-prefix replay reconstruct
    it without overwriting or self-rereading a cached row?
13. Does prefix publication now require bit-identical target/indexer/draft
    bytes across every admitted graph bucket, chunk split, page split, and
    boundary-replay route even though ordinary model execution need not be
    batch-invariant?
14. Do `CompletionRecord.v2` and `SessionExport.v2` correctly represent a
    client-visible terminal EOS with no target KV while keeping accepted
    draft EOS materialized and nonterminal export flush-only?
15. Are the closed identity table and coordinated amendment set complete?
    Find any old StepPlan/Input/Output, page, sampling, memory, prefix,
    session, manifest, MTP-tail, or resident-program identity that could enter
    the same generation.
16. Does the retained CPU/SM120 gate order test every new field, memory term,
    MTP0 posture, chunk boundary, publication route, terminal import, and
    pre/post-launch failure before CUDA promotion?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer each statement separately:

1. The retained shifted lineage, successor-slot sidecar, teacher/scratch
   separation, and pipelined state machine remain accepted.
2. The exact output, pending-state, and common/rank-local consensus ABI is
   complete and accepted.
3. SamplingCounter/Ticket v2, cross-step ticket ownership, generation burn,
   retry, and failure atomicity are complete and accepted.
4. All MTP memory, graph-workspace, tentative-page, and argument-buffer homes
   are exact, bounded, nonoverlapping, and accepted.
5. Layer-78 manifest membership and pipelined MTP0 identity are complete and
   accepted.
6. Chunk/replay determinism, prefix publication, terminal EOS/session
   visibility, coordinated identities, and the gate order are complete and
   accepted.

Only if all six are unqualified `YES`, end with the requested token as the
only bare acceptance line.

Do not emit the token for a conditional pass, stale candidate, arithmetic or
offset error, hidden state, generation ABA, retry ambiguity, budget alias,
incomplete layer-78 membership, two live predecessor ABIs, or a decision left
for implementation.

The token opens only coordinated CPU/reference implementation after every
other prerequisite design token exists. It does not accept current Rust,
authorize cn4/CUDA, accept a checkpoint, or establish model quality, capacity,
concurrency, or speed.
