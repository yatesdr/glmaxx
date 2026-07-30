# Target-program routed projection discriminator v1

Date: 2026-07-30

Status: design candidate; adversarial review required before CPU
implementation

GPU claim: none

## Problem

`docs/target-layer-execution-v1.md` defines one runtime binding as:

```text
tensor_id:u32
role_id:u16
expert_id:i16
codec_id:u16
```

That tuple is not a unique semantic key for the current capacity-EXL3 rank
format. For every routed expert, gate and up are separate authenticated
descriptors. They have the same layer, role `0x0501`, expert, codec, and TP
axis. Only their exact source name/metadata projection and tensor ID differ.

Inferring gate versus up from descriptor adjacency, lexicographic order,
payload offset, or an unchecked name would make the target program depend on
an unstated conversion accident. Treating the pair as one combined descriptor
would address bytes that do not exist in the current rank file.

The NVFP4 execution path has the opposite useful representation: one combined
gate/up matrix with rank-local logical shape `[1024,6144]`. The target program
must express both representations without a runtime repack.

## Frozen discriminator

Every routed tensor binding in `TargetProgram.v1` adds:

```text
projection_id:u8
```

with this closed enumeration:

| ID | Meaning | Legal role | Legal representation |
|---:|---|---:|---|
| 0 | not a routed projection | any non-routed role | protected tensor |
| 1 | gate | `0x0501` | split EXL3 source |
| 2 | up | `0x0501` | split EXL3 source |
| 3 | down | `0x0502` | EXL3 or NVFP4 |
| 4 | combined gate/up | `0x0501` | combined NVFP4 |

Other values fail closed. A non-routed tensor with a nonzero discriminator,
or a routed tensor with zero, is invalid.

The canonical runtime binding becomes:

```text
tensor_id:u32_le
role_id:u16_le
expert_id:i16_le
codec_id:u16_le
projection_id:u8
reserved:[u8;5] = 0
```

It is exactly 16 bytes. The five reserved bytes are hashed and must be zero.
All target-program entry hashes use this amended record. The target-program
hash domain changes from `glmaxx.target-program.v1\0` to:

```text
glmaxx.target-program.v1.projection-discriminator-v1\0
```

No old and amended program may share a graph-profile or step-input identity.

## Startup derivation

The discriminator is engine-derived after production manifest validation. It
is not supplied by a request, rank, graph, route, or kernel.

For target layer `L` and expert `E`, the engine constructs and hashes these
canonical UTF-8 names:

```text
model.layers.L.mlp.experts.E.gate_proj.weight
model.layers.L.mlp.experts.E.up_proj.weight
model.layers.L.mlp.experts.E.down_proj.weight
model.layers.L.mlp.experts.E.gate_up_proj.weight
```

`L` and `E` use canonical unsigned decimal with no sign or leading zero. The
hash is SHA-256 of the exact bytes with no terminator. It must equal the
already validated `ValidatedTensorSemantic.name_sha256`; runtime strings are
then discarded.

The engine may alternatively derive the same discriminator from a
format-native projection enum only after that enum is added to the validated
semantic catalog and hashed into its ABI. V1 does not accept metadata-only
derivation because protected/NVFP4 metadata does not yet expose one common
projection field.

Tensor ID, name hash, layer, role, expert, codec, TP axis, dtype, rank/global
shape, and plane geometry must all agree. A name match alone is insufficient.

## Legal representation per expert

Each target sparse layer and expert must select exactly one process-common
gate/up representation:

### Split EXL3

Required:

- one `projection_id=1` gate descriptor;
- one `projection_id=2` up descriptor;
- both codec `CODEC_EXL3_SOURCE`;
- each rank shape `[512,6144]`;
- each global logical shape `[2048,6144]`;
- identical layer/expert/rank/TP semantics; and
- one `projection_id=3` EXL3 or NVFP4 down descriptor with rank shape
  `[6144,512]`.

