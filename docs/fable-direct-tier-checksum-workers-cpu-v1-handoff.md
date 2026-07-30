# Fable handoff: direct-tier fixed checksum workers CPU proof v1

Date: 2026-07-30

Status: adversarial CPU implementation review requested

Review candidate commit:
`59f70da4dbeca8a5d542f3e5947002d3ee975bdb`

Required result path:
`docs/reviews/fable-direct-tier-checksum-workers-cpu-v1.md`

Requested acceptance token, only if every blocker and major is resolved:
`direct-tier-checksum-workers-cpu-v1-accepted`

GPU, host, process, container, network, model, checkpoint, conversion, or
storage authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4, query its state, start or stop a process,
build, test, create a CUDA context, access a checkpoint, convert weights, or
launch work for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if needed. Hash every candidate input at review start and finish.
Any mismatch must withhold the token as stale.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `spec/engine-v0.md` | `277a97c675c5021b1e310146bdf04896ccec9dea312a73a188379e633423e6d8` |
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `docs/direct-tier-state-cpu-proof-v1.md` | `3f58a9c1b7ad7cc4806598b467f02eb746013e75a72e4566f9e9ba55f466df66` |
| `docs/direct-tier-checksum-authority-cpu-v1.md` | `37b84f3d33020f535df0e6cf60123827d7dd04f40871bd8d02b0f42cd1143536` |
| `docs/direct-tier-checksum-workers-cpu-v1.md` | `58cb503b968d5ee5c651ba6dafd2d7b1716b72015cff66202b3c6d68ac78fdeb` |
| `crates/glm-cache/src/direct.rs` | `7514ad205b84f24c2a2f58647c2b50b0f6dab4398dfb04a2fc36186f09a82dd3` |
| `crates/glm-cache/src/direct_state.rs` | `398d47a4ea974241e574d134ca877bc3acedead2b410c71657c89254008657b9` |
| `crates/glm-cache/src/direct_restore.rs` | `fbadcf6c1a1c4ef9a1e55be16e37815bb2aa6df0fa432b8fe254baf64df9bcfe` |
| `crates/glm-cache/src/lib.rs` | `331c724306021969f7f9174589b680ca93077017a3248c678a693358187ba4f4` |
| `crates/glm-cache/Cargo.toml` | `176be2353dcee1c479714247fedf380cd36de29a8390069406e4853250d89e67` |
| `crates/glm-cli/src/main.rs` | `192e8d83f48668100780f2bd80ef0f1a9b578634e2f86d8ae10f06c57f8bb968` |
| `fixtures/direct-tier-state-proof-v1.json` | `58f19d6b506e969c91561938eb45a509ce820d936b9bb4d901c9028a5ca17c75` |
| `fixtures/direct-tier-checksum-authority-proof-v1.json` | `431dc6143dad33bd271e4905ade9bad2149e2cf3be998d68128048b4b17c1fa9` |
| `fixtures/direct-tier-checksum-workers-proof-v1.json` | `6b07cc9497ff02c779fbe2243b0437f9dbfa52e12b6da18b86877fe44f6b715b` |
| `scripts/local-checks.sh` | `803fc7f106f86d1f2b81ddea19d9c40b26a30691ea8406fa84e0f361928e51d2` |
| `docs/production-punchlist.md` | `94816cf7eb75bd9c99cddc3ce3df7840ec73352151a76d15ac7fae2636550e88` |
| `docs/results-index.md` | `69c52f4fcccc09b6abea30c3965801193dfa17e5e61b30672c6ceda5a97bffb7` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-direct-tier-checksum-workers-cpu-v1-handoff.md
git diff --check \
  6e5ebe1ec2ebed3c655620accb7421428280316a^..6e5ebe1ec2ebed3c655620accb7421428280316a
git diff --check \
  6e5ebe1ec2ebed3c655620accb7421428280316a \
  59f70da4dbeca8a5d542f3e5947002d3ee975bdb
cargo test --offline -p glm-cache direct_state --no-fail-fast
cargo test --offline -p glm-cache direct_restore --no-fail-fast
cargo clippy --offline -p glm-cache -p glm-cli --all-targets -- -D warnings
cargo run --release --offline -p glm-cli --bin glmaxx -- \
  direct-tier-checksum-worker-proof \
  /tmp/direct-tier-checksum-workers-proof-v1-release.json
cargo run --offline -p glm-cli --bin glmaxx -- \
  direct-tier-checksum-worker-proof \
  /tmp/direct-tier-checksum-workers-proof-v1-debug.json
cmp fixtures/direct-tier-checksum-workers-proof-v1.json \
  /tmp/direct-tier-checksum-workers-proof-v1-release.json
cmp /tmp/direct-tier-checksum-workers-proof-v1-debug.json \
  /tmp/direct-tier-checksum-workers-proof-v1-release.json
cargo run --release --offline -p glm-cli --bin glmaxx -- \
  direct-tier-checksum-proof \
  /tmp/direct-tier-checksum-authority-proof-v1-release.json
cmp fixtures/direct-tier-checksum-authority-proof-v1.json \
  /tmp/direct-tier-checksum-authority-proof-v1-release.json
