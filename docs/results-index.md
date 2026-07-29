# Results index

Date: 2026-07-29

Current source baseline:
`d0a09d7c62f1943112eaa703a9ef3f6b25e9ebc9`

The complete gate ran against implementation commit `63b13fd`; `8aaef8e`
changes only the accompanying terminal-delivery wording. `d0a09d7` adds only
the unimplemented online-publication design candidate.

This index separates proved results from preparation artifacts and missing
evidence. An entry here is not an acceptance token, GPU authorization, or
permission to convert a full checkpoint.

## Current local CPU/reference gate

The complete local gate passed at the source baseline:

- `scripts/local-checks.sh`: 211 Rust tests, workspace formatting, Clippy with
  warnings denied, CUDA FFI type checks, deterministic proof regeneration,
  and the external pinned-tokenizer proof;
- platform: local CPU development host;
- CUDA compiler or GPU context: not used;
- kernel/device correctness, one-layer replay, model quality, and performance:
  not established.

Pinned inputs:

| Artifact | SHA-256 |
|---|---|
| `scripts/local-checks.sh` | `c3456173f504372c2ae2cd7dc391a8886ea838c2703a6ac38bfece47b426ebef` |
| `fixtures/cpu-serving-proof-v1.json` | `fb76dd1cdc83501ff35ef192dc2be012b5e5cc52ced9a7f8ff4b0b1313698db1` |
| `fixtures/engine-contract-proof-v1.json` | `a28686829ae46d62ab449eacae3a1b64bf965c43c22699bb4c9130ecedc9c1a2` |
| `fixtures/nvfp4-actual-shape-v1.json` | `56bca55ab3489fe6f50cd864f73a21f3b83367d79faa8bc70cb26f325f9b1099` |
| `fixtures/sm120-fc1-matrix-proof-v1.json` | `5ebf329ee29e4cd95e2c92a41a99625808dcf4212f996c874d651d637cdb6eef` |
| `fixtures/tokenizer-contract-proof-v1.json` | `bb0a29719ffc69e6676ac3edf156ea47ff6dc6e1424a0d866fbd5d2d76db5223` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `profiles/profile-budget-v0.json` | `cdbe4eaad9465181b2ba60b3656fe5207eee54467abfbf8d9bc398c3e68c23e0` |

The profile validates as arithmetic but remains
`conversion_allowed=false`.

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
hashes, and raw evidence hashes. cn4 has since been released to another
workload. This repository currently has no authorization to reconnect or
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
