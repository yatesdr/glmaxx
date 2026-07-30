# Fable handoff: fixed-capacity page transaction v1 r2

Date: 2026-07-30

Status: corrective adversarial design review requested

Review candidate commit:
`b59114734e1fb18761725444e27fbe9c64b6ad43`

Required result path:
`docs/reviews/fable-fixed-page-transaction-v1-r2.md`

Requested acceptance token, only for an unqualified design pass:
`fixed-page-transaction-v1-r2-design-accepted`

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch GPU, container, storage-device,
or network work for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if needed. Hash every candidate input at review start and finish.
Any mismatch must withhold the token as a stale candidate.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/fixed-page-transaction-v1-r2.md` | `aa5e40db3902425735e43665bd104124970179d15b8354a783c3dd7eb90ca495` |
| `docs/fixed-page-transaction-v1.md` | `c03dd66f78b8e81ce5b0743d34091449d84c43d08e620a694a0c66b318a5d6fc` |
| `docs/serving-page-transaction-v1.md` | `31983cce95ee01a5968213d5daf12c7a855f75f8735314700f2b4a9e55625d1a` |
| `docs/prefill-graph-profile-abi-v2.md` | `37154c9e31109acdf35a382c6be87b3a865e2b7f6ae8f801969526789dd41f91` |
| `docs/step-execution-abi-v3.md` | `1cde3bcabba0a0d861691b06ddb140cb64dfbefaab1129c8a04bc302c0ce609e` |
| `docs/page-reuse-quarantine-proof-v1.md` | `94b6c39ee57fafa926d6bc375bf2841c00f8586c38fe99700d54e9b86065d84c` |
| `docs/fable-fixed-page-transaction-v1-handoff.md` | `f81525100be078d4e455bd9c7c63b3b833bd4ec5dfd430aa4cab73a751e33c41` |
| `crates/glm-cache/src/delta.rs` | `71ac2da15e869a6f2470c3551a7cd6ec4ff387850a23240e9a44ad96a538ff16` |
| `crates/glm-cache/src/sequence.rs` | `8c0491d4f2d3e50da12e15961c8ac65a2fe5449a3527d40a38cdaa5ef27d644e` |
| `crates/glm-engine/src/worker.rs` | `3533f606400c8aa5c571caa360ba516abd69d39de0489b87be4658143a9bdc24` |
| `crates/glm-serving/src/lib.rs` | `bc7eff0297e14b73df7eec5ade3352ad0f75ceabeaca1862c4866a51efb948e3` |
| `docs/production-punchlist.md` | `17c55db9e90e3c96eeebd729c222d49132614445e244132f627d890dc21b4967` |
| `docs/results-index.md` | `8c4d67cb0bc6640ab9389cbeae74fb3dfd00d18a545d52bd3fae698f7d5b190a` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-fixed-page-transaction-v1-r2-handoff.md
git diff --check b59114734e1fb18761725444e27fbe9c64b6ad43^ \
  b59114734e1fb18761725444e27fbe9c64b6ad43
```

The handoff itself is coordination metadata added after the candidate and is
not a candidate input. The first review is operator-owned read-only input at
`docs/reviews/fable-fixed-page-transaction-v1.md`; do not modify or add it to
the candidate.

## Review purpose

The first review independently proved that 174 prefill edits, 128
decode/verify edits, 64 row undos, and 64 rejected-page retirements are exact
and sufficient. It withheld acceptance on two architectural defects:

1. the retained rank mirror clones and validates complete mappings on every
   delta, making ordinary application O(total context pages); and
2. first install and complete resend can carry 16,384 entries despite only
   174 entries of named rank staging.

This amendment makes ordinary mirror work suffix-bounded and observable,
streams first install directly into admission-owned inactive final storage,
and removes complete resend from production. It also resolves all six minor
findings and both questions in the first review.

This is a design review. Do not accept an implementation or infer that the
current owned `PageTableMirror` is production-capable.

## Review boundary

Acceptance covers only:

- amendment precedence and production-route selection;
- fixed startup page indexes, reverse ownership, indexes, journals, roots,
  admission pipelines, and queues;
- bounded ordinary suffix application and incremental invariant maintenance;
- accessor-level touched-work and allocation proof requirements;
- admission-owned block capacity and bounded first-install/restart streams;
- active-publication serialization across concurrent inactive installs;
- generation, abort, retirement, and quarantine behavior;
- standalone terminal removal and cache-only command choice;
- the corrected compute transaction state machine; and
- the required CPU oracle/mutation proof.

Acceptance does not cover:

- any current Rust implementation;
- `PageTableDelta.v2` or the coordinated execution ABI as a whole;
- a CUDA-visible table, device-memory integrity hash, stream event, graph, or
  collective;
- KV payload arenas or HBM/DRAM/NVMe movement;
- checkpoint loading or execution;
- GLM-5.2 output, quality, context capacity under live payload allocations,
  latency, or throughput;
- production health; or
- cn4 access.

## Required adversarial questions

1. Do all thirteen candidate-input hashes match at review start and finish
   in one detached worktree?
