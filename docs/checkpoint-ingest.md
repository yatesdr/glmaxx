# Checkpoint ingest contract

Date: 2026-07-29

Status: strict structural reader, exact pinned checkpoint manifest,
role-specific TP slice readers, protected-tensor and EXL3 payload definitions,
and resumable bounded-memory native rank writer implemented.

## Scope

`glm-format::safetensors` reads a single safetensors file, a standard sharded
Hugging Face index, or the pinned EXL3 checkpoint's flat directory of
per-layer safetensors files directly from Rust. It does not import PyTorch,
Python, a generic tensor framework, or an alternate model runtime.

The reader:

- bounds the JSON header and index before allocation;
- rejects duplicate keys, unknown tensor fields, unsafe shard paths,
  unsupported dtypes, arithmetic overflow, holes, overlaps, trailing data,
  index/shard inventory disagreement, duplicate tensors across directory
  shards, and symlinked directory entries;
- supports the complete current safetensors dtype vocabulary, including
  sub-byte F4/F6 byte accounting;
- exposes positional bounded reads, streaming copies, per-tensor SHA-256,
  and complete-file SHA-256;
- reopens and revalidates a shard header and tensor descriptor before every
  sharded tensor read;
- imports the pinned EXL3 `mcg`, `suh`, `svh`, and `trellis` tensors without
  transposition or an intermediate packed representation; and
- validates the exact GLM-5.2 layer, expert, rank, projection, shape, dtype,
  marker, rotation, and trellis contract before constructing an
  `Exl3Trellis`.

`glm-format::checkpoint` freezes the pinned revision and its 81-shard
inventory:

| Term | Exact value |
|---|---:|
| index SHA-256 | `346227a4ea44b6063017739ee38a830319dc10305ccf714734095e27b28064c2` |
| tensors | 935,105 |
| EXL3 components | 933,888 |
| protected tensors | 1,217 |
| tensor payload bytes | 316,304,795,648 |

Validation requires every canonical layer/expert/projection/rank/component,
every protected role, exact dtype and source shape, all 81 canonical shard
names, the complete dtype-byte inventory, and the exact index identity.
Unknown, duplicate, omitted, renamed, or shape-compatible substitute tensors
fail the proof.

`glm-format::stream` writes the native rank format without retaining rank
payloads in memory. It:

- precomputes canonical offsets from a deterministic tensor plan;
- writes through an 8 MiB bounded buffer;
- rejects short and overlong plane readers;
- syncs payload bytes before publishing a tensor descriptor as its durable
  completion marker;
- resumes only descriptors whose geometry and payload/aux hashes revalidate;
- rejects changed completed payloads, nonzero padding, incompatible plans,
  symlinks, hard links, and finalized files;
- audits every payload plane and all canonical padding while computing the
  rank payload digest;
- derives the same deterministic conversion and file UUIDs as the in-memory
  reference builder; and
- produces byte-for-byte identical files to the reference builder in tests.

The rank format carries protected BF16, FP16, and FP32 tensors directly with
their exact logical rank and shape. Plain payloads have no auxiliary or
metadata plane, require zero canonical padding when padded, and preserve the
source element bytes without numeric conversion.

The safetensors readers expose bounded `Read` implementations for both
single-file tensors and verified sharded tensors. Axis-0 TP slices stream as
one contiguous range; axis-1 matrix slices stream only the rank-local column
span of each row. A converter can therefore chain EXL3 `mcg+suh+svh`, stream
the trellis directly, slice protected tensors, and avoid loading a checkpoint
shard, source matrix, or rank image into RAM.

The cheap inventory digest is explicitly named `structure_sha256`: it is the
single-file header digest, sharded-index digest, or a domain-separated digest
of sorted directory shard names, header hashes, and file lengths. It is not a
payload content hash. Conversion provenance must record complete-file or
per-tensor hashes.

## Commands

Inventory a checkpoint without materializing tensor data:

```text
cargo run -p glm-cli --release -- \
  safetensors-inventory /external/model
```

Prove one actual EXL3 projection directly from a single file, sharded index,
or flat shard directory:

```text
cargo run -p glm-cli --release -- \
  exl3-safetensors-proof \
  /external/model 3 0 0 gate
```

The proof uses the canonical source stem, hashes every source component,
reconstructs the native FP16 matrix with the pinned CPU oracle, and reports
the source, metadata, native-plane, and reconstruction hashes as JSON.

Prove the complete pinned source structure:

```text
cargo run -p glm-cli --release -- \
  checkpoint-proof /external/model/model.safetensors.index.json
```

The repository includes `scripts/cn4-download-pinned-exl3.sh` for an external,
resumable, per-file SHA-256-verified download. It never places weights in Git.

## Fail-closed boundary

This code proves source discovery, the complete pinned structural inventory,
byte-exact EXL3 and protected-tensor payload definitions, role-specific
bounded TP slicing, and a bounded-memory rank-file write primitive. It does
not claim that a complete model has been loaded or that codec `0x0200` is
GPU-loadable. The next conversion gate must add four-rank staging-directory
publication and an external real-checkpoint smoke. The in-memory
`RankFileBuilder` remains a fixture oracle rather than a production
conversion path.

Raw checkpoints, conversion scratch, and proof output remain external to
Git.
