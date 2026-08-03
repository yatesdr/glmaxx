# cn4 SM120 first-device result

Date: 2026-08-03

Status: failed qualification; useful diagnostic evidence only

## Provenance and isolation

The reviewed source candidate was
`0edfc8d796aeaeb969668005149bcb6286aa1e85`. A cn4-local integration commit,
`972379140e88504164115b74c02fdefa7604f335`, added only the accepted root
review artifact `fable-manifest-abi-v022-r2.md`; it did not change compiled
kernel or Rust sources.

- worktree:
  `/home/derek/glmaxx/worktrees/phase-b-0edfc-20260803T153359Z`
- image: `glmaxx-dev:cuda13.3-rust1.92-7ebc39c`
- image digest:
  `sha256:2e401388dcc9c180401cb9997e3e8394c2db695c7bf4e3139ff8a9a517940719`
- CUTLASS: `e05f953a5b3d38adc240df2ff928e0421c2abba3`
- review artifact SHA-256:
  `b95fc05837ef1de91fda44bc1b3df49224dde93c76554efbeef3a9a58a70d882`
- library SHA-256:
  `c0dd59ca7236ae83391f8611c21f7465d015fbea5e06cac8195eb565f03e6503`
- executable SHA-256:
  `2d3e01be9031435be7b432fdfac8b104dedb246b0049b85e60e93cad7f9b35d2`

All paths, caches, worktrees, containers, and evidence were under
`/home/derek/glmaxx`. No vLLM worktree, image, container, volume, cache, port,
or result was used or changed. Failed GLMAXX containers were retained rather
than deleted.

## Runs

The first container attempt stopped before a CUDA context because its offline
Cargo view lacked `libc`:

```text
/home/derek/glmaxx/evidence/20260803T153359Z-phase-b-9723791
record-set-sha256=a165ddbbf6c0a7e1e888c98d472f4a190f5b17fc0c8a0e5f18b381f274c32403
```

The cache-corrected run compiled a real `sm_120` image and launched the first
GLMAXX CUDA kernel on an RTX PRO 6000 Blackwell:

```text
/home/derek/glmaxx/evidence/20260803T153900Z-phase-b-9723791-r2
raw-record-set-sha256=f8a4eb30a8c87ed1d3f37aa51cd74cc7813296cb61f40334b10b8fce6ca3b760
```

That aggregate is SHA-256 over the sorted `sha256sum` stream for every regular
file except the reproducible `build/` and `cargo-target/` trees.

FC1 M=1 passed with ABI `glmaxx.sm120.nvfp4.routed_moe.v2`, shape
`[1,6144,1024]`, zero failing elements, maximum absolute error `2.0`, and
maximum relative error `0.027397260069847107` under the frozen
`0.5 + 0.02*abs(reference)` element rule. The raw report hash is
`d7a9030cb1ce43027df86bae1cc3945ebb15bc4859b77bcc944fcc9013450f65`.

The next command, `gpu-fc2-smoke 1`, returned `Driver(-3)`. Inspection proved
that the grouped CUTLASS control placed metadata and CUTLASS workspace at
`token_output_f32`, whose fixture allocation was only 24,576 bytes at M=1.
The failure report hash is
`981721e8f85ee9b94b3f5f1b9ac007e107555234dda94231b31fc51294a42e4c`.
The phase stopped fail-closed; FC2 did not pass.

A separate continuation reused the exact library and executable:

```text
/home/derek/glmaxx/evidence/20260803T154500Z-fc1-continuation-9723791
record-set-sha256=9f10f27b885a31d4e40ba9e8ba59edc15eed627085181df4bb714111b9294c75
matrix-summary-sha256=2b66d1e7b7cab3bd0a7ff6a5f03755c93a0272315649438ad71f7b9c322ca97e
```

It executed 135 positive cases and nine negative-route cases. All negative
routes rejected, both twenty-run eager cases were deterministic, and 43
elements exceeded the semantic-oracle tolerance. Every deviation was the
same M=256 deterministic-random boundary: column 20, semantic BF16 `-172`
(`0xc32c`), device BF16 `-177` (`0xc331`). The deviations appeared once in
each one-hot report and once per affected assignment in the multi-route
reports: `1+8+1+1+8+8+8+8 = 43`.

An independent CPU reconstruction then used the CUDA control's specified
schedule: 256 FP32 FMA lanes over K=6,144 followed by reductions at strides
128, 64, 32, 16, 8, 4, 2, and 1. For row 239, column 20 it produced BF16
`-177` exactly, while the sequential semantic oracle produced `-172`. This
identifies an oracle/schedule mismatch; it does not by itself revise or pass
the frozen matrix.

After the runs all four GPUs reported 2/2/2/10 MiB used, zero utilization,
and no compute application. No TP4 layer, checkpoint, quality, capacity, or
performance claim follows from this record.
