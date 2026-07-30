# NVFP4 laboratory rank manifest and M4 load plan v1

Date: 2026-07-30

Status: design candidate; adversarial review required before CPU
implementation

GPU evidence: none

## Purpose

This contract supplies the missing manifest, semantic catalog, memory-budget,
and type-state boundary for the 533-tensor M4 checkpoint defined by
`docs/small-checkpoint-runner-v1.md`.

The current production schema
`glmaxx.rank-manifest.v0.2.2` is intentionally capacity-EXL3-only. It requires
59,585 tensors and rejects every NVFP4 descriptor. M4 must not weaken,
reinterpret, or add a profile to that schema.

The laboratory path is a separate closed schema:

```text
glmaxx.rank-manifest.nvfp4-laboratory.v1
```

It may create a bounded M4 executor, but can never produce a production
weight handle, enter production `HEALTHY`, bind an HTTP endpoint, populate a
serving prefix namespace, or establish full-model capacity.

## Preconditions

Implementation remains blocked until all of these inputs have accepted,
hash-bound artifacts:

1. NVFP4 format and canonical decoder;
2. fused routed-MoE r3 design and its 1D `0x1202` CPU proof;
3. actual-shape SM120 operator correctness for the exact codecs/layouts;
4. layer-6 M3 replay and its TP4/DCP4 collective routes;
5. target-program v2 layout binding;
6. M4 source checkpoint/conversion identity; and
7. M4 graph and measured laboratory memory profile.

This document does not accept those dependencies or authorize hardware.

## Corrected native header identity

M4 contains both source-precision protected tensors and NVFP4 tensors, with
no EXL3 payload. Its required header flags are:

```text
bit 0 direct kernel layouts       = 1
bit 1 contains NVFP4              = 1
bit 2 contains EXL3               = 0
bit 3 contains protected tensors  = 1
bit 4 hybrid EXL3/NVFP4 policy    = 0

flags = 0b0_1011 = 11
```

The current `derive_header_flags` implementation records `saw_plain` but
never sets bit 3. It would emit `3`, contradicting `spec/format-v0.md`. M4
cannot use those bytes.

Before laboratory conversion, the format implementation must:

- add a named protected flag constant at bit 3;
- set it iff at least one plain protected descriptor exists;
- reject a missing or extra protected bit on read and streaming read;
- mutation-test plain-only, NVFP4-only, plain+NVFP4, EXL3-only, and
  EXL3+NVFP4+plain files;
- regenerate every affected deterministic fixture; and
- pass the current-tree format/manifest re-pin gate.

This is a correction to required v0.2.3 semantics, not permission to accept
both flag spellings. Existing bytes with a false protected bit remain
laboratory-invalid after the correction.

## Exact profile identity

The profile block is:

```text
name                         "nvfp4-laboratory-m4"
scope                        "layer6-final-norm-head"
serving_allowed              false
production_health_allowed    false
http_allowed                 false
prefix_namespace_allowed     false
mtp_depth                    0
tensor_count                 533
rank_payload_bytes           1,982,245,376
file_codec_metadata_bytes    65,536
device_weight_arena_bytes    1,982,245,376
device_metadata_arena_bytes  130,944
```

All fields are mandatory and unknown fields fail. The profile block binds:

- the accepted M2 and M3 result digests/tokens;
- the r3 layout/conversion policy digest;
- the exact M4 fixture digest;
- the exact laboratory budget digest; and
- the compiled subset-catalog digest.

Changing only the profile name, plan profile byte, or one permission boolean
cannot create a valid manifest or load plan.

## Immutable source and conversion identity

The manifest has distinct `source` and `conversion` blocks.

`source` binds:

```text
repository
revision
complete_checkpoint_sha256
source_index_sha256
source_manifest_sha256
selected_source_catalog_sha256
selected_source_bytes
M2_result_sha256
M2_acceptance_token
```

`conversion` binds:

