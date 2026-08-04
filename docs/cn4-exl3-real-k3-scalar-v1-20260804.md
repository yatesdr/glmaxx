# cn4 real TR3 K=3 scalar-v1 qualification

Date: 2026-08-04

Status: passed as a real-payload scalar correctness and timing control; this
does not accept K=4, promote the warp-staged route, prove TP4 layer execution,
or constitute a checkpoint smoke.

## Scope and provenance

The isolated qualification at source commit
`e8e459373b6198cea26bbdfb29dbf7e2799cacca` used physical GPU 0 on cn4 and
the accepted scalar ABI `glmaxx.sm120.exl3.source_projection.v1`. The source,
CUTLASS, and checkpoint were mounted read-only in a network-disabled
container. Build and evidence paths were new and separate from every vLLM
path:

```text
worktree  /home/derek/glmaxx/worktrees/exl3-real-k3-e8e4593
build     /home/derek/glmaxx/build/exl3-real-k3-e8e4593-20260804T052506Z
evidence  /home/derek/glmaxx/evidence/20260804T052506Z-exl3-real-k3-e8e4593-r1
```

Pinned identities:

| Input | Identity |
|---|---|
| Container | `sha256:2e401388dcc9c180401cb9997e3e8394c2db695c7bf4e3139ff8a9a517940719` |
| CUTLASS | `e05f953a5b3d38adc240df2ff928e0421c2abba3` |
| Layer-3 shard | `31bc19eabf05d0782e33103672094f1d8aca2a8bb9fb5b88a502cd6caab61bd0` |
| Executable | `ad54a6aa17e065ef2ed72e23ab9d60ac7026eb1a080614dc01cd8e9ae5e0cf51` |
| SM120 library | `43ef722a3bc8c6ed4cf88127755141862ab9792c809c77a0edf3487bf7b5e70c` |

The run observed exactly four compute-capability 12.0 devices and no compute
process before launch. Afterward cn4 returned to 2/2/2/10 MiB reported use,
0% utilization, and no compute process.

## Result

Layer 3, expert 0 was loaded directly from the real TR3 3.25-bpw shard for
gate, up, and down on TP ranks 0 and 3. Each projection ran M=1,2,4,8. All 24
cases had:

- an exact CPU-output/GPU-output FP16 SHA-256 match;
- zero absolute and relative error;
- zero failed elements; and
- three bitwise-identical device repetitions.

The runner uploaded the 1,192,960-byte source-ordered trellis and rotation
payload directly. It reported zero runtime weight-repack bytes and zero
persistent reconstructed-weight bytes. The separate K=4 control, layer 3
expert 6 rank 0 gate, failed closed at its incompatible trellis component:

```text
glmaxx-exl3-real-k3-v1: Component("model.layers.3.mlp.experts.6.gate_proj.rank0.trellis")
```

CUDA-event p50 scalar projection time in microseconds:

| TP rank | Projection | M1 | M2 | M4 | M8 |
|---:|---|---:|---:|---:|---:|
| 0 | gate | 461.408 | 462.400 | 465.536 | 467.552 |
| 0 | up | 461.408 | 461.472 | 465.536 | 467.488 |
| 0 | down | 47.712 | 47.744 | 47.744 | 68.224 |
| 3 | gate | 461.408 | 461.664 | 465.536 | 467.456 |
| 3 | up | 461.376 | 461.440 | 465.536 | 466.528 |
| 3 | down | 47.712 | 47.744 | 47.744 | 68.224 |

Each cell retains 1,000 raw CUDA-event samples through its report's summary
statistics and exact output/input hashes. These times are scalar controls;
they are not compared as a performance claim against the separately measured
synthetic warp-staged candidate.

## Evidence integrity and next gate

The top-level artifact manifest hashes to
`e41a282967c2ce747d08888fe3b59c6a5e3c5eec936a1c91cdea7e38c7d4ad61`.
A fresh `sha256sum -c artifact-manifest.txt` after the run verified all 36
listed records. The compact summary hashes to
`561aa20a00f5d592ec326b38c7da39957ea13a355d44e122d679b840d99e3f59`.

This closes the real K=3 scalar-projection microbenchmark gap. The TR3
checkpoint remains blocked from TP4 sparse-layer replay by K=4 execution and
by the unaccepted staged implementation. The next matched device result must
qualify reviewed K=3 and K=4 optimized paths on real payloads before composing
the 192:64 mixed-expert layer.