2. Is amendment precedence explicit enough to remove ordinary complete
   resend and compute-plan `CACHE_ONLY` without weakening any unaffected base
   rule?
3. Is the 174 bound correctly ordered after acceptance of the 3,072-row
   prefill ABI, and does the amendment require rederivation/static assertions
   if any controlling bound changes?
4. Does eight successor positions still fit the 128 decode/verify edit bound
   for all 64 rows and tail occupancies?
5. Does startup name every container that would otherwise grow, including
   page blocks/directories, target/draft reverse ownership, request/prefix
   indexes, journals, admission pipelines, commitment nodes, and queues?
6. Does reserving exact blocks from a startup-sized arena charge large
   sequence final storage without requiring either hidden allocation or
   `64 * 16,384` committed page entries?
7. Are the host 256-block and owner-local 64-block maxima exact for a
   1,048,576-token sequence with 64-entry blocks and DCP4 ownership?
8. Is sequence-slot and slot-generation identity collective, digest-bound,
   nonwrapping, and independent of rank-local hash placement?
9. Does ordinary preflight inspect only descriptors, the one unchanged
   boundary, old/new suffix records, reverse-owner cells, and fixed hash
   paths?
10. Are both old and new suffix lengths bounded, including in-place
    replacement and shrink-only rollback, with one old/optional-new operation
    record per ordinal?
11. Can deterministic owner checks plus arena-indexed target/draft
    reverse-owner cells preserve physical uniqueness without scanning any
    unchanged sequence?
12. Does incremental leaf/path/root maintenance commit to the full logical
    predecessor/successor while doing work proportional only to touched
    pages?
13. Is the logical-root codec correctly held for a separate byte-level
    implementation review rather than overclaimed as already frozen or
    production accepted?
14. Do accessor-bound touched-work counters distinguish a nonallocating
    full-table scan, one unchanged-prefix read, and another-sequence visit?
15. Do touched-work plus allocator mutations distinguish both O(context)
    work and hidden heap growth at 1M context?
16. Does first install write at most 174-entry chunks directly into
    admission-owned inactive final blocks, with no sequence-sized rank
    staging?
17. Is the eight-slot pipeline resource-balanced under out-of-order
    completions, slot reuse, duplicate/skipped chunks, saturation,
    cancellation, and rank failure?
18. Can multiple inactive installs stream concurrently without altering the
    active table root or colliding on the global page-table generation?
19. Does serializing only final active publication bind the then-current
    predecessor/root and avoid stale-successor or lost-install executions?
20. Can any partial begin/chunk/commit/abort make an uncertain block or
    physical ID allocator-visible, or host-publish a sequence absent from one
    live rank?
21. Is deliberate rebuild/restart forced through acknowledged removal and
    bounded install, with no rank-, request-, tenant-, or operation-local
    complete resend fallback?
22. Does the corrected state machine include both `Reserved -> RolledBack`
    and `Executed -> RolledBack`, and is exact ID reservation now clearly the
    first journaled mutation?
23. Are all terminal sequence IDs placed only in arena-sized quarantine and
    excluded from the 174/128 edit and 64 rejected-page retirement counts?
24. Does following a completed/rolled-back physical step with standalone
    removal keep a 16,384-page cancellation out of the compute hot path?
25. Is standalone `ApplyDelta` implementable without a model plan, graph
    execution, or collective, while correctly leaving CUDA upload/receipt
    details unaccepted?
26. Does the proof matrix explicitly cover shrink-only rollback, 1M bounded
    install/restart/removal, every pipeline failure point, root mutations,
    route mutations, touched-work mutations, and zero post-health allocation?
27. Is the distinction between an incremental CPU logical-state commitment
    and the pending full CUDA device-memory integrity gate accurate and
    fail-closed?
28. Are every implementation, GPU, KV payload, tier, checkpoint, model,
    quality, capacity, performance, production-health, and cn4 exclusion
    accurate?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately whether:

1. ordinary production mirror work is bounded by changed suffixes rather
   than total context;
2. startup storage and admission ownership have no hidden large-mapping
   buffer;
3. streamed first install and concurrent inactive preparation are bounded
   and generation-safe;
4. active install publication and abort/retirement are atomic and
   fail-closed;
5. rollback, removal, quarantine, and cache-only routes are unambiguous;
6. touched-work/allocation proofs distinguish both original defects;
7. process-global route selection forbids complete-resend fallback;
8. the CPU logical commitment does not overclaim CUDA memory integrity;
9. the required CPU proof is sufficient; and
10. all non-acceptance and GPU exclusions are accurate.

Only if all twenty-eight questions and all ten statements are unqualified
`YES`, end with:

```text
fixed-page-transaction-v1-r2-design-accepted
```

Withhold for stale provenance, context-linear ordinary work, an unchanged
prefix scan, incomplete startup storage, uncharged page-index or rank
staging, a generation collision between admissions, partial publication,
early reuse, an ordinary complete resend, request/rank-local fallback,
compute-plan cache-only dependency, nondistinguishing proof, device-integrity
overclaim, or any implementation/GPU/model/performance overstatement.
