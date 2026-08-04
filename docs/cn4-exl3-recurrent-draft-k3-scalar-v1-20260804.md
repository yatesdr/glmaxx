# cn4 recurrent-draft K=3 scalar-v1 qualification

Date: 2026-08-04

Status: passed as a real-payload scalar projection correctness and timing
control. This result does not accept MTP semantics, draft-token generation,
verification, K=4, the warp-staged route, TP4 layer execution, or a checkpoint
smoke.

## Scope and provenance

The isolated qualification at source commit
`99ba366a15a3b4388f9946004c784a2504851c3c` used physical GPU 0 on cn4 and
the scalar ABI `glmaxx.sm120.exl3.source_projection.v1`. The source, CUTLASS,
and checkpoint were mounted read-only in a network-disabled container. Build
and evidence paths were unique and separate from every vLLM path:

```text
worktree  /home/derek/glmaxx/worktrees/exl3-draft-k3-99ba366
build     /home/derek/glmaxx/build/exl3-draft-k3-99ba366-20260804T054043Z
evidence  /home/derek/glmaxx/evidence/20260804T054043Z-exl3-draft-k3-99ba366-r1
```

Pinned identities:

| Input | Identity |
|---|---|
| Container | `sha256:2e401388dcc9c180401cb9997e3e8394c2db695c7bf4e3139ff8a9a517940719` |
| CUTLASS | `e05f953a5b3d38adc240df2ff928e0421c2abba3` |
| Layer-78 shard | `5448b63a32e394e8cbff5a4737fb50b40fc53c6c3c41305f1a7ee540c4d9a6e3` |
| Executable | `ad54a6aa17e065ef2ed72e23ab9d60ac7026eb1a080614dc01cd8e9ae5e0cf51` |
| SM120 library | `b04fc5717a70868585ee249821f33d7762eb19244e17e69a42217ff9252f3f0b` |

The run observed exactly four compute-capability 12.0 devices and no compute
process before launch. Afterward cn4 returned to 2/2/2/10 MiB reported use,
0% utilization, and no compute process.

## Result

Layer 78, expert 0 was loaded directly from the real TR3 3.25-bpw shard for
gate, up, and down on TP ranks 0 and 3. Each projection ran M=1,2,4,8. All 24
cases had:

- an exact CPU-output/GPU-output FP16 SHA-256 match;
- zero absolute and relative error;
- zero failed elements; and
- three bitwise-identical device repetitions.

The runner uploaded the source-ordered trellis and rotation payload directly,
reported zero runtime weight-repack bytes, and reported zero persistent
reconstructed-weight bytes. All six selected recurrent-draft projections are
K=3, so a K=4 negative control is not applicable to this run. The separate
target-layer result exercises fail-closed K=4 rejection.

CUDA-event p50 scalar projection time in microseconds:

| TP rank | Projection | M1 | M2 | M4 | M8 |
|---:|---|---:|---:|---:|---:|
| 0 | gate | 461.312 | 461.440 | 465.536 | 467.520 |
| 0 | up | 461.280 | 461.440 | 465.536 | 466.528 |
| 0 | down | 47.712 | 47.744 | 47.744 | 68.224 |
| 3 | gate | 461.408 | 462.432 | 465.536 | 467.520 |
| 3 | up | 461.344 | 461.536 | 465.568 | 467.488 |
| 3 | down | 47.712 | 47.744 | 47.744 | 68.224 |

Each cell retains 1,000 CUDA-event samples through its report's summary
statistics and exact output/input hashes. These times are scalar controls and
are not an optimized-route, MTP, model-layer, or end-to-end performance claim.

## Evidence integrity and next gate

The top-level artifact manifest hashes to
`812aa10a7989350df90537cb06961b78abe9f004ed7098dfdb8acba1e8ec27f3`.
A fresh `sha256sum -c artifact-manifest.txt` after the run verified all 34
listed records. The compact summary hashes to
`50f014660a86b5dc0ef07b153d2fdefbaa084de57bcb9ebd7491b2a1ca41f0c0`.
The source was clean before and after qualification.

This closes only the real recurrent-draft K=3 scalar projection control. The
next draft-model acceptance evidence must compose the reviewed draft layer,
draft KV, logits, token proposal, and target verification semantics. The TR3
target checkpoint still requires reviewed K=4 execution and a real mixed-K
TP4 sparse-layer replay before a checkpoint smoke.
