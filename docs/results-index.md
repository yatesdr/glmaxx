# Results index

Date: 2026-07-29

Current CPU implementation baseline:
`dc16273b019cf3a3dd8eb810cf9caeb26c99bced`

The complete local gate most recently ran against the asynchronous
restore-identity implementation bytes committed as
`4b60040566d530ddd12d8a72e10adb9d0fcdb22f`; its provenance record was then
committed at `dc16273b019cf3a3dd8eb810cf9caeb26c99bced`. The target CUDA/kernel
and strict production-manifest baseline remains `4bf7bb5`; the later CPU
candidates add review integrity, cache-lifecycle evidence, bit-exact
indexer-scale handling, atomic publication, finite KV reconstruction, and
exact restore-result identity.

This index separates proved results from preparation artifacts and missing
evidence. An entry here is not an acceptance token, GPU authorization, or
permission to convert a full checkpoint.

## Current local CPU/reference gate

The latest local run at restore-identity candidate `4b60040`
passed:

- `scripts/local-checks.sh`: 232 Rust tests, workspace formatting, Clippy with
  warnings denied, CUDA FFI type checks, deterministic proof regeneration,
  and all 33 then-present candidate-based review-handoff hash proofs;
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
| `fixtures/cache-lifecycle-proof-v1.json` | `d8fb9505f7c2af912554d59f9dd3b84e32d5f2492fa8a3539a9ea908ae7d1dab` |
| `fixtures/cpu-serving-proof-v1.json` | `fb76dd1cdc83501ff35ef192dc2be012b5e5cc52ced9a7f8ff4b0b1313698db1` |
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
hashes, and raw evidence hashes. A final read-only inventory is recorded in
`docs/cn4-release-20260729.md`: it found an existing four-rank vLLM allocation
and no GLMAXX process, launched no CUDA work, and immediately released cn4.
This repository currently has no authorization to reconnect or launch work
there.

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
