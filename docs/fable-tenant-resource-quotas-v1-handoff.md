# Fable handoff: tenant resource quotas v1

Date: 2026-07-29

Status: adversarial design review; implementation token withheld by Sol

GPU authorization conveyed by this handoff: none

Review candidate commit:
`7e810c43a8856e09d48314dfef3959ded93c5f8f`

Requested acceptance token, only if every blocker and major is resolved:
`tenant-resource-quotas-v1-accepted`

## Provenance

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/tenant-resource-quotas-v1.md` | `d779e4d6a4e4a6b5b57e4c76ab1cee504361df76ff8d2d78b174db00e4528cab` |
| `docs/serving-page-transaction-v1.md` | `e3a9a1d9f2eb26dc5312d7c42297fa3d832e444f7e3f269094746a85fb3deac2` |
| `docs/online-prefix-publication-v1.md` | `67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873` |
| `docs/step-execution-io-v1.md` | `e8681e9278034b25fe6928c059ad58730818ce014fb3e0251549f678aa1621d5` |
| `crates/glm-serving/src/http.rs` | `0cd2f66e45a1e79e14035c44b34ecaa73d7da80fc9b3ba580771937a6c9b5c41` |
| `crates/glm-serving/src/backend.rs` | `34396a06b459e060af0c5f6b0cfb6451522af0f72536312da24804b25fe40c6c` |
| `crates/glm-serving/src/lib.rs` | `9d011012cb103149aed5ff531f356746d50f0ed29398854f2d2516c42d82aeab` |
| `crates/glm-scheduler/src/lib.rs` | `5651a507ad240f19755d50336f09eb3ca97e32f8be51f90e0fe49ef304350f38` |
| `crates/glm-cache/src/sequence.rs` | `fe42a717a42b53f0c739b87f84303715a2a7b0c79c2efdf4af8691fe02e16b08` |
| `crates/glm-cache/src/tier.rs` | `c31b07d7f9054f3d51bc5d24c2c414b6c9a134d88f042502bc0f82e29cad500f` |
| `crates/glm-engine/src/memory.rs` | `3a50581a8a60970a92ccf5a2c0e83c23d25ad975f1124c2332e9a2e646dbc837` |
| `profiles/profile-budget-v0.json` | `cdbe4eaad9465181b2ba60b3656fe5207eee54467abfbf8d9bc398c3e68c23e0` |
| `docs/serving-observability-v1.md` | `4058d01d58c0d8f4d7222803e05577a9419cfa6f5d0f20a65c41e9e2779213e6` |
| `docs/production-punchlist.md` | `2637824e9c0107f76db7f14c23dbcd0190d6b02cf0ef4f6a9b01fa85f7be9705` |
| `docs/results-index.md` | `c12f5fb5b2807d291a83a0430a8b9051e4a14865902cc9de694f21aefa8181df` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/native-engine-plan.md` | `33552cd81e3d79b8b484856a99620420f3e2eddfdfa529a23b191353a702ed80` |
| `docs/benchmark-contract.md` | `cd51d22a8faf2baacfb4682ff5e1dcb5986edc27d8aa3af188105842bb49a507` |

Hash every input at review start and finish. Review the exact candidate commit
in a separate worktree if `main` advances. The candidate deliberately
contains no quota implementation or GPU evidence.

## Requested adversarial questions

1. Does one move-only permit and one serializable ledger close every
   ownership gap between HTTP ingress, tokenization, backend queue, scheduler,
   cache restore, page table, and terminal drain?
2. Must HTTP authenticate and acquire the declared-body reservation before
   allocating or reading the body, and does the proposed header/body split
   avoid an unauthenticated memory-amplification path?
3. Is the ingress-to-queue exchange truly atomic, including tokenizer error,
   command queue full/disconnected, concurrent fatal drain, and request-ID
   exhaustion?
4. Are requested-context claims and immutable context bands the right
   protection against future KV overcommit without pessimistically reserving
   all future physical HBM?
