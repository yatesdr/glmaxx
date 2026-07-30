# TP4 worker admission-state CPU proof v1

Date: 2026-07-30

Status: corrective CPU implementation candidate; independent review required

GPU evidence: none

## Scope

The accepted TP4 step-operation quota review confirmed that an uncloneable
permit correctly owns each queued/running physical four-rank operation. It
also reported two release-build robustness gaps:

1. impossible ordinary-permit underflow was non-wrapping but observable only
   through a debug assertion; and
2. after the dispatcher failed closed, the pool could still reserve a slot
   before discovering that its command channel was disconnected.

The current worker also uses one exclusive sentinel for page-table,
checkpoint-identity, weight-load, and weight-shutdown transactions. That
sentinel previously occupied `usize::MAX` without constraining the public
ordinary maximum below it.

This correction replaces the overloaded counter with one explicit atomic
admission-state word. It does not change rank execution, collectives,
checkpoint bytes, page deltas, model output, or GPU behavior.

## State word

The state word is partitioned as:

```text
highest bit       POISONED
next bit          CLOSED
next bit          EXCLUSIVE
remaining bits    ordinary queued/running operation count
```

The configured ordinary maximum must be positive and fit entirely within
the count field. Therefore no legal count can alias a flag.

The permitted healthy states are:

```text
count = 0..maximum, no flags
EXCLUSIVE, count = 0
CLOSED with a draining ordinary count
CLOSED, count = 0
```

`POISONED` may coexist with retained evidence of the state in which an
internal ownership invariant failed. No admission path clears either
`POISONED` or `CLOSED`.

## Ordinary operation ownership

Ordinary reservation uses one atomic `fetch_update` and succeeds only when:

- no flag is set; and
- count is below the configured maximum.

Failure reports:

- `Poisoned` if poison is present;
- `Closed` if the dispatcher has retired; or
- `Saturated` for an active exclusive transaction or full ordinary quota.

`OutstandingPermit::drop` atomically:

- decrements a positive count while preserving all flags; or
- leaves count zero and sets `POISONED`.

Impossible double release is consequently visible in optimized builds,
cannot wrap, and blocks both ordinary and exclusive admission.

## Exclusive operation ownership

Exclusive reservation is a compare-exchange from exact zero to
`EXCLUSIVE`. A count, another exclusive owner, poison, or closure rejects
the reservation.

`ExclusivePermit::drop` accepts exactly one owned-exclusive state with no
ordinary count and no poison. It clears `EXCLUSIVE` while preserving
`CLOSED`, because a terminal exclusive operation publishes closure before
releasing its permit. Any other release state clears the exclusive bit,
sets `POISONED`, preserves other evidence, and permanently blocks admission.

`outstanding()` retains the previous public behavior: it reports the
configured maximum while an exclusive transaction owns the pool and the
exact count otherwise. `quota_poisoned()` and `is_closed()` expose the two
terminal reasons separately.

## Closure publication ordering

Every published-pool terminal path now sets `CLOSED` before:

1. releasing its ordinary or exclusive permit; and
2. sending the terminal result to the caller.

This covers:

- failed page-table initialization;
- failed page-delta application;
- failed checkpoint-device identity collection;
- failed weight load;
- successful or failed terminal weight shutdown; and
- any failed rank step, output validation, or consensus operation.

A dispatcher-lifetime guard sets `CLOSED` on every other return or unwind,
including receiver shutdown and startup cleanup. Startup failure still
returns no pool, so no caller can admit through that transient.

Because channel delivery occurs after the release-store/RMW that sets
closure, a caller that receives a terminal result must observe `Closed` on
subsequent admission. A race that reserved before the terminal decision
remains an already-owned physical operation and is drained by receiver
shutdown; no slot or command is lost.

## Distinguishing regressions

`step_quota_underflow_is_release_visible_and_blocks_all_admission`:

- drops an ordinary permit against zero;
- proves count remains zero and poison becomes visible; and
- requires both ordinary and exclusive reservation to return `Poisoned`.

`lost_exclusive_ownership_poison_is_release_visible`:

- drops an exclusive permit without the exclusive state;
- proves no exclusive/count alias remains;
- proves poison is visible; and
- requires both admission classes to fail as `Poisoned`.

`worker_quota_rejects_counts_that_overlap_state_flags` proves configuration
fails before channel or thread construction when the requested count reaches
the first reserved flag bit.

`one_rank_backend_failure_aborts_the_whole_step` now also proves:

