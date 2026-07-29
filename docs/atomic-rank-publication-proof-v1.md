# Atomic rank-set publication CPU proof v1

Date: 2026-07-29

Implementation commit:
`4e126ea45b835bcedd36aed8eac6384fa1a87689`

Status: host-filesystem implementation passed; independent review pending

GPU claim: none

## Defect closed by the candidate

The first manifest/ABI review identified a check-then-rename race in
`StreamingRankSet` publication outside Linux. A destination directory could
appear after `symlink_metadata` returned `NotFound` but before `fs::rename`,
allowing publication to replace that empty destination.

The candidate removes that fallback:

- Linux uses `renameat2(AT_FDCWD, ..., RENAME_NOREPLACE)`;
- Apple uses `renameatx_np(AT_FDCWD, ..., RENAME_EXCL)`; and
- another Unix target without an implemented atomic no-replace primitive
  returns `AtomicPublishUnsupported` without inspecting or renaming either
  path.

Destination nonexistence and the rename are therefore one filesystem
operation on both development and production platforms. `EEXIST` and
`ENOTEMPTY` map to the existing `Published` outcome. Other syscall failures
retain their OS error and fail publication.

The Linux implementation bytes are otherwise unchanged. The production
target remains Linux SM120; the Apple path makes local conversion proofs
obey the same no-replacement invariant.

## Proof

The focused format suite passed 60 unit tests and three integration tests.
The publication cases prove:

- a destination created before final publication is not replaced;
- the staged directory remains intact after the collision;
- the destination remains empty and unmodified;
- a complete four-rank set publishes and verifies atomically; and
- recovery after a partially finalized rank set still publishes correctly.

Workspace Clippy passed with warnings denied. Formatting and diff checks
passed.

Commands:

```text
cargo fmt --check
cargo test -p glm-format
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

Relevant hashes:

```text
crates/glm-format/src/stream.rs
72841a32635c4abace3095c9759eeb9cced69aec95952b3f4f39c470fcb0ae63

spec/format-v0.md
619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a

fable-manifest-abi-v022.md
505bf452895cde7598e8e03141bd8bd381729f31f5ee95c11c036d26c79c8d42
```

The local host does not have the Linux Rust standard-library target
installed, so no new cross-target build is claimed. The Linux branch had
already compiled in the pinned cn4 preparation; this candidate does not
change that branch.

No CUDA toolchain or GPU was used. This proof does not accept the broader
manifest ABI gate, authorize cn4, establish checkpoint conversion
performance, or establish device correctness.
