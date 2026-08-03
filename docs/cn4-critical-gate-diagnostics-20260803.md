# cn4 critical-gate diagnostics

Date: 2026-08-03

Status: two fail-closed diagnostics; no FC2, checkpoint, TP4, quality, or
performance acceptance

## Reviewed manifest/ABI Phase B

The manifest/ABI r2 qualification ran from clean isolated commit
`8aa70cc5b10e0d0217c79f1aa601bd6349ec5653`, which is Fable's accepted
candidate `0edfc8d796aeaeb969668005149bcb6286aa1e85` plus the byte-identical
tracked EXL3 and manifest/ABI acceptance results.

The run passed 163 Rust tests, both CUTLASS layout probes, real `sm_120f`
compilation, the exact 256-instruction owned-NVFP4 OMMA count, and all required
exported-symbol checks. FC1 M1 then passed with zero failures, maximum absolute
error 2.0, and maximum relative error 0.027397260069847107.

`gpu-fc2-smoke 1` stopped with:

```text
glmaxx: Driver(-3)
```

No FC2 report was published and the matrix, graph, dense-control, and grouped-
control suites did not run. Source inspection locates this fail-closed value
at the known grouped-control scratch check: the M1 `token_output_f32` extent is
24,576 bytes, smaller than the grouped metadata and CUTLASS workspace. This
independently reproduces the defect already frozen by
`docs/fc2-grouped-control-scratch-r1.md`; it does not accept the pending
correction.

Provenance:

```text
evidence root      /home/derek/glmaxx/evidence/20260803T172009Z-sm120-phase-b-8aa70cc
record-stream SHA  324d7bfe9631a87efc9d47d2c9df0c0edbd40d630860a2a2605e24e403013e9b
record files       26 (build and cargo-target trees excluded)
FC1 output SHA     bbfcca41c01851ac3356ffdbdfed25a505df8e50ba6beb092b5d23d71a860d1f
FC2 error-log SHA  7024901674ad1d3a317e00348f46898c4ce79ab680af6fc7feac7236a70bc4a8
library SHA        3ef1f5c214cb3453770183fc7793a118d77a6d62057231d4fe8cdcbc32f8bde8
binary SHA         5d43cfe66a2eb9d78f9d00c530febfce667d05e0b2c6c220735723336f92f17d
```

The source worktree remained clean. cn4 returned to 2/2/2/10 MiB used, 0%
utilization, and no compute process.

## Real TR3 K=3 admission probe

Qualification commit `54c72f66ea228ac4a04402747b639adc517b3365`
adds a K=3-only real-tensor runner while retaining
`validate_pinned_exl3_checkpoint` as a mandatory pre-device boundary. The
runner pins the exact raw TR3 index SHA-256, requires the standard index name,
uses only the accepted K=3 ABI, refuses K=4, preserves component hashes, and
would compare two device repetitions against the Rust oracle.

The run passed 163 Rust tests, rebuilt the reviewed `sm_120f` library, and
verified the exact raw index:

```text
f5dcd976a64ca70808dd4d8bd3ad07e9610c8ca6c30e3a6ed77ddefdac4c1d21
```

It then stopped before a tensor upload or kernel launch with:

```text
glmaxx: Index
```

This is the production inventory validator rejecting the real checkpoint at
the unresolved index-identity/`metadata.total_size` convention boundary. The
gate was not weakened to force a replay. Therefore zero real K=3 device
projection results exist from this attempt.

Provenance:

```text
evidence root      /home/derek/glmaxx/evidence/20260803T173037Z-exl3-real-k3-54c72f6
record-stream SHA  aa3479ac2fb9b24e3c5fce4417f536352bca6a95cf8f72ff9f285d0cf3fbce00
record files       21 (kernel-build and cargo-target trees excluded)
error SHA          82e6b7cc752f1792caf3ee87a30e389c7e7dbf767dc6e6f8582c548270e29ec0
library SHA        24e244d6a0369d1fe59d1cc3b1fc77c14ce5820292323ff3b121d662b366de26
binary SHA         6724507ec684867e6d0d07fa3b5e08afb739c381062e6ace1a2e0d5db245f14f
```

The source worktree remained clean and cn4 again returned idle.

## Immediate review gates

These diagnostics corroborate, but do not alter or become provenance inputs
to, the existing immutable Fable handoffs. The shortest safe route to the next
device results is:

1. `docs/fable-fc2-grouped-control-scratch-r1-handoff.md` — token
   `fc2-grouped-control-scratch-r1-design-accepted`;
2. `docs/fable-safetensors-index-total-size-v1-handoff.md` — token
   `safetensors-index-total-size-v1-design-accepted`; and
3. `docs/fable-exl3-mixed-k-source-and-kernel-v1-handoff.md` — token
   `exl3-mixed-k-source-and-kernel-v1-design-accepted`.

After acceptance, the sequence remains CPU/native implementation and proof,
implementation re-review where required, a fresh isolated cn4 rerun, real
K=3/K=4 projection microbenchmarks, and only then the 192:64 TP4 layer replay.
