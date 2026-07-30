# Fable review: protected-precision streaming padding v1

Date: 2026-07-30

Reviewer: Fable (adversarial CPU implementation review)

Handoff: `docs/fable-plain-padding-streaming-v1-handoff.md` (queue row 44)

Reviewed candidate commit:
`a3f44531c7494cd9c0aee8bd58dd7c43bb657fb6`

Location note: the handoff declares the result path at the repository root;
the operator directed all review artifacts into `docs/reviews/`, so this
file may need moving to the declared path on acceptance.

## Provenance

All 8 input hashes were verified with `git show <commit>:<path> |
shasum -a 256` at review start and re-verified at review finish; both sets
matched the handoff table exactly at the pinned candidate. `main` has
drifted on `stream.rs`, `production-punchlist.md`, and `results-index.md`,
so the review ran in a detached worktree at the pinned commit. The handoff
file itself postdates the candidate (only the proof doc is in-tree at the
pin), so `review-proof` was replaced by direct hash verification of every
table row, which is what it proves.

| Input | SHA-256 (start = finish) |
|---|---|
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `crates/glm-format/src/container.rs` | `7ff63e753982716067207ecf6ba071995f00753273957af332cfa4bae42d182a` |
| `crates/glm-format/src/native_reader.rs` | `5f920b8a8b2a49a128b2ab23e6f32bfed4aa0bf9225958a20d016e7fa5a3ea95` |
| `crates/glm-format/src/stream.rs` | `9a7e561eca8f6722f202596e7572e46836fa5ee5f0f2f6c9aaca0bc34f349114` |
| `docs/plain-padding-streaming-proof-v1.md` | `c4e5c4b31c525d5a4b08bdbcc2150169e869baf285883c09f24d4fe8b3a81b3b` |
| `docs/production-punchlist.md` | `db4272bc55a9efa9b3a9daa196e3522c01eba9a7d2d1d9557be5444c21f31324` |
| `docs/results-index.md` | `11eb2b8f29daed28595204f22f6910d5d688deca3ff55971dd473850a2bb1353` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Commands run in the worktree:

- `cargo test --offline -p glm-format` — 65 unit tests passed, 0 failed;
  3 external NVFP4 proof tests passed; doc-tests passed. Matches the proof's
  claimed counts exactly.
- `cargo clippy --offline -p glm-format --all-targets -- -D warnings` —
  clean.

Independent computational checks (throwaway scripts, not in-tree):

- brute-force cross-check of the row-major coordinate decomposition in
  `validate_plain_padding_chunk` against an independent implementation over
  2,000 random shapes, ndim 1–4, every linear index — identical
  classification everywhere;
- randomized element-aligned chunk splits over 500 random padded tensors —
  chunked acceptance identical to whole-plane acceptance in every case;
- `38,720 * 6,144 * 2 = 475,791,360` — the proof's BF16 byte figure is
  exact.

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

1. **Element-wise div/mod cost on the hot path.**
   `validate_plain_padding_chunk` (`crates/glm-format/src/container.rs:898`)
   performs `ndim` divisions per element for every element of a padded
   plane. For the worst-case `[38720, 6144]` BF16 plane that is ~238M
   elements × 2 divisions. It is linear (not quadratic) and only runs when
   `logical != padded`, and conversion is explicitly not a performance
   claim, so this is not blocking — but an incremental odometer (carry-based
   coordinate advance, division only at chunk entry) would remove ~475M
   divisions per padded worst-case tensor if conversion throughput is ever
   gated.
2. **EXL3 whole-plane allocation remains in `validate_planes`**
   (`crates/glm-format/src/stream.rs:973-985` reads full primary and aux
   into `Vec`s with non-fallible `read_range_vec`). This is outside this
   handoff's boundary (plain padding only) and is addressed by the later
   single-pass candidate (`5ff1f854`), but note the bounded-scratch claim in
   this proof is scoped to plain padding, and correctly so — the proof does
   not overclaim.
3. **Dtype dispatch duplication.** The codec→`PlainDtype` match appears in
   `stream.rs::validate_planes`, `native_reader.rs::
   validate_plain_padding_stream_chunk`, and the in-memory path. A single
   `PlainDtype::from_codec(u16) -> Option<PlainDtype>` would remove three
   hand-kept copies of the same table.

### QUESTION

1. `validate_plain_padding_chunk` rejects a chunk whose length is not a
   whole element multiple. All current callers produce element-multiple
   chunks (8 MiB buffer, element sizes 2/4, plane bytes element-aligned),
   so this can never fire today — intentional defense in depth, or should a
   carrying partial-element path ever be needed for a future unaligned
   transport? (Current behavior is the safe choice; no change requested.)

## Answers to the handoff's 12 questions

1. **Yes.** The chunk validator derives each element's coordinates from
   `plane_offset / element_bytes + local_index` — the absolute linear index
   — and decomposes row-major from the last axis. Verified equivalent to
   whole-plane validation by exhaustive brute force over random shapes for
   ndim 1–4 (see computational checks above). Whole-plane validation is
   literally the chunk helper called at offset 0, so divergence is
   impossible by construction.
2. **Yes, fail-closed.** Misaligned `plane_offset`, non-element-multiple
   chunk length, and `end > total_bytes` all return `Descriptor`; all
   products/sums use `checked_mul`/`checked_add` returning `Overflow`;
   element sizes are 2 (BF16/FP16) and 4 (FP32) from `PlainDtype::
   element_bytes`.
