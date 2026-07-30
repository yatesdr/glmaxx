# Streaming tensor write single-pass proof v1

Date: 2026-07-30

Implementation commit:
`cbe3e9f0881ebc3d8f8c0b3bc1abc571c29a4888`

Status: CPU format/converter correction passed; independent review pending

GPU claim: none

## Corrected conversion I/O

`StreamingRankWriter::write_tensor_deferred` previously copied and hashed
each source plane into the staging rank file, then called
`validate_planes`. That second call reread the complete just-written primary
and auxiliary planes from the file before the descriptor could be published.
Across a checkpoint conversion, valid bytes therefore incurred one source
read, one file write, and one immediate file reread.

The writer now constructs one codec-specific `TensorWriteValidator` and
feeds every source chunk to it before writing that chunk. Hashing and exact
source-length checks remain in the same bounded copy loop. After both planes
finish, the validator's cross-plane check must pass before the descriptor is
added to the pending publication set.

New tensor writes no longer call `validate_planes`. That file-backed path
remains mandatory when reopening a staging file: every nonzero completed
descriptor is hash-checked and semantically revalidated from the durable
file bytes before it is adopted.

## Validator state

- BF16, FP16, and FP32 validate padded coordinates directly from each
  offset-aware chunk and retain no tensor-sized semantic scratch.
- NVFP4 retains the existing scale plane plus one bit per scale. It checks
  value padding, finite canonical scales, zero-scale/nonzero-code relations,
  scale padding, and exact 2D scale replication.
- EXL3 retains one actual source projection's primary and auxiliary planes
  and then uses the canonical `Exl3Trellis::from_container_planes` decoder.
  Allocation is fallible and fails closed. At either routed-expert GLM-5.2
  rank-slab shape, the primary plane is 1,179,648 bytes and the rotation
  plane is 14,340 bytes; this is projection-bounded, not rank-file-sized.

The common copy buffer remains capped at 8 MiB. The EXL3 path is not yet a
fully incremental semantic decoder; this correction removes the immediate
staging-file reread without claiming zero-copy EXL3 validation.

## Publication and retry semantics

Validation runs before each chunk write and the final cross-plane validation
runs before pending-descriptor insertion. A late semantic failure may leave
unadvertised bytes in their predetermined staging offsets, but the
descriptor stays all zero and retry overwrites those bytes. Durable
publication ordering is unchanged:

1. sync all pending payload data;
2. write the completed descriptors;
3. sync the descriptor publication; and
4. mark the corresponding in-memory tensor slots complete.

Too-short sources, trailing source bytes, allocation failures, invalid
offset order, and every codec semantic failure remain errors.

## Regressions

Dedicated tests mutate:

- an NVFP4 scale to zero over a nonzero value block;
- one BF16 padded byte; and
- the EXL3 codebook marker.

All three writes fail with their exact semantic error, leave the pending set
empty, keep the completed count at zero, and prove the corresponding
on-disk descriptor remains 256 zero bytes. Existing tests retain
byte-for-byte equality with the in-memory builder, deferred batch
invisibility, durable resume, short/trailing-source rejection, payload
corruption rejection, and four-rank atomic publication.

## Verification

```text
cargo test -p glm-format --offline
cargo clippy -p glm-format --all-targets --offline -- -D warnings
./scripts/local-checks.sh
```

Results:

- 67 `glm-format` unit tests passed;
- 3 external NVFP4 proof tests passed;
- format doc tests passed;
- targeted and workspace Clippy passed with warnings denied;
- the complete workspace gate passed 293 Rust tests;
- CUDA-FFI host checks and all deterministic fixture comparisons passed; and
- 75 review handoffs were provenance-verified with 0 of 56 configured review
  results present.

The local host did not have `GLMAXX_TOKENIZER_DIR` set, so the pinned
tokenizer bundle proof was skipped. It also had no `nvcc`, so this run did
not compile or launch CUDA.

Implementation hashes:

```text
spec/format-v0.md
619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a

crates/glm-format/src/stream.rs
a93e564bb29e823dbd9ddd922873b87080359f6004d872661ca8d800c1ec632d
```

## Exclusions

This proves one-pass source consumption and one-pass staging-file writes for
new valid tensors while preserving semantic and crash-publication checks. It
does not measure conversion throughput, qualify a complete checkpoint,
upload weights, establish device residency, or make a CUDA, model-quality,
capacity, or serving-performance claim.
