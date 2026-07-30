# NVFP4 streaming canonicality proof v2

Date: 2026-07-29

Implementation commit:
`3049a5a325dc7d08202a0aa9f15343aa82935e79`

Status: CPU format and loader correction passed; independent review pending

GPU claim: none

This proof extends, rather than invalidates, the metadata checks recorded in
`docs/nvfp4-metadata-canonicality-proof-v1.md`. It closes the production
file-backed paths that v1 did not cover.

## Corrected acceptance holes

The in-memory `RankFile` decoder enforced joint value/scale semantics, but
the file-backed paths did not:

- `NativeRankReader::verify_and_stream`, the intended tentative direct-upload
  path, checked only that scale codes were finite and nonnegative;
- `StreamingRankWriter`, the bounded checkpoint-conversion path, performed
  the same scale-only check; and
- the shared in-memory validator did not enforce codec `0x0101`'s requirement
  that one computed 2D scale be repeated across all 16 N rows.

A producer could therefore recompute every descriptor/header hash around
nonzero padded nibbles, nonzero values behind a zero scale, or unequal 2D
scale replicas. The in-memory inspection path rejected the first two, but a
file-backed tentative upload or streaming conversion could accept them. The
third case was accepted everywhere despite contradicting format-v0 section
12.2.

## Bounded streaming validator

`Nvfp4PlaneValidator` is now the single semantic implementation used by:

- `PackedNvfp4::validate`;
- in-memory `RankFile::read`;
- file-backed `NativeRankReader::verify_and_stream`; and
- `StreamingRankWriter` for new writes and completed-tensor resume.

Values remain row-major and arrive before the swizzled scale plane. While
streaming values, the validator:

1. consumes exact sequential 8-byte/16-value blocks;
2. rejects every nonzero nibble outside logical N or K; and
3. records one bit per swizzled scale offset when a block contains any
   nonzero code.

While streaming scales, it:

1. consumes the exact declared scale byte count in sequence;
2. rejects negative or nonfinite E4M3 encodings; and
3. retains the direct hardware scale plane.

Finalization then rejects:

- a zero scale paired with any nonzero value code;
- a nonzero scale outside the codec's logical domain; and
- unequal scale replicas anywhere in a codec-`0x0101` 16-row tile, including
  the final partial logical tile.

Padded rows in the final partial 2D tile retain the same shared scale as the
logical rows, but their E2M1 values remain positive zero. A fully padded tile
has zero values and zero scale replicas.

Allocation uses `try_reserve_exact` and fails closed. Scratch is:

```text
scale_plane_bytes + ceil(scale_plane_bytes / 8)
```

For the actual TP4 FC1 slab `[1024,6144]`, this is:

```text
393,216 + 49,152 = 442,368 bytes
```

The direct reader includes those bytes in its reported maximum scratch
accounting. The normal 8 MiB I/O buffer remains separately charged. No value
plane or dequantized weight matrix is retained.

## Distinguishing regressions

The proof covers:

- a valid 2D tensor split at separate value and scale chunk boundaries;
- out-of-order value chunks rejected before state mutation;
- exact 442,368-byte actual-FC1 scratch arithmetic;
- one unequal but individually finite 2D scale replica;
- a streaming converter input with a zero scale over nonzero values;
- a file-backed rank image with a nonzero padded value nibble; and
- a file-backed rank image with a zero scale over nonzero values.

Both file-backed mutations recompute the affected tensor-plane SHA-256,
descriptor-region SHA-256, complete payload SHA-256, file UUID, and header
CRC32C. `NativeRankReader::open` accepts their integrity envelope, while
`verify_and_stream` rejects their NVFP4 semantics. This distinguishes the
semantic gate from checksum rejection.

The direct-upload sink contract remains fail-closed: received chunks are
tentative and unreachable by execution until `verify_and_stream` returns
success. Semantic finalization occurs before `finish_tensor`.

## Verification

```text
cargo test -p glm-format --offline
cargo clippy -p glm-format --all-targets --offline -- -D warnings
./scripts/local-checks.sh
```

Results:

- 65 `glm-format` unit tests passed;
- 3 external NVFP4 proof tests passed;
- format doc tests passed;
- targeted Clippy passed with warnings denied;
- the complete workspace gate passed 291 Rust tests;
- workspace Clippy and CUDA-FFI host checks passed with warnings denied;
- CPU, matrix, manifest, pack/inspect, budget, ABI, engine, serving, and cache
  proof commands passed;
- every generated checked-in fixture comparison passed; and
- 73 review handoffs were provenance-verified with 0 of 54 configured review
  results present.

The local host did not have `GLMAXX_TOKENIZER_DIR` set, so the pinned tokenizer
bundle proof was skipped. It also had no `nvcc`, so this run did not compile or
launch CUDA.

Implementation hashes:

```text
spec/format-v0.md
619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a

crates/glm-format/src/nvfp4.rs
af9211c7df2c74b446d234ed215580614ce58c415963a1038eb86df48ad8b11a

crates/glm-format/src/container.rs
802cd4eee7090ebcad9cce11127bc09271038614466198a84e5045271bdeeb25

crates/glm-format/src/native_reader.rs
937ad3883af69d956213492afdf8fa21db304809c3c3fb1c1ebff7518a18c965

crates/glm-format/src/stream.rs
363969e454fb7e851d4b73a355bbc4ebc33c79b710326aa2c7ddf1d17e9aff94

crates/glm-format/tests/nvfp4_proof.rs
74b312d65566db5414dd012c2d9b5222aa39808dfb5e979b11c6dadb7c45c734
```

## Exclusions

This is a CPU codec, converter, and tentative-loader semantic result. It does
not claim an adopted device allocation, CUDA loading, SM120 execution,
block-scaled MMA, complete checkpoint conversion, profile fit, model quality,
capacity, or performance.

The existing v1 handoff remains a valid review of its narrower in-memory
metadata/container boundary. Production loader acceptance requires the v2
review that pins this proof and implementation.
