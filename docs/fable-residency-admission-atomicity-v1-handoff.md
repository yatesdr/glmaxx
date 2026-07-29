# Fable handoff: HBM residency admission atomicity v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`c84da2a4686c37227de5a0dd4694409fdf42f25b`

Required result path:
`fable-residency-admission-atomicity-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`residency-admission-atomicity-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/residency.rs` | `cd15cbbcf1031adb1fc73e5416fbf5d5149ff87096f193c8ad1b0709417f9629` |
| `crates/glm-cache/src/tier.rs` | `c31b07d7f9054f3d51bc5d24c2c414b6c9a134d88f042502bc0f82e29cad500f` |
| `crates/glm-cli/src/cache_proof.rs` | `3371395bb723d2ec092c16cfd28bcb25b54ca1e38fc2096dff471941b2ac9358` |
| `fixtures/cache-lifecycle-proof-v1.json` | `8d75a281e127f669f52065c7ca2fa0945a4d090e3624f17f857410122dde0dfc` |
| `docs/cache-lifecycle-proof-v1.md` | `11ad4936fea7cd0887e660911f50778d5b0918c21a6cebaca1a98a244b2e2de1` |
| `docs/residency-admission-atomicity-proof-v1.md` | `2412d03f3f1f91cf4bfa12556281b962792da5752805ec1d58c467b385908e97` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-residency-admission-atomicity-v1-handoff.md
cargo test --offline -p glm-cache
cargo test --offline -p glm-cli
cargo test --offline -p glm-serving
cargo clippy --offline -p glm-cache --all-targets -- -D warnings
```

## Review boundary

This review covers only the CPU residency manager's HBM admission planning,
victim placement, counters, pin handling, and deterministic lifecycle proof.
It does not accept the pending direct-I/O design, real HBM/DRAM/NVMe
transfers, io_uring, CUDA, prefix publication transactions, checkpoint
execution, or performance.

## Required adversarial questions

1. In the prior incremental `make_hbm_room` path, could one or more eligible
   pages be demoted before a later `ResidencyError::Pinned` made the
   admission fail?
2. Is `plan_hbm_admission` free of state mutation on every path, including
   pinned-capacity failure, incoming-byte overflow, victim-byte failure,
   counter underflow, and DRAM-counter overflow?
3. Does the planner select the exact deterministic LRU order
   `(last_touch, page_key)`, exclude the target page, and exclude every
   pinned HBM page?
4. Does the planner compute each DRAM/NVMe destination and the final HBM and
   DRAM counters before either the target or any victim changes state?
5. After a successful plan, can any fallible operation occur between target
   mutation and `apply_hbm_admission`, leaving a partial commit?
6. On failed restore admission, are the target's `Restoring` state, pending
   identity, resident pages, restored payloads, pins, byte counters, and
   logical clock all unchanged?
7. On failed DRAM promotion, is the target still in DRAM and is its byte
   charge still present exactly once?
8. Is subtracting `dram_release_bytes` before placing promotion victims
   correct, and does it avoid only unnecessary NVMe spills without
   overcommitting DRAM?
9. Do the no-victim and multi-victim branches use checked arithmetic
   sufficiently, including the apparently plain addition whose identical
   operands were just checked?
10. Can `apply_hbm_admission` clear restored payloads only for NVMe victims
    while preserving payloads for DRAM victims?
11. Does the `pin_hbm` preflight prevent state, clock, or pin-count mutation
    on every error?
12. Are the failure and success tests independent enough to fail the old
    incremental implementation and to exercise a real two-victim plan using
    the larger MTP sidecar geometry?
13. Is the cache-lifecycle fixture change exactly explained by moving the
    corruption probe to the page that is actually on NVMe after bounded
    promotion?
14. Are the 234-test claim and all GPU/direct-I/O/model non-claims accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- all failure paths are mutation-free;
- deterministic victim selection and tier placement are accepted;
- success commits the complete bounded plan exactly once;
- promotion, pin, and restored-payload accounting are accepted;
- both regression tests distinguish the relevant old and new behavior; and
- the CPU proof and its non-claims are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, partial mutation, overflow
hole, pinned-page eviction, nondeterministic selection, counter drift,
payload loss, or a regression test that cannot distinguish the defect.

The token accepts only this CPU correction. It does not open cn4, direct I/O,
checkpoint conversion, or model execution.
