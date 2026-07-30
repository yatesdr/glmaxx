# Fable handoff: Linux direct-tier feature and fault probe v1

Date: 2026-07-30

Status: adversarial design review requested

Review candidate commit:
`a3771e31dd73132db1caa306c097408e05388988`

Required result path:
`docs/reviews/fable-direct-tier-linux-probe-v1.md`

Requested acceptance token, only for an unqualified scoped pass:
`direct-tier-linux-probe-v1-design-accepted`

GPU or storage authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch GPU, container, storage-device,
filesystem-probe, or network work for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if needed. Hash every candidate input at review start and finish.
Any mismatch must withhold the token as stale.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/direct-tier-linux-probe-v1.md` | `ff77a0fc035b6a84270c2e177d9a248f7708133f348096cadf96f02a97b1ee52` |
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `docs/direct-tier-extent-cpu-proof-v1.md` | `d54ad467e8f2219ec31638416ff5a0a74cf972a6077695b6eea7dd1b8eb859b1` |
| `docs/direct-tier-state-cpu-proof-v1.md` | `3f58a9c1b7ad7cc4806598b467f02eb746013e75a72e4566f9e9ba55f466df66` |
| `docs/direct-tier-durable-format-v1.md` | `19ca03edeab89b560d674689ca96ce497f2c5859a91d5fe5d4b50c78645e79e6` |
| `docs/direct-tier-scheduler-cpu-proof-v1-r3.md` | `3ba91bddf8ac24fecf2a8ee880a97031edac07d46f527e840b4d6e39e33eed64` |
| `crates/glm-cache/src/direct.rs` | `7514ad205b84f24c2a2f58647c2b50b0f6dab4398dfb04a2fc36186f09a82dd3` |
| `crates/glm-cache/src/direct_state.rs` | `386c414a302a489c1e8c6fefa6d11b304e86ffa2040ad105e81cc8572999f814` |
| `crates/glm-cache/src/direct_restore.rs` | `73578aa42bf944c37bfe431da21df5c27ad12ee58dfb81f64a24a180830b1c1f` |
| `crates/glm-cache/src/direct_schedule.rs` | `9dd9feacaa04e927ffa0d1153a797a6dd1ce34ed8c8e07634c846fd03b7bcb04` |
| `docs/production-punchlist.md` | `0a2023ca7ec8caf6c096c10e0fda6d9cffa6651c5df16116ce9eb8c7ca8e6dbf` |
| `docs/results-index.md` | `8c1ed5da0e07a98f3d564094e0f66f2ea83c3524f1ef17a3c44f7af517cc1b38` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-direct-tier-linux-probe-v1-handoff.md
git diff --check a3771e31dd73132db1caa306c097408e05388988^ \
  a3771e31dd73132db1caa306c097408e05388988
```

The handoff and review queue are coordination metadata added after the
candidate and are not candidate inputs. Do not run the proposed command:
there is no implementation, no authorized scratch filesystem, and no
authority to use cn4.

## Review purpose

Decide whether the design is a safe, exact, and implementable bridge from the
accepted direct-tier CPU policy to a later nonproduction Linux `io_uring`
probe. The review must attack both false passes and paths that could disturb
unrelated data or be mistaken for production qualification.

The design deliberately leaves implementation blocked on four pending
dependency tokens. Acceptance says only that implementation may begin after
those dependencies pass; it does not accept any dependency early.

## Review boundary

Acceptance covers only:

- the proposed Linux-only Rust module and command contract;
- external scratch/evidence path safety;
- fixed-file, fixed-buffer, SQ/CQ, completion, and cancellation arithmetic;
- the proposed injected and real boundary-fault matrices;
- cleanup ownership and normalized evidence requirements; and
- the staged implementation and qualification order.

Acceptance does not cover an implementation, executable, syscall trace,
Linux host, filesystem, NVMe device, cn4, CUDA, HBM/DRAM transfer, durable
store, cleaner, serving integration, model execution, capacity, latency,
throughput, or production health.

## Required adversarial questions