3. **Yes.** An element is padding if any axis coordinate reaches or exceeds
   its logical extent, and a padding element is rejected unless every one of
   its stored bytes is zero (`element.iter().any(|&b| b != 0)`).
4. **No to all four.** Coordinates derive from the absolute offset, so a
   boundary cannot reset them; a chunk that would split an element is
   rejected by the element-multiple check; skipping a padded element is
   impossible for the in-tree callers because each caller advances
   `plane_offset` by exactly the chunk length over the exact plane byte
   count (`read_chunks` in `stream.rs`, `stream_plane` in
   `native_reader.rs`, offset 0..len in-memory); a nonzero suffix inside
   any padded element is caught byte-exactly. Note the helper itself does
   not enforce cross-call contiguity — that obligation sits with the three
   callers, and all three provably iterate gapless sequential chunks.
5. **Yes.** `validate_plain_padding` calls `validate_plain_geometry` (ndim
   bounds, logical ≤ padded, trailing extents = 1, exact payload byte
   count) before any padding scan, and returns early when
   `logical == padded`.
6. **Yes.** The native reader validates descriptor geometry at open (before
   `verify_and_stream` can run), and the streaming writer validates every
   spec in `StreamLayout::new` before any descriptor exists; both therefore
   reach chunk validation only with validated geometry.
7. **Yes.** The plain path in `validate_planes` scans through `read_chunks`
   with the fixed 8 MiB `STREAM_BUFFER_BYTES` buffer; no plane-sized
   allocation exists on the plain path (the remaining EXL3 whole-plane read
   is out of this boundary — MINOR 2).
8. **Yes.** `write_tensor_deferred` runs `validate_planes` before pushing
   into `pending`; descriptors are written and synced only in
   `commit_pending`, so validation strictly precedes descriptor
   publication. On resume, `verify_body` hash-verifies each nonzero
   descriptor and then calls `validate_planes(index)` for it — completed
   descriptors are revalidated.
9. **Yes.** `verify_and_stream` validates each plain chunk
   (`validate_plain_padding_stream_chunk`) before `sink.primary_chunk`;
   the sink contract explicitly treats all delivered bytes as tentative
   until the final proof, which covers the plane-hash check that
   necessarily completes only after the last chunk.
10. **Yes, distinguishing.** The split-chunk positive (chunks at offsets
    0/4/12 of the 2×3-in-2×4 BF16 fixture with nonzero logical data) fails
    against any coordinate-resetting implementation, which would demand
    zeros at wrong positions and reject the valid fixture; the offset-1
    misalignment negative fails against an implementation without the
    element-alignment check; the `bytes[6] = 1` padding mutation (linear
    element 3 = coordinate (0,3), a padding element) fails against any
    padding hole, and the file-backed native-reader mutation test covers
    the streamed path.
11. **Yes.** `38,720 × 6,144 × 2 = 475,791,360` exactly, and the proof
    presents it only as the removed worst-case allocation conditional on
    physical padding being present ("If a protected tensor at that geometry
    required physical padding"), not as a claim about the current
    checkpoint.
12. **Yes.** 65 unit + 3 external NVFP4 + doc-tests reproduced in the
    worktree; `cargo test --offline --workspace` in the detached worktree
    passed exactly 291 tests with zero failures, matching the proof's 291;
    74 provenance-verified handoffs matches the 76 tracked handoff files at
    the candidate minus the 2 skipped historical umbrella handoffs; the
    proof claims no checkpoint/device/model/quality/capacity/performance
    result and its exclusions section matches the code's actual scope.

## Separate statements required by the handoff

- Coordinate and dtype-element arithmetic are correct: **YES** (verified by
  independent brute force).
- Every padding and chunk-boundary case fails closed: **YES**.
- In-memory, native-reader, and converter semantics remain identical:
  **YES** (single shared helper; in-memory path is the helper at offset 0).
- Streaming conversion is tensor-size independent in scratch: **YES** for
  the plain-padding scope of this handoff (fixed 8 MiB buffer; EXL3
  whole-plane retention is outside this boundary and separately addressed).
- Publication, resume, and tentative-sink ordering are safe: **YES**.
- The regressions are distinguishing: **YES**.
- Proof arithmetic, results, and exclusions are accurate: **YES**.

## Architecture & maintainability

- Consolidating three separate padding implementations into one
  `validate_plain_padding_chunk` is exactly the right move; the in-memory
  path being the chunk helper at offset 0 makes divergence structurally
  impossible rather than test-enforced.
- The remaining duplication worth removing is the codec→dtype table
  (MINOR 3) and the near-identical `read_chunks`/`stream_plane` chunk
  drivers in `stream.rs` and `native_reader.rs` — same loop, different
  hasher plumbing; one shared driver with an optional hasher would cut the
  fourth copy of this loop before more codecs arrive.
- The helper's caller-must-be-contiguous contract (Q4) is currently
  documented only by usage. A one-line doc comment on
  `validate_plain_padding_chunk` stating that completeness of coverage is
  the caller's obligation would protect the next caller.
- API surface is appropriately `pub(crate)`; nothing here leaks outside
  `glm-format`.

## Token decision

All seven required answers are unqualified YES; zero blockers, zero majors;
tests and clippy reproduced green in the detached worktree; input hashes
identical at start and finish. The token accepts only bounded CPU padding
validation and does not open cn4, authorize CUDA work, or accept production
serving.

plain-padding-streaming-v1-accepted
