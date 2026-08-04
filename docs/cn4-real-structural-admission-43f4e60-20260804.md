# cn4 real checkpoint structural admission at 43f4e60

Date: 2026-08-04

Status: hybrid structural inventory passed; TR3 rejected at the known
`metadata.total_size` convention gate; no CUDA device exposed

## Scope and isolation

The detached GLMAXX worktree
`/home/derek/glmaxx/worktrees/admission-43f4e60` ran exact committed source
`43f4e6065d19712b73f20ea35e180dfe7f567ae8`. The container used no network,
no `--gpus` option, and read-only checkpoint mounts. All four cn4 GPUs were
idle before and after the run with 97,232--97,249 MiB free. The complete
265-row shard fingerprint stream was byte-identical before and after.

Raw evidence is retained outside Git at:

```text
/home/derek/glmaxx/evidence/20260804T034000Z-real-structural-admission-43f4e60
```

The ordered artifact-hash manifest has SHA-256:

```text
8ce590fb86830629830a07a3edf9cfac1b96ebf80fb13c7d137610e39c22e0ac
```

The exact runtime identities were:

| item | identity |
|---|---|
| source commit | `43f4e6065d19712b73f20ea35e180dfe7f567ae8` |
| container | `sha256:0b400cb8ba8dc58d8ae9729702260b5c3d1abaa063a8ef9e14380d72df773842` |
| executable SHA-256 | `912c7a39c5681f8ba1d0284731aeeee3d56e134c1aa01f699730d060620bf683` |
| TR3 index SHA-256 | `f5dcd976a64ca70808dd4d8bd3ad07e9610c8ca6c30e3a6ed77ddefdac4c1d21` |
| hybrid index SHA-256 | `6eb773222d932418dd0530c63aca498f86ef424da2a4526ccba76b59726da234` |

## Results

The NVFP4/NF3 hybrid index passed retained-descriptor parsing and final source
revalidation:

| field | observed |
|---|---:|
| tensors | 148,289 |
| shards | 184 |
| tensor payload bytes | 365,968,736,768 |
| BF16 tensors / bytes | 1,141 / 37,781,026,816 |
| F32 tensors / bytes | 30,412 / 199,168 |
| F8_E4M3 tensors / bytes | 58,368 / 28,915,531,776 |
| U8 tensors / bytes | 58,368 / 299,271,979,008 |

The TR3 command returned exit code 1 with the typed CLI surface
`glmaxx: Index`; hybrid returned 0. A separate read-only diagnostic over only
the 81 eight-byte prefixes and file lengths reproduced the already-reviewed
design arithmetic:

```text
declared/file bytes = 339,069,245,936
tensor payload bytes = 338,954,037,248
prefix/header bytes =     115,208,688
```

The failure is therefore not a missing shard, unsafe path, unsupported dtype,
or GPU-access problem. Current `ShardedSafetensors::open` incorrectly treats
every index `metadata.total_size` as tensor payload bytes. The real TR3 index
uses complete shard-file bytes, while the hybrid index uses payload bytes.
`docs/safetensors-index-total-size-v1-r2.md` defines the exact two-convention
accounting record, but implementation remains gated on the requested token
`safetensors-index-total-size-v1-r2-design-accepted`.

## Claim boundary

This result proves only real-source structural behavior and the exact current
TR3 admission blocker. It did not hash weight payloads, authenticate either
complete source tree, create a CUDA context, convert a rank image, load HBM,
execute a model layer, or establish quality, capacity, startup, or throughput.

The earlier directory
`20260804T033000Z-source-inventory-43f4e60` is diagnostic only: its inner
pipeline did not propagate the TR3 exit code and produced an empty TR3 output
file. The fresh result above supersedes it.
