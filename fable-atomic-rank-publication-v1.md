# Fable adversarial review: atomic rank-set publication v1

Date: 2026-07-31

Reviewer: Fable (adversarial implementation review; CPU/filesystem only)

Handoff: `docs/fable-atomic-rank-publication-v1-handoff.md`

Reviewed candidate commit (detached worktree; no modification, no commit):

aaeffeaf9899f32c353015965142bd0d25b91e3c

Result-path note: the handoff requested `fable-atomic-rank-publication-v1.md`
at the repository root. The operator directed reviews into `docs/reviews/`;
this artifact is written there under that directive.

GPU/cn4 work performed: none.

## Provenance

All pinned inputs were hashed with `shasum -a 256` in the detached worktree at
review start and again at review finish. Both measurement sets matched the
handoff table exactly; no stale candidate. The only non-pinned file present in
the worktree was a byte-identical copy of the handoff itself (verified
identical to the main-tree copy, SHA-256
`08c95dddfc719a6b713ab21cfd50a2f431e3c28f3683838e1c218da0b13069f3`), required
to run `review-proof`, which returned `"verdict": "PASS"`.

| Input at candidate commit | Pinned = Start = Finish SHA-256 |
|---|---|
| `spec/format-v0.md` | 619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a |
| prior `fable-manifest-abi-v022.md` (repo root) | 505bf452895cde7598e8e03141bd8bd381729f31f5ee95c11c036d26c79c8d42 |
| `crates/glm-format/src/stream.rs` | 72841a32635c4abace3095c9759eeb9cced69aec95952b3f4f39c470fcb0ae63 |
| `docs/atomic-rank-publication-proof-v1.md` | ffd1659c10d212ab352c0d2817e6a0633ef6ba43dcb06cc573e48f2702ee52f3 |

