# Results index

Date: 2026-07-30

Current host implementation baseline:
`1770563713722685db26b0d3378f32e4ecf92519`

The complete local gate most recently ran against native checkpoint-load smoke
implementation
`1770563713722685db26b0d3378f32e4ecf92519`. The
target CUDA/kernel and strict production-manifest baseline remains
`4bf7bb5`; the later CPU candidates add review integrity, cache-lifecycle evidence,
bit-exact indexer-scale handling, atomic publication, finite KV
reconstruction, exact restore-result identity, all-or-nothing HBM
admission, captured-shape prefill progress, and all-or-nothing scheduler
step completion, prefix release, selected-step failure finalization, and
multi-request terminal cleanup, plus retryable pending restore/admission
rollback and fail-stop ownership propagation through backend admission and
event cancellation, retryable active-sequence removal, and fail-stop durable
cache writes after uncertain publication errors, plus exclusive durable
writer ownership and read-only rank restore snapshots, and same-key logical
piece collision rejection with monotonic MTP capability through both the
prefix index and owner-rank residency, plus a shared no-write dedup/MTP
upgrade matrix in the prefix index, file writer, and journal replay.
The latest corrections also reject overlapping or out-of-bounds live
catalog extents during startup, resume allocation after physical EOF, and
enforce the same retain/upgrade/collision matrix directly at every
rank-residency registration boundary. Restore quota now remains owned by
queued/running physical work after response timeout or abandonment.
The retained HTTP path additionally dispatches exact request cancellation
when initial streaming headers fail, enforces an exact chunk-independent
32 KiB header bound, rejects already-buffered trailing request bytes, and
queues only sockets whose blocking-I/O bounds were installed.
The retained TP4 worker quota now remains owned by queued/running physical
steps through all four rank results and consensus after response timeout or
abandonment.
The retained pool constructor now waits for exact readiness receipts from
all four rank threads and synchronously joins/destroys partial startup state
instead of publishing a disconnected pool.
The retained HTTP server now synchronously joins connection workers after a
worker or accept-thread spawn failure, surfaces cleanup panics, and proves
its saturation rollback against barrier-held physical TP4 work.
The coordinator API backend now waits for a receipt from inside its runtime
thread before publishing production health and synchronously joins and
destroys the complete coordinator/TP4 ownership tree after a pre-ready
failure.
The distributed greedy CPU reference now rejects a globally all-masked
vocabulary row instead of turning four `-inf` rank winners into token zero,
while retaining legal rank-local masked partitions.
The retained durable store now also rejects a nonempty data file when the
journal contains no complete record, preventing total journal loss from
silently reopening as an empty cache.
It additionally rejects missing complete transaction groups whenever the
remaining records expose a skipped, decreasing, or non-`Begin` transaction
boundary.
The coordinator API backend now records authenticated cancellation in an
owner-bound coalescing registry rather than the bounded submission channel.
A queued request retains its marker until it becomes active, and the runtime
dispatches cancellation before admission polling or another scheduler step.
The active sequence table now consumes validated immutable prefix attachments
instead of page keys plus caller-supplied draft booleans. Namespace,
generation, target hashes, and optional draft hash follow the authoritative
restored tier record; stale or colliding upgrades fail atomically.
The serving coordinator now owns that table, reserves every selected batch
before rank submission, commits exact consensus counts, removes terminal
rows before releasing pins, and bounds MTP depth by the remaining generation
budget. A CPU boundary test accounts and releases all target and draft pages
at 1,048,576 positions without claiming model KV payloads.
Committed page mutation is now page-granular. A separate canonical page-table
delta carries sorted changed suffixes/removals, global and rank-local digests,
and reconstructs atomically in an arena-bounded independent CPU mirror.
Immutable `StepInput.v1` now binds the delta successor/digest to exact
row-ordered prompt IDs, configured/effective MTP posture, sampling float bits,
seed, RNG counter, context, and generation limits. Serving now constructs and
delivers it with one reservation delta to four persistent rank mirrors.
Admission/removal and post-output commit/rollback deltas receive exact
global/rank-local receipts before host publication.
Accepted tentative target/draft pages are now committed in place. Rejected
or removed IDs enter an owner-rank quarantine bound to one successor
generation and return to the allocator only after all four exact rank
receipts.
The fixed-capacity successor design is pinned in
`docs/fixed-page-transaction-v1.md`. It derives an exact 174-page edit bound
for C64/3,072-row prefill, a 128-page bound for MTP6 verification, separates
large admission/removal from the compute journal, and makes the standalone
four-rank delta command the cache-only ABI. It is a design candidate, not an
implementation or performance result.
The sustained serving qualification design is pinned in
`docs/sustained-serving-load-fault-v1.md`. It specifies one bounded
black-box Rust driver, deterministic open/closed-loop arrivals, multi-tenant
fairness, cancellation/slow-client/rank/tier/restart faults, raw per-request
evidence, the complete SM120 posture matrix, and a six-hour soak. No driver
or load result exists yet.
The NVFP4 canonical decoder correction is pinned in
`docs/nvfp4-metadata-canonicality-proof-v1.md`. It rejects resigned
rounding/dtype/global-scale-mode and reserved-field lies, inconsistent
amax/global-scale pairs, nonzero codes behind zero scales, and nonzero
value/scale padding while preserving partial-tile 2D scale repetition. It is
a CPU format result only. Independent review is requested by
`docs/fable-nvfp4-metadata-canonicality-v1-handoff.md`.
The production file-backed extension is pinned in
`docs/nvfp4-streaming-canonicality-proof-v2.md`. A bounded shared validator
now covers the in-memory decoder, streaming converter, and tentative direct
upload reader; it additionally enforces exact 2D scale replication. The v2
review is requested by
`docs/fable-nvfp4-streaming-canonicality-v2-handoff.md`.
The protected-precision converter padding correction is pinned in
`docs/plain-padding-streaming-proof-v1.md`. The in-memory decoder,
file-backed native reader, and streaming writer now share one offset-aware
chunk validator; conversion no longer allocates a complete tensor plane only
to inspect padding. Independent review is requested by
`docs/fable-plain-padding-streaming-v1-handoff.md`.
The new-write I/O correction is pinned in
`docs/streaming-write-single-pass-proof-v1.md`. Each source chunk is now
semantically validated before its single staging-file write, and final
cross-plane validation precedes pending descriptor insertion. Completed
descriptors are still hash-checked and semantically reread on resume. The
complete local gate passes 293 Rust tests; no conversion-speed claim is made
without a matched benchmark. Independent review is requested by
`docs/fable-streaming-write-single-pass-v1-handoff.md`.
The startup-order prerequisite is pinned in
`docs/normative-startup-order-proof-v1.md`. The retained Rust coordinator now
uses the exact engine-v0 sequence, places memory planning before weight load,
and fails terminally when the obsolete ordering is attempted. This is a CPU
state-machine result, not a real CUDA startup or checkpoint-load result.
Independent review is requested by
`docs/fable-normative-startup-order-v1-handoff.md`.
The deterministic M4 target is specified in
`docs/small-checkpoint-runner-v1.md`. It fixes the layer-6 NVFP4 laboratory
subset, exact rank-local tensor/byte inventory, captured M3 input boundary,
load/adoption/execution sequence, and fault/repetition evidence. It is a
design candidate only; no subset builder, rank files, device runner, logits,
or GPU evidence exists. Independent review is requested by
`docs/fable-small-checkpoint-runner-v1-handoff.md`.
The checkpoint-load CPU core is pinned in
`docs/checkpoint-load-cpu-core-proof-v1.md`. Candidate `6561492` implements
the exact plan, tensor-layout, prepared-receipt, prepared-set, adoption, and
execution-permit encodings; planned streaming into quarantined arenas; and an
adoption-bound `WeightsLoaded` startup transition. The exact candidate passes
309 tests and the complete local gate. This is not a native-rank plan
builder, CUDA upload, checkpoint smoke, device-residency, or production
health result.
The native-rank load-plan successor is pinned in
`docs/native-rank-load-plan-proof-v1.md`. Candidate `dfc1253` requires four
authenticated capacity-EXL3 readers, derives the common 59,585-entry semantic
catalog, and constructs each 81,605,027,840-byte weight plus 14,942,048-byte
metadata arena from the compiled checkpoint contract rather than
file-controlled plane lengths. The exact candidate passes 314 tests and the
complete local gate. Complete rank files, device allocation, CUDA upload,
checkpoint smoke, and GPU evidence remain absent.
The process-wide fault successor is pinned in
`docs/rank-set-load-coordinator-proof-v1.md`. Candidate `c331468` requires
all four prepared receipts and all four adoption acknowledgements, rejects
stale owner generations and duplicate/malformed messages, and emits one
common abort route for every rank position in preparation and adoption. The
exact candidate passes 318 tests and the complete local gate. Physical
thread cleanup, CUDA, full rank files, and checkpoint execution remain
absent.

