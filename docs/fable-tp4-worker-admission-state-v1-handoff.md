# Fable handoff: TP4 worker admission state v1

Date: 2026-07-30

Status: adversarial CPU implementation review requested

Review candidate commit:
`46f251e8af7d0b75593c7ad66c00ae41dcd3f7a8`

Required result path:
`docs/reviews/fable-tp4-worker-admission-state-v1.md`

Requested acceptance token, only for an unqualified pass:
`tp4-worker-admission-state-v1-accepted`

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch GPU, container, storage-device,
or network work for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if needed. Hash every input at review start and finish. Any mismatch
must withhold the token as a stale candidate.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-engine/src/worker.rs` | `3533f606400c8aa5c571caa360ba516abd69d39de0489b87be4658143a9bdc24` |
| `docs/tp4-worker-admission-state-proof-v1.md` | `0d64d16208affa012497e89c1628748445b539176508b3e826a86f7efbfa20e9` |
| `docs/tp4-step-operation-quota-proof-v1.md` | `ab5e025afe5f4c236738ea6658d1bbcd9d7a3eac73fd53d4bf0b1cc4600f2d88` |
| `docs/tp4-rank-startup-handshake-proof-v1.md` | `4bbdf757d288d065ddbb75aa090e5f5a757fa1f55abbbe71dd2edd5880984b0d` |
| `docs/sm120-rank-executor-v1-r2.md` | `4f40ea7652858b4cebbe4093dc81149cb30aa26bedc69edef72fa627c987df89` |
| `docs/production-punchlist.md` | `822f7748aeccc7caca9bbc8bd00115761b3745383aa316abbbb47abb170477df` |
| `scripts/local-checks.sh` | `56f728cdf3f047f9633509a57341d25a977efa802f0d5b371c9716830517db59` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-tp4-worker-admission-state-v1-handoff.md
git diff --check 46f251e8af7d0b75593c7ad66c00ae41dcd3f7a8^ \
  46f251e8af7d0b75593c7ad66c00ae41dcd3f7a8
cargo fmt --all -- --check
cargo test --offline -p glm-engine worker::tests
cargo clippy --offline --workspace --all-targets -- -D warnings
```

Then run the failure-capturing repeated filter:

```text
for run_index in {1..100}; do
  test_output=$(cargo test --offline --quiet -p glm-engine worker::tests 2>&1) ||
    {
      printf 'failed_run=%s\n%s\n' "$run_index" "$test_output"
      exit 1
    }
done
```

The handoff itself is coordination metadata added after the candidate and is
not a candidate input.

## Prior verdict and correction

The accepted TP4 step-quota review found the operation-owned permit sound. It
reported two MINORs:

1. impossible underflow was silent in optimized builds; and
2. after fail-stop dispatch termination, callers could still reserve a slot
   before channel disconnection returned `Closed`.

The candidate makes count, exclusive, closed, and poison state disjoint in
one atomic word. Ordinary underflow or lost exclusive ownership sets
release-visible poison and blocks all future admission. Every
published-pool terminal path sets closure before permit release and terminal
result visibility. Normal exclusive shutdown preserves closure without
fabricating poison.

The implementation pass also found that the old public maximum could overlap
the `usize::MAX` exclusive sentinel. The candidate reserves explicit flag
bits and rejects a maximum reaching the first flag.

Pre-freeze stress captured two invalid retained-test assumptions: a divergent
step could close before the test's racy saturation assertion, and a 5 ms
timeout does not promise exactly three cleanup acknowledgements. The final
candidate splits bounded-queue and divergence closure into deterministic
tests and validates the exact incomplete timeout receipt rather than a
scheduler-dependent count. The corrected 26-test filter passed 500
consecutive fresh-process invocations. If any required reviewer repeat fails,
retain its complete output and withhold the token.

## Review boundary

Acceptance covers only:

- CPU TP4 ordinary-operation quota ownership;
- CPU exclusive page/checkpoint/weight transaction ownership;
- release-visible admission poison;
- stable fail-stop closure and result ordering;
- count/flag configuration boundaries; and
- retained worker error mapping and tests.

Acceptance does not accept:

- the pending production SM120 executor design or native ABI;
- CUDA contexts, graphs, kernels, collectives, memory, KV, or events;
- model/checkpoint execution on a device;
- MTP recurrence, attention, sampling, logits, quality, capacity, or
  performance;
- C05 as passing; or
- cn4 access.

## Required adversarial questions

1. Do all eight input hashes match at review start and finish in a detached
   worktree?
2. On 32-bit and 64-bit `usize`, are poison, closed, exclusive, and count
   fields disjoint and nonzero?
3. Can any accepted `maximum_outstanding` or legal increment alias a flag or
   overflow?
4. Does ordinary reservation use one linearizable update and reject every
   flag plus saturation without mutation?
5. Does positive-count ordinary release decrement once while preserving
   flags?
6. Does zero-count ordinary release preserve count zero, set poison, and
   block ordinary and exclusive admission?
7. Does exclusive reservation require exact zero and reject every count or
   terminal flag?
8. Does a valid exclusive release clear only exclusivity while preserving a
   closure published by the operation?
9. Does invalid exclusive release clear the stale exclusive marker, preserve
   other evidence, set poison, and block both admission classes?
10. Can any public or private path clear poison or closure?
11. Does `outstanding()` report exact ordinary count and preserve the former
    configured-maximum report during exclusive ownership?
12. Does error priority report `Poisoned` before `Closed`, `Closed` before
    saturation, and map all three exactly into weight admission?
13. Does every failed published-pool command set closure before permit drop
    and response send?
14. Does successful terminal weight shutdown do the same without poison?
15. Does the lifetime guard close receiver return, startup cleanup, and
    unwind paths not covered by command branches?
16. Can queued ordinary commands after a terminal failure leak a permit or
    become executable after closure?
17. Do channel send failure, dropped response handles, and dispatcher unwind
    still release every owned permit exactly once?
18. Do the ordinary-underflow, exclusive-mismatch, flag-boundary, regular
    fail-stop, and normal exclusive-shutdown regressions each distinguish
    their prior unsafe or unobservable behavior?
19. Do the exact tests report 26 passes, and do formatting and workspace
    warnings-denied Clippy pass?
20. Does the 100-run failure-capturing loop complete without one failure?
21. Are all retained behavior statements and CPU/GPU exclusions accurate?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately:

1. ordinary and exclusive ownership states are disjoint and linearizable;
2. impossible release corruption is visible, non-wrapping, and fail-closed;
3. terminal closure precedes result visibility and later admission;
4. normal closure never fabricates poison;
5. all regressions and the repeat gate distinguish the prior gaps; and
6. scope and exclusions are accurate.

Only if all twenty-one questions and all six statements are unqualified
`YES`, end with:

```text
tp4-worker-admission-state-v1-accepted
```

Withhold for stale provenance, flag/count aliasing, wrap, lost/double release,
poison/closure clearing, response-before-closure, false poison, leaked queued
permits, incorrect error mapping, any repeat failure, nondistinguishing tests,
or any GPU/model overstatement.
