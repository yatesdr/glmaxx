# cn4 tentative page preflight matched profile

Date: 2026-08-04

## Result

Replacing nested whole-table rollback snapshots with immutable tentative
reserve/commit preflight plans improved all 40 matched page-transaction cells
on cn4. Median speedup ranges from 1.55x to 21.29x, with a 3.87x median across
the matrix.

Selected medians and optimized p99 values, in microseconds:

| Context/sequence | C | MTP | Baseline p50 | Optimized p50 | Speedup | Optimized p99 |
|---:|---:|---:|---:|---:|---:|---:|
| 0 | 1 | 0 | 1,424.183 | 409.445 | 3.48x | 439.886 |
| 0 | 8 | 0 | 9,018.361 | 423.568 | 21.29x | 442.738 |
| 65,536 | 8 | 0 | 30,238.844 | 5,420.786 | 5.58x | 6,192.692 |
| 131,072 | 8 | 0 | 50,911.674 | 10,925.627 | 4.66x | 11,255.873 |
| 0 | 1 | 3 | 1,515.117 | 415.760 | 3.64x | 434.631 |
| 0 | 8 | 3 | 8,946.263 | 427.551 | 20.92x | 446.587 |
| 65,536 | 8 | 3 | 28,967.063 | 7,356.267 | 3.94x | 7,474.833 |
| 131,072 | 8 | 3 | 48,562.256 | 15,055.558 | 3.23x | 15,679.247 |

The 65,536/C8 cells contain 524,288 aggregate active positions; the 131,072/C8
cells contain 1,048,576. These are page-table metadata positions in an
8,192-page-per-rank synthetic arena, not physically allocated KV payloads and
not evidence that the production HBM capacity target passes.

At 65,536/C8, MTP0 reserve/commit mutations fell to 13.343/472.267 us and MTP3
to 13.562/470.739 us. At 131,072/C8 they are 22.548/981.078 us for MTP0 and
22.705/978.361 us for MTP3. The two full `PageTableDelta::between` scans now
dominate: 4.114/4.250 ms at MTP0 and 6.200/6.315 ms at MTP3 for the 1M aggregate
cell.

This is synthetic CPU transaction evidence only. It excludes scheduler,
worker, CUDA, kernels, collectives, checkpoint data, KV payload allocation,
quality, end-to-end latency, and serving throughput.

## Matched inputs

Both runs used the same cn4 host, CPU affinity, network-disabled container,
40-cell matrix, 10 warmups, 100 retained iterations, 8,192 target and draft
pages per rank, and no GPU device exposure. The only source change is child
commit `d225904` over baseline `f03fc2c`.

| Field | Baseline | Optimized |
|---|---|---|
| Source | `f03fc2cce425481bb9d38bbf712ffb8446dd9693` | `d225904d2992c047a8a3d7400f88a4c4cfc8f79c` |
| Evidence | `/home/derek/glmaxx/evidence/20260804T152243Z-page-transaction-profile-f03fc2c` | `/home/derek/glmaxx/evidence/20260804T153557Z-page-transaction-profile-d225904` |
| Raw SHA-256 | `6ac1e7eaf221f0a53b8928964cff5dd32b8a4d5392c81dd7f58ea8776a584077` | `2a778bcb2cd4276bb50c8dd3ea19d2094324ace4dea919ca5ddbc26cf9b2177e` |
| Manifest SHA-256 | `58519ad3a1185a50e8b23386caa9f8b94365a5ddf2b501c120af8cf1383aceba` | `4c1a985dbd2dee8c44bdc107b1c6cdb7f5cfa8c50a1df80afd5f3a3cdfeecc48` |
| Binary SHA-256 | `8c7db8385d12364dcb0ded55d5661feabf644e3333c753e0212bc6abc6f167cc` | `3fb4b33fe18c69556dc27a401dfbc8ff1a1493f583ff31f488f367ed93e82be0` |
| Wall interval | `15:22:43Z–15:23:52Z` | `15:35:57Z–15:36:31Z` |

Both evidence sets independently pass the sealed-file verifier. The optimized
receipt is:

```text
evidence-run-verify=pass state=COMPLETE files=49
manifest-sha256=4c1a985dbd2dee8c44bdc107b1c6cdb7f5cfa8c50a1df80afd5f3a3cdfeecc48
```

The four GPUs were at zero utilization and 2/2/2/10 MiB before and after;
both `compute-apps-after.csv` files were empty. No vLLM resource was touched.
