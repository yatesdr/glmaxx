# cn4 pinned preparation at 01b6176

Date: 2026-08-03

Status: release tests and `sm_120f` compilation pass; no device kernel launch

The pushed source commit
`01b617646b2644c8012b06f2587179d0420237d4` was fetched into the isolated
GLMAXX repository on cn4 and checked out as a clean detached worktree:

```text
/home/derek/glmaxx/worktrees/prep-01b6176-20260803T201000Z
```

No vLLM path, container, image, cache, process, port, volume, checkpoint, or
result was used or changed. The build containers used `--network none`, did
not receive `--gpus`, and wrote only to GLMAXX-owned build and evidence paths.

## Results

- `cargo test --workspace --release --offline`: 413 tests passed, zero
  failures.
- CUDA 13.3/CUTLASS commit
  `e05f953a5b3d38adc240df2ff928e0421c2abba3` completed the full CMake/Ninja
  build with `-arch=sm_120f`.
- `cuobjdump --list-elf` retained five real SM120 cubins:
  `nvfp4_routed_fc1`, `nvfp4_routed_fc2`, `exl3_projection_control`,
  `cutlass_nvfp4_dense_control`, and `cutlass_nvfp4_fc2_control`.
- The Rust `cuda-ffi` release binary linked to the built
  `libglmaxx_sm120.so`.
- After the build all four GPUs reported zero utilization and 2/2/2/10 MiB
  used, with no compute process.

Pinned artifacts:

| Artifact | SHA-256 |
|---|---|
| Development image | `4a041313a952def9eb7353f055ee4061f5d76416e090aca04529a597b0bd549a` |
| `libglmaxx_sm120.so` | `e706a7ff7f7faa787de6b98e8e7ae66e9c3e4c29e8bb66701069027413c1492e` |
| Rust `glmaxx` binary | `046f4df96a78c3480297c3e3cddca02bb89c1a6c3c48f061347999dde2746884` |
| `summary.json` | `39251c1e5f8e65d143e5b1fdb0437bb557fc766c2001dc5ef64f4f09a7a7d761` |
| Ordered 17-record hash manifest | `c6ed9f64467fb6eea0e15a8d57f311badf5760458282c17106cc816e469d7ff8` |

Authoritative raw evidence:

```text
/home/derek/glmaxx/evidence/20260803T201500Z-prep-01b6176
```

The image lacks the `rustfmt` and `clippy` components, so the attempted full
container-local gate stopped at formatting before compilation. That is a
toolchain limitation, not a passing gate; formatting and Clippy remain covered
by the local host gate. A first release-test attempt also used an incomplete
offline Cargo cache and is retained as a failed preparation attempt. The
second attempt used the previously pinned GLMAXX-only offline cache and is the
413-test result above.

This record proves only clean source/toolchain preparation. It does not accept
the pending FC2, safetensors, mixed-K, hybrid, target-program, or executor
reviews and is not real-weight correctness, TP4 layer, checkpoint, quality,
capacity, latency, throughput, or serving evidence.
