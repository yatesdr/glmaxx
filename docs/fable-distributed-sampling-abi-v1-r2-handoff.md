# Fable handoff: distributed sampling ABI v1 r2

Date: 2026-08-04

Status: bounded adversarial corrective-design re-review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4, launch CUDA, inspect model payloads, or mutate any
runtime resource. This is a source, arithmetic, serialization, state-machine,
and CPU-design gate only.

Review candidate commit:
`98ac16a98943d810cb3b8d86552e625aadc7be98`

Required result path:
`fable-distributed-sampling-abi-v1-r2.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`distributed-sampling-abi-v1-r2-design-accepted`

The original withheld review is an operator-inbox input at
`docs/reviews/fable-distributed-sampling-abi-v1.md`, SHA-256
`901481e5b1d6b26283a7c7e8eb1a1f7af1968df2b2e5d2ddda1c54d0075aa61c`.
Hash it before evaluating the closure claim and again at review finish.

## Provenance

Review the exact candidate in a detached worktree. Hash every candidate input
at start and finish. Withhold the token for any mismatch or incomplete input.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/distributed-sampling-abi-v1-r2.md` | `f2fb8ec8c81c63e76b7a0639fddc8c74719faff2a972bafcdf0b1d5de8db3db7` |
| `docs/distributed-sampling-abi-v1.md` | `383e328a527cc780ed553af0b78382cf200ad60f97afb26d96a2a1494b57c89b` |
| `docs/step-execution-abi-v3.md` | `1cde3bcabba0a0d861691b06ddb140cb64dfbefaab1129c8a04bc302c0ce609e` |
| `docs/mtp-layer-execution-v1-r2.md` | `d75710b3b552f229cc3bef34a8977a7c30e5b03b4c4a268f27c0efb2a3d1f12c` |
| `docs/target-layer-execution-v1-r2.md` | `808da35c2e54eb5692512996650839fb6f127cb91658603eb2fb5ce049c56ed2` |
| `docs/quality-acceptance-v1.md` | `705bb0611464bd5d76a08943b3122ecb8a78506e78f9c20a46d4e1ce24fc7be6` |
| `crates/glm-reference/src/sampling.rs` | `3205f2b11d5253c51176434337be8a3e4738a1cc84a4f2d16975248d816edfb5` |
| `crates/glm-engine/src/input.rs` | `c3d090429015030416f6c03ddb6fef2dfd569859ff6e0fcc05bcb2d6a163ffa2` |
| `crates/glm-engine/src/output.rs` | `1a82e9990af4e2892831e950f5cd3db256032ab5e43642faeb08229cbf2f1c2c` |
| `crates/glm-engine/src/step.rs` | `4963a58da7c9c6bbed0fb57fb7ef56d90d1e0f09fe54da8cc02c35891f743359` |
| `crates/glm-engine/src/memory.rs` | `2131c999b6762a9b7e505cfe542c957877d95af4ee04056affa9d677156e9491` |
| `crates/glm-serving/src/http.rs` | `c4a1700cef43ecfdc3578155670ae31d92d4a1e31bbfc8bdf82782c3bada4887` |
| `crates/glm-serving/src/backend.rs` | `07a4d53de6755ed8180ff90aed82f9efc71b8461070a909a10891c783a8fcf78` |
| `crates/glm-scheduler/src/lib.rs` | `5fd0c4506002c4da5679f1ca3bf96a880ca7b0b348d5f55ada26a2e06ae7ff4d` |
| `crates/glm-scheduler/src/compile.rs` | `220cf549c0b5882d109ebce4ebd646e9b28ebbab80a83fa579ef5a2c591a070a` |
| `scripts/local-checks.sh` | `2d1882be9afd91f4a54c1d3ff9b9f02cd5087357eeb5668d4094c2114c3003ce` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

The original v1 review covered candidate
`7c718188b167615affabdb66f34939dcd6b22587`, whose sampling document hash was
`d717508e4d90f6ef378d486c0bd3e93e7dad522e6529b8504ccb687a0280fdce`.
The current retained v1 differs by one six-line hunk that replaces the
unfrozen greedy-MTP quality sentence with a reference to the revision-2
quality contract. Independently inspect that diff; do not assume it is
nonsemantic or accepted merely because this handoff describes it.

Run the full CPU-only repository gate and record its exit status:

```text
./scripts/local-checks.sh
```

No current Rust sampling type implements the r2 design. Green current tests
are compatibility/preflight evidence only, not implementation acceptance.

## Required independent work

Do not accept the amendment by prose inspection alone. Independently:

1. reconstruct the two-step bootstrap/verify and verify/replacement timelines
   for every depth `0..6`, proving which immutable input owns each current
   proposal count/mask and which output owns each early-EOS successor count;
2. enumerate ticket ranges across two adjacent physical steps for target,
   draft, acceptance, rejection/residual, all-accepted/bonus, accepted EOS,
   output clamp, and context clamp;
3. implement an independent bit-level SplitMix64 and bounded-draw oracle,
   including products that round to mass and local subtraction that rounds to
   owner mass;
