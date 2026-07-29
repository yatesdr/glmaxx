# Fable handoff: atomic rank-set publication v1

Date: 2026-07-29

Status: adversarial implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`aaeffeaf9899f32c353015965142bd0d25b91e3c`

Required result path:
`fable-atomic-rank-publication-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`atomic-rank-publication-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| prior `fable-manifest-abi-v022.md` | `505bf452895cde7598e8e03141bd8bd381729f31f5ee95c11c036d26c79c8d42` |
| `crates/glm-format/src/stream.rs` | `72841a32635c4abace3095c9759eeb9cced69aec95952b3f4f39c470fcb0ae63` |
| `docs/atomic-rank-publication-proof-v1.md` | `ffd1659c10d212ab352c0d2817e6a0633ef6ba43dcb06cc573e48f2702ee52f3` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-atomic-rank-publication-v1-handoff.md
cargo test --offline -p glm-format
cargo clippy --offline -p glm-format --all-targets -- -D warnings
```

## Review boundary

This is the narrow corrective review for prior manifest-review finding `m5`.
It covers only the final staged-directory-to-published-directory operation
used by `StreamingRankSet`.

It does not re-review or accept rank-file contents, conversion quality, the
production rank manifest, the manifest gate's CUDA controls, any checkpoint,
or any device operation. It does not authorize cn4 or full conversion.

## Required adversarial questions

1. On Linux, does `renameat2` with `RENAME_NOREPLACE` make destination
   nonexistence and directory rename one atomic operation? Are source and
   destination path buffers valid for the complete call?
2. On Apple, does `renameatx_np` with `RENAME_EXCL` provide the equivalent
   atomic no-replace behavior for directories? Are the flags and argument
   types correct for the pinned `libc` API?
3. Do the mutually exclusive `cfg` branches compile exactly one
   implementation on Linux, Apple platforms, and every other supported Unix
   target? Are imports also correctly gated?
4. Do `EEXIST` and `ENOTEMPTY` map to `Published` without deleting,
   overwriting, or consuming the staged directory? Do all other errors
   retain their OS error and fail closed?
5. Can any unsupported platform reach `fs::rename`, a preflight existence
   check, or another replacement-capable fallback, or does it return
   `AtomicPublishUnsupported` before touching either path?
6. Can a symlink, embedded NUL, missing parent, cross-device path,
   concurrent destination creator, or already-published destination cause
   replacement or a false success?
7. Do the existing collision and successful-publication tests exercise the
   new Apple syscall path on the candidate's development platform without
   weakening the production Linux invariant?
8. Does this fully close prior finding `m5`, or does any rank-set
   publication path still perform a check-then-rename sequence?
9. Are the proof document's evidence and non-claims exact, including the
   absence of a new Linux cross-target build?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- Linux atomic no-replace publication is accepted;
- Apple atomic no-replace publication is accepted;
- unsupported-platform fail-closed behavior is accepted;
- the CPU/filesystem proof and its non-claims are accurate; and
- prior manifest-review finding `m5` is closed.

Only if all five answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, replacement-capable race,
incorrect platform gate, staged-data loss, or false success.

The token accepts only this publication correction. It does not satisfy the
separate `manifest-abi-v0.2.2-accepted` gate.
