# Page reuse quarantine and in-place commit CPU proof v1

Date: 2026-07-29

Implementation commit:
`cd42ad4640f9d9d519ee970aaab07a4f51cad6d5`

Status: integrated CPU proof passed; independent review pending

GPU claim: none

## Corrected failure mode

Before this milestone, `SequencePageTable::release_page` returned a target
page ID and its optional draft page ID directly to the free sets.
`commit_tentative` first rolled the complete tentative range back and then
appended the accepted count. Deterministic lowest-ID allocation usually
reacquired the same accepted pages, but that behavior was not an ownership
proof and left an ABA path between rank-visible removal and allocator reuse.

The corrected implementation has two coupled rules:

1. tentative commit changes every accepted reserved physical page in place
   from `HbmTentative` to its exact committed mutable or sealed state; and
2. every fully released target/draft ID enters a disjoint quarantine rather
   than either free set.

Rejected MTP suffix pages, rollback pages, private terminal tails, and the
last reference to a shared prefix page all use the same quarantine route.
Accepted target and draft IDs are never freed and reacquired.

## Generation-bound reuse

The host table exposes one explicit two-phase API:

```text
bind_reuse_quarantine(generation)
acknowledge_reuse_quarantine(generation)
```

Binding freezes the table against further mutations and associates every
currently retired ID with exactly one nonzero page-table successor
generation. A wrong, missing, repeated, or zero-generation acknowledgement
fails closed. Only exact acknowledgement moves the target and draft IDs back
to their owner-rank free sets.

`ServingCoordinator` binds the candidate table before transmitting the
commit/removal `PageTableDelta`. `Tp4WorkerPool::apply_page_delta` already
returns only after validating all four rank receipts against the exact
successor generation, global digest, and rank-local digest. The coordinator
therefore acknowledges allocator reuse only after that method succeeds and
before publishing the candidate as authoritative.

When the worker generation has failed and been retired, host cleanup does
not forge an acknowledgement. The cleaned host candidate retains its bound
quarantine and cannot perform another mutation. The existing fail-stop
coordinator cannot continue using that closed worker pool.

## Atomicity and capacity

Quarantined IDs are absent from both free sets and the active physical map.
They therefore count against allocatable arena capacity until receipt even
though no active sequence can resolve them. Shared pages enter quarantine
only when their reference count reaches zero.

All mutators preserve the existing clone-backed CPU-oracle rollback: an
error restores active mappings, reference counts, free sets, quarantines,
prefix bindings, and tentative state together. This milestone does not claim
that clone as the production fixed-capacity hot path.

## Regressions

The cache suite proves:

- removed target and draft IDs cannot be allocated before acknowledgement;
- an unbound, wrong-generation, and bound-table mutation all fail closed;
- exact generation acknowledgement makes the same deterministic IDs
  reusable;
- tentative MTP commit retains accepted target and draft IDs in place;
- rejected cross-page suffix IDs enter quarantine;
- empty and shared-prefix removals do not invent retirements; and
- the prior atomic removal and all 1M/tail/depth arithmetic tests still pass.

The serving suite proves that admission, reservation, commit, terminal
removal, cancellation, late rollback, fatal drain, prefix cleanup, MTP
fallback, and the exact 1,048,576-position lifecycle still pass through the
integrated four-rank mirror path.

Repository-wide verification against the documented implementation:

```text
scripts/local-checks.sh
```

Results:

- 286 Rust tests passed with zero failures, including 70 `glm-cache` and 41
  `glm-serving` tests;
- workspace formatting and Clippy with warnings denied passed;
- CUDA FFI type checks and deterministic CPU proofs passed; and
- all 69 then-present review handoffs passed provenance validation with 0/50
  configured result artifacts.

The external tokenizer proof was skipped because `GLMAXX_TOKENIZER_DIR` was
unset. CUDA compilation was skipped because `nvcc` is not installed on this
CPU host.

Implementation hashes:

```text
crates/glm-cache/src/lib.rs
bc3f31265e26638afd40307262afa1947d5cc2e88cfea96a18399d9fcee1cf7d

crates/glm-cache/src/sequence.rs
8c0491d4f2d3e50da12e15961c8ac65a2fe5449a3527d40a38cdaa5ef27d644e

crates/glm-serving/src/lib.rs
362312a48e1269f09f2f3f6e090dffcf896a8b6c688b65d6060e6b505aae0bae
```

## Exclusions

This is a CPU metadata and four-rank receipt-ordering proof. The rank mirrors
still contain CPU metadata, not CUDA-visible page tables. The receipts are
not CUDA upload events and do not establish stream visibility, payload
zeroization, or device-arena teardown.

The page table still clones `BTreeMap`, `BTreeSet`, and `Vec` state for
rollback. `PageTableDelta` and rank mirrors still allocate owned storage.
The fixed-capacity transaction journal, preallocated rank-local allocator,
explicit `CACHE_ONLY` plan ABI, CUDA stream receipt, direct tier movement,
checkpoint execution, model quality, capacity under live payloads, and
performance remain open.
