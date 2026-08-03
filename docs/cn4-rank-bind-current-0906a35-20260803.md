# cn4 current-tree TP4 rank binding

Date: 2026-08-03

Verdict: `SM120_TP4_RANK_BIND_PASS`

Current source commit `0906a3591aa97d12e0ef80140be37d675bce7c75`
passed all 416 Rust tests in the isolated cn4 development image, built the
native library for `sm_120f`, and bound four persistent owner threads to four
distinct RTX PRO 6000 Blackwell GPUs. Each owner created, synchronized, and
destroyed one nonblocking CUDA stream.

Raw evidence is retained outside Git at:

```text
/home/derek/glmaxx/evidence/20260803T234000Z-rank-bind-0906a35
```

Provenance:

- worktree:
  `/home/derek/glmaxx/worktrees/rank-bind-0906a35-20260803T234000Z`;
- image digest:
  `sha256:0b400cb8ba8dc58d8ae9729702260b5c3d1abaa063a8ef9e14380d72df773842`;
- CUTLASS: `e05f953a5b3d38adc240df2ff928e0421c2abba3`;
- CUDA compiler: `13.3.33`; and
- driver: `595.71.05`.

Pinned results:

| Artifact | SHA-256 |
|---|---|
| `rank-bind.json` | `e295575d39cdb7aa81cfb204e1481fc8e9bca03aa53400c5e064163e9cd942a0` |
| `libglmaxx_sm120.so` | `a9cb5a8ebc24282e0eca4a0e7e225f4f5dc9256ae35957cf8aeb6a7320d5155a` |
| release `glmaxx` | `124e49e14ef987356051da5a7789ea8faf78405ea078fb23c1411fd563eb1eb0` |
| `cargo-test.txt` | `8d10da1b3126c74786261ec05d77c1710be3110eadbc6146fd544b38d2192b93` |
| sorted raw-record stream | `6cd60f858b32cb73ca456379f01c944db7d40d9ee181d0e73871f0a720e2590a` |

The post-run check found 2/2/2/10 MiB used, zero utilization, no compute
application, and no retained container. A first container invocation stopped
before GPU inventory because the linked-worktree Git metadata path was not
mounted; it created no evidence directory or CUDA context.

This qualifies current-tree rank/device ownership and stream lifetime only.
No device kernel, checkpoint, model output, quality, capacity, or performance
claim follows from it.
