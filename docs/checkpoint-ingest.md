# Checkpoint ingest contract

Date: 2026-07-29

Status: strict structural reader and EXL3 projection importer implemented;
bounded-memory native rank conversion is the next checkpoint gate.

## Scope

`glm-format::safetensors` reads a single safetensors file or the standard
sharded Hugging Face index directly from Rust. It does not import PyTorch,
Python, a generic tensor framework, or an alternate model runtime.

The reader:

- bounds the JSON header and index before allocation;
- rejects duplicate keys, unknown tensor fields, unsafe shard paths,
  unsupported dtypes, arithmetic overflow, holes, overlaps, trailing data,
  and index/shard inventory disagreement;
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

The cheap inventory digest is explicitly named `structure_sha256`: it is the
single-file header digest or sharded-index digest and is not a content hash.
Conversion provenance must record complete-file or per-tensor hashes.

## Commands

Inventory a checkpoint without materializing tensor data:

```text
cargo run -p glm-cli --release -- \
  safetensors-inventory /external/model/model.safetensors.index.json
```

Prove one actual EXL3 projection directly from a single file or sharded
index:

```text
cargo run -p glm-cli --release -- \
  exl3-safetensors-proof \
  /external/model/model.safetensors.index.json 3 0 0 gate
```

The proof uses the canonical source stem, hashes every source component,
reconstructs the native FP16 matrix with the pinned CPU oracle, and reports
the source, metadata, native-plane, and reconstruction hashes as JSON.

## Fail-closed boundary

This code proves source discovery and byte-exact EXL3 import. It does not
claim that a complete model has been loaded, that a rank container can yet be
produced with bounded memory, or that codec `0x0200` is GPU-loadable.
The current in-memory `RankFileBuilder` remains suitable only for fixtures.
A production converter must stream tensor planes and hashes into temporary
rank files, atomically finalize all four headers after deriving the shared
conversion UUID, and resume only from verified tensor boundaries.

Raw checkpoints, conversion scratch, and proof output remain external to
Git.