The TP4 checkpoint-load transaction successor is pinned in
`docs/checkpoint-load-transaction-v1.md`. Candidate `e30017b` executes one
exclusive prepare, acknowledge, adopt, and finalize transaction across four
persistent rank workers, applies one common abort route on failure, and binds
all receipts to the plan, attempt, rank, and owner generation. Independent
review is requested by
`docs/fable-tp4-checkpoint-load-protocol-v1-handoff.md`.

The native rank adapter and one-call startup composition are pinned in
`docs/native-checkpoint-rank-adapter-proof-v1.md` and
`docs/native-checkpoint-startup-composition-v1.md`. Candidates `944d176` and
`83b8eff` put CUDA context, stream, allocation, bounded pinned-ring upload,
full device readback, and cleanup operations on the owning persistent rank
threads and expose only worker-observed identity to consensus. Independent
reviews are requested by
`docs/fable-native-checkpoint-rank-adapter-v1-handoff.md` and
`docs/fable-native-checkpoint-startup-composition-v1-handoff.md`.

The fail-closed load-only SM120 command is pinned in
`docs/native-checkpoint-load-smoke-proof-v1.md`. Candidate `1770563` binds a
complete measured profile budget, a reconstructed typed system-memory plan,
the operation manifest, compiled weight policy, linked native codec
capability, physical rank arenas, live per-device HBM, running executable,
and full rank payloads into one four-rank load. It adds normal all-rank
teardown with four exact cleanup acknowledgements and emits `summary.json`
only after all rank threads join. The exact host candidate passes 340 Rust
tests and all 87 review-handoff provenance checks. No `nvcc`, CUDA context,
cn4 access, checkpoint load, or model kernel was used; independent review is
requested by
`docs/fable-native-checkpoint-load-smoke-v1-handoff.md`.

