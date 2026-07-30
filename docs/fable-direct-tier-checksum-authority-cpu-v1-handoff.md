# Fable handoff: direct-tier checksum authority CPU proof v1

Date: 2026-07-30

Status: adversarial CPU implementation review requested

Review candidate commit:
`7267b505fd4b83c9b421e5050277bf806a1e4867`

Required result path:
`docs/reviews/fable-direct-tier-checksum-authority-cpu-v1.md`

Requested acceptance token, only if every blocker and major is resolved:
`direct-tier-checksum-authority-cpu-v1-accepted`

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
| `crates/glm-cache/src/direct.rs` | `7514ad205b84f24c2a2f58647c2b50b0f6dab4398dfb04a2fc36186f09a82dd3` |
| `crates/glm-cache/src/direct_state.rs` | `386c414a302a489c1e8c6fefa6d11b304e86ffa2040ad105e81cc8572999f814` |
| `crates/glm-cache/src/direct_restore.rs` | `ff4cb553a3c39b26f688640b1f935d2836360c10d8422b45c18a6114394e40e6` |
| `crates/glm-cache/src/lib.rs` | `331c724306021969f7f9174589b680ca93077017a3248c678a693358187ba4f4` |
| `crates/glm-cache/Cargo.toml` | `176be2353dcee1c479714247fedf380cd36de29a8390069406e4853250d89e67` |
| `crates/glm-cli/src/main.rs` | `9c53fc4830b2b207cc77edd395e01c5f377d6221c7a0c24620a2c3d6f3b8360e` |
| `fixtures/direct-tier-state-proof-v1.json` | `58f19d6b506e969c91561938eb45a509ce820d936b9bb4d901c9028a5ca17c75` |
| `fixtures/direct-tier-checksum-authority-proof-v1.json` | `431dc6143dad33bd271e4905ade9bad2149e2cf3be998d68128048b4b17c1fa9` |
| `scripts/local-checks.sh` | `5f3d0c859a63a9d63a025e6f70779900a9c757a00b1777758e523baa4be69e01` |
| `docs/production-punchlist.md` | `46381903944ae236b647bb5303df3984178d92739b241ad059756c7f9cf154fe` |
| `docs/results-index.md` | `f09f17387cd9dbdf0f4ec60d05b1ec5e124316930333fdcb11a60880285bda30` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-direct-tier-checksum-authority-cpu-v1-handoff.md
git diff --check \
  1aa6c46ffd59407c9b43118169da4caf6132b0bd^..1aa6c46ffd59407c9b43118169da4caf6132b0bd
git diff --check \
  1aa6c46ffd59407c9b43118169da4caf6132b0bd \
  7267b505fd4b83c9b421e5050277bf806a1e4867
cargo test --offline -p glm-cache direct_restore --no-fail-fast
cargo clippy --offline -p glm-cache --all-targets -- -D warnings
cargo run --release --offline -p glm-cli --bin glmaxx -- \
  direct-tier-checksum-proof \
  /tmp/direct-tier-checksum-authority-proof-v1-release.json
cargo run --offline -p glm-cli --bin glmaxx -- \
  direct-tier-checksum-proof \
  /tmp/direct-tier-checksum-authority-proof-v1-debug.json
cmp fixtures/direct-tier-checksum-authority-proof-v1.json \
  /tmp/direct-tier-checksum-authority-proof-v1-release.json
cmp /tmp/direct-tier-checksum-authority-proof-v1-debug.json \
  /tmp/direct-tier-checksum-authority-proof-v1-release.json
cargo run --release --offline -p glm-cli --bin glmaxx -- \
  direct-tier-state-proof /tmp/direct-tier-state-proof-v1-release.json
cmp fixtures/direct-tier-state-proof-v1.json \
  /tmp/direct-tier-state-proof-v1-release.json
