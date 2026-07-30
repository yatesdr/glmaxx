# Fable handoff: cache arena budget v2 r2

Date: 2026-07-30

Status: corrective CPU implementation review requested

Review candidate commit:
`04e4c3ac60a50ba6bb3a9767bbe43c3d68cec614`

Required result path:
`docs/reviews/fable-cache-arena-budget-v2-r2.md`

Requested acceptance token, only for an unqualified scoped pass:
`cache-arena-budget-v2-r2-cpu-accepted`

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch GPU, container, storage-device,
or network work for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if needed. Hash every candidate input at review start and finish.
Any mismatch must withhold the token as stale.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/native-engine-plan.md` | `33552cd81e3d79b8b484856a99620420f3e2eddfdfa529a23b191353a702ed80` |
| `docs/offline-engine-contract.md` | `b5a51b15a0a600031fcddb7d840d4d499cf915d4c22f26099cdb1dc188d74fe1` |
| `docs/offline-serving-spine.md` | `500628e6da720a760a242034678e402ab7fb0e78bd479c901254e6603cd35c99` |
| `docs/checkpoint-ingest.md` | `186ce5985ca1adbf280a011a7692ae07780736f842c786dccb599ed8a458d07d` |
| `docs/fable-cache-arena-budget-v2-handoff.md` | `adf34724616de4062f38cdb433005837604e1c3d7597c936e5b222925183f38c` |
| `docs/cache-arena-budget-v2-r2-proof.md` | `9133beb637395bfbe10cf857348159d442d9cdb5aaa88c6a7d8b6ae2374c3e37` |
| `crates/glm-engine/src/memory.rs` | `2131c999b6762a9b7e505cfe542c957877d95af4ee04056affa9d677156e9491` |
| `crates/glm-engine/src/lib.rs` | `e3a70f7906c7a0d33a6a43e8bf791e1de0daa47e4ab918825adb47ecd64fb4b9` |
| `crates/glm-engine/src/step.rs` | `4963a58da7c9c6bbed0fb57fb7ef56d90d1e0f09fe54da8cc02c35891f743359` |
| `crates/glm-cache/src/lib.rs` | `d89c83c595275eeb10ed3e84b4d68c8153744372b735b28bc4108e499a536942` |
| `crates/glm-cache/src/sequence.rs` | `a0839ff83d70102369afe6c2f3ff6ee5bd64bd52d53997faf1a826deff848d4a` |
| `crates/glm-cache/src/page.rs` | `d32d70b46f8e09c31923b6fb574db07ef6a8a7dfc7489392b39785dd563217ed` |
| `crates/glm-cache/src/kv.rs` | `fe5f4b8e07c8a32c6534f6217d62057f3ddd7c4b1abfcc00489c550a39660721` |
| `profiles/profile-budget-v0.json` | `cdbe4eaad9465181b2ba60b3656fe5207eee54467abfbf8d9bc398c3e68c23e0` |
| `fixtures/engine-contract-proof-v1.json` | `a28686829ae46d62ab449eacae3a1b64bf965c43c22699bb4c9130ecedc9c1a2` |
| `docs/production-punchlist.md` | `7a3d09b1f03356b919a5b69e798978de6d17241e951457001dcaa68f6d876a84` |
| `docs/results-index.md` | `f51c82f6afe97272854e6381a178b4ed0163c14c6dd743b554bff60be684d284` |
| `scripts/local-checks.sh` | `56f728cdf3f047f9633509a57341d25a977efa802f0d5b371c9716830517db59` |
| `Cargo.toml` | `863c28560b339f1fd7fb6b80c1b812e9fa7bc3f8f8c782126d2a29ceeffc06ea` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-cache-arena-budget-v2-r2-handoff.md
cargo test --offline -p glm-engine memory::tests -- --nocapture
cargo test --offline -p glm-cache sequence::tests -- --nocapture
cargo clippy --offline -p glm-engine -p glm-cache --all-targets -- -D warnings
git diff --check 04e4c3ac60a50ba6bb3a9767bbe43c3d68cec614^ \
  04e4c3ac60a50ba6bb3a9767bbe43c3d68cec614
```

The handoff is coordination metadata added after the candidate and is not a
candidate input. The first design review is operator-owned read-only input at
`docs/reviews/fable-cache-arena-budget-v2.md`, currently SHA-256
`ec6a50467a5af9646e788556fdddfc505d65085734bbef8be8a96eb204e034b1`;
do not modify or add it to the candidate.

## Review purpose

The first review found zero blockers and zero majors, independently proved
the serving arithmetic, and identified seven minors. This r2 candidate claims
to close only its six static-planner/test minors:

1. profile validation derives all four cache byte terms from the shared
   shape, record-size, page-slack, and verifier-row constants;
2. TP4 rank disagreement in any target/draft cache-arena component is rejected
   instead of silently min-composing and charging stranded capacity;
3. the system planner and table constructor share one maximum physical-page
   constant and successful plans always emit a constructible configuration;
