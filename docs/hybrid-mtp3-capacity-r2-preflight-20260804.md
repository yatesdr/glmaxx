# Hybrid MTP3 capacity-ledger r2 preflight

Date: 2026-08-04

Status: review-readiness and sensitivity evidence only; not design acceptance,
physical allocation, checkpoint residency, or capacity certification

## Candidate and local gate

The exact capacity candidate was rechecked in clean detached worktree
`/tmp/glmaxx-nf3-source-preflight-2b87` at commit
`2b8785907c11d2b58d8c5fa7f782845fae03e3ad`. All nine candidate-input hashes
in `docs/fable-hybrid-mtp3-capacity-ledger-v1-r2-handoff.md` matched. The same
candidate had passed the complete 413-test local gate; current clean
integration passed 427 tests after adding this record.

The required Fable result and
`hybrid-mtp3-capacity-ledger-v1-r2-design-accepted` token remain absent.

## External evidence verification

The handoff's required read-only cn4 evidence directory was verified in
place:

```text
/home/derek/glmaxx/evidence/20260803T162522Z-memory-baseline-36584b0
```

Its `evidence-sha256.txt` hashes to
`5f52baf8af7d0ca8060825962884696eafbe0e29680cd1aab642a138d49d9d41`,
and all 24 listed records passed `sha256sum -c`. The five independent JSON
samples are byte-identical with SHA-256
`8d489a48e2fc31061e59bb1d526140900d1d03346c93f307dc915ae212350fd8`.
Each held four CUDA contexts and one nonblocking stream per rank and measured
the same minimum post-context free-HBM floor:

```text
rank 0    101,384,978,432 bytes
rank 1    101,384,978,432 bytes
rank 2    101,384,978,432 bytes
rank 3    101,367,742,464 bytes
minimum   101,367,742,464 bytes
```

The source was clean commit `36584b08235052883159788d53cb62c9eba00941`
in container
`sha256:2e401388dcc9c180401cb9997e3e8394c2db695c7bf4e3139ff8a9a517940719`.
The records make no kernel or allocation claim beyond runtime-owned context
state and one stream per rank.

The supplemental eager-module evidence at
`/home/derek/glmaxx/evidence/20260803T165800Z-eager-memory-baseline-36584b0`
was also verified. Its evidence list hashes to
`887a52a3a48c5eb5bde5d0db33dbc4b876c1aabfb0b1544c6e524697e1c1d09b`,
and every listed record passed. Its five byte-identical samples have SHA-256
`05d44a0957de948ad14bce7d0cd497865d2217503067a5c6d7ee0608261fac8c`
and reduce the minimum floor by exactly 2,097,152 bytes to
101,365,645,312. This covers only the current linked module, not the final
engine graph/module set.

## Independent capacity derivation

Global admission of 524,288 positions under DCP4 gives 131,072 committed
slots per rank. At C64, 64 active sequences can place one mutable page on the
same owner, requiring `64 * 64 = 4,096` owner-slack slots. MTP3 exposes four
tentative positions per sequence, requiring `64 * (3 + 1) = 256` more:

```text
131,072 + 4,096 + 256 = 135,424 slots/rank
135,424 / 64           =   2,116 pages/rank
```

The independent byte derivation is:

```text
target KV       135,424 * 78 * 368 = 3,887,210,496
target indexer  135,424 * 21 * 132 =   375,395,328
draft KV        135,424 *  1 * 368 =    49,836,032
draft indexer   135,424 *  1 * 132 =    17,875,968
MTP3 cache total                       4,330,317,824
```

The logical page table retains 16,384 page ordinals for the 1,048,576-position
address domain. Admission permits only 8,192 committed global pages, or 2,048
per DCP rank, while the separate physical owner/tentative reserve raises each
rank arena to 2,116 pages. Thus the address domain does not imply physical
residency beyond the quota.

The seven older fixed sensitivity terms independently sum to the pinned
2,550,136,832 bytes:

| Term | Bytes |
|---|---:|
| graphs | 268,435,456 |
| maximum workspace | 536,870,912 |
| collectives | 268,435,456 |
| model metadata | 67,108,864 |
| page tables and journals | 67,108,864 |
| allocator padding | 268,435,456 |
| independently unallocated escrow | 1,073,741,824 |

Using the independently preflighted immutable hybrid arenas gives:

```text
immutable weight plus metadata          94,016,235,456
MTP3 cache                               4,330,317,824
subtotal                                98,346,553,280
post-context minimum                   101,367,742,464
remaining before older fixed terms      3,021,189,184
older fixed terms                        2,550,136,832
lazy-module provisional residual           471,052,352
eager-module provisional residual          468,955,200
```

The 1-GiB escrow is already inside the fixed-term total and is not double
charged. The final startup gate must still observe more than that escrow free
after all real resident allocations.

## Risk and nonclaims

The candidate arithmetic and both external baselines are internally exact.
The eager residual is only 468,955,200 bytes, so the serving implementation
must reuse mutually exclusive scratch deliberately and measure all final
module, graph, NCCL/collective, allocator, journal, and fragmentation charges.
An unexplained increase above this residual invalidates the 524,288-token
profile unless a matched measured saving is made elsewhere.

No weight or KV arena was allocated or touched in this preflight. It does not
establish that the checkpoint fits, that 524,288 tokens are physically backed,
or that the stated phase aliases are implemented. Those claims still require
the reviewed CPU planner, accepted native manifests, complete four-rank upload,
full cache writes/device checksums, allocation reconciliation, and the final
per-rank escrow check.