This index separates proved results from preparation artifacts and missing
evidence. An entry here is not an acceptance token, GPU authorization, or
permission to convert a full checkpoint.

## Current local CPU/reference gate

The latest complete local run against native checkpoint-load smoke
implementation `1770563`
passed:

- `scripts/local-checks.sh` passes 340 Rust tests with zero failures,
  workspace formatting, Clippy with warnings denied, CUDA FFI type checks,
  deterministic proof regeneration, and all 87 candidate-based review-handoff
  hash proofs with 0/68 configured result artifacts;
- new cache regressions prove exact target/draft ID quarantine, wrong or
  missing generation rejection, mutation freeze while bound, post-receipt
  reuse, accepted-page identity preservation, and rejected-suffix retirement;
- new worker/serving regressions prove persistent four-rank mirror
  initialization, exact reservation and commit receipts, uninitialized/stale
  rejection, input-derived CPU output, exact serving-to-rank sampling/context
  delivery, and generation alignment after late publication rollback;
- new engine regressions cover exact prompt/delta hashing, configured MTP6
  tail fallback to effective MTP0, top-k route binding, output/context bounds,
  all three canonical sampling classes, and invalid float/filter rejection;
- cache regressions cover page-granular append equivalence, all 448
  tail/depth reservations, canonical tentative/admission/removal delta
  reconstruction, unchanged-prefix suffix omission, and fail-closed digest,
  owner, arena, generation, and no-op checks;
- serving regressions cover mandatory active-table admission, pre-worker
  capacity rejection, exact MTP0/MTP6-capable 1M-boundary accounting and
  cleanup, dynamic MTP tail selection, cancellation-before-peer cleanup, and
  authoritative restored-prefix attachment;
- review verifier v2 rejects handoff self-review and requires the exact
  candidate commit, every pinned SHA-256, and the declared result path before
  classifying a supplied token artifact as accepted; declared result files
  are automatically ingested by the repository-wide gate when present;
- deterministic cache-lifecycle proof covers three-page target+draft
  publication, torn-journal restart, MTP prefix reuse, bounded
  HBM/DRAM/NVMe pressure, pinning, COW/speculative transactions, cleanup, and
  corrupt-restore rejection;
- the external pinned-tokenizer proof was skipped because
  `GLMAXX_TOKENIZER_DIR` was unset; its checked fixture and implementation are
  unchanged from the earlier complete proof;
- platform: local CPU development host;
- CUDA compiler or GPU context: not used;
- kernel/device correctness, one-layer replay, model quality, and performance:
  not established.

Pinned inputs:

| Artifact | SHA-256 |
|---|---|
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |
| `fixtures/cache-lifecycle-proof-v1.json` | `c1151c34a3a9bee4fd97dea11e807603a56c2af4d37deab813cc9b5631177d6a` |
| `fixtures/cpu-serving-proof-v1.json` | `c95e1049bc52f8a8aaacd5a2d704008df9e8cfe72c8f3486982568adbaa7b47e` |
| `fixtures/engine-contract-proof-v1.json` | `a28686829ae46d62ab449eacae3a1b64bf965c43c22699bb4c9130ecedc9c1a2` |
| `fixtures/nvfp4-actual-shape-v1.json` | `56bca55ab3489fe6f50cd864f73a21f3b83367d79faa8bc70cb26f325f9b1099` |
| `fixtures/sm120-fc1-matrix-proof-v1.json` | `5ebf329ee29e4cd95e2c92a41a99625808dcf4212f996c874d651d637cdb6eef` |
| `fixtures/tokenizer-contract-proof-v1.json` | `bb0a29719ffc69e6676ac3edf156ea47ff6dc6e1424a0d866fbd5d2d76db5223` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `profiles/profile-budget-v0.json` | `cdbe4eaad9465181b2ba60b3656fe5207eee54467abfbf8d9bc398c3e68c23e0` |

