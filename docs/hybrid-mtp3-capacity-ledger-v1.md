# Hybrid MTP3 capacity ledger v1

Date: 2026-08-03

Status: weight-accounting boundary superseded by
`hybrid-mtp3-capacity-ledger-v1-r2.md`; do not implement or issue the v1 token

GPU evidence: none

## Purpose

This contract defines the first fit target for the real GLM-5.2 NVFP4/NF3
checkpoint: 524,288 admitted target positions with MTP3 on four DCP ranks,
while retaining 1,048,576-position address and page-table arithmetic. It does
not claim that the checkpoint fits or that any KV byte is allocated on SM120.

The existing `glmaxx.profile-budget.v0` artifact remains a capacity-EXL3,
1,048,576-position, MTP6 artifact. It cannot admit the NVFP4/NF3 checkpoint
and must not be reinterpreted.

## Fixed identities and weight charge

The source checkpoint identities and tier membership are those pinned in
`docs/cn4-hybrid-source-inventory-20260803.md` and
`docs/nf3-nvfp4-hybrid-source-and-kernel-v1.md`:

```text
target NF3 experts                         14,400
target NVFP4 experts                        4,800
draft NVFP4 experts                           256
all NVFP4 experts                            5,056
```

The exact codec-record charge per rank before native-container alignment is:

```text
protected allocations                  11,959,396,352
NF3 records       14,400 * 3,834,240 = 55,213,056,000
NVFP4 records      5,056 * 5,308,672 = 26,840,645,632
minimum rank weight-record bytes        94,013,097,984
```

The completed native rank manifest remains allocation authority. It records
the exact weight arena, metadata arena, every alignment gap, descriptor table,
and allocator rounding. The finalized budget must charge those measured
arena bytes, not the 94,013,097,984-byte minimum. Alignment or metadata cannot
be hidden in KV, escrow, or nominal free HBM.

## Logical capacity and physical arena

The profile constants are:

```text
maximum addressable target positions        1,048,576
admitted target positions                      524,288
DCP degree                                           4
committed target slots per rank                 131,072
maximum active sequences                             64
maximum MTP depth                                      3
page tokens                                           64
```

The scheduler preserves the one-million-position coordinate domain even
though only 524,288 positions are admitted concurrently. It enforces the
global logical quota and every rank's physical-page quota before publication.
No aggregate-free-memory or rank-local fallback is allowed.

At C64, every mutable tail can require the same DCP owner. Each rank therefore
reserves 4,096 owner/page-slack slots. An MTP3 verifier can expose four target
positions per active sequence, so target and draft arenas each reserve
`64 * (3 + 1) = 256` tentative slots. The exact arena is:

```text
131,072 committed + 4,096 page slack + 256 tentative
  = 135,424 slots
  =   2,116 pages per rank
```

The current compressed record geometry gives:

| Per-rank term | Formula | Bytes |
|---|---:|---:|
| target KV | `135,424 * 78 * 368` | 3,887,210,496 |
| target indexer | `135,424 * 21 * 132` | 375,395,328 |
| draft KV | `135,424 * 1 * 368` | 49,836,032 |
| draft indexer | `135,424 * 1 * 132` | 17,875,968 |
| MTP3 cache arena | sum | 4,330,317,824 |

Committed payload alone is 4,191,158,272 bytes per rank and
16,764,633,088 bytes across TP4. The 4,352 slack/tentative slots add
139,159,552 bytes per rank. Target and draft capacity remain symmetric so an
effective-depth fallback cannot change allocation or ownership.

## Phase-aware HBM accounting

The completed artifact uses schema `glmaxx.hybrid-mtp3-budget.v1`. For every
rank it records exact bytes and allocation identities for:

1. immutable resident weight and metadata arenas;
2. CUDA contexts, modules, libraries, and allocator state;
3. load staging, readback, conversion scratch, and loader-only metadata;
4. graph executables and graph-private allocations;
5. maximum concurrent inference workspace;
6. collective buffers and library-internal allocations;
7. target and draft cache arenas and page tables;
8. model/program/serving metadata and transaction journals;
9. allocator padding and measured fragmentation; and
10. at least 1 GiB of unallocated emergency escrow.

The budget reports separate load and serving peaks:

```text
load_peak  = common resident terms + simultaneous loader-only terms
serve_peak = common resident terms + simultaneous serving-only terms
required   = max(load_peak, serve_peak)
```

Terms may be lifetime-aliased only when the implementation enforces the
transition collectively: quiesce all ranks, destroy or free every old owner,
verify the release on every rank, then allocate the new owner. Merely observing
that two terms are usually inactive at the same time does not permit aliasing.
Inference workspaces and collectives are summed unless a reviewed execution
schedule proves they cannot overlap and the allocator uses the same bounded
region deliberately.

The old provisional capacity-EXL3 constants are useful only as a sensitivity
check. Using its minimum observed pre-context floor of 101,955,141,632 bytes,
the minimum hybrid records plus MTP3 cache leave 3,611,725,824 bytes for every
other resident term. Reusing all old fixed terms would require
102,235,729,920 bytes, a 280,588,288-byte deficit. Enforcing loader-staging
release removes 268,435,456 bytes from the serving peak but still leaves a
minimum 12,152,832-byte deficit before native weight alignment. Therefore
fit requires measured savings elsewhere; arithmetic alone does not pass.

## Executable planner requirements

The CPU planner takes explicit `maximum_total_positions`,
`admitted_target_positions`, `dcp_degree`, and `maximum_mtp_depth`. It derives
local committed, page-slack, tentative, and rounded slots with checked `u64`
arithmetic. `HybridServe` no longer inherits the capacity-EXL3 local
262,144-slot/MTP6 floor.

It fails closed for:

- a non-four-rank or non-DCP4 profile;
- a maximum address domain other than 1,048,576;
- an admitted count below 524,288 or not divisible by DCP4;
- MTP depth other than three for this profile;
- asymmetric rank arenas or routes;
- incomplete native weight/metadata charges;
- a hidden, duplicated, overlapping, or unmeasured HBM term;
- load or serving peak above the minimum measured rank floor;
- less than 1 GiB escrow; or
- checked-arithmetic, page-table, or physical-page overflow.

The startup ledger emits both decimal bytes and GiB for every term, both phase
peaks, minimum-rank headroom, logical capacity, physical pages, and the full
one-million-position address limit.

## SM120 physical-capacity gate

Capacity passes only after a unique, provenance-complete cn4 run does all of
the following on four ranks under the production posture:

1. admits complete, hash-verified native manifests for the real checkpoint;
2. creates the final CUDA contexts, modules, libraries, collectives, and graph
   set and records allocation deltas;
3. loads and verifies every immutable weight and metadata byte;
4. destroys loader-only allocations collectively before cache allocation;
5. allocates target and draft arenas at exactly 2,116 pages per rank plus the
   reviewed page tables and journals;
6. writes every cache byte, reads every byte through a device checksum, and
   retains the per-arena checksum and elapsed time;
7. proves at least 1 GiB remains free independently on every rank;
8. reconstructs the ledger from observed allocation deltas and fails for any
   unexplained difference; and
9. frees only GLMAXX-owned allocations and records the post-run state.

A configuration value, successful virtual reservation, aggregate free HBM,
single-rank allocation, or sampled page touch is not physical-capacity
evidence.

## Gate sequence

```text
adversarial design review
-> checked CPU planner and mutation proof
-> reviewed implementation
-> exact native hybrid manifests
-> cn4 four-rank physical allocation/checksum
-> checkpoint smoke and quality
-> matched throughput/capacity profile
```

This candidate opens only adversarial design review. It does not authorize
conversion, checkpoint loading, serving, or a fit claim.
