# Fable handoff: normative startup order v1

Date: 2026-07-30

Status: adversarial CPU prerequisite review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`7420657a8528ef2ed780974bb0b8a699db9cfb0f`

Required result path:
`fable-normative-startup-order-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`normative-startup-order-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/checkpoint-load-transaction-v1.md` | `79d9c376201f3540f247344c24c37dd7d819d629f10459899a73c15f8b27015f` |
| `crates/glm-engine/src/startup.rs` | `54d41acc810c90cc49fe4acc0623b6a13bb2c09b72b2f8e5fb6615250ead2ddd` |
| `crates/glm-engine/src/lib.rs` | `b3ca0da8e0e61f05a92a3b15bc9dc7822395545733ebbdc270c9ff1fb21d6a54` |
| `crates/glm-serving/src/backend.rs` | `c1f9e9d06b44674a1d1d0ef3c24553a9ebe63e913805946bff7c2780233fe94b` |
| `docs/normative-startup-order-proof-v1.md` | `a46f88464b030a348b2041581ce63f620770fc854ff4a889a341ab383c4d9c27` |
| `docs/production-punchlist.md` | `002edf6e86679aefab6507a465b99db2ff02d9e984c9f101bcf9304daef5038c` |
| `docs/results-index.md` | `c986587f513a9dc1b30621aead73dca7b40c2462704f257ad2eaac3c4e6fd5cc` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-normative-startup-order-v1-handoff.md
```

## Review boundary

This review covers only the retained CPU coordinator's state names, exact
successful ordering, next-stage consensus, terminal failure behavior, and
continued serving-health check after replacing its obsolete mock sequence.

It does not accept the checkpoint-load transaction, a real rank executor,
CUDA contexts or allocations, graph capture, KV initialization, collective
setup, a kernel launch, a checkpoint smoke, or real production health.

## Required adversarial questions

1. Does `StartupState::NORMATIVE_ORDER` exactly match the successful sequence
   in engine-v0, with no omitted, duplicated, or transposed stage?
2. Does `successor` encode exactly the same sequence and make both `Healthy`
   and `Failed` terminal?
3. Does a new coordinator begin at `Created`, advance only to the immediate
   successor, and require exactly the rank set `{0,1,2,3}`?
4. Is `MemoryPlanned` necessarily reached before `WeightsLoaded` on every
   successful path?
5. Does the distinguishing obsolete-order regression reach
   `ModulesReady`, attempt `WeightsLoaded`, observe `RankAgreement`, and
   prove terminal `Failed`?
6. Can a wrong stage, rank error, rank count, zero/different immutable digest,
   changed digest, channel failure, or worker panic leave the coordinator
   recoverable or healthy?
7. Does the four-thread mock now traverse all ten transitions rather than
   returning healthy through the old seven-stage path?
8. Do the public discriminants collide, wrap, or accidentally treat
   `Failed=255` as a successful ordered stage?
9. Is any retained code still referring to an obsolete state variant or
   relying on the old numerical discriminants?
10. Does the serving backend continue refusing admission unless the state is
    exactly `Healthy`?
11. Are the test counts, hashes, host exclusions, and no-GPU/no-real-health
    claims accurate?
12. Is this correction properly described as a prerequisite rather than a
    checkpoint-load or executor implementation?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- state identities and normative order exactly match engine-v0;
- memory planning is impossible to bypass before weight load;
- rank consensus and terminal failure remain fail-closed;
- the obsolete-order regression is distinguishing;
- retained serving health admission remains exact;
- no old variant/discriminant dependency survives;
- results and exclusions are accurate; and
- the proof makes no CUDA, checkpoint-load, executor, smoke, or production
  health claim.

Only if all eight answers are unqualified `YES`, end with the requested
token. Withhold it for a conditional pass, stale input, missing/transposed
stage, divergent successor array, memory-after-load path, recoverable
failure, rank-consensus hole, nondistinguishing regression, stale variant
dependency, health-admission regression, false count/hash, or overstated
CUDA/checkpoint/executor/smoke/health claim.

The token accepts only the CPU startup-order prerequisite. It does not open
cn4, authorize CUDA work, or accept production serving.
