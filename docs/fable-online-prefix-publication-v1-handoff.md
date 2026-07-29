# Fable handoff: online prefix publication v1

Date: 2026-07-29

Status: adversarial design review; implementation token withheld by Sol

GPU authorization conveyed by this handoff: none

Review candidate commit:
`d0a09d7c62f1943112eaa703a9ef3f6b25e9ebc9`

Requested acceptance token, only if every blocker and major is resolved:
`online-prefix-publication-v1-accepted`

## Provenance

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/online-prefix-publication-v1.md` | `67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873` |
| `docs/serving-page-transaction-v1.md` | `e3a9a1d9f2eb26dc5312d7c42297fa3d832e444f7e3f269094746a85fb3deac2` |
| `docs/step-execution-io-v1.md` | `e8681e9278034b25fe6928c059ad58730818ce014fb3e0251549f678aa1621d5` |
| `docs/native-engine-plan.md` | `33552cd81e3d79b8b484856a99620420f3e2eddfdfa529a23b191353a702ed80` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `crates/glm-cache/src/page.rs` | `d32d70b46f8e09c31923b6fb574db07ef6a8a7dfc7489392b39785dd563217ed` |
| `crates/glm-cache/src/tier.rs` | `c31b07d7f9054f3d51bc5d24c2c414b6c9a134d88f042502bc0f82e29cad500f` |
| `crates/glm-cache/src/store.rs` | `d37a1400dc0c393b26c121f72694945bef78c28eda29796abf41a2ed713a17ac` |
| `crates/glm-cache/src/prefix.rs` | `1dc89bd966a6bf63e3257e43eae75a2c485047173337959b5b103a9ed57adcd0` |
| `crates/glm-cache/src/sequence.rs` | `fe42a717a42b53f0c739b87f84303715a2a7b0c79c2efdf4af8691fe02e16b08` |
| `crates/glm-cache/src/residency.rs` | `b2495d7f656616ee0cd1eeadfa234f9e7555af6bd7b32f06da9d772bbed6e629` |
| `crates/glm-serving/src/cache.rs` | `786c7c7e5ce2f417749a78e8c48aa8a7d0a5cb617e0883e960a8e7c17d781720` |
| `crates/glm-serving/src/lib.rs` | `9d011012cb103149aed5ff531f356746d50f0ed29398854f2d2516c42d82aeab` |
| `docs/serving-observability-v1.md` | `4058d01d58c0d8f4d7222803e05577a9419cfa6f5d0f20a65c41e9e2779213e6` |

Hash every input at review start and finish. Review the exact candidate commit
in a separate worktree if `main` advances. The candidate deliberately
contains no publisher implementation; do not infer a CPU, durability,
transfer, restart, or GPU result from the design.

## Required review decisions

1. Are sequence-table generation, HBM allocation generation, and durable
   content revision genuinely distinct identity domains? Identify every
   current struct or serialized field that must change.
2. Does a publication lease fully close local-page-ID ABA across successful
   copy, cancellation, failed I/O, sequence removal, cache-only generation,
   and shutdown?
3. Can publication remain orthogonal to `HBM_SEALED`, or does any existing
   page-state invariant require a new physical state?
4. Is the bounded committed-token chain sufficient for cold prefill, a
   restored prefix, a partial uncached tail, decode, MTP commit/rollback,
   accepted draft EOS, session fork, and one million positions?
5. Can the coordinator derive every seal ticket only after exact commit
   without retaining or publishing tentative tokens? Check multi-page
   prefill bounds.
6. Re-derive the three logical piece sizes, target-only/MTP totals, aligned
   physical append spans, and `4 * Q` pinned-staging formula.
7. Does one process-wide append authority plus a shared immutable catalog
   give every restore worker linearizable online visibility without
   serializing all read I/O?
8. Is durable-before-catalog publication correct at every crash point? Can
   any restart expose a nondurable piece, orphan draft sidecar, or child with
   no visible parent?
9. Are parent key, page ordinal, valid count, and advisory writer sufficient
   durable metadata for restart reconstruction? If original token IDs are
   absent, what can and cannot be validated?
10. Must the journal/container move to a new version, and should old v2
    estates fail closed, migrate offline, or coexist read-only?
11. Is the deduplication/MTP-upgrade matrix complete? In particular, is
    rewriting all three pieces for an upgrade necessary and sufficient?
12. Is a different payload digest for the same token-derived key correctly
    engine-fatal? Decide whether qualification must require bit-identical KV
    bytes across graph and batch shapes or whether the namespace needs an
    additional numerical-execution identity.
13. Can child copies finish out of order while visibility remains
    parent-ordered without unbounded pending state or deadlock?
14. Are saturation, NVMe capacity, rolling write budget, filesystem failure,
    and post-durability registration failure classified correctly as
    publication-local? Identify any condition that must instead fail a
    request or engine.
15. Does skipping a cache candidate before lease acquisition satisfy the
    mandatory prefix-cache requirement, or must admission reserve publication
    capacity for some request classes?
16. Are the proposed queue/staging/catalog/write limits sufficient to prevent
    hidden unbounded memory, disk growth, or write amplification?
17. Does the contract keep all rank decisions invariant while allowing only
    the modulo owner to copy payload bytes?
18. Can a target-only durable page be upgraded safely from a later MTP
    execution without changing its page key or target behavior?
19. Are the metrics sufficient to distinguish model compute, D2H transfer,
    durability, catalog visibility, deduplication, and dropped candidates?
20. Is the 15-item CPU proof matrix complete enough to authorize subsequent
    CUDA transfer work?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Withhold
the token unless every blocker and major is resolved. State separately
whether:

- the design may amend the page-transaction and durable-format contracts;
- CPU implementation may begin;
- any part can reuse the existing `FileTierStore` bytes without a format
  version change;
- any finding changes the 1M cache arithmetic;
- any finding blocks independent NVFP4/EXL3 kernel qualification; and
- no cn4 access or GPU launch is authorized by the verdict.