The profile validates as arithmetic but remains
`conversion_allowed=false`.

The review-handoff verifier v2 contract, commands, and exclusions are pinned
in `docs/review-provenance-verifier-v2.md`. It validates candidate bytes,
declared result paths, candidate/input attestations, and exact review-token
presence; it does not accept any gate.

The discovered candidate-versus-current-build gap and its proposed v3
acceptance command are pinned in
`docs/current-tree-review-acceptance-v3.md`. The design would bind every
reviewed input to the current worktree before device inventory or conversion,
and would repair the stale Phase-C result path through a complete re-pin. It
is not implemented and its adversarial design token is absent.

The integrated cache lifecycle implementation and scope boundary are pinned
in `docs/cache-lifecycle-proof-v1.md`. Its deterministic fixture proves the
CPU file-store/prefix/residency/page-table lifecycle but does not qualify
CUDA, direct I/O, real NVMe, model attention, or long-context performance.

The indexer-key correction and exhaustive binary32 boundary proof are pinned
in `docs/indexer-key-scale-proof-v1.md`. It removes host-libm scale
construction and validation, and rejects records whose reconstructed key
would overflow. Its dedicated handoff passed local provenance validation;
independent acceptance is still absent.

The rank-set publication correction is pinned in
`docs/atomic-rank-publication-proof-v1.md`. Linux and Apple publication now
use their native atomic no-replace operation, while other platforms fail
closed. All 63 `glm-format` tests and workspace Clippy passed; its dedicated
handoff passed local provenance validation, but independent acceptance is
absent.

The target/draft KV finite-reconstruction correction is pinned in
`docs/kv-finite-reconstruction-proof-v1.md`. It rejects overflow from finite
record factors before non-finite cache values can become observable. Its
dedicated handoff passed local provenance validation; independent acceptance
is absent.

The asynchronous restore-identity correction is pinned in
`docs/restore-identity-proof-v1.md`. Pending pages now require exact request
ID, logical ordinal, and full durable-record equality before adoption. Its
dedicated handoff passed local provenance validation; independent acceptance
is absent.

The HBM residency admission correction is pinned in
`docs/residency-admission-atomicity-proof-v1.md`. It precomputes the complete
deterministic multi-victim plan and final counters before any target or
victim mutation, so pinned-capacity and arithmetic failures cannot leave
partial demotions. Its dedicated handoff passed local provenance validation;
independent acceptance is absent.

The captured-shape prefill correction is pinned in
`docs/prefill-captured-shape-proof-v1.md`. The scheduler now evaluates legal
profile entries and chunks work to the highest-work fitting shape instead of
stalling when configuration limits exceed every capture. The same proof
records the adjacent `GraphKey` inability to encode multiple prefill
prompt-row buckets under one sequence/transport key. The correction handoff
passes local provenance validation; independent acceptance and the separate
ABI extension are absent.

The proposed ABI correction is pinned in
`docs/prefill-graph-profile-abi-v2.md`. It replaces the mode-specific
verifier bucket with a mode-neutral row bucket, splits the 3,072-row prefill
limit from the 448-row decode/verify limit, and bumps both step-plan and
graph-profile identities while retaining the 85-byte hash-input layout. It
is a design candidate only; its dedicated handoff passes local provenance,
but independent design acceptance and all implementation evidence are
absent.

The scheduler batch-completion correction is pinned in
`docs/scheduler-batch-atomicity-proof-v1.md`. Request updates, cumulative
tenant service totals, completion binding, and decode-burst state are now
preflighted in fixed C64 arrays before the inflight step is removed. Its
forced late-overflow regression proves the exact batch remains retryable
with no partial row mutation. The dedicated handoff passes local provenance;
independent acceptance is absent.

The prefix-release correction is pinned in
`docs/prefix-release-atomicity-proof-v1.md`. Cache release now counts and
preflights every rank/page unpin before changing residency, while serving
retains the request lease and token reservation until cache release succeeds.
Its regressions distinguish both the prior partial-unpin ordering and the
prior lost-lease error path. The dedicated handoff passes local provenance;
independent acceptance is absent.

