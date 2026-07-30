# Fable handoff: HBM↔DRAM sealed-page transfer v1

Date: 2026-07-30

Status: adversarial design review requested

Review candidate commit:
`839f377473e08994269bcac68881f3e7afa14790`

Required result path:
`docs/reviews/fable-hbm-dram-transfer-v1.md`

Requested acceptance token, only for an unqualified pass:
`hbm-dram-transfer-v1-design-accepted`

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch GPU, filesystem, NVMe, or
container work for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if needed, run `review-proof`, and hash every input at review start
and finish. A mismatch is a stale candidate and must withhold the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/hbm-dram-transfer-v1.md` | `ed701b2d96ceccb7257ae0ee2bb09988ad9f31309cc01ac28e220648f69f1464` |
| `docs/sm120-rank-executor-v1-r2.md` | `4f40ea7652858b4cebbe4093dc81149cb30aa26bedc69edef72fa627c987df89` |
| `docs/sm120-rank-executor-native-abi-v1.h` | `0d0f0357a17eba4e678d5c82da4dbff552e292fb7948931496a4382289ae4d6e` |
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `docs/direct-tier-durable-format-v1.md` | `19ca03edeab89b560d674689ca96ce497f2c5859a91d5fe5d4b50c78645e79e6` |
| `docs/online-prefix-publication-v1.md` | `67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873` |
| `docs/checkpoint-load-transaction-v1.md` | `184199144075e3f7c511b31002ea81c1d2dbaad1087e01fe7dd4533dc36a3c21` |
| `docs/serving-page-transaction-v1.md` | `31983cce95ee01a5968213d5daf12c7a855f75f8735314700f2b4a9e55625d1a` |
| `crates/glm-cache/src/page.rs` | `d32d70b46f8e09c31923b6fb574db07ef6a8a7dfc7489392b39785dd563217ed` |
| `crates/glm-cache/src/residency.rs` | `2846361e521f66752cb4455c908b2f30fa2f2a27a59a8059866e43b2402a2d6d` |
| `crates/glm-engine/src/memory.rs` | `0ae657905a1b2091980c4904643e35a7a53b282ef112be44447362add89f023b` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/production-punchlist.md` | `c262993427b055ae84b5fded46965cab51ae30dfdd0733cef305a16b900c4e2e` |
| `scripts/local-checks.sh` | `56f728cdf3f047f9633509a57341d25a977efa802f0d5b371c9716830517db59` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-hbm-dram-transfer-v1-handoff.md
git diff --check 839f377473e08994269bcac68881f3e7afa14790^ \
  839f377473e08994269bcac68881f3e7afa14790
