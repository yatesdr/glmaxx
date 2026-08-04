# cn4 cold-load timing preparation

- UTC run: `2026-08-04T00:10:00Z`
- Source: `a90d32d216800be5a69aa0c9019ae036fe442121`
- Host: `cn4` (`192.168.13.34`), four RTX PRO 6000 Blackwell GPUs
- Container: `glmaxx-dev:cuda13.3-rust1.92-ae02a0d`, digest
  `sha256:0b400cb8ba8dc58d8ae9729702260b5c3d1abaa063a8ef9e14380d72df773842`
- CUTLASS: `e05f953a5b3d38adc240df2ff928e0421c2abba3`
- Evidence: `/home/derek/glmaxx/evidence/20260804T001000Z-cold-timing-a90d32d-r2`
- Top-level evidence files: 34
- Sorted top-level record-stream SHA-256:
  `6a09572eecece5c2fc1da5a5fd2473ae750b0ce2e293901e4e55ddcef048d5da`

The exact committed tree passed all 416 tests on cn4. CUDA 13.3 built the
library for `sm_120f`; `cuobjdump` found the five expected SM120 cubins, the
build retained exactly 256 expected NVFP4 OMMA instructions, the required
FC1/FC2/EXL3 symbols were present, and the Rust release binary linked the
generated library. The preparation verdict is `PREPARED_NO_DEVICE_LAUNCH`.
No CUDA device kernel was launched.

The unsuffixed evidence directory is an excluded partial attempt that stopped
at offline Cargo resolution because the project cache was not mounted. An
earlier attempt stopped before evidence allocation on Git safe-directory
validation. Neither attempt launched a device kernel. The `-r2` directory is
the successful record. Final occupancy was 2, 2, 2, and 10 MiB with zero GPU
utilization and no compute applications or containers.
