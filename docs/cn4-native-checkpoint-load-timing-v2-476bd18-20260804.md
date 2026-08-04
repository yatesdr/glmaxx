# cn4 native checkpoint-load timing v2 preparation

- UTC run: `2026-08-04T00:25:00Z`
- Source: `476bd1862cfdbb764be236e66abd696646a23f08`
- Host: `cn4` (`192.168.13.34`), four RTX PRO 6000 Blackwell GPUs
- Container: `glmaxx-dev:cuda13.3-rust1.92-ae02a0d`, digest
  `sha256:0b400cb8ba8dc58d8ae9729702260b5c3d1abaa063a8ef9e14380d72df773842`
- CUTLASS: `e05f953a5b3d38adc240df2ff928e0421c2abba3`
- Evidence:
  `/home/derek/glmaxx/evidence/20260804T002500Z-cold-timing-v2-476bd18`
- Sorted top-level `sha256sum` record-stream SHA-256:
  `9588bf8cca430f5340d58a399947d7f62acfa40e587ee3a4ee9cb0f36c5f52b5`

The clean detached source passed all 416 committed-tree tests. CUDA 13.3 built
five real `sm_120f` cubins, retained exactly 256 expected NVFP4 OMMA
instructions, exported the required NVFP4 FC1/FC2 and EXL3 symbols, and linked
the release Rust binary to the generated library. The additional
`cuda-ffi`-gated startup timing arithmetic test passed against that linked
build; its record SHA-256 is
`1dba34590670b206fba89ae110d3548ea86182e5f5b49be3f2ce5d483de06238`.

This was a compile/link preparation and pure unit test only. It launched no
CUDA device kernel, read no checkpoint, and makes no model execution,
checkpoint-load timing, cold-start, quality, KV-capacity, latency, serving, or
throughput claim. The first authenticated four-rank image run and independent
review of the v2 timing partition remain pending.

All paths were under `/home/derek/glmaxx`; no vLLM resource was used or
changed. Before and after the run the GPUs reported 2, 2, 2, and 10 MiB used,
zero utilization, and no compute process. No GLMAXX container remained.
