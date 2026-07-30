# Fable handoff: checkpoint load transaction v1 r3

Date: 2026-07-30

Status: consolidated corrective contract and CPU/mock implementation review
requested

Review candidate commit:
`fc96d90836e32d7c582a1bddbf1521a28638ccfa`

Required result path:
`docs/reviews/fable-checkpoint-load-transaction-v1-r3.md`

Requested acceptance token, only for an unqualified scoped pass:
`checkpoint-load-transaction-v1-r3-accepted`

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch GPU, container, storage-device,
or network work for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if needed. Hash every candidate input at review start and finish.
Any mismatch must withhold the token as stale.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/checkpoint-load-transaction-v1.md` | `bc3c938f488bdcbf002c788ce9c5ac493addfe81866f39875d583c4312842ccf` |
| `docs/fable-checkpoint-load-transaction-v1-r2-handoff.md` | `24cd8f8502d6a6f2a34c0e28cb46083ef2585924e1fc60935dc0cae0f1c3118f` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-engine/src/checkpoint_load.rs` | `052198f4265ab2569eb19feb074c96b83daa34fa0566575e9517ec59f7ca5957` |
| `crates/glm-engine/src/checkpoint_cuda.rs` | `0b5e411d68a61fa1a39ccb7cc6b36702b85b3d385098764fa2d33b18227efdbe` |
| `crates/glm-engine/src/worker.rs` | `3533f606400c8aa5c571caa360ba516abd69d39de0489b87be4658143a9bdc24` |
| `crates/glm-engine/src/startup.rs` | `1a5f1ac8aae94e6eb2aaf2cf4701dfc290604013103eacc1423046211609a5fc` |
| `crates/glm-engine/src/memory.rs` | `0ae657905a1b2091980c4904643e35a7a53b282ef112be44447362add89f023b` |
| `crates/glm-engine/src/weight.rs` | `d658cefefc17757a28258bafd0e13f5309e8adcbf2b30c4d2bdc97be9899ca19` |
| `docs/checkpoint-load-cpu-core-proof-v1.md` | `a3cbd93be0b7f131d98d996601c75e653764ec429839f19e2c26835fa4bd20c1` |
| `docs/cuda-checkpoint-arena-cpu-proof-v1.md` | `399c5b9a276870210d1e389269b602862091e3ef42ccc22a2175ad7589a09a76` |
| `docs/native-rank-load-plan-proof-v1.md` | `19558f87ef912c8ead99c31cf0f1a1867dcc384ab79d8efbbee96f66abfe0e63` |
| `docs/rank-set-load-coordinator-proof-v1.md` | `4076872eec5adcdb2f4a0445418ed58d695a188a1d8aeb4e95496ef7ec52196a` |
| `docs/tp4-checkpoint-load-protocol-proof-v1.md` | `d536e281d694eb0cbdd123d2cea9527e0d7cb9348556c6c3be0c9da13323ccb7` |
| `docs/native-checkpoint-rank-adapter-proof-v1.md` | `ce55dbdb0eb12a60c891f945312899a0a32e783a0cf83bc7fb3e729210be5129` |
| `docs/native-checkpoint-startup-composition-proof-v1.md` | `7461f1db160520331e9552aabc11d6999f6e6bececf9187279d209f956bb3dab` |
| `docs/native-checkpoint-load-smoke-proof-v1.md` | `2b6c5ef4cb3e37c25b2260ec165ffbc8a3c416d85d611a48b08c256e4fc6a3a2` |
| `docs/resident-tensor-device-binding-proof-v1.md` | `15a21ae2ff24758d2f115540a895191f7aeb2acf13c31f2972dcb0700adbab6d` |
| `docs/production-punchlist.md` | `a62e924d837278d199a10adff4a0afcaae5e7358b18f27b2769b789de0fc682e` |
| `docs/results-index.md` | `9968b9b47b8d40681cb1cac08d5891eff2122117361a1834fec114ad1da1927e` |
| `scripts/local-checks.sh` | `56f728cdf3f047f9633509a57341d25a977efa802f0d5b371c9716830517db59` |
| `Cargo.toml` | `863c28560b339f1fd7fb6b80c1b812e9fa7bc3f8f8c782126d2a29ceeffc06ea` |
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-checkpoint-load-transaction-v1-r3-handoff.md
cargo test --offline -p glm-engine
git diff --check fc96d90836e32d7c582a1bddbf1521a28638ccfa^ \
  fc96d90836e32d7c582a1bddbf1521a28638ccfa
