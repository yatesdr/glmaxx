# Hybrid MTP3 capacity ledger v1 r2

Date: 2026-08-03

Status: corrective design candidate; adversarial acceptance required before
CPU implementation

GPU evidence: none

## Correction and retained target

This contract supersedes `hybrid-mtp3-capacity-ledger-v1.md`. It retains the
reviewed cache geometry and physical-allocation standard, but replaces v1's
128-byte ModelOpt record premise with the direct W4A16 codec and 192-byte
metadata from `nf3-nvfp4-native-rank-manifest-v1-r2.md`.

The target remains:

```text
maximum addressable positions       1,048,576
admitted target positions             524,288
DCP degree                                  4
committed positions per rank           131,072
maximum active sequences                    64
maximum MTP depth                             3
page tokens                                  64
```

It preserves one-million-position page-table arithmetic without claiming one
million physically resident tokens.

## Corrected weight boundary

The exact pre-alignment codec-record cross-check is:

```text
protected payload                                      11,959,396,352
NF3 records       14,400 * (3,833,856 + 384)            55,213,056,000
ModelOpt records   5,056 * (5,308,416 + 384)            26,841,292,800
minimum rank codec-record bytes                        94,013,745,152
```

This minimum is not allocation authority. The complete ordered native planner
charges:

```text
device weight arena       94,006,274,048
device metadata arena          9,961,408
total immutable arenas    94,016,235,456
```

The exact arena remains numerically unchanged from the earlier native
sensitivity because 192-byte ModelOpt records still occupy one 256-byte device
slot and the final metadata tail is 192 bytes. Raw file metadata rises while
device alignment padding falls by the same 647,168 bytes. File/host metadata
copies remain separate measured lifetime terms.

## Cache arena

Each rank reserves:

```text
131,072 committed + 4,096 owner/page slack + 256 MTP3 tentative
  = 135,424 physical slots
  = 2,116 pages
```

The exact compressed cache charge is:

| Per-rank term | Formula | Bytes |
|---|---:|---:|
| target KV | `135,424 * 78 * 368` | 3,887,210,496 |
| target indexer | `135,424 * 21 * 132` | 375,395,328 |
| draft KV | `135,424 * 1 * 368` | 49,836,032 |
| draft indexer | `135,424 * 1 * 132` | 17,875,968 |
| MTP3 cache arena | sum | 4,330,317,824 |

Target and draft reserve the same slot count so an effective-depth change does
not alter ownership or allocation. Scheduler admission enforces both the
global logical quota and each rank's physical page quota before publication.

## Exact sensitivity

Using the measured minimum rank floor from the native-manifest sensitivity:

```text
immutable arenas             94,016,235,456
MTP3 cache arena              4,330,317,824
weight plus cache            98,346,553,280
measured minimum rank floor 101,367,742,464
remaining for all else        3,021,189,184
older fixed sensitivity       2,550,136,832
provisional margin              471,052,352
```

This is not a fit claim. The final ledger separately charges CUDA contexts,
modules, libraries, allocator state, collectives, graphs, maximum simultaneous
scratch, serving metadata, page tables, journals, fragmentation, and at least
1 GiB of genuinely unallocated emergency escrow.

The budget records `load_peak`, `serve_peak`, and their maximum. Terms may
alias only across an implemented four-rank transition that quiesces, destroys
the old owners, verifies release on every rank, then allocates the new owners.
Hot reload may not overlap a loader arena with KV merely because the normal
cold path does not.

## Planner and physical gate

The checked planner rejects profile, rank, DCP, address-domain, admitted-count,
MTP-depth, page, codec, metadata, arena, scalar-arity, or rank-consensus drift;
all arithmetic overflow; unexplained resident bytes; less than 1 GiB escrow;
and either phase peak above the minimum physical rank.

Capacity passes only when one isolated four-rank cn4 run:

1. authenticates and loads every real native weight and metadata byte;
2. instantiates the final modules, collectives, graphs, and maximum workspace;
3. enforces and records the loader-to-serving lifetime transition;
4. allocates exactly 2,116 target and draft pages per rank with all page tables
   and journals;
5. writes and device-checksums every cache byte, not a sample;
6. reconciles observed per-allocation deltas to the machine-readable ledger;
7. proves at least 1 GiB remains free independently on every rank; and
8. frees only GLMAXX-owned allocations and records the post-run state.

Virtual reservation, configuration, aggregate HBM, or one-rank success cannot
pass.

## Nonclaims and gate

Design acceptance opens only checked CPU ledger/planner implementation,
followed by separate implementation review. This document accepts no native
image, device allocation, checkpoint, quality, capacity, or performance result.
