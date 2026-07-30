# Fable handoff: direct-tier scheduler CPU v1

Date: 2026-07-30

Status: superseded by the r2 correction; do not review and do not issue the
v1 token

Superseding proof:
`docs/direct-tier-scheduler-cpu-proof-v1-r2.md`

Review candidate commit:
`6cdbeae417e053d08751c8102304064bf86c360e`

Required result path:
`docs/reviews/fable-direct-tier-scheduler-cpu-v1.md`

Requested acceptance token, only for an unqualified scoped pass:
`direct-tier-scheduler-cpu-v1-accepted`

GPU or storage authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch GPU, container, storage-device,
or network work for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if needed. Hash every candidate input at review start and finish.
Any mismatch must withhold the token as stale.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `fable-direct-tier-io-v1.md` | `739313fd952b7000a6d9789699a36fa36e5ca35152ec28ae959c6f0ac0932882` |
| `docs/fable-direct-tier-io-v1-handoff.md` | `1c021b284ce4e27bfa0d5ffe1890b92b7515253341a0dccd90cdc4051b2775ac` |
| `docs/direct-tier-state-cpu-proof-v1.md` | `3f58a9c1b7ad7cc4806598b467f02eb746013e75a72e4566f9e9ba55f466df66` |
| `docs/direct-tier-scheduler-cpu-proof-v1.md` | `514b40ccd75352c3b3a243ee63a8781a16c51f95aba87edb4e4f41252f25ed2f` |
| `crates/glm-cache/src/direct_schedule.rs` | `a4a830a00f3a96d710ac2abc92fac4cdf7f2356ce83328b34f1d5544f56e681e` |
| `crates/glm-cache/src/direct_restore.rs` | `73578aa42bf944c37bfe431da21df5c27ad12ee58dfb81f64a24a180830b1c1f` |
| `crates/glm-cache/src/direct_state.rs` | `386c414a302a489c1e8c6fefa6d11b304e86ffa2040ad105e81cc8572999f814` |
| `crates/glm-cache/src/direct.rs` | `7514ad205b84f24c2a2f58647c2b50b0f6dab4398dfb04a2fc36186f09a82dd3` |
| `crates/glm-cache/src/lib.rs` | `6a7c4bae1ec942f6a304c702b4ac5a4b1dc4b86c37c52b15cd4c2ea8c8cf0603` |
| `docs/production-punchlist.md` | `683f1c9574cf97214b6ccc63015634882f4b773c9118be45a0e25c99c080a153` |
| `docs/results-index.md` | `a2431d9ebf0507d4a8d7a3258e1b00bd730e1c44de2a3bcd53eecd6e4993546d` |
| `scripts/local-checks.sh` | `56f728cdf3f047f9633509a57341d25a977efa802f0d5b371c9716830517db59` |
| `Cargo.toml` | `863c28560b339f1fd7fb6b80c1b812e9fa7bc3f8f8c782126d2a29ceeffc06ea` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-direct-tier-scheduler-cpu-v1-handoff.md
cargo test --offline -p glm-cache direct_schedule::tests -- --nocapture
cargo test --offline -p glm-cache
cargo clippy --offline -p glm-cache --all-targets -- -D warnings
git diff --check 6cdbeae417e053d08751c8102304064bf86c360e^ \
  6cdbeae417e053d08751c8102304064bf86c360e
