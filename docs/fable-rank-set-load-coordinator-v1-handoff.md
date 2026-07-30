# Fable handoff: rank-set load coordinator v1

Date: 2026-07-30

Status: adversarial implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`51e8dc185d328f6fb9cda84ec5e75de14e756776`

Required result path:
`docs/reviews/fable-rank-set-load-coordinator-v1.md`

Requested acceptance token, only if every blocker and major is resolved:
`rank-set-load-coordinator-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `crates/glm-engine/src/checkpoint_load.rs` | `f674cad6d575fc4bed74730a9d1ffb0dd5d64e21afaa4cf92bf4fc04b54d375b` |
| `crates/glm-engine/src/lib.rs` | `06f668fa567057c895e29bd97515c58588347caac1ffa10c824bd6745ada867b` |
| `docs/rank-set-load-coordinator-proof-v1.md` | `4076872eec5adcdb2f4a0445418ed58d695a188a1d8aeb4e95496ef7ec52196a` |
| `docs/native-rank-load-plan-proof-v1.md` | `19558f87ef912c8ead99c31cf0f1a1867dcc384ab79d8efbbee96f66abfe0e63` |
| `docs/checkpoint-load-cpu-core-proof-v1.md` | `a3cbd93be0b7f131d98d996601c75e653764ec429839f19e2c26835fa4bd20c1` |
| `docs/checkpoint-load-transaction-v1.md` | `79d9c376201f3540f247344c24c37dd7d819d629f10459899a73c15f8b27015f` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/production-punchlist.md` | `7ce52428dcd2fe5857f8479e5449841bed21b82a70e143ab937f00ed674d8ca2` |
| `docs/results-index.md` | `e616eebf26289a634d2e953fa8b05c8ff5cd40933bc1db0f2ea04cdcacb76f5e` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-rank-set-load-coordinator-v1-handoff.md
cargo test --offline -p glm-engine checkpoint_load
cargo clippy --offline -p glm-engine --all-targets -- -D warnings
```

## Review boundary

This review covers only the process-wide receipt/adoption/failure state
machine and its CPU fault matrix. It assumes the plan, receipt, lifecycle,
and native-reader boundaries reviewed by their separate handoffs; it does not
accept those candidates by implication.

It does not accept rank-thread transport, timeouts, physical cleanup
acknowledgements, a native-rank stream, CUDA resources, a checkpoint smoke,
production health, SM120 execution, capacity, quality, or performance.

## Required adversarial questions

1. Is there any transition from `Preparing` to `Adopting` before exactly one
   valid prepared receipt from each rank zero through three?
2. Does each receipt have to match the coordinator's plan and the owner
   allocation generation registered at attempt construction? Can a valid
   receipt from an older allocation generation be replayed?
3. Is there any transition from `Adopting` to `Adopted` before exactly one
   acknowledgement from each rank, all binding the same plan, prepared-set
   digest, rank, and registered generation?
4. Can an execution permit exist from this flow before the final
   `AdoptedRankSetReceipt`, including when some rank-local lifecycles have
   already moved to `Adopted`?
5. Does every rank-reported preparation or adoption failure enter one
   terminal process-wide `Aborted` state? Does the same common abort command
   bind both the plan and nonzero load-attempt generation?
6. Are duplicate receipts, duplicate acknowledgements, malformed ranks,
   stale generations, phase violations, zero generations, and invalid
   receipts all terminal rather than ignored or retried rank-locally?
7. On abort, are all retained prepared receipts, prepared-set state,
   acknowledgements, and any adopted receipt erased? Can any later message
   reopen or complete the attempt?
8. If a failure arrives after the coordinator has reached `Adopted`, does it
   revoke coordinator completion and emit abort rather than preserve a
   success result? Is process termination still required because an already
   issued permit cannot be revoked?
9. Independently walk the four preparation-failure and four
   adoption-failure tests. Do they cover failure before any receipt, after
   partial preparation, after partial rank-local adoption, and at the final
   rank?
10. Do the lifecycle tests prove exactly-once cleanup obligations for every
    allocated/staging/prepared/adopted mock state without claiming that
    physical synchronize/free has occurred?
11. Is the coordinator suitable for one central process-common route, with
    no rank-local fallback or profile substitution? What exact binding must
    the future persistent-rank command channel add for the attempt
    generation?
12. Are the proof's 318-test exact-candidate claim and all exclusions
    accurate, especially the missing rank threads, timeout, post-sync cleanup
    acknowledgement, full files, CUDA, checkpoint, GPU, capacity, quality,
    and performance evidence?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- preparation requires four exact current-generation receipts;
- adoption requires four exact current-generation acknowledgements;
- execution permits cannot precede global completion;
- every rank and protocol failure selects one process-wide abort;
- aborted coordinator state is terminal and nonreusable;
- stale, duplicate, and malformed messages fail closed;
- the exhaustive four-rank CPU fault matrix is sufficient for this boundary;
  and
- proof claims and exclusions are accurate.

Only if all eight answers are unqualified `YES`, end with the requested
token. Withhold it for a stale candidate, replayable receipt, partial
preparation/adoption success, early permit, rank-local failure route,
reopenable abort, ignored duplicate, missing failure position, cleanup
overclaim, or evidence overstatement.

The token accepts only this CPU coordinator boundary. It does not accept
physical cleanup, open cn4, authorize CUDA work, or accept a checkpoint
smoke.
