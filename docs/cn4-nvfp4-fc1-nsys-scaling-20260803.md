# cn4 NVFP4 FC1 Nsight Systems scaling diagnostic

Date: 2026-08-03

Status: accepted-artifact synthetic diagnostic; no retained-event,
top-8 MoE, layer, token-throughput, or performance-acceptance claim

## Result

The exact Rust executable and SM120 library from the reviewed manifest/ABI
Phase-B tree were reused without rebuilding. The synthetic single-expert FC1
smoke passed its frozen numerical gate at `M=1,2,4,8`. One Nsight Systems
trace sample per row count separated activation quantization from the direct
CUDA-core FC1 plus SwiGLU control:

| M | quantize | FC1 + SwiGLU | aggregate kernels | aggregate per row |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 12.063 us | 48.767 us | 60.830 us | 60.830 us |
| 2 | 11.999 us | 73.918 us | 85.917 us | 42.9585 us |
| 4 | 12.064 us | 146.269 us | 158.333 us | 39.58325 us |
| 8 | 12.000 us | 266.202 us | 278.202 us | 34.77525 us |

Quantization is effectively launch-bound across this small-M range: the four
samples span only 65 ns. It consumes 19.8% of the M1 kernel pair but 4.3% at
M8. The direct core gains useful row concurrency, reducing aggregate time per
row by 42.8% from M1 to M8, but its latency remains the dominant term. This
supports the existing optimization order: connect the compiled SM120
block-scaled MMA path first, group real top-8 expert work across rows, and then
remove or fuse the fixed quantization launch.

This control executes one expert assignment per row. A real GLM-5.2 sparse
layer has top-8 routing, FC2, scatter/reduction, TP4 communication, and the
rest of the target program. These durations therefore cannot be multiplied or
inverted into model token throughput. The earlier Nsight Compute M1 replay
reported 14.464/75.872 us for the same named kernels; the new values are not a
contradiction because counter replay and trace collection have different
measurement perturbations. Acceptance timing still requires the reviewed
CUDA-event package.

## Profiler-toolchain correction

The first attempt used image
`sha256:2e401388dcc9c180401cb9997e3e8394c2db695c7bf4e3139ff8a9a517940719`.
It contained the Nsight target collector exposed by Nsight Compute 2026.2 but
not a compatible host `QdstrmImporter`. M1 correctness passed and a `.qdstrm`
was retained, but report import failed; no duration from that attempt was
admitted.

Commit `db1e8025317c29e0a357fc79ba1bb0852d2aa2d7` pins the complete
`nsight-systems-2026.1.3=2026.1.3.425-261338342291v0` package in the GLMAXX
development image. cn4 built image
`sha256:4a041313a952def9eb7353f055ee4061f5d76416e090aca04529a597b0bd549a`,
verified both the collector and importer, and then produced importable
`.nsys-rep`, SQLite, and CSV artifacts. No existing image was deleted or
changed.

## Provenance

```text
kernel source commit  8aa70cc5b10e0d0217c79f1aa601bd6349ec5653
Rust binary           5d43cfe66a2eb9d78f9d00c530febfce667d05e0b2c6c220735723336f92f17d
SM120 library         3ef1f5c214cb3453770183fc7793a118d77a6d62057231d4fe8cdcbc32f8bde8
toolchain commit      db1e8025317c29e0a357fc79ba1bb0852d2aa2d7
summary tool commit   0124b3937b9b6e1300b561070bfd4d358746fe34
Nsight Systems        2026.1.3.425-261338342291v0
```

The successful 45-artifact hash stream is outside Git at:

```text
/home/derek/glmaxx/evidence/20260803T182500Z-nvfp4-fc1-nsys-scaling-8aa70cc
```

Its `evidence-sha256.txt` SHA-256 is
`c55e140391b373af18c976b3ec54f52b41ebc7db949a4c4ea865b59c03ba5c64`.
The separate toolchain build record is at
`/home/derek/glmaxx/evidence/20260803T182000Z-nsys-toolchain-db1e802`; its
six-entry artifact stream hashes to
`f441527520fcab79b00395f20fc2cde45066f364e18d2a42f5b098e6ae003e4f`.

The failed importer attempt is preserved at
`/home/derek/glmaxx/evidence/20260803T181500Z-nvfp4-fc1-nsys-scaling-8aa70cc`.
Its 14-artifact stream hashes to
`417fb315f0f0ec4379afcc0b7780efa4c666f81f2e9040870eba0ffc94f30c9c`
and explicitly points to the successful replacement. cn4 returned to
2/2/2/10 MiB used, 0% utilization, with no compute process.