The selected-step failure correction is pinned in
`docs/selected-step-failure-finalization-proof-v1.md`. Once the scheduler
selects a batch, graph/compile/observation/submit/receive/output/completion
errors now finalize that exact batch as failed before fallible resource
cleanup. Forced compiler and bounded-worker saturation regressions prove the
next tick is idle rather than permanently blocked by stale inflight state.
The dedicated handoff passes local provenance; independent acceptance is
absent.

The multi-request cleanup correction is pinned in
`docs/terminal-cleanup-transaction-proof-v1.md`. Successful step events,
prompt accounting, prefix leases, and cumulative shared-page pin releases
are now fully preflighted before scheduler commit, then published through an
infallible commit. Failure and idle-cancellation cleanup use the same counted
plan. Its shared-prefix/late-corruption regression proves no earlier user is
partially released or published, and the exact C64/MTP6 boundary fits 512
fixed event slots. The dedicated handoff passes local provenance;
independent acceptance is absent.

The pending restore/admission rollback correction is pinned in
`docs/pending-admission-rollback-proof-v1.md`. Cancellation and poll failure
now preflight cumulative pin counts and exact request/ordinal-bound restores
before rollback mutation. A blocked poll rollback reinserts the exact pending
record, and serving retains its prompt tokens and byte reservation while the
cache still owns pending work. Multi-page corruption, serving-level repair,
and deterministic per-rank queue saturation regressions distinguish the old
lost-handle and partial-abort behavior. The dedicated handoff passes local
provenance validation; independent acceptance is absent.

The backend ownership correction is pinned in
`docs/backend-admission-rollback-fatal-proof-v1.md`. When a coordinator error
retains pending admission ownership, poll and cancellation now preserve the
backend active request, pending ID, and external tenant owner before
propagating a structured fatal result. The runtime drains active and queued
users and drops the coordinator rather than continuing with unattributed
cache work. Its real file-backed corruption/repair regression distinguishes
both prior lost-owner paths. The dedicated handoff passes local provenance
validation; independent acceptance is absent.

The backend event-cancellation correction is pinned in
`docs/backend-event-cancellation-fatal-proof-v1.md`. Admission, prefill, and
output mismatches plus slow-client, decoder-stop, and decoder-error paths now
preserve the exact active request and owner until coordinator cancellation is
accepted. Immediate cancellation rejection becomes a structured fatal drain
instead of ignored continuation. Its deterministic generation-overflow
regression distinguishes the prior lost-request path. The dedicated handoff
passes local provenance validation; independent acceptance is absent.

The active-sequence removal correction is pinned in
`docs/sequence-removal-atomicity-proof-v1.md`. The clone-on-error CPU page
table now restores the sequence, physical references, prefix mappings, and
target/draft free sets together when a late page release fails. Its
two-owner-page corruption/repair regression distinguishes the old
remove-before-release path and proves exact retry. The dedicated handoff
passes local provenance validation; independent acceptance is absent.

The durable-store write correction is pinned in
`docs/durable-store-write-fail-stop-proof-v1.md`. Request and extent
validation now complete before journal mutation, while any error after that
boundary poisons later writes until close/reopen replay. Regressions cover
begin-journaled, data-synced, and piece-journaled failures, prove no second
file mutation occurs, preserve earlier readable pages, and keep the failed
page invisible. The dedicated handoff passes local provenance validation;
independent acceptance is absent.

The durable-store ownership correction is pinned in
`docs/durable-store-single-writer-proof-v1.md`. A writable store now holds a
nonblocking exclusive journal lock for its lifetime, and rank restore workers
use read-only snapshot handles with no publication method. The regression
rejects a second writer, proves four concurrent readers restore identical
bytes, permits a later writer after snapshot construction, and proves those
readers cannot observe the later record. The dedicated handoff passes local
provenance validation; independent acceptance is absent.

The prefix-generation correction is pinned in
`docs/prefix-generation-integrity-proof-v1.md`. Same-key target and indexer
logical identities must match, two MTP records must also agree on their draft
sidecar, target-only records cannot downgrade MTP capability, and every
collision or reference overflow is rejected before mutation. The dedicated
handoff passes local provenance validation; independent acceptance is absent.

The cross-component prefix/residency correction is pinned in
`docs/prefix-residency-coherence-proof-v1.md`. Registration now uses the
post-insert index record as its sole residency authority, proves existing
index/rank agreement, plans all four rank changes before mutation, preserves
exact dedup and MTP capability, and rejects an unreconstructable prepopulated
index at coordinator construction. The dedicated handoff passes local
provenance validation; independent acceptance is absent.

The durable logical-content correction is pinned in
`docs/durable-content-dedup-proof-v1.md`. One validated relation now drives
prefix insertion, file-store preflight, and restart replay. Exact dedup makes
no write and retains the existing revision, only a strictly newer
target-only→MTP transition replaces a record, and target/indexer/draft
collisions fail before mutation and during recovery. It explicitly
supersedes the earlier candidate's same-content revision-refresh behavior.
The dedicated handoff passes local provenance validation; independent
acceptance is absent.