```

The handoff itself is coordination metadata added after the candidate and is
not a candidate input.

## Dependency posture

The candidate consumes the pending executor-r2 design without claiming that
design is accepted. It identifies two required native changes:

- checked 2-D asynchronous copies without an implicit per-plane event; and
- owner-thread registration of a precreated NUMA-bound host mapping.

It also adds pinned DRAM cache bytes as an explicit term in the pending
executor's process-wide pinned-host equation.

An acceptance token for this handoff accepts those requirements as a design
dependency. It does not accept the executor-r2 candidate or authorize
implementation against a withheld upstream token. If executor-r2 changes
these ownership boundaries, this transfer design must be reconciled and
re-reviewed.

## Review boundary

This review covers only the sealed-page transfer design. Acceptance opens a
separate CPU/mock pitched-memory/event/generation proof after all upstream
design dependencies are accepted.

Acceptance does not accept:

- the pending executor-r2 or native ABI;
- an actual native 2-D copy or host-registration function;
- a CUDA row-digest kernel;
- pinned memory, HBM, DRAM, io_uring, NVMe, or NUMA evidence;
- cn4 access;
- K02/K03/K04, prefix smoke, serving, model quality, capacity, or
  performance; or
- any A6000 result as SM120 evidence.

## Required adversarial questions

1. Do all candidate hashes match at review start and finish in a detached
   worktree regardless of later `main` movement?
2. Re-derive all geometry: 23,552/8,448 row bytes,
   1,837,056/177,408/32,000 logical pieces, direct host offsets, and
   2,019,328/2,052,096 physical extents. Are they exact?
3. Re-derive 266,688 slots = 4,167 pages and the
   98,141,184/35,202,816 device pitches. Do page 0, page 4,166, every final
   row, and one-past bounds follow the displayed address formula?
4. Are the two target and four MTP pitched planes a bijection between the
   adopted HBM layouts and canonical host extent, with no copied padding,
   missing byte, overlap, or layer/group transposition?
5. Is the diagnosis of the current 1-D native ABI correct: absent a 2-D
   operation it requires 99/101 copies, while the proposed route needs only
   two/four?
6. Does refusing a silent scalar serving fallback preserve measurement
   honesty while retaining a separately labeled diagnostic control?
7. Are the target/MTP DRAM slot classes, capability lattice, full-extent MTP
   upgrade, 4,096 alignment, zeroing, generation, and overflow-retirement
   rules safe?
8. Re-derive one full 1M chain's physical DRAM allocation:
   33,084,669,952 target bytes and 33,621,540,864 MTP bytes, balanced as
   8,271,167,488/8,405,385,216 per rank. Are logical and physical accounting
   kept distinct?
9. Is pinning DRAM capacity explicit in the memory plan/process cap, without
   hiding it as staging or assuming free RAM, and is a future pageable path
   properly excluded?
10. Does the rank-partitioned dual-registered bridge have one unambiguous CUDA
    owner, one io_uring authority, double memlock accounting, DONTFORK/
    DONTDUMP behavior, and exact startup/teardown order?
11. Is the proposed precreated-host-mapping registration ABI genuinely needed
    to prove NUMA/mapping policy that the current `arena_create` cannot, and
    does it preserve the no-arbitrary-post-health-allocation rule?
12. Do HBM IDs/generations resolve to adopted arenas only on the owner thread,
    with no coordinator-supplied device pointer or rank-local layout choice?
13. Does `TierTransfer.v1` bind every content, catalog, HBM, host, event,
    deadline, geometry, and digest identity needed to reject ABA or route
    drift?
14. Are event generations retained through query, async-status check, status
    validation, receipt, and coordinator terminal acknowledgment, with named
    events rather than stream idle controlling reuse?
15. Is the checksum-worker ownership handoff a sufficient host memory fence so
    H2D can never see a mutable or unverified source?
16. Does a predecessor event plus immutable sealed-page lease make D2H device
    row hashing and data copy race-free?
17. Is 99/101 independent standard row SHA-256 a complete byte-coverage
    transfer receipt without falsely replacing catalog piece/physical SHA?
18. Does a 4,096-byte status have enough bounded space for 101 32-byte row
    digests plus identity/status/hash, and is deferring its exact layout to the
    separate native ABI gate explicit rather than an implementation claim?
19. For D2H, does device-row hash before data copy, status/data D2H, completion
    event, then host row/piece/physical hash detect transfer corruption and
    catalog drift?
20. For H2D, does verified host source, data copy, destination row hash,
    status D2H, completion event, then row-vector comparison prevent
    publication of corrupted HBM?
21. Does H2D reserve an unreachable HBM destination and publish it only after
    owner success, three explicit no-data receipts, four-rank delta consensus,
    and serving/cache commit?
22. Does MTP restore attach target, indexer, draft KV, and draft indexer
    atomically, with an MTP result satisfying target but never the reverse?
23. Does D2H demotion preserve the old HBM mapping on every destination
    failure and distinguish metadata-only durable eviction from transfer
    bandwidth?
24. Does D2H publication preserve the seal lease through cancellation, hand
    off the exact bridge generation, and leave HBM valid after durable-tier
    failure?
25. Are every legal DRAM/NVMe bridge route and its host-copy accounting
    explicit, with no hidden GDS bypass?
26. Can device transfer overlap compute only under exact range/event/resource
    disjointness without authorizing early next-step page-table or argument
    H2D?
27. Are cancellation before versus after CUDA submission, stale completion,
    owner failure, and context teardown all fail-closed with quarantine/leak
    rather than reuse?
28. Are failure classes correct, especially treating digest/CUDA/page-table
    divergence as fatal and capacity/queue/overlap refusal as pre-mutation
    `WAIT`?
29. Does the CPU/mock matrix actually model pitched memory, asynchronous event
    visibility, four-rank receipts, mutation, saturation, and final-zero
    ownership rather than reducing the proof to synchronous memcpy?
30. Is the SM120 matrix sufficient to compare pitched/scalar routes, target/
    MTP, depths, PCIe/NUMA layouts, corruption, cancellation, sustained
    resource stability, and resident-decode isolation?
31. Are the upstream dependency and K02/K03/K04 exclusions accurate, with no
    native/CUDA/cn4/capacity/performance evidence implied?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer each statement separately:

1. actual-shape address and capacity arithmetic are exact;
2. the two/four pitched-copy representation is complete and preferable to the
   current 99/101-call ABI route as a design;
3. pinned DRAM and bridge allocation/accounting/ownership are bounded and
   fail-closed;
4. HBM/host/event generations prevent ABA and early reuse;
5. row-digest plus host catalog-digest ordering covers both H2D and D2H
   transfer corruption;
6. restore, demotion, publication, MTP atomicity, and cancellation preserve
   page ownership;
7. four-rank prepare/receipt/commit and overlap rules forbid rank-local
   divergence or data races;
8. native ABI amendments are necessary, sufficiently scoped, and explicitly
   unimplemented;
9. CPU/mock and SM120 gates are implementable and adequate; and
10. acceptance opens only the declared proof sequence and no production
    claim.

Only if all thirty-one questions and all ten statements are unqualified
`YES`, end with:

```text
hbm-dram-transfer-v1-design-accepted
```

Withhold the token for stale provenance, arithmetic/layout error, uncovered
page bytes, unsafe scalar fallback, hidden pinned capacity, rank/NUMA/bridge
ownership ambiguity, generation ABA, stream-idle reuse, checksum/copy race,
incomplete integrity coverage, partial MTP attachment, HBM loss on failed
demotion/publication, rank-local transfer choice, unsafe overlap/cancellation,
an unimplementable proof, upstream acceptance leakage, or evidence
overstatement.
