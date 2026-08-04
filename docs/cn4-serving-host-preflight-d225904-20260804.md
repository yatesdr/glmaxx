# cn4 serving-host preflight result

Date: 2026-08-04

## Result

The tentative-page preflight change at
`d225904d2992c047a8a3d7400f88a4c4cfc8f79c` reduces the retained median
synthetic Rust serving-step time in every tested C1/C2/C4/C8 MTP0/MTP3 cell.
Relative to the pinned `10040b3` baseline, the p50 speedup ranges from 2.54x
to 10.63x.

This is a CPU-only scheduler, page-table, request-lifecycle, and four-worker
host-path result. It contains no model execution, checkpoint, CUDA launch,
collective, quality, physical KV-capacity, useful serving-throughput, or
end-to-end latency evidence.

## Matched measurements

Both runs used the same unchanged `serving-host-profile` implementation and
sealed wrapper, the same container, CPU set, 100 warmup steps, and 1,000
retained steps per cell. The intervening `f03fc2c` commit only adds the
independent page-transaction profiler; the measured serving behavior change
is `d225904`.

| MTP | C | baseline p50 | optimized p50 | optimized p99 | p50 speedup |
|---:|---:|---:|---:|---:|---:|
| 0 | 1 | 1.141 ms | 0.432 ms | 0.461 ms | 2.64x |
| 0 | 2 | 1.792 ms | 0.450 ms | 0.496 ms | 3.98x |
| 0 | 4 | 3.353 ms | 0.475 ms | 0.508 ms | 7.05x |
| 0 | 8 | 5.788 ms | 0.545 ms | 0.599 ms | 10.63x |
| 3 | 1 | 1.187 ms | 0.467 ms | 0.502 ms | 2.54x |
| 3 | 2 | 1.873 ms | 0.513 ms | 0.598 ms | 3.65x |
| 3 | 4 | 3.580 ms | 0.595 ms | 0.765 ms | 6.02x |
| 3 | 8 | 6.215 ms | 0.796 ms | 1.148 ms | 7.81x |

The optimized C8 coordinator-overhead p50 is 0.474 ms for MTP0 and 0.671 ms
for MTP3. The corresponding synthetic four-rank worker round-trip p50 is
0.071 ms and 0.125 ms. These components remain separated from future real
kernel and collective time.

## Provenance

- Host: `cn4`
- Baseline source:
  `10040b352a01a74d9ab62a65b9f4fd8558c6f34c`
- Optimized source:
  `d225904d2992c047a8a3d7400f88a4c4cfc8f79c`
- Container:
  `sha256:0b400cb8ba8dc58d8ae9729702260b5c3d1abaa063a8ef9e14380d72df773842`
- Optimized run:
  `/home/derek/glmaxx/evidence/20260804T154609Z-serving-host-profile-d225904`
- Run interval: `2026-08-04T15:46:09Z` through
  `2026-08-04T15:46:36Z`
- Raw profile SHA-256:
  `1f4a243b5f47a52032ba72a561edf09416186a03d1c3e0a1470833628f266437`
- Summary SHA-256:
  `5799aac2994ecf6b899eb7731000158acd8915b6b2a9e721aece2c6286e5e7ba`
- Evidence manifest SHA-256:
  `d873a47aecacceb3fb8b2fce3cc7e149baedfdfa29fb1fdd2cb3bf73a3f28d2c`
- Release binary SHA-256:
  `3fb4b33fe18c69556dc27a401dfbc8ff1a1493f583ff31f488f367ed93e82be0`
- Sealed wrapper SHA-256:
  `ece14eda80643f39037c79e32e1e1c7e4a2a90d7e401b7e3ee61def4fc8ca134`
- Command record SHA-256:
  `1549220d7cc0b5860b8226d2e5ca1e4788a3b62156e980033eb24e4f6eb8f9c1`

`verify-evidence-run.sh` passed with terminal state `COMPLETE`, 65 retained
files, and the manifest hash above. The container used
`NVIDIA_VISIBLE_DEVICES=void`; compute-process records were empty before and
after, and final GPU utilization was zero on all four devices.

## Consequence

The measured host coordinator is no longer the immediate C1 throughput
ceiling for the requested 50 tok/s MTP0 and 100 useful tok/s MTP3 targets.
Actual SM120 model kernels, PCIe TP4 collectives, and MTP acceptance now need
to be measured before making any throughput claim. The remaining context-size
cost in the isolated page profile is dominated by fixed full-table delta
construction; changing that representation remains gated by the pending
fixed-page-transaction r2 review.