The journal-tail correction is pinned in
`docs/journal-tail-corruption-proof-v1.md`. Every complete 512-byte journal
record, including the last, must validate; only a genuinely short trailing
fragment is ignored. Corrupting a real final publish record now rejects both
writer and snapshot-reader construction instead of silently losing the page.
The dedicated handoff passes local provenance validation; independent
acceptance is absent.

The torn-tail resume correction is pinned in
`docs/torn-journal-resume-proof-v1.md`. After all complete records and replay
state validate, writable open truncates and syncs only the incomplete
fragment before resuming append. Read-only snapshots do not mutate it. The
regression publishes after repair and restores both old and new pages on a
second reopen; the lifecycle fixture is refreshed to the repaired journal
digest. The dedicated handoff passes local provenance validation;
independent acceptance is absent.

The quality source audit is recorded in
`docs/quality-corpus-manifest-v1.md` and
`manifests/quality-corpus-sources-v1.json`. It pins and byte-verifies the
ungated public sources and proves exact 1,000-item reasoning, 500-item coding,
and 500-item offline-tool selections. It is a design candidate, not the
materialized `corpus_manifest_sha256`: generated behavior/retrieval prompts,
gated FLORES+ content hashes, tokenized windows, evaluator code, and every
model result remain absent.

The new bounded native-rank reader CPU proof is pinned in
`docs/native-rank-reader-proof-v1.md`. It establishes one-pass file-backed
payload verification and four-rank semantic consensus, but explicitly excludes
actual full-rank evidence, CUDA upload, device residency, and checkpoint
startup.

The strict production-manifest extension is pinned in
`docs/production-rank-manifest-validation-v2.md`. It binds the reviewed
capacity-EXL3 manifest to native headers, descriptors, source provenance,
fixed rank-specific complete tensor contracts, the complete 92-file observed
source map, and compiled operation/weight-policy identity. Its
implementation review is requested in
`docs/fable-production-rank-manifest-validation-v2-handoff.md`; v1 was
superseded before review, and no acceptance
artifact or token is present.

## Historical cn4 preparation evidence

The newest checked-in cn4 preparation record is
`docs/cn4-review-fixes-preparation-20260729.md`, built from source
`c25e55843062dd777c4778a9f5d19cd9221a3278`.

Proved scope:

- 162 Rust tests passed;
- five CUDA translation units compiled and linked for `sm_120f`;
- the library contained five real `sm_120f` cubins and 256 expected
  block-scaled NVFP4 OMMA instructions;
- independent SFB and SFA layout probes passed;
- expected native symbols and Rust/native linkage were present.

Excluded scope:

- the container had no GPU access and did not create a CUDA context;
- no kernel was launched;
- no device-correctness, device-timing, profiler, collective, graph,
  one-layer, checkpoint, quality, capacity, or serving claim was made.

The record pins the container digest, toolchains, source state, artifact
hashes, and raw evidence hashes. The final read-only confirmation in
`docs/cn4-release-20260729.md` found an unrelated four-rank vLLM job occupying
95,496–95,632 MiB per GPU, no GLMAXX process, and no project-named shared
memory. It launched no CUDA work, disturbed no process, and immediately
released cn4. This repository currently has no authorization to reconnect or
launch work there.

## Adversarial gate state

The initial Fable v0.2 re-review accepted the revised specification; its
scope is documented in `fable-adversarial-v2.md` and
`docs/fable-v2-disposition.md`. It does not accept later implementations.

The first implementation reviews remain explicitly withheld:

| Gate | Review artifact | State |
|---|---|---|
| EXL3 source projection v1 | `fable-exl3-source-projection-v1.md` | token withheld |
| EXL3 warp-decode v2 | `fable-exl3-warp-decode-v2.md` | token withheld |
| NVFP4 manifest ABI v0.2.2 | `fable-manifest-abi-v022.md` | token withheld |

Corrective r2 handoffs are pinned at `0edfc8d`; no corresponding accepted
review artifact or token is present:

- `docs/fable-exl3-source-projection-v1-r2-handoff.md`;
- `docs/fable-exl3-warp-decode-v2-r2-handoff.md`;
- `docs/fable-manifest-abi-v022-r2-handoff.md`.

The following later CPU/control-plane candidates also await adversarial
verdicts:

