# cn4 parallel retained-shard open at f91429f

Date: 2026-08-04

Status: measured win accepted for the CPU checkpoint-open phase; hybrid output
and expected TR3 rejection output are byte-identical to the baseline; no CUDA
device was exposed or used

## Scope and provenance

This experiment compares the serial retained-shard opener at
`43f4e6065d19712b73f20ea35e180dfe7f567ae8` with the bounded, deterministic
16-worker opener at `f91429f8c3d5b549c96d4ec5c859712f4055bcbd` on cn4.
Both commands were pinned to CPUs 0--15 and run five times after one warmup.
Odd samples ran baseline first; even samples ran candidate first.

The candidate was built offline from a clean detached worktree using Rust
1.92.0 and container image
`sha256:0b400cb8ba8dc58d8ae9729702260b5c3d1abaa063a8ef9e14380d72df773842`.
The exact build and benchmark commands are retained with the raw evidence:

```text
/home/derek/glmaxx/evidence/20260804T035800Z-parallel-shard-open-f91429f
```

The evidence artifact-hash manifest has SHA-256:

```text
889e3ad4b20eb9b2bd69febd09fc5db061d335db211105b294b0932f079530aa
```

| item | SHA-256 |
|---|---|
| baseline executable | `912c7a39c5681f8ba1d0284731aeeee3d56e134c1aa01f699730d060620bf683` |
| candidate executable | `3a91d1480640e2747c0e8615cc7dfb9507a3ba75f00d06e7c8cd52794e2139d4` |
| hybrid index | `6eb773222d932418dd0530c63aca498f86ef424da2a4526ccba76b59726da234` |
| TR3 index | `f5dcd976a64ca70808dd4d8bd3ad07e9610c8ca6c30e3a6ed77ddefdac4c1d21` |

The executable was built without CUDA FFI. `nvidia-smi` showed no compute
processes, zero utilization, and identical 2/2/2/10 MiB allocations before
and after the matrix.

## Results

| profile | baseline samples (s) | candidate samples (s) | median reduction | speedup |
|---|---|---|---:|---:|
| NVFP4/NF3 hybrid | 0.643623, 0.643183, 0.640332, 0.641884, 0.643596 | 0.482550, 0.483151, 0.481545, 0.486247, 0.485318 | 24.88% | 1.331x |
| TR3 expected index rejection | 3.680623, 3.668602, 3.665748, 3.695612, 3.697915 | 2.502438, 2.503310, 2.500120, 2.504272, 2.489858 | 32.01% | 1.471x |

Hybrid inventory JSON matched byte-for-byte for every baseline/candidate pair.
TR3 stdout and the expected `glmaxx: Index` stderr also matched byte-for-byte
for all five pairs. The candidate preserves original index order when merging
thread results, caps concurrency at 16, and retains a direct single-worker
path. The complete local gate passed 423 tests, clippy with warnings denied,
release proofs, serving/cache proofs, and deterministic profile validation.

An initial run at
`20260804T035500Z-parallel-shard-open-f91429f` stopped before the first
checkpoint operation because `/usr/bin/time` is absent on the host. It is
retained as a failed experiment with artifact-manifest SHA-256
`df5772d91ebf7275dd3300d0dec30fc59e7ae68e4ac7a3be32924b262b5eb395`
and is excluded from the measurements above.

## Claim boundary

This is a real-checkpoint CPU structural-open and cold-start component result.
It does not make TR3 admissible: the reviewed dual `metadata.total_size`
contract must still land. It does not prove payload authentication, weight
conversion, HBM loading, CUDA kernel speed, layer/model correctness, KLD,
serving throughput, or end-to-end cold boot.
