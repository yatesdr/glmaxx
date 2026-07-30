# Fable handoff: TP4 rank startup handshake v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`1eb8e1c2f6c98a2d20b8e4f168b8e88aadeb97ac`

Required result path:
`fable-tp4-rank-startup-handshake-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`tp4-rank-startup-handshake-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-engine/src/worker.rs` | `8c0742920847145e13975aae3db1b3a76054f94475b5a0b1ac4a4a9d05cba3ff` |
| `docs/offline-serving-spine.md` | `27b24d4cbafc8203937d3620e7bcd85d47fcb86cc4d8b89e237025e5d40a62f9` |
| `docs/sm120-rank-runtime.md` | `19638590ee3b42da32bfab7673986c26488da064649c635df895700838da5624` |
| `docs/sm120-rank-executor-v1.md` | `e97c54b865ed50c40ff8b15f6580d0edc18dbd0783135bc1c17d11cc19986fd4` |
| `docs/fable-sm120-rank-executor-v1-handoff.md` | `fe6fc7060d17db41901d545f4328a863b45737fd7e01be9c32a83bf013c2c031` |
| `docs/tp4-step-operation-quota-proof-v1.md` | `ab5e025afe5f4c236738ea6658d1bbcd9d7a3eac73fd53d4bf0b1cc4600f2d88` |
| `docs/tp4-rank-startup-handshake-proof-v1.md` | `4bbdf757d288d065ddbb75aa090e5f5a757fa1f55abbbe71dd2edd5880984b0d` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-tp4-rank-startup-handshake-v1-handoff.md
cargo test --offline -p glm-engine worker::tests -- --nocapture
cargo clippy --offline -p glm-engine --all-targets -- -D warnings
```

## Review boundary

This review covers only the retained CPU pool's synchronous four-rank
thread-start handshake and partial-start cleanup. It does not accept
construction of device executors on their owner threads, the pending
production factory or startup state machine, a startup watchdog, CUDA
contexts, device identity, weights, graphs, collectives, checkpoint
execution, model output, throughput, or performance.

## Required adversarial questions

1. Did the prior public constructor return immediately after spawning only
   the dispatcher, before that dispatcher attempted any rank-thread spawn?
2. Did a later rank-thread spawn error silently return from `dispatch_loop`,
   detach already-started join handles, and leave the caller with a pool that
   appeared healthy until later channel use?
3. Does the corrected public constructor wait for one dispatcher startup
   result and publish a pool only on `Ok(())`?
4. Does each rank receipt originate inside its successfully spawned rank
   thread before `rank_loop` begins?
5. Does the dispatcher require the exact, nonduplicate ready mask for ranks
   0, 1, 2, and 3 before reporting success?
6. On a rank spawn error, are all stored rank senders dropped before joining
   every partial worker, so cleanup cannot deadlock on `recv`?
7. Are unstarted executors also destroyed before the failed constructor
   returns?
8. Does any panic during partial cleanup fail startup as `WorkerPanic`
   instead of being ignored?
9. If the dispatcher panics or closes before its startup response, does the
   constructor join it and return `WorkerPanic` or `Closed`, never a pool?
10. Does the deterministic rank-2 fault exercise the same internal spawn
    branch used by the public constructor and return `Thread` synchronously?
11. Does the exact drop count of four prove cleanup of ranks 0–1 plus the
    unstarted rank-2/rank-3 executors before return?
12. Do success tests still establish four live persistent rank threads and
    exact rank-set execution after the added barrier?
13. Is the proof explicit that there is no startup deadline and that
    executors are still supplied from outside rather than factory-constructed
    on owner threads?
14. Are the 263-test, 57-handoff, CPU-only boundary, and all exclusions
    accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- the constructor cannot publish before exact four-rank readiness;
- rank-thread spawn failure is synchronously visible;
- partial workers and all executor objects are cleaned before failure returns;
- dispatcher disconnect/panic cannot masquerade as startup success;
- the injected regression distinguishes the prior silent-failure path; and
- the CPU proof and all exclusions are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, pre-readiness publication,
detached worker, incomplete executor destruction, swallowed panic,
nondistinguishing injection, or overstated production claim.

The token accepts only this retained CPU startup correction. It does not open
cn4, authorize CUDA work, or accept the production rank executor.