```

The first `git diff --check` confirms the handoff's parent baseline was
itself clean; the second isolates the candidate under review.

## Review purpose

Determine whether the candidate closes the unsafe caller-supplied checksum
boolean without introducing an unbounded queue, read-before-capacity race,
generation/ABA hole, cancellation leak, corrupt-data publication path, or
false concurrent-I/O/performance claim.

The machine-accepted `direct-tier-io-v1` design explicitly permits this
direct-format and pure CPU state-machine implementation. Its required rule is
that a full checksum queue returns WAIT before work starts, results carry the
exact buffer generation, and only the authority may transition verified
bytes to `HOST_READY`.

## Review boundary

Acceptance covers only:

- the bounded checksum reservation and queue state added to
  `DirectRestoreTable`;
- the generation-bound `DirectHashJob`/`DirectHashResult` safe API;
- real direct-extent physical/piece/padding verification before
  `HOST_READY`;
- exact release/quarantine/cancellation accounting for that new state;
- the focused CPU tests and deterministic proof fixture; and
- preservation of the prior state-proof output.

Acceptance does not accept or implement the separately pending extent/state
CPU gates in their entirety, checksum worker threads, an authority thread,
io_uring, registered memory, filesystem/storage behavior, CUDA/HBM transfer,
the Linux probe, durable publication/recovery, model execution, capacity,
quality, latency, throughput, serving health, or cn4 evidence. It does not
authorize any host access or later implementation.

## Required adversarial questions

1. Do all seventeen candidate-input hashes match at review start and finish
   in a detached worktree?
2. Does `maximum_hash_jobs` reject zero, reject values larger than the fixed
   buffer pool, and remain represented in the global invariants?
3. Is one hash slot reserved before descriptor allocation, CQ submission,
   or `READ_INFLIGHT`, so `HashWait` occurs before physical work starts?
4. On `HashWait`, does the exact ticket remain `BUFFER_RESERVED` with no
   descriptor, CQE, buffer-state, or hash-accounting mutation?
5. Are descriptor, CQ, and buffer failures after capacity checking rolled
   back without leaking or consuming the reserved hash slot?
6. Does the hash charge cover the complete
   `RESERVED -> QUEUED -> RUNNING` lifetime and release exactly once on
   success, abandonment, read failure, or integrity failure?
7. Can invariant validation independently rederive every active hash charge
   and reject every impossible restore/hash/buffer state combination?
8. Is the bounded queue deterministic without an unbounded side channel, and
   does it select only a completed, noncancel-pending exact read?
9. Does `DirectHashJob` bind both the ticket generation and full
   `DirectBufferId { slot, generation }`?
10. Are `DirectHashResult` fields private so safe callers cannot replace
    cryptographic verification with a boolean success assertion?
11. Does `run_hash_job` revalidate RUNNING ownership and the exact buffer
    generation before reading exactly `physical_length` bytes?
12. Does it call the canonical direct-extent decoder, thereby checking the
    physical SHA, every piece SHA, every piece boundary, capability, and all
    mandatory zero padding?
13. Is there any safe-API mutation path between verification and publication,
    or any way to publish a result for a different/stale ticket or buffer?
14. Do duplicate/replayed results, wrong generations, wrong states, missing
    tickets, and jobs after delivery fail closed without changing ownership?
15. Does one corrupted physical byte produce an integrity error, quarantine
    the buffer, clear logical/physical/hash accounting, and never reach
    `HOST_READY`?
16. Is it safe to discard an exact completion for an already-abandoned read
    without hashing because no waiter can consume the bytes, while
    cancellation after `DATA_READY` still waits for hash acknowledgement?
17. Do original/cancel completion orders, read failure, integrity failure,
    and cancellation leave no hash job or reusable in-flight buffer behind?
18. Does `read_destination_mut` expose only the record's physical extent and
    only during the exact submitted-read ownership window?
19. Does the one-slot/two-read proof genuinely exercise atomic backpressure
    both while the first job is reserved and while it is running?
20. Does the proof genuinely hash buffer bytes rather than compare an
    expected boolean, and does the corrupt case mutate the actual destination
    buffer?
21. Are debug and release checksum-proof bytes identical to the canonical
    fixture, and is the previously pinned state-proof fixture unchanged?
22. Do the focused tests and Clippy command pass from the detached candidate
    with no network, GPU, model, or checkpoint dependency?
23. Do the documentation, punchlist, result index, and proof JSON avoid
    claiming worker concurrency, io_uring/storage qualification, CUDA/model
    execution, capacity, or performance?

## Token rule

Write the requested token as one exact bare line only if every required
question is an unqualified YES and no blocker or major remains. Record every
input hash and the candidate commit in the result. If any answer is NO,
qualified, stale, untested, or outside the candidate's evidence, withhold the
token and state the smallest corrective scope.