```text
repository = "https://github.com/yatesdr/glmaxx.git"
commit
container_digest
rust_toolchain
CUDA_toolkit
CUTLASS_commit
format_spec_sha256
engine_spec_sha256
operation_manifest_sha256
conversion_policy_sha256
kernel_abi_sha256
```

`complete_checkpoint_sha256` is a canonical digest of the complete source
file inventory, not a hash of a directory name. `selected_source_bytes`
counts exact source tensor bytes and is not overloaded with native output
payload bytes.

Each tensor record separately binds source tensor name, source file, byte
range or tensor identity, source dtype/shape, and source SHA-256. A selected
source tensor cannot appear twice or supply two output tensor IDs.

The conversion route is closed:

```text
BYTE_EXACT_PROTECTED
NVFP4_1D_1201_TO_1202_PERMUTE
NVFP4_1D_1202_RETAIN
NVFP4_2D_TO_1D_REQUANTIZE_REVIEWED
NVFP4_DOWN_RETAIN
```

The 2D-to-1D route is unavailable unless its separately accepted conversion
and per-position quality artifact is named exactly. No filename, shape, or
desired kernel may imply a route.

## Exact subset projection

The expected subset is derived from the compiled GLM-5.2 operation manifest,
not manifest-provided names or a string-prefix filter.

It contains:

```text
layer 6 protected roles                             19
layer 6 experts 0..255 combined gate/up            256
layer 6 experts 0..255 down                        256
final norm                                           1
rank-sharded vocabulary head                         1
total                                               533
```

Tensor IDs are assigned by ascending exact UTF-8 name within that closed
set. Every rank derives the same IDs, names, roles, layer/expert identities,
logical/global shapes, TP axes, projection IDs, codec IDs, and layout IDs.

For every expert:

- combined gate/up is `CODEC_NVFP4_1D`, logical rank shape
  `[1024,6144]`, projection `4`, and layouts `0x1202/0x1202`;
- down is the exact M2-accepted `CODEC_NVFP4_1D` or
  `CODEC_NVFP4_2D`, logical rank shape `[6144,512]`, projection `3`, and
  layouts `0x1201/0x1201`;
- no gate, up, split-NVFP4, EXL3, extra combined, duplicate, or alternate
  expert record exists.

The 21 protected records have the exact source precision, shape, TP axis,
padding, and collective behavior projected by the operation manifest. Their
projection and NVFP4 layout IDs are zero.

## Strict laboratory JSON

The top-level manifest contains exactly these blocks and fields:

```text
schema
rank
tp_degree
profile
model
tokenizer
source
conversion
review
integrity
tensor_count
rank_payload_bytes
file_codec_metadata_bytes
tensor_contract_sha256
tensors
```

The JSON is the repository's canonical strict encoding: UTF-8, no duplicate
keys, no unknown fields, no insignificant alternate spelling, and byte-equal
re-encoding. Integers are JSON integers within their fixed Rust target
ranges. Digests are exactly 64 lowercase hexadecimal characters. Booleans
are JSON booleans, not `0/1` or strings.

The manifest contains no timestamp, random UUID, host path, credential,
environment dump, or unbounded free-form metadata.

Header model/tokenizer/template/policy/kernel/manifest/payload digests must
equal their typed manifest values. Header rank/TP/count/flags and every
region offset/length must also agree before a tensor record is parsed.

## Tensor record validation

Each tensor record binds at least:

```text
tensor_id
name
role_id
layer_id
expert_id
projection_id
codec_id
logical_dtype
stored_dtype
tp_shard_axis
ndim
manifest_flags
rank_shape
global_shape
padded_shape
quant_group_elements
value_layout_id
scale_layout_id
layout_source_sha256
quant_policy_sha256
primary_bytes
auxiliary_bytes
codec_metadata_bytes
primary_sha256
auxiliary_sha256
codec_metadata_sha256
source_binding
conversion_route
collective_after
```

