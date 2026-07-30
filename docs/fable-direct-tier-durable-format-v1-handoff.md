# Fable handoff: direct-tier durable format v1

Date: 2026-07-30

Status: adversarial design review requested

Review candidate commit:
`96be26e8a1d43cac047cd57a38bf3d13f6dbb756`

Required result path:
`docs/reviews/fable-direct-tier-durable-format-v1.md`

Requested acceptance token, only for an unqualified pass:
`direct-tier-durable-format-v1-design-accepted`

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch GPU, filesystem, NVMe, or
container work for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if necessary, run `review-proof`, and hash every input at review
start and finish. A mismatch is a stale candidate and must withhold the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |
| `docs/direct-tier-durable-format-v1.md` | `19ca03edeab89b560d674689ca96ce497f2c5859a91d5fe5d4b50c78645e79e6` |
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `docs/online-prefix-publication-v1.md` | `67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873` |
| `docs/direct-tier-extent-cpu-proof-v1.md` | `d54ad467e8f2219ec31638416ff5a0a74cf972a6077695b6eea7dd1b8eb859b1` |
| `docs/direct-tier-state-cpu-proof-v1.md` | `3f58a9c1b7ad7cc4806598b467f02eb746013e75a72e4566f9e9ba55f466df66` |
| `docs/tenant-resource-quotas-v1.md` | `d779e4d6a4e4a6b5b57e4c76ab1cee504361df76ff8d2d78b174db00e4528cab` |
| `docs/production-punchlist.md` | `47acb6544d8912f778779e53f0da67a2283d10f6238ddd025377f5acd6870da2` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/native-engine-plan.md` | `33552cd81e3d79b8b484856a99620420f3e2eddfdfa529a23b191353a702ed80` |
| `scripts/local-checks.sh` | `56f728cdf3f047f9633509a57341d25a977efa802f0d5b371c9716830517db59` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-direct-tier-durable-format-v1-handoff.md
git diff --check 96be26e8a1d43cac047cd57a38bf3d13f6dbb756^ \
  96be26e8a1d43cac047cd57a38bf3d13f6dbb756
```

The handoff itself is coordination metadata added after the candidate and is
not a candidate input.

## Why this gate exists

The accepted direct-tier review allowed the extent and pure CPU state-machine
work but explicitly blocked cleaner implementation until exact relocation
journal, checkpoint, segment, half-complete recovery, and garbage-rebuild
rules existed. It also required one atomic durable-format version boundary
and five minor contract fixes.

The candidate is that missing amendment. It deliberately contains no codec or
storage implementation, because repository gate order requires this design
review first.

## Review boundary

This review covers only the durable-format contract in
`direct-tier-durable-format-v1.md` and its consistency with the pinned
accepted design inputs.

Acceptance permits a separate pure CPU implementation/proof of the exact
codec, catalog, journal, checkpoint, recovery, publication, eviction,
visibility, relocation, and retirement state machines.

Acceptance does not accept or authorize:

- an `io_uring` service or actual `O_DIRECT` path;
- filesystem, crash-power-loss, NVMe, bandwidth, or endurance evidence;
- registered or CUDA-pinned memory;
- HBM-to-DRAM or DRAM-to-NVMe device integration;
- cn4 access;
- K03, K05, serving, checkpoint-model, quality, capacity, or performance
  completion; or
- cleaner code before this token is present.

## Required adversarial questions

1. Do all pinned hashes match at review start and finish in a detached
   worktree, regardless of later `main` movement?
2. Are all fixed-object offset/size tables arithmetically exact, nonoverlapping,
   fully covering their declared byte count, and unambiguous about zero
   reserved bytes?
3. Are every SHA and CRC coverage range, self-field-zero rule, domain string,
   journal predecessor hash, and logical-versus-physical digest boundary
   unambiguous and noncircular?
4. Does the 4,096-byte format descriptor atomically bind every schema named by
   the prior review's answer 31, including the io-uring dependency lock
   record, while preserving deterministic fixtures?
5. Do the exact store filenames, bootstrap order, lifetime control-file lock,
   and one-authority rule prevent two writers or a private-catalog path?
6. Is the retained `GLTJRNL2` store unambiguously incompatible, with neither
   cross-replay nor silent fallback?
7. Are segment header/naming, 1-GiB fixed size, 4,096-byte first extent,
   531/523 extent maxima, state/purpose separation, tail-slack rule,
   nonreusable ID high-water, and fixed-capacity accounting exact?
8. Is the 512-byte catalog entry sufficient and exact for target and MTP
   records, including piece offsets, physical digest, parent metadata,
   visibility state, writer rank, revision, and restore-bound entry SHA?
9. Can out-of-order durable children be represented as pending, remain
   unavailable to restore/prefix hits, recover across checkpoint, and become
   visible only in deterministic parent-first transactions?
10. Is the dedup/MTP-upgrade/collision matrix preserved for pending and visible
    records, and is catalog eviction leaf-first and safe despite revision one
    being reused only after an entry is absent?
11. Is the journal common header exactly 4,096 bytes with a 3,972-byte maximum
    payload, strict sequence/hash chain, globally nonreused transaction IDs,
    and no interleaved transaction ambiguity?
12. Does each journal header bind its checkpoint, transaction high-water,
    prior generation, and prior tail strongly enough for safe two-generation
    rotation?
