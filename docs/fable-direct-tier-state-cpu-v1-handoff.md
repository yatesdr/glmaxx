# Fable handoff: direct-tier buffer and restore state CPU proof v1

Date: 2026-07-30

Status: adversarial CPU-proof review requested

Review candidate commit:
`ccd636967bec031c8a8b0349a18b39113c0a6ae6`

Required result path:
`docs/reviews/fable-direct-tier-state-cpu-v1.md`

Requested acceptance token, only for an unqualified pass:
`direct-tier-state-cpu-v1-accepted`

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch GPU, filesystem, or NVMe work
for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if necessary, run `review-proof`, and hash every input at review
start and finish. A mismatch is a stale candidate and must withhold the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `docs/direct-tier-extent-cpu-proof-v1.md` | `d54ad467e8f2219ec31638416ff5a0a74cf972a6077695b6eea7dd1b8eb859b1` |
| `docs/direct-tier-state-cpu-proof-v1.md` | `3f58a9c1b7ad7cc4806598b467f02eb746013e75a72e4566f9e9ba55f466df66` |
| `crates/glm-cache/src/tier.rs` | `0a1541f13462bcdec92284911f96531b06869b60c7fe85fc5e9669c80fabe693` |
| `crates/glm-cache/src/direct.rs` | `7514ad205b84f24c2a2f58647c2b50b0f6dab4398dfb04a2fc36186f09a82dd3` |
| `crates/glm-cache/src/direct_state.rs` | `386c414a302a489c1e8c6fefa6d11b304e86ffa2040ad105e81cc8572999f814` |
| `crates/glm-cache/src/direct_restore.rs` | `73578aa42bf944c37bfe431da21df5c27ad12ee58dfb81f64a24a180830b1c1f` |
| `crates/glm-cache/src/lib.rs` | `c9576ebb8d7f79df6c6ae93d328dd73a8e6fdaab23e40a0645a6c3b54e119bd3` |
| `crates/glm-cli/src/main.rs` | `0bdae601b74009186ef54431e27795b12a8c5adc8a57290a07a8c09fae4d773d` |
| `fixtures/direct-tier-extent-proof-v1.json` | `eb5efc3faefc67a932ed4b86e1af29bee89b53cf0483b6a39c373c938b047d6c` |
| `fixtures/direct-tier-state-proof-v1.json` | `58f19d6b506e969c91561938eb45a509ce820d936b9bb4d901c9028a5ca17c75` |
| `scripts/local-checks.sh` | `56f728cdf3f047f9633509a57341d25a977efa802f0d5b371c9716830517db59` |
| `docs/production-punchlist.md` | `add43a0af6c6254c33afa42842d6c24b2146319d190b03626bc34db1e5f3f610` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-direct-tier-state-cpu-v1-handoff.md
cargo test --offline -p glm-cache direct_ -- --nocapture
cargo clippy --offline -p glm-cache -p glm-cli --all-targets -- -D warnings
cargo run --release --offline -p glm-cli --bin glmaxx -- \
  direct-tier-state-proof /tmp/direct-tier-state-proof-v1-release.json
cargo run --offline -p glm-cli --bin glmaxx -- \
  direct-tier-state-proof /tmp/direct-tier-state-proof-v1-debug.json
cmp fixtures/direct-tier-state-proof-v1.json \
  /tmp/direct-tier-state-proof-v1-release.json
cmp /tmp/direct-tier-state-proof-v1-debug.json \
  /tmp/direct-tier-state-proof-v1-release.json
