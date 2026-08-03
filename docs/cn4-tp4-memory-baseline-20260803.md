# cn4 TP4 post-context memory baseline

Date: 2026-08-03

Verdict: `SM120_TP4_MEMORY_BASELINE_DIAGNOSTIC`

This is a five-sample diagnostic of four simultaneous CUDA contexts and one
nonblocking stream per rank. It launches no device kernel and makes no model,
KV-capacity, fit, quality, or performance claim.

Raw evidence remains outside Git at:

```text
/home/derek/glmaxx/evidence/20260803T162522Z-memory-baseline-36584b0
```

## Provenance

| Input | Identity |
|---|---|
| source commit | `36584b08235052883159788d53cb62c9eba00941` |
| source status | detached clean worktree before and after |
| container | `sha256:2e401388dcc9c180401cb9997e3e8394c2db695c7bf4e3139ff8a9a517940719` |
| Rust/Cargo | `1.92.0` |
| CUDA compiler | `13.3.33` |
| CUTLASS | `e05f953a5b3d38adc240df2ff928e0421c2abba3` |
| driver | `595.71.05` |
| target | 4x RTX PRO 6000 Blackwell, SM120, 188 SMs/rank |

The source and build were isolated under
`/home/derek/glmaxx/worktrees/memory-baseline-36584b0-20260803T162522Z`
and `/home/derek/glmaxx/build/memory-baseline-36584b0`. The run used no
network, a private IPC namespace, and the existing GLMAXX image. It did not
reuse or modify a vLLM worktree, image, container, cache, port, or result.

The linked library contains `sm_120f` ELF code. Exact artifact hashes are:

| Artifact | SHA-256 |
|---|---|
| `libglmaxx_sm120.so` | `79f15e86edf5ed14d7cd3402fab21cac47e191a24341a5b3daa0b344955f60b5` |
| release `glmaxx` | `b941c8434c3665d179d71fc9db6c103a14f12868a0ca9178014fb2a8c8c047ae` |

## Measurement

All five fresh-container samples were byte-identical:

| Rank | CUDA total bytes | Post-context free bytes | Total minus free |
|---:|---:|---:|---:|
| 0 | 101,973,491,712 | 101,384,978,432 | 588,513,280 |
| 1 | 101,973,491,712 | 101,384,978,432 | 588,513,280 |
| 2 | 101,973,491,712 | 101,384,978,432 | 588,513,280 |
| 3 | 101,964,644,352 | 101,367,742,464 | 596,901,888 |

The minimum post-context free-HBM floor is therefore
`101,367,742,464` bytes. Every context and stream remained alive until all
four ranks had sampled `cudaMemGetInfo`. The post-run inventory exactly
returned to 2/2/2/10 MiB used with zero compute applications.

## Capacity implication

The hybrid MTP3 design's minimum record and cache charges are:

```text
minimum native weight records        94,013,097,984
524,288-token MTP3 cache arena         4,330,317,824
subtotal                              98,343,415,808
measured post-context floor          101,367,742,464
remaining after weights and cache      3,024,326,656
```

Reusing the old conservative non-context, non-loader-staging terms gives:

```text
graphs                                  268,435,456
maximum workspace                       536,870,912
collectives                             268,435,456
model metadata                           67,108,864
page tables and journals                 67,108,864
allocator padding                       268,435,456
emergency escrow                      1,073,741,824
provisional other terms               2,550,136,832
provisional remainder                   474,189,824
```

This changes the sensitivity result from a provisional deficit to 474,189,824
bytes of possible per-rank margin because the measured context delta is below
the old 1 GiB allowance and loader staging is absent from the serving phase.
It is not a fit result. Native weight alignment, lazy CUDA module loading,
final graphs, collective-library allocations, peak workspaces, fragmentation,
and full physical cache allocation/checksum remain unmeasured and can consume
more than this margin.

## Evidence hashes

| Record | SHA-256 |
|---|---|
| each of five raw JSON samples | `8d489a48e2fc31061e59bb1d526140900d1d03346c93f307dc915ae212350fd8` |
| five-sample `summary.json` | `68e49ad07efa76ecb1ffef560dc367b7b1c36065b1e257127d501e076c57df3b` |
| complete evidence hash list | `5f52baf8af7d0ca8060825962884696eafbe0e29680cd1aab642a138d49d9d41` |

The next capacity evidence must load the final native modules, graphs,
collectives, weight arenas, and workspaces, then allocate, write, and
device-checksum all 2,116 target and draft pages per rank while retaining at
least 1 GiB free independently on every rank.
