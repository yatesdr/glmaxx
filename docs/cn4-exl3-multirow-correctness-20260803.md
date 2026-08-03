# cn4 EXL3 multi-row correctness

Date: 2026-08-03

Status: supplemental accepted-artifact SM120 correctness; no real payload,
TP4, quality, serving, or performance claim

## Result

The exact executable and shared library from the reviewed EXL3 source-
projection Phase-B pass were reused without rebuilding. Gate, up, and down
projections ran at `M=1,2,4,8`, covering 12 cases at the actual per-rank
GLM-5.2 expert projection dimensions.

Every case reported:

- zero failed elements and zero absolute/relative error;
- bitwise-identical Rust CPU and SM120 GPU output hashes;
- bitwise-identical output across two device repetitions; and
- zero persistent reconstructed-weight and runtime weight-repack bytes.

The emitted summary is:

```json
{
  "schema": "glmaxx.sm120-exl3-multirow-correctness.v1",
  "source_commit": "ccf0162e236e8a8b5d4d6a308d6491759750e83e",
  "rows": [1, 2, 4, 8],
  "projections": ["down", "gate", "up"],
  "cases": 12,
  "failed_elements": 0,
  "all_repeat_bitwise_deterministic": true,
  "all_cpu_gpu_hashes_equal": true,
  "claim": "synthetic multi-row correctness only; not real payload, TP4, or performance"
}
```

This extends the accepted M=1 result across small multi-user decode row
counts. It does not establish throughput: the runner includes fixture
creation, a deliberately slow CPU oracle, allocation, upload, launch, download,
and comparison rather than CUDA-event kernel timing.

## Provenance

```text
source commit       ccf0162e236e8a8b5d4d6a308d6491759750e83e
review artifact     cac885880345fb2f02e940bcf0cd32420acf5ac8a6a3e34fc76e7971a5aa2964
Rust binary         ad2fb57c7cb25588f3cea3bc9f421994f4c16e84eea9c42a530b3342dd14187f
SM120 library       0d95723eb9eb3ed625d6f4933177006faa870eca9624dd3ee1a4fc200813d43d
runner script       cb0eeff8ec69c67c1185a1e579fc4fd4ac9a5da567bea1ee57d9c4310ddac8a9
summary             913416f90bde73156c4169e1d7674d1cd49ed64aec5975d74f3d9d8c68156465
verdict              3096fb6f8929798a45255ae90edf4d6fdbe4dacac421caa3917756fbf0ce18ad
```

The source worktree was clean and detached at the exact qualification commit:

```text
/home/derek/glmaxx/worktrees/exl3-phase-b-ccf0162-20260803T171038Z
```

Raw evidence is outside Git at:

```text
/home/derek/glmaxx/evidence/20260803T174449Z-exl3-multirow-ccf0162
```

The SHA-256 of the sorted relative-path 21-file evidence hash stream is
`004f14e853a30a568b7a2b229180a27615acd4e1e5db67b4a98d58072fc621a1`.
The run used GPU 0 for each projection while requiring all four visible GPUs
to be SM120. After the run cn4 was at 2/2/2/10 MiB used, 0% utilization, with
no compute process.

## Concurrent metadata check

Before launch, a read-only cn4 check reconfirmed the two real index producer
conventions without reading tensor payloads:

| checkpoint | index SHA-256 | shards | declared bytes | file bytes |
| --- | --- | ---: | ---: | ---: |
| TR3 3.25 bpw | `f5dcd976a64ca70808dd4d8bd3ad07e9610c8ca6c30e3a6ed77ddefdac4c1d21` | 81 | 339,069,245,936 | 339,069,245,936 |
| NVFP4/NF3 | `6eb773222d932418dd0530c63aca498f86ef424da2a4526ccba76b59726da234` | 184 | 365,968,736,768 | 365,987,273,208 |

This corroborates the pending `total_size` design but does not accept or
implement it. The TR3 value is the complete shard-file total; the hybrid
value is its payload total, with 18,536,440 additional container bytes.

## Next gate

Real TR3 replay remains blocked by adversarial acceptance and implementation
of the exact `total_size` accounting contract, followed by the mixed K=3/K=4
source/kernel contract. The grouped NVFP4 FC2 lane separately remains blocked
on its scratch-accounting contract. No production admission gate was weakened
for this result.