4. independently implement TOP_K filtering from finite rank candidates,
   including exact subtraction/division/exp rounds, post-top-k mass, inclusive
   top-p crossing, raw-weight sampling, and normalized support storage;
5. independently encode and corrupt `TopKSupportRecord.v2`, including support
   sizes 1/255/256, sentinel placement, token ordering, duplicate IDs, selected
   q lookup, and replicated-rank disagreement;
6. construct target/draft TOP_K unions of sizes 1, 256, 257, and 512 with
   equal, disjoint, and partially overlapping supports; compare the rank-zero
   residual sampler to a gathered full-vocabulary oracle;
7. independently implement MASS max/mass/selection and residual phases over
   four 38,720-row shards, checking every rank boundary and all 24 padded rows;
8. serialize every fixed request, message, state-digest, and trace preimage;
   recompute all stated byte counts and maximum logical payloads;
9. recompute the candidate's composite sampling digest from the two raw
   inner file hashes. The expected value is
   `8edd0d940273ee2e242b8164b611b8d997f7616f4618b0c1d894ea4dc114aa0f`;
10. recompute C64/MTP6 TOP_K and MASS proposal-state bounds and determine
    whether any step/memory predecessor text still gives greedy q state a
    contradictory nonzero home;
11. verify trace completeness for selected probability, fallback, no-target
    reason, route identity, counter continuation, rank state, and retry; and
12. trace the pinned Rust/API/scheduler sources and list every surface that
    must change atomically, without treating current code as r2 implementation.

## Required decisions

Answer each decision with an unqualified `YES` or `NO`:

1. Does r2 resolve the original BLOCKER by making the current verify bundle a
   prior-step committed input and early EOS an output for only the successor
   bundle, with no same-step immutable-input mutation?
2. Are current/next proposal counts, masks, generations, ticket ranges,
   proposal state, padding, and four-rank installation rules complete for
   bootstrap, verify, pipelined zero, and terminal paths?
3. Is the SplitMix64 mapping still exact, including the canonical
   seed-zero/counter-zero word, and is every FP64/FP32 bounded-draw rounding,
   clamp, comparison, and local subtraction boundary fully pinned?
4. Are temperature, finite-candidate selection, top-k, top-p, normalization,
   probability retention, negative infinity, and fewer-than-k behavior exact
   and independently implementable?
5. Is the 2,048-byte TOP_K support representation sufficient and canonical,
   and do its state/common digests prevent mutation or rank substitution?
6. Is moving TOP_K residual sampling wholly to rank zero correct, bounded by a
   512-token sparse union, and free of hidden full-vocabulary traffic?
7. Is the per-rank MASS proposal state sufficient to reconstruct acceptance
   and residual distributions without a full-vocabulary gather or a missing
   normalizer?
8. Are all fixed message layouts, phases, directionality, selected
   probabilities, and maximum logical payload calculations complete enough
   to produce exclusive PCIe route manifests?
9. Are `p(d)`, `q(d)`, acceptance ratios, uniforms, residual values, fallback,
   and result probabilities evaluated in an exact, stable numerical order?
10. Is retaining target fallback on a zero numerical residual safe only with
    the stated trace, counter, and nonzero-rate promotion gate?
11. Does the corrected greedy zero-q-state invariant remove the predecessor
    contradiction without weakening proposal identity or consensus?
12. Do the exact state sizes equal 786,432 TOP_K and 59,473,920 MASS bytes per
    rank at C64/MTP6, with no uncharged digest-only state?
13. Does the sampling trace distinguish accepted draft EOS, output clamp, and
    context clamp without widening or ambiguously interpreting StepOutput.v2?
14. Are CSPRNG failure, effective-seed exposure, and replay scope safe and
    explicit, with no cross-build or cross-math promise?
15. Does the composite sampling digest bind both retained v1 and corrective r2
    bytes into the target-program final-head identity without a hash cycle?
16. Does r2 close every original blocker, major, minor, and question without
    weakening the accepted ticket, transaction, quality, or fail-closed rules?
17. Does the expanded CPU gate cover every new byte, arithmetic boundary,
    topology, state, retry, EOS/clamp, and corruption case before CUDA work?
18. Are all implementation, CUDA, checkpoint, KLD, capacity, concurrency,
    cold-start, latency, throughput, and serving nonclaims accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, followed
by the independent derivations and all eighteen decisions. Only if every
decision is `YES`, attest the candidate commit, all seventeen candidate input
hashes, both start/finish operator-review hashes, and the composite digest,
then end with the requested token as the only bare acceptance line.

Withhold the token for a conditional pass, stale candidate, same-step proposal
mutation, unpinned draw boundary, incomplete support reconstruction, missing
wire field, contradictory q-state home, digest cycle, rank-local route choice,
or any decision left for implementation.

Acceptance opens only coordinated CPU/reference implementation. It does not
accept current Rust, authorize cn4/CUDA, permit checkpoint conversion, or
establish model quality, KV capacity, concurrency, cold-start, latency,
throughput, or serving readiness.
