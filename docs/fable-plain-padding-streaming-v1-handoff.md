# Fable handoff: protected-precision streaming padding v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`a3f44531c7494cd9c0aee8bd58dd7c43bb657fb6`

Required result path:
`fable-plain-padding-streaming-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`plain-padding-streaming-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `crates/glm-format/src/container.rs` | `7ff63e753982716067207ecf6ba071995f00753273957af332cfa4bae42d182a` |
| `crates/glm-format/src/native_reader.rs` | `5f920b8a8b2a49a128b2ab23e6f32bfed4aa0bf9225958a20d016e7fa5a3ea95` |
| `crates/glm-format/src/stream.rs` | `9a7e561eca8f6722f202596e7572e46836fa5ee5f0f2f6c9aaca0bc34f349114` |
| `docs/plain-padding-streaming-proof-v1.md` | `c4e5c4b31c525d5a4b08bdbcc2150169e869baf285883c09f24d4fe8b3a81b3b` |
| `docs/production-punchlist.md` | `db4272bc55a9efa9b3a9daa196e3522c01eba9a7d2d1d9557be5444c21f31324` |
| `docs/results-index.md` | `11eb2b8f29daed28595204f22f6910d5d688deca3ff55971dd473850a2bb1353` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-plain-padding-streaming-v1-handoff.md
```

## Review boundary

This review covers only bounded CPU validation of zero padding for BF16,
FP16, and FP32 tensor planes in the in-memory decoder, streaming converter,
and tentative native reader.

It does not accept complete checkpoint conversion, a device sink, CUDA
loading or execution, model quality, capacity, or performance.

## Required adversarial questions

1. Does an absolute chunk byte offset map to the same row-major
   N-dimensional element coordinates as whole-plane validation for ndim 1–4?
2. Are BF16, FP16, and FP32 element alignment, short chunks, plane-end bounds,
   and arithmetic overflow fail-closed?
3. Does padding on any logical axis require every byte of that stored element
   to be zero?
4. Can a chunk boundary reset coordinates, split an element, skip a padded
   element, or accept a nonzero suffix?
5. Does in-memory validation still validate complete geometry before calling
   the chunk helper?
6. Have the native reader and streaming writer already validated the same
   descriptor geometry before chunk validation?
7. Does `StreamingRankWriter` use only its fixed 8 MiB read buffer instead of
   allocating the complete tensor plane?
8. Does converter validation occur before descriptor publication and repeat
   for completed descriptors on resume?
9. Does the native reader validate before forwarding each chunk to its
   tentative sink?
10. Are the split-chunk, misalignment, and nonzero-padding regressions
    distinguishing?
11. Is 475,791,360 bytes the exact BF16 byte count for a rank-local
    `[38720,6144]` vocabulary matrix, and is it presented only as the removed
    worst-case allocation if physical padding is present?
12. Are the test counts, host exclusions, and absence of checkpoint/device/
    model/quality/capacity/performance claims accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- coordinate and dtype-element arithmetic are correct;
- every padding and chunk-boundary case fails closed;
- in-memory, native-reader, and converter semantics remain identical;
- streaming conversion is tensor-size independent in scratch;
- publication, resume, and tentative-sink ordering are safe;
- the regressions are distinguishing; and
- proof arithmetic, results, and exclusions are accurate.

Only if all seven answers are unqualified `YES`, end with the requested
token. Withhold it for a conditional pass, stale input, coordinate reset,
misaligned acceptance, padding hole, unchecked overflow, geometry bypass,
whole-plane allocation, descriptor publication before validation, resume
bypass, sink publication, nondistinguishing test, false byte arithmetic,
false test count, or overstated checkpoint/device/model/quality/capacity/
performance claim.

The token accepts only bounded CPU padding validation. It does not open cn4,
authorize CUDA work, or accept production serving.
