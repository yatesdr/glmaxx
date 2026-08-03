# cn4 current-tree preparation at 5641497

Date: 2026-08-03

Status: complete CPU proof and `sm_120f` build pass; no CUDA kernel launch

## Scope and isolation

The clean, dedicated cn4 checkout `/home/derek/glmaxx/src` was fast-forwarded
to committed source
`5641497d6e0d871acf570592730a19f1fbcbe24e`. No vLLM path, container,
image, cache, port, result, checkpoint, process, or shared-memory object was
used or changed. Before the run all four RTX PRO 6000 Blackwell GPUs were at
zero utilization with 97,232--97,249 MiB free and no compute process.

The immutable development image was:

```text
sha256:0b400cb8ba8dc58d8ae9729702260b5c3d1abaa063a8ef9e14380d72df773842
```

Raw evidence is retained outside Git at:

```text
/home/derek/glmaxx/evidence/20260803T220100Z-current-5641497-prepare
```

## Reproducible command

The successful run used the repository read-only, CUTLASS at pinned commit
`e05f953a5b3d38adc240df2ff928e0421c2abba3` read-only, the dedicated Cargo
cache, a fresh evidence directory, all four devices for inventory only, and
no network:

```bash
docker run --rm --gpus all --network none \
  --name glmaxx-prepare-5641497-20260803T220100Z \
  -e CARGO_HOME=/cargo \
  -e CUTLASS_DIR=/cutlass \
  -e GLMAXX_EVIDENCE_DIR=/evidence/20260803T220100Z-current-5641497-prepare \
  -e GLMAXX_CONTAINER_DIGEST=sha256:0b400cb8ba8dc58d8ae9729702260b5c3d1abaa063a8ef9e14380d72df773842 \
  -v /home/derek/glmaxx/src:/workspace:ro \
  -v /home/derek/glmaxx/deps/cutlass:/cutlass:ro \
  -v /home/derek/glmaxx/cache/cargo:/cargo:rw \
  -v /home/derek/glmaxx/evidence:/evidence:rw \
  -w /workspace \
  sha256:0b400cb8ba8dc58d8ae9729702260b5c3d1abaa063a8ef9e14380d72df773842 \
  bash -lc 'git config --global --add safe.directory /cutlass && \
    git config --global --add safe.directory /workspace && \
    exec bash scripts/cn4-phase-b-prepare.sh'
```

An initial offline attempt failed closed before compilation because the
dedicated cache lacked the newly locked `tokenizers` dependency. Its partial
record is retained at
`/home/derek/glmaxx/evidence/20260803T215500Z-current-5641497-prepare`.
The exact locked dependency graph was fetched once into only
`/home/derek/glmaxx/cache/cargo`; the successful run above then used
`--network none`.

## Results

- All 413 tests in the clean published source passed. The 414 count observed
  in the local development worktree includes an unrelated uncommitted test
  and is not attributed to this commit.
- The independent CUTLASS SFB layout probe passed 393,216 comparisons.
- The independent CUTLASS SFA layout probe passed 42,564,864 comparisons.
- CMake/Ninja produced five `sm_120f` cubins: NVFP4 FC1, NVFP4 FC2, EXL3
  projection, and the two CUTLASS controls.
- The built library retained exactly 256 expected Blackwell NVFP4 OMMA
  instructions and all required FC1, FC2, and EXL3 symbols.
- Postflight observation found zero compute processes. Every GPU remained at
  zero utilization with 2/2/2/10 MiB used.

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `libglmaxx_sm120.so` | 4,250,440 | `e070c4080827452f868ecd88f9e83cf6fb0fb31cd46e4a98153683aa5e68c89b` |
| release `glmaxx` | 9,655,048 | `8fca4bc356c06fbacfd3357dd3edf5c4d704ddd2a00d8b1e655a309c96e08174` |
| complete Cargo test log | - | `1f2cf76b5545baf1d5b8b04749c96f8dba04e95d53096d81563f2536a18b06b6` |
| CUDA build log | - | `6bb296648241814635665b162f3f16daa1c0064f9bf947b9648eee35d7e28dfb` |
| ordered evidence hash manifest | 1,562 | `df990d33bdef97b9dd187c3f57f051ff85152a4506da6e31d1be042b54af0015` |

## Gate boundary

The corrected EXL3 source, EXL3 warp, and manifest ABI review results are now
machine-accepted at their exact required root paths. They accept candidate
`0edfc8d796aeaeb969668005149bcb6286aa1e85`, not arbitrary later source.
Between that candidate and `5641497`, route-relevant FFI, headers, kernels,
build definitions, profile, and engine contract files changed. The corrective
current-tree acceptance v3-r2 design remains unreviewed and unimplemented.

Therefore this run deliberately stopped at `PREPARED_NO_DEVICE_LAUNCH`.
It proves that the current committed source and toolchain compile; it does not
qualify a current-tree kernel, run FC1/FC2/EXL3, load weights, execute a model
layer, establish quality, capacity, latency, or throughput, or remove the
native program stub. The next device gate is an accepted and implemented
current-tree v3-r2 rebind followed by a fresh immutable Phase-B run.
