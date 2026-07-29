# Results index

Date: 2026-07-29

Current implementation baseline:
`4bf7bb5e817e01cc299058b56a488b35011fd79d`

The complete local gate most recently ran against implementation commit
`4bf7bb5e817e01cc299058b56a488b35011fd79d`. Subsequent proof and handoff
documents do not change the implementation candidate.

This index separates proved results from preparation artifacts and missing
evidence. An entry here is not an acceptance token, GPU authorization, or
permission to convert a full checkpoint.

## Current local CPU/reference gate

The implementation remains `4bf7bb5`; the latest local run at rank-executor
candidate `b64cb6d` plus its handoff/status delta passed:

- `scripts/local-checks.sh`: 226 Rust tests, workspace formatting, Clippy with
  warnings denied, CUDA FFI type checks, deterministic proof regeneration,
  and all 27 candidate-based review-handoff hash proofs;
- review verifier v2 rejects handoff self-review and requires the exact
  candidate commit, every pinned SHA-256, and the declared result path before
  classifying a supplied token artifact as accepted; declared result files
  are automatically ingested by the repository-wide gate when present;
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
| `scripts/local-checks.sh` | `378a717bc9d592bac68ac999c6a91d4655b633177a9005529288096a3d2a029b` |
| `fixtures/cpu-serving-proof-v1.json` | `fb76dd1cdc83501ff35ef192dc2be012b5e5cc52ced9a7f8ff4b0b1313698db1` |
| `fixtures/engine-contract-proof-v1.json` | `a28686829ae46d62ab449eacae3a1b64bf965c43c22699bb4c9130ecedc9c1a2` |
| `fixtures/nvfp4-actual-shape-v1.json` | `56bca55ab3489fe6f50cd864f73a21f3b83367d79faa8bc70cb26f325f9b1099` |
| `fixtures/sm120-fc1-matrix-proof-v1.json` | `5ebf329ee29e4cd95e2c92a41a99625808dcf4212f996c874d651d637cdb6eef` |
| `fixtures/tokenizer-contract-proof-v1.json` | `bb0a29719ffc69e6676ac3edf156ea47ff6dc6e1424a0d866fbd5d2d76db5223` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `profiles/profile-budget-v0.json` | `cdbe4eaad9465181b2ba60b3656fe5207eee54467abfbf8d9bc398c3e68c23e0` |

The profile validates as arithmetic but remains
`conversion_allowed=false`.

The review-handoff verifier contract, implementation hashes, commands, and
exclusions are pinned in `docs/review-provenance-verifier-v1.md`. It validates
candidate bytes and exact review-token presence; it does not accept any gate.

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
