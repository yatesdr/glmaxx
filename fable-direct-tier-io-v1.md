# Fable review: direct DRAM/NVMe tier I/O v1

Date: 2026-07-30

Reviewer: Fable (adversarial design-gate review)

Handoff: `docs/fable-direct-tier-io-v1-handoff.md` (queue row 14)

Reviewed candidate commit:
`69895e040617a79dea78d7eaf1ced88234ccb193`

Location note: the operator directed all review artifacts into
`docs/reviews/`; the handoff declares no result path, so this file may need
moving/renaming on acceptance.

## Provenance

All 20 input hashes from the handoff table were verified with
`git show <commit>:<path> | shasum -a 256` against the pinned candidate at
review start and re-verified at review finish; both sets matched exactly.
`main` (HEAD `7b1f912`) has drifted on 9 of the 20 inputs
(`store.rs`, `residency.rs`, `tier.rs`, `sequence.rs`, serving `cache.rs`,
`glm-cache/Cargo.toml`, `Cargo.lock`, `production-punchlist.md`,
`results-index.md`), so the review was performed in a detached worktree at
the pinned commit, never on moving `main`.

Verified table (identical at start and finish):

| Input | SHA-256 |
|---|---|
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `docs/online-prefix-publication-v1.md` | `67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873` |
| `docs/tenant-resource-quotas-v1.md` | `d779e4d6a4e4a6b5b57e4c76ab1cee504361df76ff8d2d78b174db00e4528cab` |
| `docs/serving-page-transaction-v1.md` | `e3a9a1d9f2eb26dc5312d7c42297fa3d832e444f7e3f269094746a85fb3deac2` |
| `docs/serving-observability-v1.md` | `4058d01d58c0d8f4d7222803e05577a9419cfa6f5d0f20a65c41e9e2779213e6` |
| `docs/benchmark-contract.md` | `cd51d22a8faf2baacfb4682ff5e1dcb5986edc27d8aa3af188105842bb49a507` |
| `crates/glm-cache/src/store.rs` | `d37a1400dc0c393b26c121f72694945bef78c28eda29796abf41a2ed713a17ac` |
| `crates/glm-cache/src/residency.rs` | `b2495d7f656616ee0cd1eeadfa234f9e7555af6bd7b32f06da9d772bbed6e629` |
| `crates/glm-cache/src/tier.rs` | `c31b07d7f9054f3d51bc5d24c2c414b6c9a134d88f042502bc0f82e29cad500f` |
| `crates/glm-cache/src/sequence.rs` | `fe42a717a42b53f0c739b87f84303715a2a7b0c79c2efdf4af8691fe02e16b08` |
| `crates/glm-serving/src/cache.rs` | `786c7c7e5ce2f417749a78e8c48aa8a7d0a5cb617e0883e960a8e7c17d781720` |
| `crates/glm-cache/Cargo.toml` | `5858d83830af59d4b491a42e978ec0bdaf72f36c253c233d93378c8d05f9ea93` |
| `Cargo.toml` | `863c28560b339f1fd7fb6b80c1b812e9fa7bc3f8f8c782126d2a29ceeffc06ea` |
| `Cargo.lock` | `ed694e41e6f1ba1723480d1052846d14d12086d85514b628133f5c2390d69bc1` |
| `crates/glm-engine/src/memory.rs` | `3a50581a8a60970a92ccf5a2c0e83c23d25ad975f1124c2332e9a2e646dbc837` |
| `profiles/profile-budget-v0.json` | `cdbe4eaad9465181b2ba60b3656fe5207eee54467abfbf8d9bc398c3e68c23e0` |
| `docs/production-punchlist.md` | `fb21a98010c8b68e811678b414be8cd1a9b6b86fd35af032b77ac9c3132a0f9f` |
| `docs/results-index.md` | `3cddd4f21fafc1cdabc4f112eb0b1e4f9b1f90bba8dcd361703fa5fd47ff623d` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/native-engine-plan.md` | `33552cd81e3d79b8b484856a99620420f3e2eddfdfa529a23b191353a702ed80` |

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. **CQ-overflow invariant is required but not stated arithmetically.** The
   contract requires "CQ sizing that cannot silently drop completions" but
   never states the invariant. The exact invariant that must hold at startup
   and at every submit is:
   `outstanding_original_SQEs + outstanding_async_cancel_SQEs + outstanding_fsync_SQEs <= CQ entries`,
   with CQ entries fixed at ring setup to
   `descriptor_table_capacity * 2` (every descriptor may produce one original
   and one cancel CQE) and submission refused (WAIT) when the bound would be
   exceeded. `IORING_FEAT_NODROP` presence should be probed but not relied on
   for correctness. This belongs in the durable CPU implementation contract.
2. **Physical-reservation ownership wording.** "The first waiter owns the
   global physical I/O reservation" combined with "cancelling one waiter
   removes only that waiter and its logical charges" leaves the physical
   charge attached to a possibly-departed waiter. The behavior described is
   correct (physical charge survives until the ticket's terminal state), but
   ownership should be restated as ticket-scoped, seeded by the first waiter,
   so no implementation reads waiter removal as physical-release.
3. **Registered+pinned buffer OS constraints are not enumerated.** The design
   defers CUDA pinning to a later gate (correct), but the eventual dual-use
   buffer has constraints that should be pinned now so the allocator is not
   rebuilt later: io_uring buffer registration and `cudaHostRegister` each
   count against `RLIMIT_MEMLOCK`-style accounting (plan for double charge);
   buffers must be `madvise(MADV_DONTFORK)` (CUDA pinned memory is not
   fork-safe; `O_CLOEXEC` alone does not protect the ring or buffers across
   `fork`); teardown order must be CUDA-unregister, then
   `io_uring_unregister_buffers`, then unmap, after zero outstanding
   descriptors.
4. **Segment tail slack is outside every digest.** Records are
   4,096-multiples and densely allocated, but the slack between the last
   record and the fixed segment size is covered by no physical SHA and no
   startup check. Harmless to correctness (no catalog extent references it),
   but the cleaner copies "still-live complete extents", so a norm ("tail
   slack is unspecified and never read") should be one sentence in the
   durable format version to avoid a later reader inventing meaning for it.
5. **W0 lease stop has no starvation bound.** New W0 leases stop above the
   read high watermark; under a sustained saturating read load, publication
   admission can be deferred indefinitely. That is fail-closed backpressure,
   not corruption, but the contract should say whether publication deferral
   is allowed to be unbounded or whether the weighted table also guarantees
   periodic W0 lease admission (not only completion of already-accepted W0).

### QUESTION

1. Proof case 11 lists `EIO` generally; it should explicitly include fsync
   failure (journal and data) as its own fault, since fsync-failure semantics
   differ from write failure on most filesystems and the design correctly
   classes durability-barrier failure as globally degrading.
2. Proof list has no explicit CQ-overflow/NODROP case; recommend adding one
   under case 10's limit matrix.
3. Registered-file/buffer invalidation is listed as a degraded class but has
   no matching proof case; recommend adding to case 11.

## Answers to the handoff's 31 questions

1. **Re-derived, all exact.** Computed independently:
   indexer starts `align4096(1,837,056) = 1,839,104`, ends `2,016,512`;
   sidecar starts `align4096(2,016,512) = 2,019,328`, ends `2,051,328`;
   target-only logical `1,837,056 + 177,408 = 2,014,464`, physical
   `align4096(2,016,512) = 2,019,328 = 493 * 4,096`; MTP logical
   `2,046,464`, physical `align4096(2,051,328) = 2,052,096 = 501 * 4,096`.
   Every padding interval in the document matches. One-page SHA boundaries:
   piece SHAs over logical bytes only, physical SHA over the full padded
   extent — consistent and unambiguous.
2. **Yes, full-extent I/O is preferable.** One 493/501-block `READ_FIXED`/
   `WRITE_FIXED` per record gives one SQE/CQE, one alignment domain, no
   inter-piece seek/merge logic, and simpler abandonment (one terminal CQE).
   Per-piece durability metadata is retained losslessly because piece
   offsets/lengths/SHAs live in the record; nothing about per-piece
   verification is lost.
3. **Yes, both must be normative.** Without mandatory zero padding plus a
   physical-extent SHA, the 2,048–4,096-byte pad ranges are unconstrained
   bytes that can hide torn writes and stale data, and restore could not
   distinguish benign garbage from corruption. With them, every byte of the
   extent is accounted. Residual ambiguity is only the segment tail slack
   outside any extent (MINOR 4).
4. **Yes, subject to the mandated probe.** 4,096-byte offset/address/length
   alignment satisfies ext4/xfs O_DIRECT on both 512e and 4Kn NVMe. The live
   startup probe (ideally `statx` `STATX_DIOALIGN` plus an actual
   read/write/fsync on the store path) is correctly mandatory, since
   filesystem/mount options (e.g. xfs realtime, dax, encryption) can change
   effective requirements.
5. **Yes, with the constraints in MINOR 3.** A single VA range can be both
   io_uring-registered and CUDA-pinned; both mechanisms pin pages
   independently and coexist. Allocation must be page-aligned `mmap`/
   `posix_memalign`; registration order is free but teardown must be
   strictly reverse and only after zero outstanding descriptors; `fork`
   requires `MADV_DONTFORK`; memlock accounting must budget the double
   charge. The design's choice to keep CPU proof on nonpinned storage is
   sound.
6. **Yes.** `TierBufferId(slot, generation)` with generation-zero invalid,
   increment-before-visibility, and permanent retirement on overflow, plus an
   operation-generation descriptor table resolved from the user-data word, is
   sufficient: a late read CQE, async-cancel CQE, checksum-worker result, or
   CUDA completion each carry a generation that no longer matches after
   reuse, and the table entry is not reused until the terminal CQE is reaped.
   The one obligation is the CQ bound in MINOR 1 so no CQE is ever dropped.
7. **Yes as specified.** Every state names one owner (the I/O authority);
   handles never free in-flight buffers; last-waiter cancellation converges
   to FREE only through the terminal CQE pair; FAILED→QUARANTINED is a
   terminal sink; shutdown reaps all CQEs before unregistering. No state has
   two writers and no state lacks a terminal edge.
8. **Complete, and nothing required is unavailable.** READ_FIXED/WRITE_FIXED,
   FSYNC (data-only and full), registered files/buffers, and single-issuer
   all exist without SQPOLL on any io_uring kernel this project can target;
   async cancel is correctly optional with the logical-abandon fallback.
   Short-I/O behavior under O_DIRECT is filesystem-visible and the contract
   treats short completion as failure, which is the right posture. Nothing
   stated is filesystem-dependent in an architecture-blocking way.
9. **Yes.** For all orders — original-then-cancel, cancel-then-original,
   cancel returns ENOENT/EALREADY, cancel succeeds — the rule "buffer and
   descriptor stay owned until the original CQE (and the cancel CQE if one
   was issued) are both reaped" makes abandonment correct; no order permits
   early reuse and cancellation-not-found simply degenerates to waiting for
   the original CQE.
10. **Overflow cannot occur only if the MINOR 1 invariant is enforced.**
    Sizing CQ to the descriptor table alone is insufficient because async
    cancels generate their own CQEs; the exact invariant is stated in
    MINOR 1 and must be checked at startup (ring geometry) and at every
    submit (refuse, do not queue, beyond the bound).
11. **Yes.** Hashing in fixed workers is safe because the buffer is in an
    owned HASHING_* state, results are generation-checked, and the authority
    is the only transition owner. The authority never blocks: full checksum
    queues surface WAIT before work starts. No early reuse is possible
    because FREE is reachable only through the authority after hash
    acknowledgment.
12. **Yes.** `required_capability: TARGET | MTP` with "MTP ticket may satisfy
    target-only waiters, never the reverse" is the correct lattice; durable
    revision pinned in the ticket plus invalidate-and-replan before
    submission and pin-through-completion after submission handles upgrades
    arriving at any point.
13. **Yes.** Waiter order is frozen to request-ID ascending after admission;
    the physical charge is counted exactly once per ticket (see MINOR 2 for
    wording) and logical charges exactly once per waiter per tenant, so
    global physical and per-tenant logical ledgers stay exact under joins
    and cancellations.
14. **Yes.** The epoch pin plus cleaner step 8 (wait for all readers of the
    old epoch before unlink) means a relocation or MTP upgrade after
    submission cannot invalidate the extent under an in-flight read; the
    submitted read completes against the immutable old segment.
15. **Yes.** One shared physical HBM page plus N logical references is
    explicit. The later CUDA contract must add: H2D copy completion event
    recorded on the owning stream; residency publication ordered after that
    event; buffer FREE gated on the event (not stream idle); and a fence
    between checksum completion and copy submission so an unverified buffer
    is never copied.
16. **Yes.** DRAM_READY/DRAM_PINNED are defined as real host allocations
    keyed by content identity; the staging pool is excluded from cache
    accounting and ownership transfer requires an explicit capacity
    transaction, which prevents double accounting by construction.
17. **Yes, acceptable for v1.** At most one private partial tail per active
    sequence is bounded by the admission limit, and mandatory offload only
    concerns sealed pages. A session-tail spill format is correctly deferred
    to its own review; it is not a prerequisite for v1.
18. **Yes, with MINOR 5.** Read reserves (R0/R1), W1 confined below both low
    watermarks, and the maximum-consecutive-read byte budget before an
    accepted W0 advances jointly prevent read-buffer starvation and cleaner
    interference; the one unbounded case is new-W0-lease deferral under
    permanent read saturation, which should be explicitly declared
    acceptable or bounded.
19. **Yes, it can still hurt, and matched measurement is the only honest
    gate.** An issued fsync is not preemptible and device-internal GC can
    amplify it; the contract correctly refuses to claim an NVMe preemption
    guarantee and instead requires the matched resident-decode isolation
    rows. That is the right epistemic posture.
20. **Yes.** The nine-step order preserves the online-publication crash
    invariants (Begin durable before data, data durable before piece events,
    piece events before Publish, Publish before catalog visibility).
    Per-piece journal syncs can be driven through io_uring without
    reordering only if each barrier is an explicit dependency: either
    linked SQEs (append then FSYNC linked) or submit-and-await-CQE per
    barrier from the single issuer. The implementation contract must forbid
    concurrently outstanding journal appends across a barrier.
21. **Yes.** The upgrade writes a complete new 501-block extent and only
    then publishes the new catalog epoch; a crash at any point exposes
    either the old target record or the fully durable MTP record. Old and
    new extents coexisting is safe because both are immutable and the
    catalog names exactly one.
22. **Yes for the stated scope.** Immutable segments, deterministic
    selection, epoch-drain-then-unlink, and the read-only fallback bound
    long-running capacity. The missing piece is the exact relocation record
    format, which the document itself defers — see Q23.
23. **Missing before CPU implementation:** relocation journal record
    (transaction ID, source segment ID, destination segment ID, per-extent
    old offset, new offset, length, physical SHA, catalog epoch at pin,
    completion marker); catalog checkpoint encoding (epoch, shard table,
    per-shard digest, oldest-live-transaction); segment header/naming
    (segment ID, creation epoch, fixed size); startup rules for a
    half-complete relocation (data durable, metadata absent → orphan copy;
    metadata durable, catalog not published → roll forward or discard —
    pick one and prove it); and the garbage-counter rebuild rule
    (recomputed from the catalog at startup, never trusted from disk).
24. **Yes.** Unlink waits for epoch drain, and epoch references are held
    through I/O completion and checksum validation, so in-flight checksum
    jobs hold the segment alive; DRAM records are content-keyed and never
    reference segments, so they cannot dangle.
25. **Yes.** Copy-on-write shard tables replace only the changed shard plus
    the top table per publication, avoiding full-estate clones while every
    consumer sees exactly one epoch; restart rebuilds snapshots from
    journal/checkpoint replay with full validation before health. This also
    retires the current four-private-`FileTierStore` split, which is the
    single biggest architectural defect of the retained path.
26. **Correctly separated; no misfiled case found.** Checksum/padding/
    generation/extent violations are engine-fatal; `EIO`, device loss,
    ring failure, timeout, registration invalidation, and durability-barrier
    failure are tier-degraded; saturation and `ENOSPC` before lease are
    write-local. I probed for misclassification: fsync failure is correctly
    degraded (not write-local) because its blast radius exceeds one
    publication; read `EIO` is correctly degraded (not integrity) because
    the device, not the content, failed; `ENOSPC` mid-relocation stops
    writes and turns read-only rather than deleting live extents. Integrity
    is never downgraded to a cache miss.
27. **Yes.** Degradation is process-global, already-resident work continues
    only if it needs no failed-tier operation, full health is refused, and
    no rank-local fallback exists. No false-healthy path was found.
28. **Yes.** Shutdown reaps every original and cancel CQE, completes or
    safely fails accepted publication durability before closing, proves
    zero outstanding descriptors, and forced termination is defined to rely
    on journal recovery only — destructor timing is contractually
    irrelevant.
29. **Sufficient in structure, with three additions** (QUESTION 1–3): add
    explicit fsync-failure, CQ-overflow, and registration-invalidation
    cases. The 25 cases otherwise cover alignment, ABA, dedup, cancellation
    orders, live catalogs, cleaning crash stages, restart, and final-zero
    accounting, including the cross-read/migration boundary (case 25).
30. **Yes.** Identical model/context/batch/clocks/power/HBM posture/graph
    route with an exclusive time ledger, plus the explicit refusal of
    other-device/page-cache/tmpfs/synthetic evidence, is tight enough that a
    buffered-cache or device-mismatch claim cannot slip through.
31. **Must version atomically as one durable format version:** the physical
    record layout (piece table + segment_id/physical_offset/physical_length/
    physical_sha256), journal event encoding (publication and relocation),
    catalog checkpoint and shard encoding, segment header/naming,
    `TierBufferId.v1`, `RestoreTicketKey.v1` and ticket state encoding,
    quota charge schema (physical vs logical), residency state set,
    page-transaction types shared with serving, publication ticket/lease
    types, the fixed metric name set, and the io_uring dependency pin in
    `Cargo.lock`. A change to any one without the others is a mixed-format
    store and must be refused at startup by version check.

## Separate statements required by the handoff

- The aligned full-extent layout **is accepted** (arithmetic independently
  re-derived and exact).
- The io_uring/registered-buffer architecture **is accepted** (no SQPOLL
  dependency; cancellation and CQ obligations as stated in MINOR 1).
- Direct-format and pure CPU state-machine implementation **may begin**,
  incorporating the MINOR items into the durable format/implementation
  contract.
- The segment cleaner **does need a pre-implementation amendment**: the
  exact relocation journal/checkpoint fields of Q23. The document already
  anticipates this; the amendment is a precondition of cleaner code, not of
  this acceptance.
- The retained blocking store **remains a nonproduction control** (fixture
  generation and matched CPU oracle only; never a silent fallback).
- **K03 and K05 must remain unpassed** (punchlist rows verified OPEN at the
  candidate; nothing in this design constitutes the required measured
  evidence).
- HBM↔DRAM CUDA work **remains a separate gate** (buffer pinning, events,
  and copy ordering are explicitly out of scope here).
- **No finding changes the 1M capacity or tenant quota arithmetic**:
  physical accounting at 493/501 blocks per page and logical accounting at
  2,014,464/2,046,464 bytes are exact; the MINOR items do not alter any
  quantity.
- **No cn4 access, target-storage probe, destructive migration, or GPU
  launch is authorized by this verdict.**

## Architecture & maintainability

- **Layering is right.** One process-wide I/O authority owning ring,
  buffers, offsets, catalog publication, and counters — with ranks reduced
  to command submitters — eliminates the current four-private-store
  topology, which is the root cause of gaps 2, 6, and 10 in the document's
  own list. The single-issuer model also matches io_uring's cheapest
  operating mode.
- **The checksum worker pool is the correct decomposition**; keeping the
  authority free of 2 MiB SHA work while remaining the only state-transition
  owner avoids both stalls and dual-writer states.
- **Duplication risk to watch:** the blocking store and the direct store
  will both encode records and replay journals. Case 25 (cross-read) forces
  either a shared codec crate-internal module or an explicit migration
  boundary. Prefer one codec used by both stores; two hand-kept encoders of
  the same format is how the v1/v2 journal divergence happened in the
  retained store.
- **API surface is appropriately narrow**: commands, tickets, snapshots, and
  metrics; no filesystem types leak to ranks. Keep `TierBufferId` and ticket
  state types crate-private; only opaque handles should cross the serving
  boundary.
- **Simplification opportunity:** R0 vs R1 differ only by queue-latency
  target and ordering key; if the implementation shows R0 adds scheduler
  states without measurable benefit, collapse to one read class with a
  resume-priority bit before freezing the metrics schema, since class labels
  are fixed-cardinality metric dimensions.
- **The 25-case proof is the de facto specification of the state machine;**
  keep it executable (table-driven schedules) rather than 25 hand-written
  tests, or the "every order" claims in cases 7–8 will silently decay.

## Token decision

Zero blockers and zero majors; the five MINOR items are contract wording
and implementation-contract obligations that do not invalidate the design.
This is an unqualified design pass within the handoff's declared scope (no
io_uring implementation, storage benchmark, migration, or GPU evidence was
claimed or reviewed).

direct-tier-io-v1-accepted