```

The handoff is coordination metadata added after the candidate and is not a
candidate input. The r2 review is operator-owned read-only input at
`docs/reviews/fable-checkpoint-load-transaction-v1-r2.md`; do not modify or
add it to the candidate.

## Review purpose

The r2 review accepted the rank-specific/common identity split, all five
binary encodings, semantic catalog, startup order, arena planning, and
four-rank adoption, but withheld its token on:

1. unspecified pinned-ring teardown while H2D DMA might still reference host
   memory; and
2. no mandatory proof that final HBM intervals contain the authenticated
   bytes.

It also reported eight minor contract gaps. The current r3 contract and CPU
mock claim to close all ten findings:

- every event/stream is drained before freeing pinned/device resources;
- synchronization failure leaks possibly referenced resources and makes
  process teardown mandatory;
- both full arenas are zero-filled, uploaded, D2H-read back in fixed 8 MiB
  chunks, and SHA-256 compared before preparation;
- typed readback and rank-evidence records bind bytes, timings, scratch,
  runtime provenance, and owner generation;
- process-common policy derives plane lengths and rejects nondivisible TP
  extents;
- common/rank tensor counts are equal, all gaps/tails are canonical zero,
  and binary fields serialize individually;
- `source_shape` exclusion has an exact derivation/version boundary; and
- unsupported NVFP4-laboratory/hybrid budget schemas remain fail-closed.

The current Rust implements the CPU plan, type states, mock arena backend,
bounded ring/event schedules, evidence, rank-set coordinator, persistent-rank
protocol, native adapter boundary, startup composition, load-only command
binding, and resident span reconciliation. Native CUDA compilation and device
execution remain unqualified.

## Review boundary

Acceptance covers only:

- the corrected r3 checkpoint-load contract;
- deterministic plan/catalog/layout/receipt/evidence encodings;
- policy-owned geometry and profile rejection;
- quarantined arena and pinned-ring CPU/mock ownership;
- mock zero-fill/upload/readback/error/teardown schedules;
- exact four-rank prepare/adopt/finalize/abort coordination;
- native adapter type/ownership boundaries visible in Rust;
- load-only command preflight and resident descriptor reconciliation; and
- the associated CPU tests and proof documents.

Acceptance does not cover:

- native library compilation or ABI qualification;
- a CUDA context, allocation, memcpy, event, readback, or device hash;
- real checkpoint files or a completed rank load;
- a model kernel, graph, collective, KV allocation, or request;
- production startup or health;
- GLM-5.2 output, quality, capacity under live allocations, latency, or
  throughput; or
- cn4 access.

## Required adversarial questions

1. Do all twenty-three candidate-input hashes match at review start and
   finish in a detached worktree?
2. Does r3 preserve the already-accepted rank-specific manifest contracts
   and independent rank-invariant semantic catalog without changing their
   encodings?
3. Are policy-derived plane lengths, exact TP4 divisibility, rank/header
   tensor-count equality, and `source_shape` derivation enforced rather than
   trusted from rank files?
4. Do all plan, semantic-entry, readback-evidence, rank-evidence,
   prepared-receipt, and rank-set digest preimages have exact widths,
   domains, field order, endianness, and reserved-zero rules?
5. Does field-by-field serialization avoid the intentionally unaligned u64
   fields without relying on `repr(C)` layout?
6. Can the current builder admit only the implemented `capacity-exl3`
   budget, or can a profile-byte mutation open NVFP4 laboratory or hybrid
   serving?
7. Before any H2D submission, are both complete planned arenas zero-filled
   and every destination interval proven disjoint, in bounds, and written
   exactly once?
8. Does the bounded pinned ring copy borrowed reader bytes before callback
   return and wait for each slot event before reuse?
9. On ordinary success, early error, panic, rank abort, and process-wide
   abort, can any pinned slot/event/device allocation/load stream be freed
   while DMA may still reference it?
10. If event/stream synchronization fails, does the Rust/mock ownership model
    prevent cleanup code from freeing uncertain resources and surface a
    terminal process condition?
11. Does the CPU mutation proof distinguish an unsafe free-after-sync-error
    implementation from the required leak-and-terminate posture?
12. After sealing, does first-load verification D2H-read the full weight and
    metadata arenas, including zero gaps/tails, through bounded 8 MiB
    storage?
13. Can wrong destination offsets, a changed device byte, skipped zero-fill,
    short readback, event failure, or expected/observed digest substitution
    produce a prepared receipt?
14. Do the typed readback/evidence digests bind exact device identity,
    plan/allocation generation, byte counts, chunk geometry, expected and
    observed hashes, timings, scratch, and software/runtime provenance?
15. Are all evidence and prepared-receipt values recomputed from owned typed
    state rather than accepted as caller-supplied digest bytes?
16. Does preparation remain non-executable until four exact receipts, one
    common adoption digest, four adoption acknowledgments, and later startup
    gates?
17. Does any preparation/adoption/finalization failure issue one common
    process-wide abort without rank-local retry, device substitution, codec
    fallback, or forged cleanup receipt?
18. Are exclusive TP4 operation ownership, timeout, dropped-handle, cleanup,
    and normal shutdown semantics exact and release-visible?
19. Does resident tensor reconciliation bind every plan-owned span to an
    authenticated descriptor/semantic before finalize without exposing raw
    mutable layout ownership?
20. Do the current CPU tests cover each rank failure position, late rank-3
    corruption, ring wrap, final partial chunk, zero gaps, readback
    corruption, sync-cleanup failure, adoption failure, and no-early-handle
    publication?
21. Are the proof documents accurate about their exact candidate boundaries
    and do they avoid turning mock backend events into CUDA evidence?
22. Does the remaining native qualification order stay fail-closed:
    adversarial acceptance, native build/ABI, authorized load-only smoke,
    then model execution gates?
23. Are every native-build, GPU, checkpoint, model, quality, live-capacity,
    performance, production-health, and cn4 exclusion accurate?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately whether:

1. the two r2 majors are closed by the governing contract and CPU mock;
2. all eight r2 minors are closed or correctly fail-closed;
3. canonical encodings and policy-owned geometry are exact;
4. quarantined/pinned resource ownership is safe through every terminal
   path;
5. full-arena readback evidence is complete and unforgeable within the CPU
   boundary;
6. four-rank adoption/finalization cannot partially publish;
7. the Rust/native adapter boundary is ready for a separately authorized
   native qualification;
8. current CPU tests and mutations distinguish the reviewed defects;
9. no CUDA/checkpoint/model evidence is implied; and
10. all other exclusions are accurate.

Only if all twenty-three questions and all ten statements are unqualified
`YES`, end with:

```text
checkpoint-load-transaction-v1-r3-accepted
```

Withhold for stale provenance, unsafe DMA teardown, free-after-sync-error,
incomplete HBM-content verification, caller-forgeable evidence, rank-file
geometry control, a profile-byte escape, partial adoption, rank-local
fallback, a nondistinguishing CPU proof, or any native/GPU/checkpoint/model/
performance overstatement.