- the exact terminal rank error reaches the caller;
- `CLOSED` was published before that receive returns;
- no poison was fabricated by normal terminal cleanup;
- outstanding count is zero; and
- the next step returns `Closed` without reserving a slot.

`successful_weight_shutdown_requires_four_exact_acks_and_retires_generation`
proves the normal exclusive terminal path publishes `CLOSED`, clears
exclusive occupancy to zero, does not fabricate poison, and rejects later
ordinary admission as `Closed`.

`queue_is_bounded_while_the_physical_step_is_active` uses a five-party
barrier to prove the second step is rejected as `Saturated` only while all
four ranks are known to be inside the first physical operation.

`rank_divergence_fails_the_step_and_closes_the_generation` separately proves
an exact consensus failure, closure-before-response, zero fabricated poison,
and a subsequent `Closed` admission result.

The prior implementation fails the first two poison regressions and has no
stable closed-state assertion for the last regression.

## Reproduced commands

The candidate passes:

```text
cargo fmt --all -- --check
cargo test --offline -p glm-engine worker::tests
cargo clippy --offline -p glm-engine --all-targets -- -D warnings
```

The exact worker filter reports 26 passing tests.

Pre-freeze failure-capturing stress found two timing assumptions in retained
tests:

1. the old combined queue/divergence test could finish the divergent step and
   correctly publish `Closed` before it asserted that a second submission
   must return `Saturated`; and
2. the phase-timeout test required exactly three cleanup acknowledgements
   within a 5 ms deadline even though the contract returns the exact partial
   set observed by that deadline.

The first test is now split into the barrier-backed bounded-queue test and the
independent closure test described above. The timeout test now requires the
delayed rank acknowledgement to be absent and the returned set to be
incomplete. Non-timeout cleanup tests continue to require all four exact
acknowledgements.

After both corrections, the failure-capturing filter passed 500 consecutive
fresh test-process invocations. The independent reviewer must repeat at least
100:

```text
for run_index in {1..100}; do
  test_output=$(cargo test --offline --quiet -p glm-engine worker::tests 2>&1) ||
    {
      printf 'failed_run=%s\n%s\n' "$run_index" "$test_output"
      exit 1
    }
done
```

No claim is made that the operating system or hardware can never fail. The
repeat gate now avoids asserting an ordering or completion count that the
worker contract does not promise.

## Retained behavior

This correction preserves:

- one permit per queued/running ordinary TP4 operation;
- handle receive/timeout/drop independence from operation lifetime;
- exclusive ownership for page-table/checkpoint/weight transactions;
- the bounded synchronous dispatch channel;
- permit release before result visibility;
- fail-stop rank/consensus behavior;
- exact rank receipts and cleanup semantics; and
- all existing page-table, checkpoint, output, and quota tests.

## Exclusions

This candidate does not prove or implement:

- the pending production SM120 executor ABI;
- CUDA contexts, graphs, kernels, collectives, HBM/KV arenas, or events;
- asynchronous step deadlines or process supervision;
- checkpoint or model execution on a device;
- MTP recurrence, distributed sampling, attention, logits, quality,
  capacity, or performance; or
- cn4 authorization.

## Required rereview

The reviewer must verify:

1. candidate hashes at review start and finish;
2. flag/count disjointness on both 32-bit and 64-bit `usize`;
3. every legal count boundary and the configuration rejection boundary;
4. ordinary reserve/release linearizability under closure and poison;
5. exclusive reserve/release linearizability while closure is published;
6. underflow/mismatched release cannot wrap, clear a terminal flag, or admit
   work;
7. a normal terminal exclusive release preserves closure without fabricating
   poison;
8. `outstanding()` retains its former exclusive-occupancy behavior;
9. all published-pool terminal paths set closure before permit release and
   response visibility;
10. dispatcher return/unwind closes every otherwise-uncovered path;
11. queued ordinary commands drain their permits after a terminal failure;
12. error mapping distinguishes poison, closure, and saturation for step and
    weight admission;
13. each new regression distinguishes the prior behavior;
14. the exact targeted, warnings-denied Clippy, and 100-run
    failure-capturing commands reproduce; and
15. all retained-behavior statements and exclusions are accurate.

Withhold acceptance for flag/count aliasing, lost or double release,
poison/closure clearing, response-before-closure, fabricated poison on a
normal terminal path, incorrect error mapping, a nondistinguishing
regression, repeat failure, or any GPU/model overstatement.