13. Does publication enforce Begin sync before data, data sync before extent
    event, each piece event sync before Commit, and Commit sync before catalog
    visibility, with no concurrently outstanding append crossing a barrier?
14. Does recovery-only `PublishAbort` close every final begun-but-uncommitted
    publication without installing content, reusing its extent, refunding its
    endurance charge, or allowing later records behind a nonterminal
    transaction?
15. Is `VisibilityCommit` constrained so it changes only pending state and
    entry SHA, checks the exact parent/ordinal, and advances one catalog epoch
    durably?
16. Is `CatalogDelete` constrained by child/reference checks, epoch/root
    binding, and old-entry digest so it cannot remove a reachable parent or a
    changed entry?
17. Are checkpoint header, segment table, 256 descriptors, padded shard
    payloads, catalog-root reconstruction, empty-table conventions,
    endurance buckets, and both high-water marks fully decodable without
    implementation guesswork?
18. Does the two-slot control algorithm select a torn-versus-corrupt slot
    safely, refuse reference corruption rather than silently falling back,
    and retain a real previous recovery generation through slot update?
19. Across crashes before and after checkpoint write/sync/rename, journal
    header write/sync/rename, control write/sync/reread, and old-file
    reclamation, is at least one accepted recovery generation complete?
20. Does every relocation plan contain transaction ID, source/destination
    segment IDs, pinned catalog epoch/root, ordered old/new offsets, lengths,
    physical SHA, old entry digest, complete new entry, mapping digest, and a
    synced completion marker as required by the prior review?
21. Can ordinary publications proceed while relocation data copies without
    weakening correctness, with every mapped upgrade/delete/visibility change
    detected by the old-entry digest check before one atomic relocation
    epoch?
22. Is destination data forbidden before PlanCommit, fully verified and
    synced before relocation publication, and always orphan rather than
    authoritative when no RelocationPublish exists?
23. Is a durable RelocationPublish always rolled forward after crash, with no
    state in which old metadata selects new bytes or new metadata selects old
    bytes?
24. Does `RelocationAbandon` safely terminate a complete but unpublished plan,
    preserve the source, retain endurance/ID high-water, and make only the
    destination reclaimable?
25. Does SegmentRetire wait for current mappings and runtime epoch/ticket/hash
    references, and does two-control-generation retention prevent deletion of
    a source needed by fallback recovery?
26. Are missing `RETIRE_PENDING` files accepted only when both valid recovery
    generations prove retirement, while every unmarked missing source remains
    corruption?
27. Are the startup decisions for short tail, complete corruption, partial
    unsynced plan, incomplete publication, complete publication, unresolved
    plan, relocation publish/abandon, visibility, deletion, and retirement
    mutually exclusive and complete?
28. Are partial unsynced relocation-plan records safely truncatable only
    because data was forbidden, with their observed transaction ID still
    retained in the checkpoint high-water?
29. Are allocated/live/garbage/tail counters reconstructed from catalog and
    allocator state with checked arithmetic, never trusted from disk, and
    correct for pending records and old runtime epochs?
30. Is the 24-bucket data-extent endurance definition conservatively
    reconstructable from Begin/PlanCommit, explicit about excluded metadata
    writes, and free of a false NAND-wear claim?
31. Do CQ arithmetic, NODROP independence, ticket-scoped physical charging,
    double memlock, `MADV_DONTFORK`, teardown order, fsync faults,
    registration invalidation, and the periodic W0 admission bound fully
    resolve every minor/question from the accepted prior review?
32. Is the post-review CPU proof list sufficient to mutation-test every
    encoding and crash decision before Linux I/O or cleaner/device code?
33. Does this amendment fully satisfy the prior review's answer 23 without
    leaving any implementation-defining relocation/checkpoint/startup choice
    unstated?
34. Are all exclusions accurate, with no CPU implementation, filesystem
    durability, io_uring, NVMe, CUDA, cn4, K03/K05, serving, model, quality,
    capacity, or performance evidence implied?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer each statement separately:

1. fixed file/object layouts and integrity coverage are byte-exact;
2. format identity, bootstrap, locking, naming, and legacy refusal are
   fail-closed;
3. catalog content, visibility, dedup, upgrade, collision, and deletion
   semantics are complete;
4. journal sequencing and publication crash recovery are complete;
5. checkpoint/control rotation retains a valid recovery generation;
6. relocation plan/copy/publish/abandon/retire rules are complete and
   crash-safe as a design;
7. startup decisions are deterministic and contain no discard-versus-roll
   forward ambiguity;
8. capacity, high-water, garbage, and endurance reconstruction are exact;
9. all prior direct-tier minor obligations are incorporated;
10. the atomic schema boundary is sufficient to reject mixed runtimes;
11. the required CPU proof is implementable without inventing format
    semantics; and
12. accepting this design opens only the declared CPU proof and no stronger
    production claim.

Only if all thirty-four questions and all twelve statements are unqualified
`YES`, end with:

```text
direct-tier-durable-format-v1-design-accepted
```

Withhold the token for stale provenance, arithmetic ambiguity, uncovered
bytes, circular hashes, mixed-format acceptance, catalog exposure of a
pending child, revision/entry ABA, an unclosed publication transaction,
unsafe checkpoint fallback, a relocation state that can select the wrong
extent, premature unlink, reusable high-water IDs, trusted garbage counters,
nonreconstructable endurance, unresolved W0/CQ/memlock obligations,
unimplementable CPU gates, or evidence overstatement.