1. Do all fifteen candidate-input hashes match at review start and finish in
   a detached worktree?
2. Is implementation unambiguously blocked until all five dependency tokens
   are machine-accepted, without treating review of this design as acceptance
   of those dependencies?
3. Can the command reach any repository, checkpoint, mount root, production
   cache, symlink, magic link, path-traversal child, pre-existing run file, or
   object it did not create?
4. Are absolute distinct directories, the caller-created sentinel,
   nonblocking lifetime lock, clean-tree policy, exact binary/source identity,
   and explicit host/filesystem authorization sufficient and mutually
   consistent?
5. Is the baseline implementable with exactly 16 returned SQ entries, 32
   returned CQ entries, 16 descriptors, 16 registered buffers, fixed files,
   `SINGLE_ISSUER`, and none of the forbidden optional routes?
6. Do sixteen original operations plus at most one cancel per original fit
   exactly within 32 CQ entries without relying on `IORING_FEAT_NODROP` or
   intentionally creating overflow?
7. Does every cancel race produce and retain ownership for exactly the
   required original and cancel CQEs, including original-first/not-found and
   cancel-won orderings, without early descriptor or buffer reuse?
8. Are 2,019,328 and 2,052,096 bytes exactly 493 and 501 aligned blocks, and
   are allocation, preallocation, file ranges, buffers, exact CQE lengths,
   decode, physical digest, logical digests, zero padding, and byte comparison
   all covered?
9. Are anonymous mapping alignment, zeroing, `MADV_DONTFORK`, fixed-buffer
   registration, effective memlock/cgroup accounting, unregistration, and the
   future CUDA double-pin reservation specified without claiming CUDA pinning?
10. Can any accepted trace use a raw descriptor, non-fixed read/write opcode,
    wrong fixed-file/buffer slot, stale generation, duplicate/unknown token,
    mismatched offset/length, or short positive completion?
11. Does the injected adapter exercise every legal transition while remaining
    structurally impossible to select for production health, and are retryable
    `EINTR`/`EAGAIN` results limited to proven pre-commit cases?
12. Do the real-adapter cases actually test address and file-offset
    misalignment, reduced length, short EOF, and removed fixed-file slots
    without assuming one errno across filesystems or performing destructive
    ENOSPC/device-loss work?
13. Are data-file durability and barrier/journal-role durability tested
    separately, with independent fsync failures and barrier reopen/readback?
14. Does every normal/error/shutdown route stop admission, abandon waiters,
    cancel eligible originals once, drain all CQEs, prove zero ownership,
    unregister files then buffers, close/unmap/unlink only owned resources,
    fsync the directory, and refuse health after incomplete teardown?
15. Is normalized evidence deterministic about semantic fields while retaining
    arrival order and environment variability, and does it preserve every
    stated nonclaim?
16. Can any wording be read as authorization for cn4, a production filesystem,
    destructive fault injection, buffered fallback, dynamic `liburing`, C++,
    storage qualification, K03/K05 acceptance, or a performance claim?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately whether:

1. the dependency gate is fail-closed;
2. the external-path and owned-object boundary is safe;
3. ring, descriptor, registered-memory, and CQ arithmetic is exact;
4. cancellation and completion ownership is complete for all races;
5. both physical extent sizes receive exact write/fsync/read/decode checks;
6. injected and real fault routes cannot produce a false production pass;
7. teardown and evidence are complete and implementable; and
8. no host, filesystem, storage, GPU, model, serving, capacity, or performance
   evidence is implied.

Only if all sixteen questions and all eight statements are unqualified
`YES`, end with exactly one bare line:

```text
direct-tier-linux-probe-v1-design-accepted
```

Withhold for stale provenance, a premature dependency pass, unsafe path or
unlink scope, reliance on CQ overflow/NODROP, incomplete cancel ownership,
short-I/O continuation, fixed-resource escape, injected-adapter production
reachability, unsafe cleanup, an ambiguous evidence claim, or any cn4/
storage/GPU/model/performance overstatement.
