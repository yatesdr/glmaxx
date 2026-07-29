# Durable tier-store write fail-stop CPU proof v1

Date: 2026-07-29

Implementation commit:
`10a7bca1b6f8d4eeac50b97cc390495918cee3d4`

Status: CPU durability correction passed; independent review pending

GPU claim: none

## Defect and fail-stop boundary

`FileTierStore::publish` previously returned an ordinary error after any
failure, including failures after it had:

1. advanced the in-memory transaction sequence;
2. appended and synchronized a begin record;
3. written and synchronized page payloads;
4. appended piece-durable records; or
5. published the transaction in the in-memory journal.

The same live store would then accept another publication. That is unsafe for
real I/O errors. A failed `write_all` may have appended only part of a
512-byte journal record. Appending another record after that partial write
turns the corrupt tail into an interior record, so replay can no longer treat
it as a trailing crash remnant. A failed data or journal sync also leaves
durability unknowable to the live process.

The corrected store has two explicit phases:

- request sorting, stale-generation checks, piece validation, checked offset
  calculation, and complete `TierRecord` validation are preflight; and
- transaction allocation, in-memory journal mutation, durable appends,
  payload writes, syncs, piece attestations, and publication are the commit
  phase.

An error during preflight leaves the writer usable. Any error returned from
the commit phase sets `write_poisoned`. Every later publication returns
`WritePoisoned` before changing the journal file, data file, transaction
sequence, or published map.

Reads of pages published before the failure remain available. Reopening the
store is the only recovery path: replay reconstructs the published set from
fully durable transactions, ignores unpublished orphans and a torn trailing
record under the existing format rules, and creates a new unpoisoned writer.

## Distinguishing CPU proof

`failed_publication_poison_writes_until_replay_but_not_preflight_errors`
first proves that two preflight failures do not poison the writer:

- a request with no pieces is rejected, after which the corrected request
  publishes successfully; and
- a stale generation is rejected, after which a newer generation publishes
  successfully.

It then injects a failure after the first piece-durable record. The test
records both file lengths and proves that:

- the next publication returns exactly `WritePoisoned`;
- neither journal nor data length changes on that rejected call;
- a page published before the failure is still readable;
- the failed page is invisible after close and replay; and
- a new publication succeeds after reopen.

`crash_before_publication_leaves_only_invisible_orphans` covers every
available durable phase failpoint:

- begin record synchronized;
- data payload synchronized; and
- first piece-durable record synchronized.

For each phase, a second live publication is rejected as poisoned and replay
keeps the failed page invisible. The prior implementation accepts that second
publication, so it fails the distinguishing assertion at all three phases.

## Gate result and exclusions

The full local gate passed 247 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, and all 45 then-present review
handoff provenance proofs.

Commands:

```text
cargo test --workspace --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo fmt --all -- --check
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-cache/src/store.rs
99a8c9d2ba049bec36dc8be16571078827371d506a02eeeac15e499a0e458eb3

docs/online-prefix-publication-v1.md
67b89027e0e5ae7a3973bb0dfd80e91df6a92afbb9e5ca2c9199bb06adec3873

docs/direct-tier-io-v1.md
7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2

docs/cache-lifecycle-proof-v1.md
11ad4936fea7cd0887e660911f50778d5b0918c21a6cebaca1a98a244b2e2de1

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a

spec/format-v0.md
619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, collective, checkpoint, or model execution
was used.

This correction does not implement the reviewed direct-I/O design, online
runtime publication leases, registered buffers, `io_uring`, segment
cleaning, asynchronous rank uploads, HBM/DRAM movement, or production error
recovery. It does not change the journal format or claim that an in-process
poisoned writer can be repaired safely. It establishes only that the current
CPU file-store never performs a second write after its durable state becomes
uncertain.