```

## Review boundary

This review covers only the deterministic CPU buffer, descriptor, CQ-budget,
and shared restore-ticket state machines described by
`direct-tier-state-cpu-proof-v1`.

Acceptance opens durable direct-record/catalog/journal design and a separate
nonproduction Linux `io_uring` feature/fault proof. It does not accept actual
registered memory, an `io_uring` service, a filesystem or NVMe path, a
durable catalog/journal, a cleaner, CUDA/HBM integration, cn4 evidence,
K03/K05, checkpoint smoke, serving readiness, model quality, capacity, or
performance.

The cleaner remains blocked on the accepted review's required relocation
journal/checkpoint amendment.

## Required adversarial questions

1. Do all candidate hashes match at review start and finish in a detached
   worktree, even if `main` advances?
2. Does every buffer own one maximum-size 2,052,096-byte allocation and
   expose an exact 4,096-aligned slice without unsafe code?
3. Do extent and pool allocation failures return typed errors rather than
   intentionally panicking, and are partially built pools dropped safely?
4. Is generation zero invalid, is generation incremented before visibility,
   does overflow permanently retire the slot, and can no stale generation
   read, mutate, transition, fail, quarantine, or free a reused buffer?
5. Does every transition to FREE zero the complete maximum extent, and can a
   FREE/FAILED/QUARANTINED/RETIRED handle expose no bytes?
6. Independently enumerate every accepted buffer transition. Is the generic
   transition table exact, with abandoned READ_INFLIGHT release isolated in
   the authority-only operation rather than admitted generally?
7. Can every operational state fail, and can FAILED only enter QUARANTINED
   with no route back to FREE?
8. Is the 64-bit user-data layout exactly nonzero u32 generation, u31 slot,
   and one role bit, with capacity bounded so no slot bit is truncated?
9. Can a raw CQE `user_data` be reconstructed into the exact token, while
   zero generation, unknown slot, stale generation, wrong role, duplicate
   completion, and missing entry fail closed?
10. Does the descriptor entry retain the complete u64 buffer generation and
    u64 operation generation, rather than relying on the narrower token
    generation as the resource identity?
11. With async cancel issued, does one logical descriptor remain owned until
    original and cancel are both completed in either order, and is it
    impossible to reuse before both?
12. Does descriptor-generation overflow permanently retire the descriptor
    slot and make a late CQE from the previous occupant fail?
13. Re-derive the CQ invariant. Is construction exactly
    `cq_entries = descriptor_capacity * 2`, and does every submit enforce
    `original + async_cancel + fsync <= cq_entries` without relying on
    NODROP?
14. Is every planned ticket bound to one complete validated extent record,
    catalog epoch, and catalog-record digest, rather than only a page key?
15. Can an MTP ticket satisfy target and MTP waiters while a target record
    can never satisfy MTP, including under concurrent target/MTP plans?
16. Do same-tenant and cross-tenant compatible waiters join exactly one
    physical ticket while each retains its own required-capability logical
    charge?
17. If the first waiter cancels but another remains, does the ticket-scoped
    physical reservation survive while only the departed waiter's tenant
    charge is released?
18. Is waiter delivery request-ID ascending independent of insertion and map
    iteration order?
19. Do ticket, waiter, tenant, physical, buffer, descriptor, and CQ
    saturation paths preserve every preexisting reservation and state?
20. For last-waiter cancellation before submit, in flight, after original
    CQE, during hash, and after verification, are buffer/descriptor/physical
    lifetimes exact?
21. Are original-then-cancel, cancel-then-original, no-async-cancel, cancelled
    original, failed original, duplicate completion, and integrity failure
    schedules all fail-closed with final accounting exact?
22. When a read or hash fails with an async cancel still outstanding, is the
    buffer quarantined immediately but the ticket and physical reservation
    retained until the cancel CQE is reaped?
23. Does a catalog binding change require replan before submit but leave the
    immutable submitted record pinned afterward?
24. Does `validate_invariants` independently reconcile physical bytes,
    per-tenant logical bytes, request membership, capabilities, buffer state,
    descriptor bindings, and CQ counts at intermediate schedules?
25. Are the fixture values derived from actual transitions, deterministic
    and byte-identical in debug/release, and do proof errors occur before the
    PASS verdict?
26. Do the proof document and punchlist keep every absent Linux, durable,
    CUDA, cn4, serving, and performance component explicit and unpassed?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer each statement separately:

1. buffer allocation, alignment, generation, zeroing, and quarantine are
   sound for the stated CPU scope;
2. completion tokens and the descriptor table prevent ABA and early reuse;
3. CQ capacity arithmetic is exact and NODROP-independent;
4. restore matching and the target/MTP capability lattice are fail-closed;
5. physical and logical accounting is exact across same/cross-tenant joins
   and every admission refusal;
6. cancellation and original/cancel CQE schedules retain ownership until all
   dependent work is acknowledged;
7. read/hash failure quarantines bytes and still reaps outstanding
   descriptors safely;
8. catalog pre-submit replan and post-submit pinning are exact;
9. internal invariant reconciliation and deterministic fixture claims are
   valid; and
10. the claim boundary is accurate and sufficient to open the next durable
    CPU design/implementation slice.

Only if all twenty-six questions and all ten statements are unqualified
`YES`, end with:

```text
direct-tier-state-cpu-v1-accepted
```

Withhold the token for stale provenance, unsafe or misaligned allocation,
generation reuse, incomplete zeroing, an invalid transition, descriptor/CQE
ABA, early release, wrong CQ arithmetic, capability inversion, duplicate
physical charging, missing tenant charge, nondeterministic waiter order,
partial admission mutation, cancellation leak, catalog drift, ineffective
invariant checking, nondeterministic fixture, pass-on-error behavior, or
evidence overstatement.
