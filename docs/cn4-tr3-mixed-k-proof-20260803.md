# cn4 TR3 mixed-K source proof

Date: 2026-08-03

Status: K=3 source reconstruction passed; K=4 support is a confirmed GLMAXX
contract blocker

## Scope and isolation

The run read the real checkpoint only from
`/home/derek/models/GLM-5.2-EXL3-TR3-3.25bpw`. The immutable source worktree,
build, and evidence remained under `/home/derek/glmaxx/`. Every container ran
with `--network none`, no `--gpus` argument, and the GLMAXX binary as the
entrypoint. No vLLM worktree, process, container, image, cache, volume, shared
memory, checkpoint, or result was changed.

The four SM120 GPUs were idle after the run. `nvidia-smi pmon` reported no
compute or graphics process, utilization was 0%, and reported memory use was
2/2/2/10 MiB.

## Pinned inputs

| Input | Identity |
|---|---|
| Source commit | `7ebc39cad3d26a4f0a41c029d192af0df48acc52` |
| Container image | `sha256:2e401388dcc9c180401cb9997e3e8394c2db695c7bf4e3139ff8a9a517940719` |
| GLMAXX binary SHA-256 | `14fec4f66c633ffac761b948f83bf82cc60025344eeed4f6fa794e75d7847f66` |
| Checkpoint index SHA-256 | `f5dcd976a64ca70808dd4d8bd3ad07e9610c8ca6c30e3a6ed77ddefdac4c1d21` |
| Tier map SHA-256 | `a287ffe816de5998fbc35a56a1ec05f69eb71087d5bbdfe631242c6b296b2a3d` |

For layer 3, the tier map assigns 192 experts to K=3 and 64 experts to K=4.
Expert 0 is K=3 and expert 6 is K=4. The source shard independently reports
trellis shapes `[384,32,48]` and `[384,32,64]`, respectively, exactly matching
`[K/16,N/16,16*bits]` for a gate projection with logical shape `[6144,512]`.

## Result

The CPU oracle reconstructed layer 3, expert 0 for gate, up, and down on TP
ranks 0 and 3. All six commands returned strict
`glmaxx.exl3-safetensors-proof.v1` JSON, consumed 1,192,964 source bytes, and
produced 6,291,456 reconstructed FP16 bytes. The six reconstruction digests
were distinct and are retained per projection and rank in raw evidence.

The first K=4 proof failed closed with:

```text
glmaxx: Component("model.layers.3.mlp.experts.6.gate_proj.rank0.trellis")
```

The component exists and has the correct K=4 shape. The failure is caused by
GLMAXX constructing `Exl3Metadata` with `bits=3`; `Exl3Metadata::validate`,
the pinned rank planner, Rust CUDA ABI, and CUDA control kernel also admit only
3. This means the current engine cannot ingest or execute the real mixed-K
3.25-bpw checkpoint. The failure is correctly fail-closed, but K=4 support is
on the critical path to a checkpoint smoke test and TR3 performance result.

## Raw evidence

Authoritative evidence root:

```text
/home/derek/glmaxx/evidence/20260803T132500Z-tr3-exl3-real-proof-7ebc39c-r2
```

The evidence manifest SHA-256 is
`29b18a87d378afbe65f2388e7c716c9aabaf1fc3c818b4d670338e85fc37501e`.
The K=3 result summary hashes as
`dee7ce9727be3ecd52941573e19f5dcaa86962aed66d01a8fdfa421a0fc807fe`,
the independently extracted K=3/K=4 header shapes hash as
`a30656613f61834ba3c76607b034002462c459e2bf606070330f1cbf8e1969ff`,
and the captured K=4 error hashes as
`53f8a8fba00b64be81ef2c923e9761da90ca7bca8623f2ec1bd9a44b43e949c3`.

The preserved superseded attempt at
`/home/derek/glmaxx/evidence/20260803T132312Z-tr3-exl3-real-proof-7ebc39c`
allowed the NVIDIA container entrypoint to prepend a banner to stdout. Its
projection succeeded, but its output is not authoritative JSON and it is not
used for acceptance.