5. Are the S/M/L/XL/MAX boundaries complete and unambiguous at every endpoint?
6. Is requiring at least one tenant and the global profile to admit a
   1,048,576-token claim sufficient to preserve the stated capacity goal?
7. Re-derive every target, indexer, draft, per-page, per-rank, and one-million
   byte figure. Does the distinction between logical HBM payload and aligned
   tier-I/O bytes remain exact?
8. Are 262,144 committed, 4,096 page-slack, and 448 tentative token slots per
   rank the correct capacity purposes, without falsely treating them as
   hard-partitioned allocator pools?
9. Can page slack and tentative demand overlap in a real C64 MTP6 step, and
   is exact reachable-plus-reserved accounting against 4,167 pages sufficient?
10. Can any successful commit require more than 4,096 committed pages on one
    owner while total positions remain at or below 1,048,576 because many
    sequences have adversarial page-ordinal alignment?
11. Does charging unique allocations once globally and every request
    reference logically to its tenant prevent both overcounting and
    cross-tenant quota bypass?
12. Is concurrent restore deduplication well-defined when waiters join,
    cancel, time out, or request different draft capability?
13. Can a read-only restore plan become stale between lookup and reservation?
    Is deterministic join-or-replan sufficient without copying or pinning
    during planning?
14. Does the restore transaction acquire all future HBM destinations,
    resident logical references, durable pins, and transfer limits before any
    worker starts?
15. Are target-only and draft-capable shared-prefix charges and upgrades
    consistent with the online-publication contract?
16. Is requested-context claim accounting compatible with prefix sharing,
    session fork, generated tokens, early EOS, and a caller-specified maximum
    output?
17. Does step reservation bind quota generation/digest tightly enough to the
    page transaction and `StepInput`, or is another ABI field required?
18. Are post-commit committed-page limits and transient arena limits checked
    at the correct times for prefill, MTP0, and MTP1–6?
19. Is rollback idempotence keyed sufficiently to prevent a retry from
    leaking or double-reserving pages?
20. Does suspension preserve every logical entitlement and durable byte while
    releasing only rank-acknowledged HBM reachability?
21. Can private partial pages be suspended safely under the stated rule, or
    must version one forbid suspension of any sequence with an unsealed tail?
22. Does DRAM/NVMe pin accounting distinguish shared physical bytes from
    tenant logical bytes and avoid falsely claiming more resident decode
    capacity?
23. Does one terminal cleanup routine cover queued, restoring, active,
    suspended, slow-consumer, rank-fatal, and accepted-but-undrained commands
    exactly once?
24. Are `ACCEPT`, `WAIT`, `REJECT`, and `FATAL` sufficiently distinct? Identify
    any limit currently classified as retryable that must instead reject, or
    vice versa.
25. Are the external reason codes and pre-stream/SSE behavior stable and
    nonleaking across tenants?
26. Can restore-aware weighted fairness be specified without first freezing
    exact normalization between query rows and bytes? If not, state the
    minimum amendment needed before CPU implementation.
27. Do fixed-cardinality metrics prove high-water and final-zero accounting
    without exposing tenant or prompt identity?
28. Does immutable quota configuration require a drained restart, and are
    profile/memory digests sufficient startup consensus inputs?
29. Is the 28-case CPU/fault matrix complete for all lifecycle edges,
    cross-tenant sharing, 1M capacity, escrow protection, and fatal cleanup?
30. Which existing API, backend, scheduler, page transaction, cache,
    observability, and startup contracts must version atomically?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Withhold
the token unless every blocker and major is resolved. State separately
whether:

- the single-ledger architecture is accepted;
- the exact capacity and byte arithmetic is accepted;
- a pure CPU ledger implementation may begin;
- HTTP/backend versioning may begin;
- page-table and restore integration must remain blocked;
- the current service must not claim S06 or production multi-tenancy;
- any finding changes the 1M HBM budget or MTP6 page escrow; and
- no cn4 access or GPU launch is authorized by the verdict.
