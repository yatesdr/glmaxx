# cn4 EXL3 staged K=3 Nsight diagnostic — 2026-08-04

Status: optimization diagnostic; not an acceptance or serving result

## Identity and retained evidence

The diagnostic used source commit
`89de6e600ff7242f4129c6fccec678fad4a72ebf`, an isolated derivative of the
pending EXL3 warp-staged v2 candidate. It launched only synthetic M1 gate and
down cases on GPU 0 of cn4. The complete external artifact set remains at:

```text
/home/derek/glmaxx/evidence/20260804T152000Z-exl3-staged-profile-once-89de6e6-r7
```

The unprofiled control and profiled replay produced the same output digest in
each case:

| Projection | Logical shape | Output SHA-256 |
|---|---:|---|
| gate | M1, K6144, N512 | `b9fcbe0c8de442d4780472fb09cef69a6a728d0b42999f1d14e6a7bf5072dc8a` |
| down | M1, K512, N6144 | `aa8eaa8b7cd82998c46306b654d769cb03d17e4758983d676aac0a9076da2e5d` |

The retained Nsight reports and raw CSV hashes are:

| Projection | `.ncu-rep` SHA-256 | Raw CSV SHA-256 |
|---|---|---|
| gate | `3a412a93e6f2ac3bd7d5022cd0e9ab9c5348a8e7449fcb555cb147cc26f49162` | `1eff3635a7812d9bff690bdf84bb35db296576495eb0eb3d416b59cc7dc35155` |
| down | `282879cce7fe0b8cb080d1ad76283f15539601d216ac1b6588d1ca21b81bd5f1` | `8260369ad0a4ed3f549ab7ab59ad8d5292ee62c82fb6271777ad795624c74116` |

## Measured staged projection kernel

| Metric | Gate M1 | Down M1 |
|---|---:|---:|
| Duration | 280.26 us | 24.29 us |
| Grid | 32 CTAs | 384 CTAs |
| Block | 256 threads | 256 threads |
| Registers/thread | 64 | 64 |
| Static shared/block | 768 B | 768 B |
| Waves/SM | 0.04 | 0.51 |
| Theoretical occupancy | 66.67% | 66.67% |
| Achieved occupancy | 16.66% | 33.97% |
| Achieved active warps/SM | 8.00 | 16.30 |
| Compute throughput | 0.86% | 10.03% |
| DRAM throughput | 0.26% | 2.94% |

The gate projection exposes only 32 CTAs to 188 SMs. Its low compute and DRAM
throughput rule out a bandwidth ceiling for this case; insufficient independent
work is the first-order bottleneck. The down projection's 384-CTA grid fills
the device substantially better and finishes 11.54 times faster despite
producing twelve times as many output columns. Sixty-four registers per thread
also cap theoretical occupancy at 66.67%, but register reduction is secondary
to fixing the gate/up grid.

The next candidate must increase independent gate/up work while retaining the
reviewed accumulation order and exact output digest. Candidate routes, in
order, are output-tile subdivision, grouped-expert aggregation in one launch,
and only then a deterministic split-K reduction if the first two cannot expose
enough work. Mixed K=3/K=4 remains behind its separate review gate.

## Profiler command correction

The repository suite spelled the Nsight Compute option as
`--force-overwrite false`. In Nsight Compute 2026.2, `--force-overwrite` is a
valueless flag; `false` was therefore selected as the target program and the
tool reported a generic application failure with no kernels. The successful
diagnostic omitted the flag because every report base is fresh. The corrected
suite uses a small wrapper whose CPU self-test captures the entire argument
vector, rejects both `false` and `--force-overwrite`, and proves that the
GLMAXX runner begins the target portion.

This evidence does not establish mixed-K execution, a target layer, TP4,
checkpoint loading, model quality, KV capacity, or end-to-end throughput.
