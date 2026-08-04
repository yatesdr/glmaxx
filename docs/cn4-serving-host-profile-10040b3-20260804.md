# cn4 synthetic serving host-path profile

Date: 2026-08-04

## Result

The accepted Rust scheduler, page-table, serving-coordinator, and persistent
four-rank worker path was profiled on cn4 at concurrency 1, 2, 4, and 8 for
MTP0 decode and MTP3 verify shapes. Each cell used 100 warmups and retained
1,000 measured samples. The hash-complete evidence verifier passed.

This is synthetic CPU host-path evidence. The workers return deterministic
tokens without loading a checkpoint or running CUDA. It is not model latency,
quality, capacity, decode throughput, or an MTP acceptance result. The
`synthetic_tps` column only divides deterministic committed tokens by host
step time and must not be reported as serving throughput.

| C | MTP | p50 step (us) | p99 step (us) | p50 worker (us) | p50 coordinator (us) | synthetic tok/s |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 0 | 1,141.239 | 1,201.448 | 71.457 | 1,069.645 | 874.6 |
| 2 | 0 | 1,791.535 | 1,850.311 | 78.827 | 1,712.040 | 1,111.9 |
| 4 | 0 | 3,353.465 | 4,299.041 | 227.893 | 3,132.359 | 1,179.9 |
| 8 | 0 | 5,788.283 | 6,648.731 | 255.435 | 5,527.316 | 1,368.5 |
| 1 | 3 | 1,187.394 | 1,230.651 | 50.787 | 1,137.242 | 3,360.2 |
| 2 | 3 | 1,873.242 | 2,266.946 | 67.041 | 1,812.050 | 4,182.4 |
| 4 | 3 | 3,580.416 | 4,631.277 | 253.253 | 3,327.686 | 4,389.8 |
| 8 | 3 | 6,214.989 | 7,308.982 | 302.024 | 5,894.953 | 5,109.7 |

The C1 host step is approximately 1.14 ms for MTP0 and 1.19 ms for MTP3 on
this Xeon W-2195. Coordinator work dominates the median and scales toward 5.5
to 5.9 ms at C8. Inspection attributes the current growth primarily to full
page-table and scheduler transaction cloning. The pending reviewed
fixed-page-transaction contract is the intended O(delta) replacement; this
measurement does not authorize implementing that unaccepted behavior.

## Provenance

- Source commit: `10040b352a01a74d9ab62a65b9f4fd8558c6f34c`
- Worktree:
  `/home/derek/glmaxx/worktrees/host-profile-10040b3-20260804T151600Z`
- Evidence:
  `/home/derek/glmaxx/evidence/20260804T150657Z-serving-host-profile-10040b3`
- Evidence verifier:
  `evidence-run-verify=pass state=COMPLETE files=65`
- Evidence manifest SHA-256:
  `4a2f2f9a7e65ee7bbde131a93a787fd7f360dc725fbf3bc6dbb1b4b37c2b7e92`
- Raw profile SHA-256:
  `16ab2f60b92cf77ea70ebf8c4d85ea4f345f7c5b7775d0a9dfcb78a2babb3886`
- Release binary SHA-256:
  `e91db93e8fd0b998aeea4623a2dd31221f709d34a4da4da8dc024d8135f58f82`
- Container:
  `sha256:0b400cb8ba8dc58d8ae9729702260b5c3d1abaa063a8ef9e14380d72df773842`
- Rust: `1.92.0`; host CPU: Intel Xeon W-2195, 18 cores / 36 threads
- Container network: disabled; NVIDIA devices: not exposed
- Wall interval: `2026-08-04T15:06:57Z` to `2026-08-04T15:07:47Z`

The four GPUs were at zero utilization and 2/2/2/10 MiB before and after;
`compute-apps-after.csv` was empty. No vLLM path, image, container, cache,
process, port, or result was used or modified.

## Retained invalid attempts

Two earlier directories remain immutable and must not be promoted:

- `20260804T150224Z-serving-host-profile-8bc7a64`: failed before profiling
  because the container does not contain `/usr/bin/time`.
- `20260804T150419Z-serving-host-profile-483a783`: the profile completed, but
  the evidence verifier rejected post-seal writes by the exit trap.

Commit `10040b3` makes the trap read-only after terminal publication. The
successful run passed the exact sealed-file-set verification after process
exit.