| Candidate | Candidate commit | Handoff |
|---|---|---|
| step execution input | `a5ef076` with transaction amendment at `e7bc477` | `docs/fable-step-execution-io-v1-handoff.md` |
| active KV page table | `3404e07` | `docs/fable-active-sequence-page-table-v1-handoff.md` |
| cache arena budget | `c33648a` | `docs/fable-cache-arena-budget-v2-handoff.md` |
| serving page transaction | `e7bc477` | `docs/fable-serving-page-transaction-v1-handoff.md` |
| coordinator/API backend and fatal drain | `8aaef8e` | `docs/fable-coordinator-api-backend-v2-handoff.md` |
| serving observability | `9607aa0`, with backend lifecycle delta at `8aaef8e` | `docs/fable-coordinator-api-backend-v2-handoff.md` |
| online target/draft prefix publication | `d0a09d7` | `docs/fable-online-prefix-publication-v1-handoff.md` |
| distributed sampling and MTP RNG | `7c71818` | `docs/fable-distributed-sampling-abi-v1-handoff.md` |
| tenant/global serving resource quotas | `7e810c4` | `docs/fable-tenant-resource-quotas-v1-handoff.md` |
| nonblocking Linux HTTP transport | `3608a03` | `docs/fable-nonblocking-http-transport-v1-handoff.md` |
| direct DRAM/NVMe tier I/O | `69895e0` | `docs/fable-direct-tier-io-v1-handoff.md` |
| quality, KLD, task, retrieval, and MTP numerical acceptance | `70222ab` | `docs/fable-quality-acceptance-v1-handoff.md` |
| quarantined checkpoint load and four-rank adoption | `4bb0708` (r1 `737603b` superseded) | `docs/fable-checkpoint-load-transaction-v1-r2-handoff.md` |
| strict production rank-manifest validation v1, superseded | `46bff28` | `docs/fable-production-rank-manifest-validation-v1-handoff.md` |
| strict production rank-manifest validation v2 | `4bf7bb5` | `docs/fable-production-rank-manifest-validation-v2-handoff.md` |
| complete target-layer execution design | `83f5005` | `docs/fable-target-layer-execution-v1-handoff.md` |
| recurrent MTP0–6 execution design | `fd80e16` | `docs/fable-mtp-layer-execution-v1-handoff.md` |
| Rust-owned SM120 rank executor design | `b64cb6d` | `docs/fable-sm120-rank-executor-v1-handoff.md` |
| quality corpus public sources and deterministic task selections | `83fb374` | `docs/fable-quality-corpus-sources-v1-handoff.md` |
| deterministic generated JSON, repetition, retrieval, and termination corpus | `27fa48e` | `docs/fable-generated-quality-corpus-v1-handoff.md` |
| bit-exact indexer-key scale and overflow rejection | `13f0c59` | `docs/fable-indexer-key-scale-v1-handoff.md` |
| atomic no-replace rank-set publication | `aaeffea` | `docs/fable-atomic-rank-publication-v1-handoff.md` |
| finite target/draft KV and indexer reconstruction | `757d5cf` | `docs/fable-kv-finite-reconstruction-v1-handoff.md` |
| current-tree-bound review acceptance and qualification re-pin | `60311cf` | `docs/fable-current-tree-review-acceptance-v3-handoff.md` |
| exact asynchronous restore request/result identity | `dc16273` | `docs/fable-restore-identity-v1-handoff.md` |
| all-or-nothing HBM residency admission | `c84da2a` | `docs/fable-residency-admission-atomicity-v1-handoff.md` |
| captured-shape prefill progress | `9bdb208` | `docs/fable-prefill-captured-shape-v1-handoff.md` |
| prefill row-bucket and graph-profile ABI v2 design | `9b04652` | `docs/fable-prefill-graph-profile-abi-v2-handoff.md` |
| all-or-nothing scheduler batch completion | `2f7d0ce` | `docs/fable-scheduler-batch-atomicity-v1-handoff.md` |
| all-or-nothing prefix release | `14b97a2` | `docs/fable-prefix-release-atomicity-v1-handoff.md` |
| selected-step failure finalization | `2ff0ac1` | `docs/fable-selected-step-failure-finalization-v1-handoff.md` |
| multi-request terminal cleanup transaction | `6535248` | `docs/fable-terminal-cleanup-transaction-v1-handoff.md` |
| retryable pending restore/admission rollback | `bfbe7f4` | `docs/fable-pending-admission-rollback-v1-handoff.md` |
| backend retained-admission fatal drain | `3ab3110` | `docs/fable-backend-admission-rollback-fatal-v1-handoff.md` |
| backend event-cancellation fatal propagation | `0f0dd21` | `docs/fable-backend-event-cancellation-fatal-v1-handoff.md` |
| atomic retryable active-sequence removal | `876e4ca` | `docs/fable-sequence-removal-atomicity-v1-handoff.md` |
| fail-stop durable-store writes after uncertain publication | `a5019aa` | `docs/fable-durable-store-write-fail-stop-v1-handoff.md` |
| exclusive durable writer and read-only restore snapshots | `535a8d6` | `docs/fable-durable-store-single-writer-v1-handoff.md` |
| same-key prefix logical integrity and monotonic MTP capability | `2e3aa22` | `docs/fable-prefix-generation-integrity-v1-handoff.md` |
| prefix index and owner-rank residency generation coherence | `72e6071` | `docs/fable-prefix-residency-coherence-v1-handoff.md` |
| no-write durable dedup, MTP-only upgrade, and collision-safe replay | `b097703` | `docs/fable-durable-content-dedup-v1-handoff.md` |
| complete journal-tail corruption fails closed | `8612ec3` | `docs/fable-journal-tail-corruption-v1-handoff.md` |
| validated torn-tail repair before resumed append | `8fb3adf` | `docs/fable-torn-journal-resume-v1-handoff.md` |
| live catalog bounds/overlap validation and physical-EOF append | `de2d43a` | `docs/fable-durable-catalog-extent-integrity-v1-handoff.md` |
| direct rank-residency dedup/MTP/content identity | `eceee04` | `docs/fable-rank-residency-content-identity-v1-handoff.md` |
| restore quota owned through physical operation completion | `95683d8` | `docs/fable-restore-operation-quota-v1-handoff.md` |
| retained HTTP parser bounds and streaming request ownership | `a7b1cc9` | `docs/fable-retained-http-request-ownership-v1-handoff.md` |
| TP4 quota owned through four-rank operation completion | `da46a30` | `docs/fable-tp4-step-operation-quota-v1-handoff.md` |
| synchronous exact four-rank startup and partial cleanup | `1eb8e1c` | `docs/fable-tp4-rank-startup-handshake-v1-handoff.md` |
| synchronous retained HTTP partial-start cleanup | `20c773c` | `docs/fable-retained-http-startup-cleanup-v1-handoff.md` |
| backend runtime readiness and synchronous pre-ready cleanup | `5ff3d48` | `docs/fable-backend-runtime-readiness-v1-handoff.md` |
| distributed greedy rejection of a globally all-masked row | `7867ed2` | `docs/fable-distributed-greedy-all-masked-v1-handoff.md` |
| durable data rejected without complete journal history | `f724379` | `docs/fable-durable-journal-data-presence-v1-handoff.md` |
| durable journal transaction sequence continuity | `397c76c` | `docs/fable-durable-journal-transaction-sequence-v1-handoff.md` |
| queue-independent backend cancellation delivery | `2ace56c` | `docs/fable-backend-lossless-cancellation-v1-handoff.md` |
| active-prefix record and capability binding | `92568f6` | `docs/fable-active-prefix-record-binding-v1-handoff.md` |
| serving active-page admission, step, and cleanup transaction | `326158a` | `docs/fable-serving-active-page-transaction-v1-handoff.md` |
| canonical page-table delta and page-granular mutation | `a1d4cb4` | `docs/fable-page-table-delta-v1-handoff.md` |
| page-reuse quarantine and in-place tentative commit | `832bf97` | `docs/fable-page-reuse-quarantine-v1-handoff.md` |
| fixed-capacity page transaction design | `e1dd8d8` | `docs/fable-fixed-page-transaction-v1-handoff.md` |
| sustained serving load and fault qualification design | `1dbab21` | `docs/fable-sustained-serving-load-fault-v1-handoff.md` |
| atomic TP4 checkpoint load protocol | `d647535` | `docs/fable-tp4-checkpoint-load-protocol-v1-handoff.md` |
| persistent native checkpoint rank adapter | `b62325a` | `docs/fable-native-checkpoint-rank-adapter-v1-handoff.md` |
| one-call native checkpoint startup composition | `b55c8a9` | `docs/fable-native-checkpoint-startup-composition-v1-handoff.md` |
| fail-closed native TP4 checkpoint-load smoke | `1770563` | `docs/fable-native-checkpoint-load-smoke-v1-handoff.md` |

Handoffs contain requested tokens as instructions; that text is not an
acceptance result. Only a reviewer artifact with the exact full-line token
and matching input hashes may open its stated gate.

## Evidence not yet produced

There is currently no accepted artifact for:

- an SM120 kernel execution;
- actual-shape NVFP4 or EXL3 device correctness or timing;
- a TP4 PCIe collective matrix or graph capture;
- a complete GLM-5.2 sparse-layer replay;
- a checkpoint smoke or full-checkpoint residency;
- target-only MTP0 model logits or per-position KLD;
- MTP1–6 model equivalence or acceptance;
- live HBM/DRAM/NVMe KV movement;
- a live 1,048,576-token model request;
- sustained concurrent serving; or
- matched end-to-end performance against another runtime.

The exact blocking work and gate order are maintained in
`docs/production-punchlist.md`.
