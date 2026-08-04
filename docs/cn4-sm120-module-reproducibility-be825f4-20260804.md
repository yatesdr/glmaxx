# cn4 same-source SM120 module reproducibility diagnostic

Date: 2026-08-04

Status: diagnostic; no kernel, hot-reload, or acceptance claim

## Observation

The sealed target and recurrent-draft regressions independently compiled
`libglmaxx_sm120.so` from the same source commit, worktree, container, CUDA
toolchain, CUTLASS revision, and build options. Their raw library hashes differ:

| Run | Raw SHA-256 |
|---|---|
| target layer 3 | `97499d51c598b06af6d8a2787f016b92720bc17d30795984ad2af7bae1f45331` |
| recurrent draft layer 78 | `ad771c7d4de151efa1527166149740a999204c0c0ef7bfcc35320d2306f4b073` |

Both files are 4,254,536 bytes. Their first difference is byte 3,975,352,
which is 16 bytes into `.strtab` at file offset `0x3ca8a8`.

## Root cause

An exhaustive `readelf -sW` comparison found exactly five differing symbol
records. Every difference is a local `FILE` symbol containing NVCC's
process-specific `tmpxft` name for one translation unit:

```text
nvfp4_routed_fc1
nvfp4_routed_fc2
exl3_projection_control
cutlass_nvfp4_dense_control
cutlass_nvfp4_fc2_control
```

No exported or loadable symbol differs. Both libraries have the same GNU build
ID:

```text
b5df6dffe65eece9f89e71c7c3605e456aee0fe5
```

Further controls:

| Comparison | Target | Draft | Equal |
|---|---|---|---|
| dynamic-symbol stream SHA-256 | `0b3606cfc4c534db534127876fee8e1924709a6326ba3d7416942c499b725263` | same | yes |
| strip-debug ELF SHA-256 | `9e7b351d4e57d34e411b3651aaf8304fb5c4b77696b3d86a18e81a0156d64eed` | same | yes |
| strip-all ELF SHA-256 | `bd8ce2439e72e31e7ed72e48f9c96c03bd7b7dfc332aaafd9fa1e6b963c67896` | same | yes |

The strip-debug equality retains `.text`, `.rodata`, and the complete
`.nv_fatbin`; it therefore proves the loadable host/device content is
byte-identical. A separate SASS dump was not captured because `cuobjdump` is
container-provided and unavailable in the host diagnostic shell.

Temporary comparison copies remain outside Git at:

```text
/home/derek/glmaxx/tmp/module-identity-be825f4-20260804T162800Z
```

## Identity consequence

The raw library SHA-256 remains the correct fail-closed identity for one exact
immutable build artifact and for Phase-B-to-Phase-C artifact reuse. GLMAXX must
not silently normalize or substitute a rebuilt library merely because its
loadable bytes appear equivalent.

Conversely, a raw `.so` mismatch between independent builds is not by itself
evidence of changed SASS or semantics. A hot runtime bundle should hash the
exact immutable cubin/module bytes it actually loads, as required by the
pending resident-generation design. Independently rebuilt but semantically
equivalent artifacts remain distinct runtime generations unless a separately
reviewed canonical module format is adopted.

This diagnostic does not accept a runtime-generation identity, module loader,
hot reload, canary, or zero-weight-traffic claim.