The validator compares every field with the authenticated descriptor,
decoded codec metadata, source record, compiled subset contract, and
immutable conversion policy. Manifest fields never define a descriptor
length or layout.

For plain protected records, layout and quant-policy digests are zero and
codec metadata is empty. For NVFP4, the metadata decoder supplies both
layout IDs/digests, global-scale semantics, plane bytes, and shape. The
manifest cannot override decoded metadata.

## Rank-invariant semantic catalog v2

M4 uses a separate 192-byte semantic entry and hash domain. It does not reuse
reserved bytes in the accepted 128-byte production v1 catalog.

The first 111 bytes retain the production semantic fields through
`source_axis`. The extension is:

| Offset | Bytes | Field |
|---:|---:|---|
| 111 | 1 | `projection_id` |
| 112 | 2 | `value_layout_id` |
| 114 | 2 | `scale_layout_id` |
| 116 | 32 | layout-source SHA-256 |
| 148 | 32 | quantization-policy SHA-256 |
| 180 | 12 | reserved; zero |

Plain protected records have zero in bytes `111..180`. NVFP4 records use the
closed projection/layout values above and exact accepted digests.

The catalog is:

```text
SHA256(
  "glmaxx.nvfp4-laboratory-tensor-catalog.v1\0" ||
  tensor_count:u32_le ||
  533 entries in tensor-ID order
)
```

All four ranks must derive byte-identical entries. Rank-local global scales,
plane digests, file offsets, source slice offsets, and physical arena offsets
remain required in rank-local records and are intentionally excluded from
the common catalog.

## Exact payload and arena arithmetic

Native file payload planes are:

```text
protected layer-6 payload                 147,487,232
routed expert payload                   1,358,954,496
final norm/head payload                   475,803,648
total file payload                      1,982,245,376
```

Every primary and auxiliary plane is a multiple of 256 bytes, so the file
payload and device weight arena have zero inter-plane alignment slack:

```text
device_weight_arena_bytes = 1,982,245,376
```

Only the 512 NVFP4 tensors have codec metadata:

```text
file codec metadata = 512 * 128 = 65,536
```

The retained load planner independently aligns each nonempty metadata record
to the descriptor's 256-byte device alignment and does not round the final
arena end:

```text
offset(i) = i * 256, i in 0..512
last end  = 511 * 256 + 128
          = 130,944

device_metadata_arena_bytes = 130,944
```

File metadata bytes, device metadata bytes, and weight payload bytes are
three distinct fields. A validator that equates any pair fails.

The load-plan preimage size is also exact:

```text
header                         416
4 rank entries        4 * 248 = 992
tensor entries 4 * 533 * 64 = 136,448
total                        137,856 bytes
```

Every arithmetic expression is checked for overflow and independently
rederived from descriptors. A one-byte-short weight or metadata arena fails
before allocation.

## Laboratory memory budget

M4 uses:

```text
glmaxx.nvfp4-laboratory-budget.v1
```

It is not `ProfileBudgetArtifact.v0` and has no
`conversion_allowed` or serving-capacity field.

The budget binds, per rank:

```text
observed post-context usable HBM
weight arena = 1,982,245,376
metadata arena = 130,944
module and context bytes
eager plus captured graph bytes
maximum M4 prefill workspace
maximum M4 decode workspace
collective bytes
load staging/device verification bytes
exact M3 fixture target KV/indexer bytes
fixture page-table bytes
model/target-program metadata bytes
allocator padding
laboratory failure/cleanup escrow
required total
observed headroom
```

Host reader and pinned-ring bytes are separate bounded host terms and never
hidden in HBM. The budget records all four ranks independently and uses the
minimum measured post-context availability; aggregate free memory cannot
rescue a failing rank.

The artifact is executable only when:

```text
measurement_status              "complete"
laboratory_execution_allowed    true
production_health_allowed       false
serving_allowed                 false
unmeasured_blockers             []
```