```

The first `git diff --check` confirms the candidate's parent metadata
milestone was itself clean; the second isolates the candidate under review.

## Review purpose

Determine whether the candidate replaces synchronous checksum execution with
a fixed, bounded, exact-allocation worker protocol without introducing a
payload copy, ABA hole, mutable-alias race, channel/shutdown deadlock,
capacity leak, partial-worker fallback, corrupt-data publication path, or
false asynchronous-I/O/performance claim.

The machine-accepted `direct-tier-io-v1` design permits fixed checksum
workers after hash capacity is reserved before physical work. This review
does not accept the prerequisite extent/state/checksum candidates wholesale;
it reviews the worker delta against their exact pinned bytes.

## Review boundary

Acceptance covers only:

- the fixed named CPU checksum-worker group and bounded command/result
  channels;
- shared-allocation, generation-bound task/result ownership;
- safe read-destination mutation closure before worker ownership;
- pool-wide failure, corruption, cancellation, and shutdown behavior;
- the focused CPU tests and deterministic proof fixture; and
- preservation of the prior checksum-authority fixture.

Acceptance does not accept or implement `io_uring`, registered memory/files,
filesystem or storage behavior, CPU affinity/NUMA/priority, checksum
performance, fused hashing, CUDA/HBM transfer, model execution, checkpoint
loading, quality, capacity, latency, throughput, serving health, or cn4
evidence. It does not authorize any host access or later implementation.

## Required adversarial questions

1. Do all nineteen candidate-input hashes match at review start and finish in
   a detached worktree?
2. Is the worker group created only with
   `1 <= worker_count <= maximum_hash_jobs`, before active hash work, with no
   per-request thread creation?
3. Are both channels truly bounded to `maximum_hash_jobs`, and does the
   pre-read hash reservation prove that commands, executing work, and
   unacknowledged completions together cannot exceed that bound?
4. Can a completion sender block forever during normal shutdown or `Drop`,
   or does the completion capacity cover every active job while the receiver
   remains alive until all workers join?
5. Does each worker release the shared receiver mutex before hashing, so two
   fixed workers can execute two extents concurrently rather than serializing
   behind the receive lock?
6. Does a task contain only an immutable record `Arc`, a private
   ticket/generation job, and an `Arc`-backed buffer handle—never a copied
   extent `Vec` or payload slice with an invalid lifetime?
7. Is each pool slot's aligned allocation created once and retained across
   generations, with generation increment/zeroing completed before a slot is
   exposed as reserved?
8. Is the authority-captured allocation address independently compared with
   the worker read-guard address and full `DirectBufferId`, and can a
   binding mismatch ever be reported as verified?
9. Is the same-allocation proof substantive rather than a comparison of two
   values derived from one copied payload or one tautological helper?
10. Can safe code obtain or retain mutable access after `READ_SUBMITTED`
    ownership closes, during `DATA_READY`, worker hashing, or before result
    publication?
11. Do the `RwLock`, buffer state machine, and private table ownership prevent
    a data race or mutation between decoder verification and `HOST_READY`?
12. Does every worker run the canonical decoder over exactly
    `physical_length`, preserving physical SHA, piece SHA, piece boundary,
    zero-padding, and capability checks?
13. Are worker results private and revalidated against the exact live ticket,
    full buffer generation, `RUNNING` state, and completion ownership before
    publication?
14. Do target-only and combined target+MTP extents both traverse the worker
    path, and does corrupting the actual destination produce quarantine rather
    than `HOST_READY`?
15. Does cancellation after `DATA_READY` retain the buffer and hash charge
    until the worker result is acknowledged, then reap it without publishing
    to a departed waiter?
16. On a poisoned buffer lock, is the exact ticket failed and quarantined
    without forging an integrity result?
17. Is a worker panic caught per job, reported to the authority, and followed
    by a pool-fatal failure of every reserved, queued, or running checksum
    ticket—including work never dispatched to the failed worker?
18. Do command or completion disconnections also fail every active checksum
    ticket instead of leaking capacity or continuing silently with fewer
    workers?
19. After pool failure, do new reads, dispatch, and polling fail closed until
    explicit shutdown, with no rank- or request-local synchronous fallback?
20. If pool failure occurs while an original/cancel CQE is still outstanding,
    does the ticket retain descriptor/CQ ownership until the normal completion
    path reaps it, while its buffer remains quarantined and its hash charge is
    released exactly once?
21. Does normal shutdown reject any live hash reservation, close the command
    queue, join every worker, and reject restart and post-retirement read
    submission?
22. Can a worker or authority panic during `Drop` cause a reusable in-flight
    allocation, use-after-free, or process hang under the bounded-channel
    arithmetic?
23. Does the proof dispatch two tasks before polling, observe two running
    jobs, normalize only nondeterministic completion order, and validate each
    result's original ticket identity?
24. Does `zero_copy_shared_allocation_verified` derive from worker-produced
    binding evidence for target, MTP, abandonment, and corruption rather than
    from a hard-coded report value?
25. Do the worker panic and queued-work regression genuinely prove pool-wide
    cleanup, zero active hash jobs, two quarantined buffers, and blocked
    dispatch/poll?
26. Are debug and release proof bytes identical to the pinned fixture, and
    does the old checksum-authority fixture remain byte-identical?
27. Does the complete offline gate pass 406 tests plus formatting, Clippy,
    host CUDA-FFI compile, ABI, and fixture checks without accessing cn4?
28. Are every claim and nonclaim in
    `docs/direct-tier-checksum-workers-cpu-v1.md` exact, particularly that
    this is checksum-worker CPU evidence and not zero-copy storage,
    `io_uring`, registered-memory, GPU, or performance evidence?

## Result contract

Write exactly one review artifact at the required path. It must include:

- candidate commit and all nineteen start/end input hashes;
- commands and results;
- a finding table with `BLOCKER`, `MAJOR`, `MINOR`, or `NOTE`;
- an explicit answer to every question above; and
- exactly one bare line containing
  `direct-tier-checksum-workers-cpu-v1-accepted` only if no blocker or major
  remains.

If any input drifts, a command cannot run, a claim is false, or a blocker or
major remains, withhold the token and say precisely why.
