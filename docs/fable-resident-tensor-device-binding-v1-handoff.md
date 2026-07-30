# Fable handoff: resident tensor device binding v1

Date: 2026-07-30

Status: adversarial host implementation and ownership review requested

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4, compile remotely, allocate a GPU, or
launch CUDA for this review

Review candidate commit:
`a49210fe384012066d80087f61668d5d8a8e2a78`

Required result path:
`docs/reviews/fable-resident-tensor-device-binding-v1.md`

Requested acceptance token, only if every blocker and major is resolved:
`resident-tensor-device-binding-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |
| `crates/glm-engine/Cargo.toml` | `1e4fafe2079b1f529a35b6153cdbfcc39420728fcba0cb72c5920715cf591401` |
| `crates/glm-engine/src/checkpoint_cuda.rs` | `0b5e411d68a61fa1a39ccb7cc6b36702b85b3d385098764fa2d33b18227efdbe` |
| `crates/glm-engine/src/checkpoint_load.rs` | `052198f4265ab2569eb19feb074c96b83daa34fa0566575e9517ec59f7ca5957` |
| `crates/glm-engine/src/native_worker.rs` | `6173e7575a5a994c9090476154621e742063282e3c17e3c87a77eaa2a30da4db` |
| `crates/glm-engine/src/lib.rs` | `e3a70f7906c7a0d33a6a43e8bf791e1de0daa47e4ab918825adb47ecd64fb4b9` |
| `crates/glm-cli/src/main.rs` | `dd8e43b1ef937d33e4ae37d10e4592161d086289addb693d764d7d267c79ed5e` |
| `crates/glm-format/src/checkpoint.rs` | `08450f0cb33e592ec76dfbe655b06580ba1743e60f3109a65218d052dbea406c` |
| `crates/glm-format/src/rank_manifest.rs` | `cb57a81ad3643a86992c4fd9c2166e9ee238cc7e66063732355d14e485f73410` |
| `docs/resident-tensor-device-binding-proof-v1.md` | `15a21ae2ff24758d2f115540a895191f7aeb2acf13c31f2972dcb0700adbab6d` |
| `docs/sm120-rank-runtime.md` | `908b8adf0e1fc230145c009db01c71e69437ab359c76a545031fd9157c1ceea9` |
| `docs/target-layer-execution-v1.md` | `7f16667d55a50441dbc81b7fe18285be5a6f0fb06c68ef008a40aad8f406f478` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-resident-tensor-device-binding-v1-handoff.md
cargo test --offline -p glm-engine
cargo clippy --offline -p glm-engine --all-targets -- -D warnings
GLMAXX_KERNEL_LIB_DIR=/tmp cargo clippy --offline -p glm-engine \
  --features cuda-ffi --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

## Review boundary

This review covers:

- the immutable shared rank-layout representation;
- post-hash `RankSetLoadPlan` field visibility;
- ownership transfer through prepare, acknowledge, and global adoption;
- typed tensor-ID-to-device-span resolution;
- full descriptor/manifest/binding validation on the persistent owner thread
  before finalize success;
- failure cleanup when resident binding validation fails;
- CPU fault tests and proof exclusions; and
- the recorded gate/up semantic-key discovery.

It assumes the earlier native reader, rank-set load plan, CUDA upload,
transaction, and rank-owner candidates only at their typed boundaries. It
does not accept them by implication.

It does not accept a complete target program, CUDA compile, real device
pointer, kernel launch, sparse-layer replay, checkpoint execution, serving,
quality, capacity-in-practice, or performance result.

## Required adversarial questions

1. Verify all candidate input hashes at review start and finish. Does the
   review stay pinned even if `main` advances?
2. Does changing rank layouts from `Vec` to `Arc<[TensorArenaEntry]>` leave
   canonical plan bytes and `plan_sha256` exactly unchanged?
3. Can any external safe Rust caller mutate the header, rank entries, or
   tensor layouts after the plan digest is computed, or replace a layout
   before load/adoption?
4. Are the layout references cloned before/through physical upload without
   allocating or copying 59,585 entries per rank after HBM residency?
5. Trace layout ownership from the exact plan sent to each rank through
   `PreparedCudaRank`, `AcknowledgedCudaRank`, and `CudaWeightArena`. Can a
   different plan, rank, attempt, or owner generation become resident?
6. Before global adoption, can any public or internal safe path resolve a
   tensor device pointer?
7. Does adoption recheck nonempty dense tensor IDs, roles, codecs, primary
   planes, power-of-two alignment, and bounded relative spans? Is its reliance
   on the already sealed plan for nonoverlap and exact arena coverage sound?
8. For every nonempty plane, independently prove that checked
   `base + offset`, end bounds, and both relative and absolute alignment
   prevent overflow, overrun, underalignment, and cross-arena resolution.
9. Do zero-byte metadata and auxiliary planes become `None` without exposing
   a usable-looking base pointer? Can a mandatory zero-byte primary pass?
10. Does `tensor_binding` reject out-of-range and non-dense IDs and return
    only role, codec, flags, alignment, and spans taken from the authenticated
    layout rather than caller labels?
11. Immediately after adoption, does the persistent owner thread resolve all
    59,585 IDs and compare tensor ID, role, codec, flags, alignment, and all
    plane byte counts against both native descriptors and validated manifest
    semantics before a finalize acknowledgement can exist?
12. On any all-tensor validation failure, is the arena synchronized and
    released before returning the original error? If release fails, is the
    state terminal with no forged finalize or cleanup acknowledgement?
13. Are raw arena bases, span types, and the resolver unavailable to
    downstream crates? Does any safe public API permit a stale device binding
    to escape arena ownership?
14. Independently inspect the pinned capacity-EXL3 plan. Are gate and up
    genuinely separate tensor descriptors sharing `(layer_id, role_id,
    expert_id)`, making the proof's target-program discovery accurate?
15. Does the discovery correctly stop short of choosing a projection
    discriminator? Could the current target-program prose accidentally map
    gate/up by role/expert only or assume a combined descriptor?
16. Re-run the exact fault tests. Do wrong ID, zero role, overrun,
    non-power-of-two alignment, absent optional planes, out-of-range lookup,
    exact address arithmetic, shared `Arc` identity, and cleanup all behave as
    claimed?
17. Do default and `cuda-ffi` Clippy builds prove both the CPU fake path and
    the owner-thread all-tensor validator without linking or launching CUDA?
18. Are all scope exclusions exact, including no target-program compiler,
    target geometry proof, CUDA execution, model kernel, or layer replay?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- rank layouts remain canonically hash-identical and externally immutable;
- no per-rank full layout copy occurs after checkpoint upload;
- adoption is exact-plan, exact-rank, and exact-owner bound;
- every device span is overflow-, bounds-, and alignment-checked;
- optional planes are represented without ambiguous pointers;
- all 59,585 bindings are reconciled before finalize success;
- validation and cleanup failures are terminal and non-forgeable;
- the target-program gate/up ambiguity is real and accurately scoped; and
- proof claims and exclusions match the exact candidate.

Only if all eighteen answers are unqualified `YES`, end with the requested
token. Withhold it for stale provenance, changed canonical bytes, mutable
post-hash plan state, hidden layout copies, pre-adoption pointer exposure,
unbound or caller-controlled offsets, unchecked arithmetic, ambiguous absent
planes, incomplete all-tensor validation, cleanup-to-success conversion,
public stale-pointer escape, false gate/up analysis, incomplete fault proof,
or evidence overstatement.

The token accepts only the host resident tensor-binding implementation. It
does not authorize cn4 access or accept target-program compilation, CUDA
compilation/linkage, a real checkpoint load, model execution, serving,
SM120 correctness, capacity, quality, or performance.