Commands executed in the worktree: `review-proof` (PASS),
`cargo test --offline -p glm-format` (60 unit tests + 3 integration tests, 0
failures — matching the proof document's claim exactly), and
`cargo clippy --offline -p glm-format --all-targets -- -D warnings` (clean).

## Findings

### BLOCKER

None.

### MAJOR

None.

### MINOR

**MINOR-1 — Unsupported-platform failure occurs late, after staged-side
finalization and read-only destination stats.** On a Unix target that is
neither Linux nor Apple, `StreamingRankSet::publish` (stream.rs:1090) still
performs its two read-only `symlink_metadata` early-out checks on the
destination and finalizes/verifies the staged rank headers before reaching
`rename_directory_no_replace`, which then returns
`AtomicPublishUnsupported` (stream.rs:1273-1276) without inspecting or
renaming either path. No replacement-capable operation is reachable and no
destination byte is written, so the fail-closed invariant holds; but the
strictest reading of the handoff ("before touching either path") is satisfied
only by the rename helper, not by `publish` as a whole. Recommend a one-line
doc note that unsupported targets fail closed after staged-side finalization,
which is harmless and idempotent under the resume model.

**MINOR-2 — Old-runtime `renameat2` degradation is fail-closed but
undocumented.** On Linux kernels older than 3.15 (or filesystems without
`RENAME_NOREPLACE` support), `renameat2` fails with `ENOSYS`/`EINVAL`; both
map to `StreamRankError::Io` and publication fails closed rather than falling
back to a replace-capable rename — correct behavior, but the proof document
does not state the minimum kernel/filesystem assumption. Documentation-only.

### QUESTION

**Q-1** — `EEXIST` from a concurrent creator maps to `Err(Published)` even
when the object at the destination is not a valid rank set (the collision test
asserts the destination stays empty). Callers are expected to follow with
`verify_published`, which independently validates content; confirm that every
production caller of `publish` treats `Published` as "verify before trust",
never as success. (Code inspection found no caller that treats `Published` as
a successful publication; this is a caller-contract confirmation, not a
defect.)

## Answers to the nine required adversarial questions

1. **Linux atomicity and buffer validity.** Yes. `renameat2(AT_FDCWD, src,
   AT_FDCWD, dst, RENAME_NOREPLACE)` (stream.rs:1217-1223) makes destination
   nonexistence and the directory rename a single atomic filesystem operation.
   Both `CString` buffers are bound to locals that live across the entire
   unsafe call; NUL-embedded paths are rejected as `UnsafePath` before the
   call. `libc` 0.2.183 (Cargo.lock) provides the correct
   `renameat2(c_int, *const c_char, c_int, *const c_char, c_uint)` signature.
2. **Apple equivalence.** Yes. `renameatx_np(..., RENAME_EXCL)`
   (stream.rs:1250-1256) is the documented atomic no-replace rename on Apple
   platforms and applies to directories; flags and argument types match the
   pinned `libc` 0.2.183 API. Trailing symlinks at the destination are not
   followed by rename, so a symlink cannot be silently replaced.
3. **cfg exclusivity and import gating.** Yes. The three implementations are
   gated `target_os = "linux"`, `target_vendor = "apple"`, and
   `not(any(...))`; the predicates are mutually exclusive and exhaustive, so
   exactly one compiles per target. The `CString`/`OsStrExt` imports are gated
   with the identical `any(...)` predicate (stream.rs:1-2), so no unused-import
   or missing-import failure exists on any branch. The crate itself is
   Unix-only (ungated `std::os::unix::fs` imports), so non-Unix targets are
   out of scope by construction.
4. **EEXIST/ENOTEMPTY mapping.** Yes. Both map to `Err(Published)` without
   deleting, overwriting, or consuming the staged directory; the regression
   `rank_set_never_replaces_a_destination_created_during_conversion` proves
   the staged directory survives and the pre-created destination remains
   empty. All other errnos are returned as `StreamRankError::Io` with the raw
   OS error retained; publication fails closed.
5. **Unsupported platform.** No replace-capable path exists: `fs::rename` and
   `.rename(` appear nowhere in the workspace crates (verified by grep), and
   the fallback returns `AtomicPublishUnsupported` without touching either
   path. See MINOR-1 for the late-failure nuance inside `publish`; it does not
   weaken the invariant.
6. **Symlink/NUL/parent/EXDEV/concurrency/already-published.** No replacement
   or false success in any case. Embedded NUL → `UnsafePath`; missing parent →
   `ENOENT` → `Io`; cross-device → `EXDEV` → `Io` (and the staging directory
   is constructed as a same-parent sibling `<name>.partial`
   (stream.rs:1192-1200), so EXDEV is unreachable in normal operation); a
   destination created concurrently → kernel-atomic `EEXIST` → `Published`
   error, not success; an existing publication (including a symlink, caught by
   `symlink_metadata`) → `Published`. Parent and staging directories are
   `sync_all`'d around the rename for durability.
7. **Test coverage of the Apple path.** Yes. The collision test and the
   four-rank success/recovery tests all drive `publish` on the development
   platform (macOS), which compiles only the `renameatx_np` branch, so the new
   Apple syscall path is exercised end-to-end. The Linux branch bytes are
   unchanged from the previously compiled cn4 preparation, and no test was
   weakened to accommodate Apple (the invariants asserted are
   platform-independent).
8. **Closure of prior finding m5.** Yes. The replace-capable
   `fs::rename` fallback is gone; the only rank-set publication rename in the
   workspace is `rename_directory_no_replace`. The two remaining
   `symlink_metadata` checks in `publish` are read-only early-outs, not the
   safety mechanism; the atomic no-replace rename is the sole protection, so
   no check-then-rename sequence remains.
9. **Proof document exactness.** Yes. The 60-unit/3-integration test counts
   reproduced exactly; the stated commands match; the three relevant hashes in
   the proof match the pinned table; the non-claims (no Linux cross-target
   build on this host, no CUDA/GPU, no broader manifest-ABI acceptance) are
   accurate. See MINOR-2 for a documentation-only gap.

## Five acceptance statements

- Linux atomic no-replace publication: **YES**.
- Apple atomic no-replace publication: **YES**.
- Unsupported-platform fail-closed behavior: **YES**.
- CPU/filesystem proof and its non-claims accurate: **YES**.
- Prior manifest-review finding m5 closed: **YES**.

## Architecture & maintainability

The correction is minimal and correctly shaped: one platform-gated helper
with a single caller, kernel-atomicity as the only safety mechanism, and an
explicit unsupported-platform error instead of a portability fallback — the
exact inversion of the m5 defect. Keeping the read-only early-outs is good
ergonomics (cheap `Published` detection) without reintroducing TOCTOU
exposure. The staged directory naming (`<final>.partial` sibling) guarantees
same-filesystem renames without configuration. The two MINORs are
documentation-level; nothing here needs restructuring, and the pattern is
reusable for any future staged-to-published transition in the format crate.

## Token decision

Findings: 0 BLOCKER, 0 MAJOR, 2 MINOR, 1 QUESTION. Provenance verified at
start and finish; `review-proof` PASS; tests and Clippy clean. All five
acceptance statements are unqualified YES, so the requested token follows.

atomic-rank-publication-v1-accepted
