# Fable handoff — manifest and cache ABI corrective re-review

Date: 2026-07-29

Candidate base commit:
`0edfc8d796aeaeb969668005149bcb6286aa1e85`

Required result path: `fable-manifest-abi-v022-r2.md` at the repository root.

Review scope: BLOCKER B1 and MAJOR M1 from the first review. Its unqualified
`YES` answers for Decisions 1 and 3 stand unless a corrected byte invalidates
them.

GPU authorization conveyed by this handoff: none.

## Gate-name contract

`manifest-abi-v0.2.2-accepted` is the stable name of the existing M2 review
gate. The current format document correctly identifies itself as v0.2.3. The
legacy gate token and `v022` result filename do not assert that the current
format bytes are v0.2.2; this re-review binds that gate name to the exact
v0.2.3 format hash below.

## Required provenance procedure

Review the exact candidate commit in a detached worktree. Hash every input at
review start and finish. If either set differs from this table, report a stale
candidate and do not emit an acceptance token.

| Input | SHA-256 |
|---|---|
| prior `fable-manifest-abi-v022.md` | `505bf452895cde7598e8e03141bd8bd381729f31f5ee95c11c036d26c79c8d42` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `profiles/profile-budget-v0.json` | `028516adc04d454317e1b76a3147be4807c3ed3ce371e1d43aead3396270400d` |
| `docs/nvfp4-physical-abi.md` | `d5a189ae06f8f39e828400e589fbd31f94f0245cb90488881369d6a806bd6d1e` |
| `crates/glm-cuda/src/abi.rs` | `28905e69300a3a8c8105752ee9aaeb4d718cbe4387cab548139c257242ec68a4` |
| `crates/glm-cuda/src/ffi.rs` | `23b4f7636b5930d6d7ef5c936b333fbcaca3c84705f37a29bd22e3895f2213f1` |
| `kernels/include/glmaxx_kernel.h` | `da233563c6bfe92885c1a3101bcafa20292365b12ab788afb4d32d44a3ed2472` |
| `kernels/sm120/cutlass_nvfp4_fc2_control.cu` | `dba61fd6bc34b659543f1b64a329603ab01406a1505954ae16bc9626b8f7ff94` |
| `kernels/sm120/nvfp4_routed_fc2.cu` | `b72fff75bf4b0ee0ef06bf65286bad73678e4d396b2bdaad72bc784da738bb31` |
| `crates/glm-cache/src/tier.rs` | `c31b07d7f9054f3d51bc5d24c2c414b6c9a134d88f042502bc0f82e29cad500f` |
| `crates/glm-cache/src/store.rs` | `d37a1400dc0c393b26c121f72694945bef78c28eda29796abf41a2ed713a17ac` |
| `crates/glm-cache/src/lib.rs` | `cda0df00f87041d2fa6a01b9fd43bc68706c1dfa30d398d3410a68ac3a068735` |
| `scripts/cn4-phase-b.sh` | `9ace5b4d4b0e8d2d1ee048bc32295cf86d7393b8420c5653b7d2f9faca23dd6d` |
| `docs/cn4-review-fixes-preparation-20260729.md` | `b94156526800da97bc30f46e0315a64ab43c488c2007f510dedb86e71b3ec805` |

The raw cn4 preparation records are outside Git at:

```text
/home/derek/glmaxx/evidence/prepare-c25e558-r1
```

The FC2 CUDA and Rust allocation bytes in that run are identical to the
candidate. Later changes only versioned the incompatible durable-journal
encoding, hardened the review scripts, and added the compact preparation
record. Hash-check the raw records rather than relying on this statement.

## Decision 2: one combined draft sidecar

Verify directly that:

1. `TierPiece` has exactly target KV, target indexer, and one draft sidecar;
2. an MTP record requires exactly those three pieces, with the sidecar exactly
   32,000 bytes and one offset/digest;
3. encoding is token-major, with each token's 368 draft-KV bytes immediately
   followed by its 132 draft-indexer bytes;
4. decode round-trips the two logical planes without changing the durable
   record representation;
5. piece ranges are checked for overflow and pairwise overlap;
6. publication, replay, and restore require and hash the combined sidecar as
   one durable piece; and
7. the incompatible development journal change is explicit as
   `GLTJRNL2`/version 2, while a syntactically valid v1 record fails closed.

Do not infer that `FileTierStore` serializes the format document's complete
4,096-byte sealed page header. This decision is only whether the durable tier
model and journal can faithfully represent and atomically publish the one
combined token-major sidecar payload.

## Decision 4: non-overlapping FC2 materialization

For `P = assignments * 6144`, verify independently that:

1. the live assignment output owns bytes `[0, 4P)`;
2. the BF16 CUTLASS materialization owns bytes `[4P, 6P)`;
3. dense and grouped `ptr_d` address only that BF16 plane;
4. the expansion kernels read only `[4P, 6P)` and write only `[0, 4P)`, so no
   CTA can clobber another CTA's unread source;
5. Rust allocates `6P` bytes for the combined allocation;
6. Rust and C workspace formulas both include the extra `2P` term and retain
   exact parity; and
7. the corrected source compiles into the pinned `sm_120f` library and
   retains the expected block-scaled OMMA instructions.

Also confirm that the Phase-B script now requires the dedicated r2 artifact,
the exact token, and all four exact reviewed contract hash attestations before
any GPU inventory or launch.

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
answer separately:

1. Does the first review's Decision 1 answer remain an unqualified `YES`?
2. Is the corrected combined draft-sidecar durable model accepted for M2?
3. Does the first review's Decision 3 answer remain an unqualified `YES`?
4. Is the FC2 data race eliminated and the corrected routed-MoE development
   control accepted for M2?

Only if every answer is an unqualified `YES`, include:

```text
engine-v0-sha256=efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
format-v0-sha256=619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a
operation-manifest-sha256=8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6
profile-budget-v0-sha256=028516adc04d454317e1b76a3147be4807c3ed3ce371e1d43aead3396270400d
```

Then end with:

```text
manifest-abi-v0.2.2-accepted
```

Do not emit the token for a conditional pass or stale input. Acceptance closes
only the independent-review half of the M2 gate. It does not authorize a GPU
launch, approve full conversion, or establish performance.
