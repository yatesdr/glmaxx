# cn4 FC1 reduction-order probe at 86fe811

- UTC launch: `2026-08-04T00:58:08Z`
- Source: `86fe8111585fdabd91312da7a0be7c3889f376b9`
- Host/device: cn4, GPU0, RTX PRO 6000 Blackwell, SM120
- Container: `sha256:0b400cb8ba8dc58d8ae9729702260b5c3d1abaa063a8ef9e14380d72df773842`
- CUTLASS: `e05f953a5b3d38adc240df2ff928e0421c2abba3`
- Raw evidence: `/home/derek/glmaxx/evidence/20260804T005300Z-fc1-reduction-probe-86fe811`
- Top-level evidence record-stream SHA-256: `99cd60a0e46c411622781a5509c2d23aa4265040386c2e304bdd3bc059144571`

The clean detached tree passed 417 tests. CUDA 13.3 built five real
`sm_120f` cubins, retained 256 expected NVFP4 OMMA instructions, and linked
the Rust binary to the generated GLMAXX library. The device run exposed only
GPU0 and used no network.

For deterministic-random M256 row 239, FC1 output column 20:

| Path | BF16 bits | Value | Absolute from sequential semantic |
|---|---:|---:|---:|
| ascending-K semantic oracle | `0xc32c` | -172 | 0 |
| CPU 256-lane fixed tree | `0xc331` | -177 | 5 |
| CUDA-core M1 | `0xc331` | -177 | 5 |
| CUTLASS dense M1 | `0xc330` | -176 | 4 |
| CUTLASS dense M256 | `0xc330` | -176 | 4 |

The CUDA-core value exactly matched the independently modeled lane-strided
FMA/tree schedule. CUTLASS was identical at M1 and M256, and the M256 repeat
was bitwise identical. This rules out batch-shape drift for the observed
failure and localizes it to deterministic FP32 accumulation-order differences near a
cancellation boundary. It does not qualify either path: the frozen semantic
tolerance at -172 is 3.94, so both remain outside that gate.

No runtime weight repack or persistent dequantization occurred. The run did
not change the frozen tolerance, establish model quality, or qualify a layer,
checkpoint, serving path, capacity, or performance result. Postflight showed
2/2/2/10 MiB used, zero utilization, no compute process, and no retained
GLMAXX container.

Key SHA-256 identities:

| Artifact | SHA-256 |
|---|---|
| probe JSON | `86392ac300f02dd9525a2e729b6627ec9a3cb1992206ea6c998dca597b193b86` |
| `libglmaxx_sm120.so` | `931c188295d79b670cd5e5459047a78ea52388e39039d84e4078fc22328d261d` |
| release `glmaxx` | `acf7d57d5decbd74f1ec56e340925a9cdf14a04913b1a240c641e292ca66f88b` |
| container inspection | `f628301940fcb456858057b40d17f653878d51001ebd38452b04e521a2c456fc` |
| timestamped stdout | `6c85c919c74f03de633b744e57b7cdf6c553dd234e68c3b8f2a4f2d5135cc4bd` |
