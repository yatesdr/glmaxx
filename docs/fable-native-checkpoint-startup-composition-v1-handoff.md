# Fable handoff: native checkpoint startup composition v1

Date: 2026-07-30

Status: adversarial host composition and concurrency review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4, compile
remotely, allocate a GPU, or launch CUDA for this review

Review candidate commit:
`b55c8a9169c72a311c59d30e5389618eef3f0d7b`

Required result path:
`docs/reviews/fable-native-checkpoint-startup-composition-v1.md`

Requested acceptance token, only if every blocker and major is resolved:
`native-checkpoint-startup-composition-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |
| `crates/glm-engine/Cargo.toml` | `1e4fafe2079b1f529a35b6153cdbfcc39420728fcba0cb72c5920715cf591401` |
| `crates/glm-engine/src/native_worker.rs` | `9f51a29ca14589adca8a530c36f41101948e68de123f662b0c2d088bf59a7f01` |
| `crates/glm-engine/src/worker.rs` | `ae3c4345bd41725516d773032669685c0a43bc72bc09c2ffdce48e04b59f97b0` |
| `crates/glm-engine/src/lib.rs` | `415bbd961b0788057dfab8db49ab0d622e7a69bc46899417fea712062a37a366` |
| `crates/glm-engine/src/checkpoint_cuda.rs` | `6bceb0e8fa8c32e8d981bc3044163b03f5d2bcc5b3c3527ccfed215012240cc4` |
| `crates/glm-engine/src/checkpoint_load.rs` | `736b8f7dd75e470a9fdae83376df40165245f6a0683fe6846f8df9f022a4e09f` |
| `crates/glm-cuda/src/ffi.rs` | `eee9a388c8c25b9d385ddf0159c7dc766d1a19560078f2915d22fe77d1642817` |
| `crates/glm-format/src/native_reader.rs` | `953a56702ba1ee000f508fe24cbbae7c6137d6496d104720f658fede5572699c` |
| `docs/native-checkpoint-rank-adapter-proof-v1.md` | `ce55dbdb0eb12a60c891f945312899a0a32e783a0cf83bc7fb3e729210be5129` |
| `docs/tp4-checkpoint-load-protocol-proof-v1.md` | `d536e281d694eb0cbdd123d2cea9527e0d7cb9348556c6c3be0c9da13323ccb7` |
| `docs/native-checkpoint-startup-composition-proof-v1.md` | `7461f1db160520331e9552aabc11d6999f6e6bececf9187279d209f956bb3dab` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-native-checkpoint-startup-composition-v1-handoff.md
cargo test --offline -p glm-engine worker
cargo clippy --offline --workspace --all-targets -- -D warnings
GLMAXX_KERNEL_LIB_DIR=/tmp cargo clippy --offline -p glm-cli \
  --features cuda-ffi -- -D warnings
```

## Review boundary

This review covers the process-common checkpoint-device-identity handshake,
its exclusive quota and common deadline, terminal failure behavior, the
atomic administrative-operation quota correction, and
`load_native_checkpoint` from four immutable paths through global CUDA-arena
adoption. It also covers the returned loaded-checkpoint owner and structured
startup error boundary.

It assumes earlier reader, plan, CUDA arena, native adapter, and TP4
transaction reviews only at their typed boundaries. It does not accept those
candidates by implication.

It does not cover a CLI/evidence writer, CUDA link or execution, a real
checkpoint load, physical shutdown evidence, target-layer execution, serving
health, SM120 correctness, capacity, quality, or performance.

## Required adversarial questions

1. Verify every candidate input hash twice. Does the proof describe the exact
   pinned candidate rather than moving `main`?
2. Is the native identity returned by the same persistent context owner that
   later creates the checkpoint backend? Can a coordinator-side or stale
   inventory digest substitute for the worker observation?
3. Does identity discovery broadcast to all four ranks and collect exactly
   four unique rank identities under one common deadline? Can missing,
   duplicate, out-of-range, zero, repeated, reordered, or errored responses
   enter the returned array?
4. Is every identity failure terminal for the worker generation, with no
   rank-local remap/retry, while success leaves the exact workers alive for
   loading?
5. Trace the exclusive permit through zero timeout, reservation conflict,
   command send, rank timeout, validation failure, response drop, success,
   and dispatcher teardown. Is it released exactly once?
6. Do page-table initialization and explicit delta application now atomically
   reserve exclusivity? Did the change close the prior check-then-send race
   without introducing a quota leak on delta validation or channel failure?
7. Does `load_native_checkpoint` reject invalid scalar configuration before
   opening files or starting workers? Are the remaining environment and
   format constraints preflighted before native startup?
8. Does the coordinator open exactly four ordered immutable readers, require
   ranks 0–3, and validate rank-set consensus and the production tensor
   contract?
9. Is the synthetic-identity plan unpublishable and discarded before workers
   start? Can any synthetic digest reach `load_weights`, a receipt, evidence,
   or returned plan?
10. After workers start, is the canonical plan rebuilt from their exact
    ordered identities and the same still-unchanged readers? Can file
    replacement between the two opens evade the reader fingerprints or the
    rank-local plan binding?
11. Does the function invoke the existing atomic load only with the rebuilt
    plan, exact attempt generation, four owner generations, and common
    timeout? Can it return before four finalize acknowledgements?
12. On post-spawn plan or load failure, are the pool, rank contexts, readers,
    and partial arenas torn down on their owner threads? Is the permanently
    wedged-thread limitation still stated accurately?
13. Does `LoadedNativeCheckpoint` retain the live pool, exact plan, complete
    load outcome, and observed identities without allowing arena lifetime to
    end before the pool?
14. Are configuration, rank-indexed reader, plan, worker/identity, and load
    failures distinguishable? Is any failure converted into apparent
    success or stripped of the rank information already available?
15. Independently execute the four-rank error matrix, per-rank zero matrix,
    duplicate identity, timeout, success-after-discovery, and concurrent-step
    exclusion tests. Do they prove the stated properties rather than only
    exercise helper code?
16. Are the exact 336-test result, host-only feature checks, missing CLI,
    absent native link/launch, and every checkpoint/GPU/serving exclusion
    accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- identities come from the exact persistent allocation owners;
- response rank and identity consensus are complete and fail-closed;
- identity failure is terminal and success preserves the workers;
- identity and administrative commands own race-free exclusive quota;
- invalid config/files are preflighted before native startup;
- the synthetic plan can never be published;
- the final plan contains only worker-observed identities;
- file mutation/replacement cannot reuse the preflight plan;
- atomic load uses the final plan and all exact generations;
- every post-spawn failure tears down owned state;
- the stuck-thread limitation remains accurate;
- the loaded wrapper retains all required lifetime and evidence owners;
- structured errors preserve the available failure class and rank;
- the CPU fault matrix covers every claimed identity case;
- proof claims and explicit exclusions are accurate; and
- the boundary is ready for a feature-gated smoke CLI/evidence writer.

Only if all sixteen answers are unqualified `YES`, end with the requested
token. Withhold it for a stale candidate, stale/coordinator identity,
incomplete consensus, rank-local remap, quota race/leak, native startup before
preflight, publishable synthetic plan, file-replacement gap, wrong plan or
generation, partial-success return, teardown ownership error, hidden
stuck-thread claim, wrapper lifetime error, lost failure identity, incomplete
fault matrix, or evidence overstatement.

The token accepts only this host-type-checked native startup composition. It
does not authorize cn4 access or accept a CLI, evidence artifact, native
compilation/linking, checkpoint load, physical GPU cleanup, target-layer
execution, serving, SM120 correctness, capacity, quality, or performance.