```

The handoff is coordination metadata added after the candidate and is not a
candidate input.

## Review purpose

The accepted direct-tier design explicitly permits pure CPU state-machine
implementation and requires its five minor obligations to be incorporated
into the implementation contract. This candidate implements the missing
scheduling-policy slice:

- fixed R0/R1/W0/W1 class semantics and deterministic in-class ordering;
- preallocated bounded queues and membership tables;
- read-reserved buffer, descriptor, and CQ capacity;
- two CQ entries per admitted W0 lease, independent of NODROP;
- normal W0 admission below the read high watermark;
- a separate bounded new-W0 admission rule under continuous reads;
- a bounded accepted-W0 service rule;
- projected-byte checks that prevent one variable-size read from overshooting
  either available bound; and
- W1 suppression above read or publication low watermarks.

This is not the io_uring authority. `DirectIoResources` is the
single-authority snapshot passed by mutable reference; an admission consumes
one buffer, one descriptor, and two CQ slots in that snapshot. Terminal
resource replenishment and external read-resource ownership are deliberately
absent and cannot be accepted by this gate.

## Review boundary

Acceptance covers only:

- the deterministic bounded CPU scheduler policy;
- its configuration, ordering, resource-reserve, fairness, and fail-closed
  arithmetic;
- construction-time collection reservation; and
- the named CPU tests.

Acceptance does not cover:

- an io_uring authority, ring, SQE, CQE, registered file/buffer, direct I/O,
  fsync, async cancel, or eventfd;
- production resource ownership or terminal W0 replenishment;
- scheduled-command cancellation;
- the durable codec, journal, catalog, checkpoint, restart, or cleaner;
- online prefix publication;
- HBM/DRAM transfer, CUDA, KV reconstruction, attention, or a model;
- target storage behavior, decode isolation, capacity, latency, throughput,
  or health; or
- cn4 access.

## Required adversarial questions

1. Do all sixteen candidate-input hashes match at review start and finish in
   a detached worktree?
2. Is the accepted direct-tier design review itself provenance-complete and
   does it explicitly permit this pure CPU implementation scope?
3. Does each class have the exact intended meaning, and is selection within
   a class controlled only by
   `(service_epoch, owner_id, page_ordinal, operation_ordinal)`?
4. Can randomized hash-table state affect service order, or are hash tables
   used strictly for duplicate membership while preordered heaps decide?
5. Are zero values, duplicate IDs, duplicate in-class order keys, queue
   overflow, queued-byte overflow, invalid class entry, and invalid resource
   snapshots rejected without partial insertion or decision?
6. Is construction fallible, capped at 65,536 commands, and preallocated so
   successful insertion through the configured bound cannot grow any of the
   seven collections?
7. Can a caller forge an already-accepted W0 item through `enqueue_ready`, or
   must every new publication traverse `offer_publication` and an admission
   decision?
8. Does normal W0 admission stop above the high watermark until either the
   admission byte bound is reached or the next read would cross it?
9. Under continuous R0/R1 arrivals, is a resource-available W0 admitted no
   later than its exact configured byte bound, including irregular command
   sizes and u64 overflow?
10. When only read-reserved resources remain, does W0 wait without consuming
    them, retain saturated admission debt, and win before the next read as
    soon as one shared slot is visible?
11. Does admission reserve exactly one buffer, one descriptor, and two CQ
    entries while preserving the configured read reserves in every resource
    boundary combination?
12. Can one shared resource slot admit two publications without an
    authoritative replenishment, or does the mutated snapshot prevent it?
13. After admission, does the independent service counter begin at zero and
    force W0 service before a variable-size read would exceed its bound?
14. Do R0 and then R1 retain priority before the service bound, without
    completion order, rank, or hash iteration changing the selected command?
15. Is W1 selected only at or below both configured low watermarks and never
    given a starvation guarantee?
16. Do focused tests exhaust all 1,377 resource snapshots, every projected
    byte pair through the frozen boundary, continuous arrivals, irregular
    reads, exact reserve use, capacity stability, and final zero accounting?
17. Does the API/proof accurately expose the remaining trust boundary:
    caller-owned authoritative resource refresh and no terminal
    replenishment implementation?
18. Are the 113-crate/394-workspace test claims, 108-handoff provenance
    claim, tokenizer/CUDA skips, and every non-claim exact?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately whether:

1. deterministic class and in-class ordering are exact;
2. all queue/configuration failures are atomic and bounded;
3. new-W0 admission cannot starve while shared resources exist;
4. accepted-W0 service cannot exceed its independent read-byte bound;
5. read-reserved buffer/descriptor/CQ capacity cannot be consumed by W0;
6. cleaner isolation is exact;
7. tests distinguish threshold, resource, overflow, and allocation defects;
8. the external resource-authority boundary remains fail-closed and
   unaccepted; and
9. no syscall, storage, CUDA, model, capacity, or performance evidence is
   implied.

Only if all eighteen questions and all nine statements are unqualified
`YES`, end with:

```text
direct-tier-scheduler-cpu-v1-accepted
```

Withhold for stale provenance, nondeterministic ordering, an admission or
service overshoot, read-reserve consumption, a stale-snapshot double
admission, unbounded allocation, W1 interference, a nondistinguishing test,
acceptance of the missing production authority, or any storage/GPU/model/
performance overstatement.