Every measured term binds source commit, native library, container,
toolchains, device identity, topology, graph profile, codec capability, and
commands. M4 does not reserve 1M KV and cannot support a serving-capacity
claim.

## Rank-set load plan

M4 reuses the reviewed `RankSetLoadPlan.v1` physical encoding with:

```text
profile byte                    1
verification mode               FULL_SHA256
rank count                      4
tensor count                    533
file payload bytes              1,982,245,376 per rank
device weight arena             1,982,245,376 per rank
device metadata arena           130,944 per rank
```

The profile byte becomes reachable only through a dedicated builder that
accepts:

- four validated laboratory manifests;
- the exact 533-entry physical contract;
- one completed laboratory budget;
- one accepted M2/M3/r3 capability set; and
- four exact device identities.

It cannot accept a production manifest or production budget. The production
builder cannot accept a laboratory manifest or budget.

The plan's common tensor catalog uses the 192-byte laboratory domain above,
not the production v1 catalog. This semantic change requires a plan-domain
amendment:

```text
plan_sha256 = SHA256(
  "glmaxx.rank-set-load-plan.v1.nvfp4-laboratory-v1\0" ||
  existing plan preimage
)
```

Capacity-EXL3 retains its original plan domain. A profile-byte mutation cannot
turn either digest into the other.

## Laboratory type state

M4 has a dedicated state machine:

```text
CREATED
  -> HOST_VALIDATED
  -> DEVICES_VALIDATED
  -> MODULES_READY
  -> MEMORY_PLANNED
  -> WEIGHTS_ADOPTED
  -> GRAPHS_READY
  -> FIXTURE_CACHE_READY
  -> RUNNING
  -> VERIFIED
  -> DESTROYED
```

Any failure transitions through terminal cleanup to `DESTROYED` or terminates
the process if DMA-safe cleanup cannot be proven.

Four-rank adoption yields `LaboratoryWeightHandle`, which is accepted only by
the M4 executor. It has no conversion to the production
`WeightArenaHandle`. `VERIFIED` is an evidence state, not health or service
readiness. The runner always destroys graphs, cache, collectives, streams,
modules, arenas, pinned rings, and rank workers before success returns.

## CPU proof after review

Before a CUDA run, the implementation must:

1. generate the exact 533-entry contract from the operation manifest;
2. prove tensor IDs and 192-byte catalogs are rank-invariant;
3. reject every missing, extra, duplicate, wrong-role, wrong-expert,
   wrong-projection, wrong-codec, wrong-layout, shape, TP-axis, dtype,
   collective, source, and conversion mutation;
4. prove the protected header bit and all five header flags across the
   required codec combinations;
5. reproduce the exact payload, file-metadata, device-arena, and plan-preimage
   arithmetic;
6. reject one-byte-short and overflow cases for every plane and arena;
7. prove production validators reject the laboratory schema and laboratory
   validators reject every production/future schema;
8. prove budget/profile/domain substitution fails before allocation;
9. run the four-rank prepare/adopt/failure matrix with CPU device fakes;
10. prove no state or handle can reach production health, serving, or prefix
    publication;
11. prove all success/failure paths reach exact cleanup; and
12. regenerate deterministic small fixtures without storing model bytes.

CPU acceptance opens only the laboratory manifest/budget/builder
implementation and mock M4 transaction. Real M4 remains behind the prior
SM120, M3, fixture, and authorization gates.

## Exit criteria and nonclaims

This design passes only if adversarial review confirms:

- schema/profile separation is substitution-resistant;
- the exact 533-tensor contract and semantic catalog are complete;
- file and device byte terms cannot be conflated;
- header flags truthfully identify protected/NVFP4 membership;
- laboratory budget and load-plan identities cannot open production;
- type state cannot expose service or health; and
- no implementation, checkpoint, device, quality, capacity, or performance
  evidence is implied.