4. required bytes must fit the planned usable-HBM floor even while the profile
   remains pending and non-convertible;
5. the exact one-page MTP0 and seven-page MTP6 physical margins are documented;
   and
6. a permanent test constructs the exact adversarial C64 rank distribution
   for both postures.

The seventh finding remains open: the retained table clones state for
rollback and the retained mirror scans complete mappings. Those are CPU
correctness oracles, not the production hot path.
`docs/fixed-page-transaction-v1-r2.md` owns the bounded implementation.

The global and tenant logical-position quota ledger also remains
unimplemented. This candidate proves physical arena sufficiency under the
reviewed C64/1,048,576-position premise; it does not prove that every serving
admission path enforces that premise.

## Review boundary

Acceptance covers only:

- derived static cache-byte validation;
- symmetric TP4 cache-arena planning;
- constructible page-table bounds;
- pending profile budget-floor validation;
- exact MTP0/MTP6 physical-margin tests; and
- the named CPU checks.

Acceptance does not cover:

- the retained clone table or full-scan mirror as a production hot path;
- a global or tenant logical-position quota ledger;
- measured HBM fit or conversion authorization;
- checkpoint loading or weight residency;
- native CUDA compilation or execution;
- real KV payload allocation, transfer, reconstruction, or attention;
- a model kernel, graph, collective, request, checkpoint smoke, quality,
  capacity, latency, or throughput result; or
- cn4 access.

## Required adversarial questions

1. Do all twenty-one candidate-input hashes match at review start and finish
   in a detached worktree?
2. Does `capacity_exl3_cache_terms` derive the four expected byte terms only
   from shared constants using checked arithmetic, with no second numeric
   source able to silently accept drift?
3. Independently re-derive the four terms from record encodings and actual
   GLM-5.2 geometry. Do they remain exactly 7,655,012,352; 739,259,136;
   98,141,184; and 35,202,816 bytes?
4. Does every cache-arena shape disagreement among the four ranks fail before
   construction, including differences in committed, slack, tentative, and
   rounded target or draft slots?
5. Can rank-local measured HBM/headroom remain different without allowing a
   rank-local arena route or fallback?
6. Does every successful system plan satisfy the exact constructor bounds:
   nonzero target pages, target pages at most 1,048,576, and draft pages at
   most target pages?
7. Is `MAXIMUM_PHYSICAL_PAGES_PER_RANK` consumed by both the planner and table
   constructor rather than copied as a second literal?
8. Do zero, oversized, non-page-aligned, overflow, target-only, excess-draft,
   or asymmetric mutations fail closed without undercharging bytes?
9. Does a pending profile now fail validation when required bytes exceed its
   planned floor, while still requiring planned floor not exceed observed
   pre-context free bytes?
10. Independently construct the C64 adversarial state. Are target and draft
    physical-page counts exactly `[4160, 4096, 4096, 4096]` after 64
    sequences each commit 16,384 positions and reserve one spill position?
11. Is one tentative position sufficient to attain the physical maximum even
    though MTP6 permits seven, because a partial page absorbs the remaining
    positions without allocating another page?
12. Do the resulting MTP0 and MTP6 margins remain exactly one and seven
    pages, and can any valid distribution under the stated C64/global-token
    premise exceed 4,160 pages on one rank?
13. Does the MTP0 plan remain exactly 266,304 slots/4,161 pages with no draft
    arena, while the MTP6 plan remains 266,688 slots/4,167 pages for both
    target and draft?
14. Can a successful plan's `PageTableConfig` be constructed by the retained
    table without a later configuration failure?
15. Do mutations removing asymmetric-rank rejection, constructor-bound
    rejection, pending-floor rejection, or either exact pressure margin cause
    a named test or review re-derivation to fail?
16. Does the proof accurately leave the global/tenant quota ledger and fixed
    bounded transaction hot path outside acceptance?
17. Does the proof avoid treating synthetic profile bytes or CPU metadata
    allocation as measured HBM, CUDA, real KV payload, model, capacity, or
    performance evidence?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately whether:

1. all six static findings from the first review are closed;
2. all four cache byte terms are independently exact and drift-safe;
3. rank arena symmetry and page-table constructibility fail closed;
4. pending profile floors cannot be arithmetically impossible;
5. the exact MTP0/MTP6 pressure tests establish the stated margins;
6. the clone/full-scan hot path and logical quota ledger remain visibly open;
7. current tests distinguish the reviewed static defects; and
8. no GPU, measured-fit, real-KV, model, quality, capacity, or performance
   evidence is implied.

Only if all seventeen questions and all eight statements are unqualified
`YES`, end with:

```text
cache-arena-budget-v2-r2-cpu-accepted
```

Withhold for stale provenance, duplicated arithmetic authority, an
asymmetric-rank escape, an unconstructible plan, a pending-floor escape,
incorrect physical-pressure arithmetic, a nondistinguishing test, acceptance
of the retained hot path/quota ledger, or any GPU/model/performance
overstatement.
