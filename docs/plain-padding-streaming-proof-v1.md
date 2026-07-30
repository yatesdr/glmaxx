# Protected-precision streaming padding proof v1

Date: 2026-07-29

Implementation commit:
`756e39a4629c1b27b8e11dee9228beffa48a2e79`

Status: CPU format/converter correction passed; independent review pending

GPU claim: none

## Corrected resource hole

The in-memory rank decoder and file-backed native reader already rejected
nonzero padding in BF16, FP16, and FP32 tensors. The streaming checkpoint
writer enforced the same semantics by reading the complete padded tensor into
a new `Vec<u8>` after it had already copied the tensor through an 8 MiB
buffer.

That preserved correctness but violated the bounded-conversion posture. A
single rank-local BF16 vocabulary matrix at `[38720,6144]` is 475,791,360
bytes. If a protected tensor at that geometry required physical padding, the
old validation branch allocated the complete plane merely to inspect padded
elements.

## Shared bounded validator

`validate_plain_padding_chunk` is now the single element-coordinate check
used by:

- the in-memory `validate_plain_padding` path;
- `NativeRankReader::verify_and_stream`; and
- `StreamingRankWriter` for new writes and completed-tensor resume.

Each chunk carries its exact byte offset within the plane. The validator:

1. verifies that the offset and chunk length are whole dtype elements;
2. rejects an offset/length range beyond the padded tensor byte count;
3. derives each element's N-dimensional coordinates from its absolute linear
   index; and
4. requires every element outside any logical extent to be all-zero bytes.

BF16, FP16, and FP32 use 2-, 2-, and 4-byte element boundaries,
respectively. Chunk boundaries therefore cannot reset coordinate calculation
or split a stored element.

The streaming writer now scans the file through its existing fixed 8 MiB
buffer and allocates no tensor-sized padding scratch. Descriptor publication
still follows successful validation, and resume revalidates every completed
descriptor. The native reader uses the same helper before forwarding each
chunk to its tentative sink.

## Regressions

The existing 2×3-logical, 2×4-padded BF16 fixture is now validated in three
separate chunks at offsets 0, 4, and 12. A deliberately misaligned byte
offset is rejected. The existing nonzero-padding mutation remains rejected
by both in-memory construction and the file-backed native-reader test.

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
- targeted and workspace Clippy passed with warnings denied;
- the complete workspace gate passed 291 Rust tests;
- CUDA-FFI host checks and all deterministic fixture comparisons passed; and
- 74 review handoffs were provenance-verified with 0 of 55 configured review
  results present.

The local host did not have `GLMAXX_TOKENIZER_DIR` set, so the pinned tokenizer
bundle proof was skipped. It also had no `nvcc`, so this run did not compile or
launch CUDA.

Implementation hashes:

```text
spec/format-v0.md
619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a

crates/glm-format/src/container.rs
7ff63e753982716067207ecf6ba071995f00753273957af332cfa4bae42d182a

crates/glm-format/src/native_reader.rs
5f920b8a8b2a49a128b2ab23e6f32bfed4aa0bf9225958a20d016e7fa5a3ea95

crates/glm-format/src/stream.rs
9a7e561eca8f6722f202596e7572e46836fa5ee5f0f2f6c9aaca0bc34f349114
```

## Exclusions

This proves bounded CPU validation of protected-tensor padding. It does not
claim that the current checkpoint requires padded protected tensors, that a
full checkpoint has been converted, or that any tensor reached a device. It
does not establish CUDA execution, model quality, capacity, or performance.
