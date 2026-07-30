# Rank residency content identity CPU proof v1 r2

Date: 2026-07-30

Status: corrective CPU implementation candidate; independent rereview required

GPU evidence: none

## Scope

This correction responds to
`docs/reviews/fable-rank-residency-content-identity-v1.md`.

The first review found the content-identity, exact-dedup, MTP-retention, and
all-or-nothing planning behavior sound, but withheld the token because the
required `glm-serving cache::tests` command could fail spuriously when two
parallel tests selected the same temporary store directory. It also found a
latent public plan/commit hazard: a caller could commit an
`NvmeRegistrationPlan` after intervening residency mutation and overwrite
current byte accounting with the plan's stale absolute totals.

R2 fixes both boundaries. It does not replace the existing synchronous
residency oracle with a production HBM/DRAM implementation.

## Deterministic test-store identity

The former helper identity was:

```text
temporary_directory = H-like-format(process_id, realtime_nanoseconds)
```

The host used during the first review exposed a microsecond-quantized
realtime clock. Parallel test threads could therefore observe the same PID
and timestamp, open the same store root, and correctly trip the store's
single-writer lock.

Both affected test modules now include one process-local
`AtomicU64` sequence. The serving helper additionally requires a
human-readable per-test label. The directory components are:

```text
test family
test label
process ID
realtime nanoseconds
strictly unique process-local atomic sequence
```

`fetch_add(Relaxed)` is sufficient because uniqueness, not memory
publication, is the property. Every call in one process receives a distinct
integer even when the clock is unchanged. PID plus wall clock prevents a
normally reused process ID from aliasing an abandoned prior-process test
directory; the atomic sequence closes the parallel same-process collision
that caused the reviewed failure.

The production single-writer lock is unchanged and remains fail-closed.

## Generation-bound registration plans

`ResidencyManager` now owns a monotonically increasing
`state_generation: u64`. It advances exactly once after each successful
mutation of plan-relevant state:

- inserted or upgraded NVMe registration;
- restore begin, abort, or completion;
- DRAM-to-HBM promotion;
- HBM pin; or
- HBM unpin.

Failed validation and exact-dedup/retain-only registration do not advance it.
Overflow fails before mutation.

Every `NvmeRegistrationPlan` captures:

```text
expected_state_generation
planned changed records
planned final HBM bytes
planned final DRAM bytes
```

`commit_nvme_registrations` now returns `Result` and rejects a plan with
`ResidencyError::Stale` unless its expected generation equals the manager's
current generation. The generation check and next-generation overflow check
both occur before any record or counter mutation.

The serving prefix coordinator:

1. constructs all four rank plans;
2. validates every plan generation before any rank commit;
3. commits each plan while it retains exclusive mutable ownership of all
   four managers; and
4. publishes the candidate prefix index only after all four commits.

No other thread or caller can mutate an individual manager between that
preflight and its commit through this ownership boundary. A programming
error still fails as `Stale` rather than silently rewinding counters.

## New distinguishing regression

`stale_registration_plan_cannot_rewind_residency_accounting` executes this
sequence:

1. register page A as NVMe;
2. plan registration of page B while HBM bytes are zero;
3. restore page A into HBM, advancing manager generation and charging one
   page of HBM;
4. attempt to commit the old page-B plan; and
5. require `ResidencyError::Stale`.

It then proves:

- page A remains HBM resident;
- its exact HBM charge remains present;
- DRAM accounting is unchanged;
- page B was not partially inserted; and
- the stale plan cannot overwrite the new absolute counters.

Under the former infallible commit, step 4 inserted page B and reset the
manager's HBM counter to the plan's stale zero snapshot. The new assertion
therefore distinguishes the unsafe prior behavior.

## Reproduced commands

The candidate passes:

```text
cargo fmt --all -- --check
cargo test --offline -p glm-cache residency::tests
cargo test --offline -p glm-serving cache::tests
cargo clippy --offline -p glm-cache -p glm-serving --all-targets -- \
  -D warnings
```

The direct residency filter reports nine passing tests, including the new
stale-plan regression. The exact five-test serving cache filter reports five
passes.

The formerly flaky serving command was then executed fifty consecutive
times with its normal parallel test harness:

```text
for run_index in {1..50}; do
  cargo test --offline --quiet -p glm-serving cache::tests >/dev/null ||
    exit 1
done
```

All fifty invocations completed with exit code zero: 250 cache-test
executions and no `WriterLocked` collision. This is host CPU evidence, not a
probabilistic proof that a filesystem can never fail.

## Retained first-review behavior

The r2 change preserves the behavior the first review independently proved:

- every same-key logical content collision fails before mutation;
- exact dedup and target-candidate-against-MTP retain record, location, pins,
  clocks, and byte counters;
- only a strictly newer, unpinned, non-restoring target-to-MTP upgrade may
  replace;
- duplicate page keys and late multi-record errors abort immutable planning;
  and
- a serving prefix registration publishes neither the index nor any rank
  update until all rank plans have passed.

The new generation binding strengthens the last point; it does not change
the content relation matrix.

## Exclusions

This candidate does not prove:

- a persistent online publisher or the pending namespace-v2 contract;
- a durable-format codec, cleaner, io_uring, registered memory, or NVMe
  device behavior;
- actual HBM/DRAM allocation or copy;
- asynchronous CUDA event ordering;
- four native SM120 rank workers;
- checkpoint or model execution;
- prefix cold/warm model reuse;
- one-million-token model execution;
- quality, capacity, or performance; or
- cn4 authorization.

The accepted token, if issued, covers only the synchronous CPU rank-residency
content identity and stale-plan/test-repeatability correction.

## Required rereview

The rereviewer must verify:

1. candidate hashes at start and finish;
2. same-process temporary-store uniqueness under a quantized clock;
3. unchanged production writer-lock semantics;
4. state generation advances on every plan-relevant successful mutation and
   never after a failed one;
5. overflow and stale generation fail before any record/accounting mutation;
6. retain-only plans remain true no-ops;
7. all four coordinator plans are preflighted before the first commit;
8. exclusive ownership prevents an intervening coordinator mutation;
9. the new regression distinguishes the prior stale-accounting behavior;
10. the exact targeted and fifty-run stress commands reproduce; and
11. no exclusion or production claim is overstated.

Withhold the token for any blocker, major, test-directory collision,
untracked plan-relevant mutation, post-mutation stale check, partial
four-rank commit path, nondistinguishing regression, or evidence
overstatement.