The EXL3 execution program launches the two direct source projections into
distinct fixed graph slots. SwiGLU consumes both slots. No combined weight
buffer is materialized.

### Combined NVFP4

Required:

- one `projection_id=4` combined gate/up descriptor;
- codec `CODEC_NVFP4_1D` or `CODEC_NVFP4_2D`;
- rank shape `[1024,6144]`;
- global logical shape `[4096,6144]`;
- output rows `0..511` are gate and `512..1023` are up on every rank; and
- one `projection_id=3` EXL3 or NVFP4 down descriptor with rank shape
  `[6144,512]`.

The gate-then-up row order is part of the quantizer and kernel ABI. A
converter must pack and CPU-decode-prove that order before the representation
can enter a production policy.

### Forbidden mixtures

For one expert, all of these fail:

- gate without up or up without gate;
- split plus combined gate/up;
- two descriptors with one discriminator;
- EXL3 combined gate/up;
- NVFP4 split gate/up in v1;
- mismatched gate/up codecs or geometry;
- a gate/up descriptor under role `0x0502`;
- a down descriptor under role `0x0501`; or
- different representation choices on different TP ranks.

A hybrid policy may choose split EXL3 for one expert and combined NVFP4 for
another only when that immutable per-tensor choice is identical across all
ranks and is already bound by the common tensor catalog, weight policy,
target-program hash, graph profile, and step input. No serving-step choice is
allowed.

## Program compilation

At startup, each rank independently derives the 16-byte records from its
validated semantic catalog. The coordinator requires one identical ordered
record stream and target-program hash from all four ranks before graph
capture.

Within a layer entry, records are ordered by:

```text
(role_id, expert_id, projection_id, tensor_id)
```

where `expert_id=-1` sorts before routed experts. Tensor ID is a final
uniqueness tie-breaker, not the semantic discriminator. Duplicate first three
fields are invalid.

The compiled layer entry stores fixed numeric tensor IDs and projection IDs.
Graph construction resolves those tensor IDs through the resident
tensor-device binding. Graph launch performs no string hashing, metadata
parsing, map lookup, representation choice, or repack.

## Shape and count consequences

For a capacity-EXL3 sparse layer:

```text
256 gate + 256 up + 256 down = 768 routed tensor bindings
```

For a fully combined-NVFP4 sparse layer:

```text
256 combined gate/up + 256 down = 512 routed tensor bindings
```

The existing 533-tensor M4 subset count is capacity-EXL3-incompatible: its
stated 512 routed tensors assumes combined gate/up, while its immutable source
identity says NVFP4. That count is valid only for the combined-NVFP4
laboratory representation. The M4 builder must name and validate that
representation explicitly; it cannot reuse a capacity-EXL3 tensor inventory
or imply both formats have 533 tensors.

The complete capacity-EXL3 production rank count remains 59,585 and is not
changed by this amendment.

## Required CPU proof after review

The implementation must:

1. derive all four canonical name hashes independently;
2. compile every expert of layers 3 through 77 from the actual pinned
   capacity-EXL3 plan and prove 768 routed bindings per layer;
3. compile a synthetic combined-NVFP4 layer and prove 512 routed bindings;
4. prove byte-stable 16-byte records and amended entry/program hashes;
5. reject every forbidden mixture above at experts 0 and 255;
6. reject name, tensor ID, role, expert, codec, TP-axis, shape, row-order,
   duplicate, missing, and cross-rank drift independently;
7. prove the compiled program retains no names or metadata parser;
8. resolve all compiled IDs through the adopted device-binding CPU fake; and
9. rederive and correct the M4 subset inventory for its explicitly selected
   representation.

## Explicit non-claims

This candidate does not implement the compiler, amend an on-disk rank file,
choose the serving hybrid policy, authorize conversion, or execute a kernel.
It does not accept the broader target-layer program, graph ABI, collective
schedule, one-layer replay, checkpoint smoke, quality, or performance.
