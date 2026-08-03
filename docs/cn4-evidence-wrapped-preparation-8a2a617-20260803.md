# cn4 evidence-wrapped SM120 preparation

Date: 2026-08-03

Status: passed preparation; no device launch

## Result

Clean detached source `8a2a617a778a7cd528f5516660de681f94be22db`
completed the evidence-wrapped Phase-B preparation path on cn4:

```text
/home/derek/glmaxx/evidence/20260803T230325Z-phase-b-prepare
state=COMPLETE
regular-files=4398
evidence-manifest-sha256=a20f1ce8a1fa97278147ecb535132dd063f2ed7d934cdbdc83e442b8bf626f0c
```

The standalone fail-closed verifier accepted the exact file set, every
recorded digest, and agreement between the allocation and terminal states.
The terminal contract SHA-256 is
`20909c8a4b4fbcdccc0ed3acaeb6fce36e7e3f30c1e25baea7d6dc9a05976bf9`.

The run passed all 413 committed Rust tests, built five real `sm_120f` cubins,
passed the independent CUTLASS SFA and SFB layout probes with 42,564,864 and
393,216 comparisons respectively, found the exact 256 owned-NVFP4 OMMA
instructions, verified the required launch symbols, and linked the release
Rust binary against the generated SM120 library. The principal artifact
hashes are:

```text
b79cb5a15b010fe71833da17006143bd2b4ea80be69687743219cc8b918ef1f8  libglmaxx_sm120.so
8a59b031e8c07b3169d61f45fbef3d0fa217863cad715a77d4839a7b5729a407  glmaxx_cutlass_layout_probe
ce3d6372bccdf026c173392ec6b72317738721673936a6343559883197e38c11  glmaxx_cutlass_activation_layout_probe
cc1c5d00ef2f2b4728d359fd69ce208a4ca6069de272fbe6de745b52255cb30a  glmaxx_cutlass_nvfp4_dense_control
1e778a241445c0e74135a3e5745150b63f960afc8e43a63dd574d44bd53718c9  glmaxx
```

## Provenance and isolation

- source clone:
  `/home/derek/glmaxx/worktrees/prepare-4828e4c-standalone-r2`
- image: `glmaxx-dev:cuda13.3-rust1.92-ae02a0d`
- image digest:
  `sha256:0b400cb8ba8dc58d8ae9729702260b5c3d1abaa063a8ef9e14380d72df773842`
- CUTLASS: `e05f953a5b3d38adc240df2ff928e0421c2abba3`
- verdict SHA-256:
  `fd88a10c6fe54ace0d4cfc892e53bd8cbf26e543bbe8e6690ea440d9c4d58c34`

Every worktree, cache, container, and evidence path was under
`/home/derek/glmaxx`. No vLLM resource or production result was used or
changed. Three earlier wrapper bring-up attempts at `20260803T225339Z`,
`20260803T225505Z`, and `20260803T225555Z` stopped fail-closed on CUTLASS
ownership and Cargo-cache mount/ownership mistakes. A fourth preparation at
`20260803T225625Z` completed before exact terminal-manifest sealing was added;
it is retained as diagnostic evidence and is not substituted for this run.

The preparation script launched no CUDA device kernel. After completion all
four RTX PRO 6000 Blackwell GPUs reported 2/2/2/10 MiB used, zero utilization,
and no compute application; no GLMAXX container remained. This result makes
no checkpoint, model-output, quality, capacity, serving, latency, or
throughput claim. Actual Phase-B device qualification remains gated by the
accepted current manifest/kernel review contract.
