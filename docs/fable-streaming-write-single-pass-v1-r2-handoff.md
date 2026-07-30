# Fable handoff: streaming tensor write single-pass v1 r2

Date: 2026-07-30

Status: corrective adversarial CPU implementation re-review requested

Review candidate commit:
`f39f23495b80dd7527c379788d39f58987ed2b52`

Required result path:
`docs/reviews/fable-streaming-write-single-pass-v1-r2.md`

Requested acceptance token, only for an unqualified implementation pass:
`streaming-write-single-pass-v1-accepted`

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch GPU, container, storage-device,
or network work for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if needed. Hash every input at review start and finish. Any mismatch
must withhold the token as a stale candidate.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `crates/glm-format/src/container.rs` | `2fbe22c55d481e40699a02be9929f9a964f50bc08199853772dd314c85ade47f` |
| `crates/glm-format/src/nvfp4.rs` | `af9211c7df2c74b446d234ed215580614ce58c415963a1038eb86df48ad8b11a` |
| `crates/glm-format/src/exl3.rs` | `f6fa1b25311d78e13e22a0c7c908da7abca636948218fef1987c89850e974edb` |
| `crates/glm-format/src/stream.rs` | `b6d7dae8adf6fbb7ebd0f08c79c3d7f9dbba6269408f6b760fa43b18028a22fb` |
| `crates/glm-format/src/native_reader.rs` | `953a56702ba1ee000f508fe24cbbae7c6137d6496d104720f658fede5572699c` |
| `docs/streaming-write-single-pass-proof-v1.md` | `41ed933bff68fcf684c23c508c67ba63b3cda14318f7181fecbee8083b321495` |
| `docs/fable-streaming-write-single-pass-v1-handoff.md` | `80ee1bd992b53fa5ce015f947ef83f1da26f86484426165a8067e658d0fbdb58` |
| `docs/production-punchlist.md` | `fef4afbc215ca100912dc71f38f0ea093d3eb5e6d64cddf872d75b69ffffbe31` |
| `docs/results-index.md` | `99db050468fc87b12f66e51c7971cac1f6d8d6b7a340c61f8dacf096822445a8` |
| `scripts/local-checks.sh` | `56f728cdf3f047f9633509a57341d25a977efa802f0d5b371c9716830517db59` |
| `Cargo.toml` | `863c28560b339f1fd7fb6b80c1b812e9fa7bc3f8f8c782126d2a29ceeffc06ea` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-streaming-write-single-pass-v1-r2-handoff.md
git diff --check f39f23495b80dd7527c379788d39f58987ed2b52^ \
  f39f23495b80dd7527c379788d39f58987ed2b52
cargo test --offline -p glm-format
cargo clippy --offline -p glm-format --all-targets -- -D warnings
```

The handoff itself is coordination metadata added after the candidate and is
not a candidate input. The prior operator-owned review is
`docs/reviews/fable-streaming-write-single-pass-v1.md`; do not modify it.

## Review purpose

The first review found no implementation defect. It withheld the token only
because the proof claimed a 14,340-byte EXL3 auxiliary plane for the routed
expert rank slab, while code and tests correctly use 13,316 bytes.

The candidate corrects the formula and makes the unpadded-plain validator
case explicit. Current format sources are pinned because later unrelated
format/load-plan work changed their hashes. Confirm that this drift does not
invalidate the first review's source-order, validation, publication, retry,
or scratch-bound conclusions.

## Review boundary

Acceptance covers only:

- sequential source consumption and pre-write semantic validation;
- exact short/trailing source rejection;
- plain, NVFP4, and EXL3 validator behavior;
- pending descriptor and durable publication ordering;
- deterministic retry over invisible stale payload bytes;
- current scratch bounds and corrected EXL3 plane arithmetic; and
- the current CPU tests and proof statements.

Acceptance does not cover:

- complete checkpoint conversion or conversion throughput;
- the native reader allocation-posture minor;
- zero-copy EXL3 validation;
- device upload/residency;
- CUDA, SM120, model, quality, capacity, serving, or performance evidence; or
- cn4 access.

## Required adversarial questions

1. Do all thirteen candidate-input hashes match at review start and finish in
   a detached worktree?
2. For gate/up `K=6,144, N=512`, does
   `4 + 2 * (K + N)` equal exactly 13,316 bytes?
3. Does swapping K and N for down leave the same 6,656 rotation words and
   13,316 auxiliary bytes?
4. Is the primary plane still exactly 1,179,648 bytes for both actual
   routed-expert rank geometries?
5. Are the proof's projection-bound and common 8 MiB buffer statements
   conservative and exact after this correction?
6. Is `TensorWriteValidator::None` semantically complete for a plain tensor
   whose logical and padded shapes are identical because no padding
   coordinate exists?
7. Does current `copy_exact_at` still present every gapless sequential chunk
   to its validator before writing and reject short/trailing sources before
   pending insertion?
8. Do current plain and NVFP4 validators preserve every semantic check from
   the first review across arbitrary chunk boundaries?
9. Does current EXL3 validation retain exactly one primary plus one auxiliary
   projection, allocate fallibly, enforce offset order, and call the
   canonical container-plane decoder before publication?
10. Is `validate_planes` still absent from a new successful write while
    remaining mandatory for adoption of completed staging descriptors?
11. Can any validation/finalization error publish a descriptor, increment
    completion, or prevent a fixed-offset retry from overwriting all
    invisible stale bytes?
12. Is payload sync, descriptor write, descriptor sync, and in-memory
    completion ordering unchanged?
13. Do all three semantic mutation tests still prove the on-disk descriptor
    is exactly zero after failure?
14. Do the 72 unit tests, three external NVFP4 tests, complete 376-test host
    gate, Clippy result, 104-handoff provenance count, and environment
    exclusions in the proof match the candidate run?
15. Does the proof avoid a conversion speedup, zero-copy, checkpoint, device,
    model, quality, capacity, serving, or performance claim?
16. Are the native-reader allocation asymmetry and reusable-copy-buffer
    observations correctly left as nonblocking follow-up rather than hidden
    as completed work?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately:

1. the corrected EXL3 byte arithmetic is exact;
2. unpadded plain validation is complete;
3. source sequencing and validation-before-write remain sound;
4. semantic errors cannot publish and retry remains deterministic;
5. durable ordering and resume validation remain unchanged;
6. current scratch bounds are accurate;
7. CPU results and exclusions are accurate; and
8. the single-pass streaming writer may remain merged.

Only if all sixteen questions and all eight statements are unqualified
`YES`, end with:

```text
streaming-write-single-pass-v1-accepted
```

Withhold for stale provenance, incorrect arithmetic, skipped/repeated source
bytes, validator-after-write behavior, semantic drift, unchecked EXL3
allocation, staging reread on new writes, descriptor publication after an
error, nondeterministic retry, changed durable ordering, false counts/hashes,
or any checkpoint/device/model/performance overclaim.
