# EXL3 mixed-K r3 implementation readiness

Date: 2026-08-04

Status: static implementation map; no r3 acceptance token, source parser,
partition implementation, K4 CPU proof, CUDA result, or checkpoint admission

## Current blocker in code

The existing EXL3 source decoder already computes tile and cyclic-window
addresses from `metadata.bits`, but production admission is K3-only in several
independent places:

- `Exl3Metadata::validate` requires `bits == 3`;
- checkpoint contracts hard-code trellis depth 48 and construct metadata with
  width 3 supplied by the caller;
- no crate parses `tier_bitmap.json` or represents the r3 target/draft plan;
- `WeightPolicy` has one widthless `ExpertCodec::Exl3` and charges every EXL3
  projection as 1,193,060 bytes;
- no engine type represents the canonical K3/K4 per-step partition or retained
  source ordinal;
- the 144-byte CUDA descriptor constructor and source-projection-v1 kernel
  require K3; and
- `NativeCheckpointRankExecutor::execute*` remains deliberately unimplemented.

The current K3 scalar and warp-staged kernels are useful controls. They are not
a functional mixed-K layer.

## Post-token CPU/source implementation cut

Only the exact token
`exl3-mixed-k-source-and-kernel-v1-r3-design-accepted` may open this CPU/source
work. The first implementation should contain no CUDA launch.

### 1. Strict tier source

Add a focused `glm-format` module for the pinned TR3 tier profile. Deserialize
through custom visitors so duplicate keys are rejected before a generic JSON
map can collapse them. The public parser takes authenticated raw bytes plus the
nonzero source-profile and publisher-manifest identities and returns:

```text
Tr3TierSource {
  raw_sha256,
  target_widths[75][256],
  draft_widths[256],
  diagnostic values retained only for source identity,
}
```

The module enforces r3's exact 76 root keys, exact target/draft nested key
sets, finite error arrays, empty NVFP4 arrays, complete ordered tails, 192:64
target membership, and K3-only draft membership. It accepts no generic or
future tier schema.

### 2. Descriptor-derived census

Refactor safetensors EXL3 loading so the validated trellis shape determines
width before `Exl3Metadata` is constructed. For actual gate/up/down shapes,
the third dimension must be exactly 48 or 64 and divide by 16 to exactly K3 or
K4. The caller may request an expected layer/expert/rank/projection, but may
not supply width.

Construct all 233,472 physical observations and join them by
`(layer,expert)`. A plan exists only after gate/up/down on ranks 0..3 agree and
the target `k` or draft K3 rule corroborates the derived value. Preserve each
projection's validated source bytes, tensor identity, metadata digest, and TP
owner for later native publication.

### 3. Canonical tier plan

Add the exact r3 tier-plan encoder as a byte-producing function before its
SHA-256 helper. Tests compare the complete preimage as well as the digest.
Inputs are the three authenticated source identities and the 19,456 physical
width bytes; no rank, filename, allocation address, float spelling, or map
iteration order is admitted.

### 4. Width-aware weight policy

Replace widthless EXL3 accounting. Each EXL3 `ExpertAssignment` must retain
descriptor-derived bits and exact rank-physical bytes. The checked projection
charges, including the 96-byte native record, are:

```text
K3 = 1,179,648 + 13,316 + 96 = 1,193,060 bytes
K4 = 1,572,864 + 13,316 + 96 = 1,586,276 bytes
```

NVFP4 membership remains a different codec. The rank-common policy hash
includes target versus draft role, bits, exact bytes, source identity, tensor
role, and the canonical four-rank ownership rule; it never includes only the
current rank. Each rank-local native manifest separately binds its concrete TP
owner. An average 3.25-bpw charge or one widthless `ExpertCodec::Exl3`
constant is not sufficient.

### 5. Canonical step partition

Add an engine-side pure `MixedKPartition` constructor. Its input router table
is already rank-common and sorted by `(expert_id,token_row,route_slot)`; the
constructor assigns the original zero-based source ordinal, filters into K3
and K4 without renumbering, encodes each exact 16-byte record, emits the full
r3 receipt preimage, and reconstructs the original table as a mandatory
self-check.

The API returns address-free common records only. Owner threads separately
turn records into local pointer arrays and emit local upload/readback receipts.
No device address is stored in or compared through `MixedKPartition`.

### 6. K4 CPU projection

Permit only K3/K4 in `Exl3Metadata`, retain the same 96-byte wire version, and
extend the independent forward-scatter/window proof to K4. Gate, up, and down
CPU controls execute each width and scatter through retained source ordinals.
The unpartitioned scalar oracle and partitioned controls must match bit-for-bit
at the FP16 projection boundary, FP32 SwiGLU/route-weighting boundary, and
final scatter-add boundary.

## Required source changes

The accepted CPU implementation will primarily touch:

- `crates/glm-format/src/exl3.rs`: K3/K4 validation and independent proofs;
- `crates/glm-format/src/safetensors.rs`: descriptor-derived width admission;
- a new strict tier-plan module exported by `glm-format`;
- `crates/glm-format/src/checkpoint.rs`: complete target/draft census and
  metadata construction without caller width;
- `crates/glm-engine/src/weight.rs`: width-aware immutable accounting;
- a new engine mixed-K partition module; and
- CLI proof/fixture plumbing plus `scripts/local-checks.sh`.

Container/native-manifest fields may need additive semantic binding, but the
implementation must first prove whether the existing 96-byte EXL3 metadata and
native tensor semantic records already carry every required bit. It may not
change an on-disk ABI casually or infer width from payload length alone.

## CPU proof matrix

The implementation review candidate must retain:

1. exact strict-parser mutations for BOM, trailing data, duplicates at every
   depth, wrong key sets/spellings, lengths, integer forms, finite numbers,
   tails, NVFP4 membership, and target/draft K rules;
2. byte-exact tier-plan preimage/digest vectors plus one mutation for every
   field and ordering boundary;
3. all 233,472 observations with exact counts 172,800 target K3, 57,600 target
   K4, 3,072 draft K3, and zero draft K4;
4. exact per-rank routed source total 75,293,233,152 bytes and K4 delta
   5,662,310,400 bytes with every checked-overflow boundary;
5. exhaustive K4 forward/inverse tile mapping, cyclic-window boundaries, and
   full gate/up/down representative reconstruction on ranks 0 and 3;
6. partitions for rows 1..8 plus 3,072-row arithmetic across all-K3, all-K4,
   192:64, empty-expert, and maximally skewed distributions;
7. duplicate/missing destination, cross-bin duplicate, bin reorder, source-
   ordinal renumber, wrong bits, K4 draft, stale identity, and count-consistent
   but nonreconstructing mutations; and
8. four-rank consensus with deliberately different local pointer values.

## Native/CUDA work remains a later gate

After the CPU implementation is independently accepted, source projection v2
can preserve the 144-byte descriptor while advancing the ABI identifier and
making width explicit. Compile separate K3/K4 gate, up, and down controls and
dispatch once before the weight loop. K3 regression runs first, then synthetic
K4, real target K3/K4, real draft K3, and only then a mixed target-layer replay.

The grouped K3 gate/up candidate is an optional measured replacement inside
the K3 bin. It cannot stand in for K4 gate/up or either width's down projection.
Until the full functional route passes, no target-layer, checkpoint, MTP,
quality, capacity, concurrency, or performance result is open.
