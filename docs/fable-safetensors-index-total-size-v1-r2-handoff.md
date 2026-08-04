# Fable handoff: safetensors index `total_size` accounting v1 r2

Date: 2026-08-03

Status: corrective adversarial design review requested

GPU authorization conveyed by this handoff: none

Read-only cn4 access is allowed only to stat the named shard files and read
the index bytes, each eight-byte safetensors prefix, and exactly the padded
JSON header selected by that prefix. Do not read tensor payload bytes, create
a CUDA context, start a container, or write on cn4.

The only authorized real sources are:

| Source | Canonical index path | Index SHA-256 |
|---|---|---|
| TR3 3.25 bpw | `/home/derek/models/GLM-5.2-EXL3-TR3-3.25bpw/model.safetensors.index.json` | `f5dcd976a64ca70808dd4d8bd3ad07e9610c8ca6c30e3a6ed77ddefdac4c1d21` |
| NVFP4/NF3 hybrid | `/home/claude/LLM/GLM-5.2-hybrid/model.safetensors.index.json` | `6eb773222d932418dd0530c63aca498f86ef424da2a4526ccba76b59726da234` |

Withhold the token if either identity differs. Resolve only the exact safe
relative shard names in each pinned index. The permission to read padded JSON
headers exists solely to rederive structural accounting; it does not permit a
payload read or authenticate either checkpoint.

Review candidate commit:
`1e2d0e2f10a363c2c3cdb79b73c419d49f5b10e2`

Required result path:
`fable-safetensors-index-total-size-v1-r2.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`safetensors-index-total-size-v1-r2-design-accepted`

## Provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Withhold the token for any mismatch or incomplete input.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `docs/safetensors-index-total-size-v1-r2.md` | `6ec170c93c612d65866dcaec8555637cfb6763246bb45e78e2cf931109e2581c` |
| `docs/safetensors-index-total-size-v1.md` | `350bbf1c52b7065276933cb97011930d9b3404c67eb3cb088303670bc38e66f6` |
| `docs/fable-safetensors-index-total-size-v1-handoff.md` | `a7937fdaf98a3e5a6a6fe43c588a54594b87e6a016b8cc709c6aa4b94fb13e10` |
| `crates/glm-format/src/safetensors.rs` | `4a7d8d4a2121a2257a5e8b7ec531c98b4b83bddb6ea140ade697088a05009594` |
| `crates/glm-format/src/lib.rs` | `27aa8052ce18423b66bebe86ddbaafecfbaab989be661ab58c823e692b5d6c3d` |
| `crates/glm-format/src/checkpoint.rs` | `08450f0cb33e592ec76dfbe655b06580ba1743e60f3109a65218d052dbea406c` |
| `crates/glm-cli/src/main.rs` | `381a74d0ef7311a95a2c5996be80b39eb76442489edcaf8a2f934beaf00cf518` |
| `docs/checkpoint-ingest.md` | `186ce5985ca1adbf280a011a7692ae07780736f842c786dccb599ed8a458d07d` |

Run the complete CPU-only local gate and record its exit status:

```text
./scripts/local-checks.sh
```

## Required independent work

Recompute both real rows from index metadata, unique shard file lengths, and
eight-byte-prefix/header lengths. Inspect the pinned parser's exact ordering
and every caller of the field being replaced. Then answer:

1. Are the exact TR3 totals 339,069,245,936 declared/file,
   338,954,037,248 payload, and 115,208,688 overhead across 81 shards?
2. Are the exact hybrid totals 365,968,736,768 declared/payload,
   365,987,273,208 file, and 18,536,440 overhead across 184 shards?
3. Does complete contiguous per-shard coverage prove that file minus payload
   is exactly the sum of prefixes and padded headers, making the two valid
   interpretations mutually exclusive?
4. Do identical pathname/open-descriptor fingerprints before prefix read and
   after header validation close replacement, resize, and concurrent-header
   mutation during structural accounting without reading payload bytes?
5. Does the typed record report declared, payload, file, overhead, and one
   unambiguous interpretation without preserving the misleading
   `declared_payload_bytes` name?
6. Do absent index metadata and directory inventory both report
   `None/Unspecified`, while every present non-`u64`, duplicate, or mismatched
   declaration fails closed with useful typed diagnostics?
7. Are totals and interpretation order-independent while the raw index digest
   correctly remains serialized-byte-sensitive?
8. Does the contract preserve exact index/header inventory validation, checked
   arithmetic, safe path/link handling, and all current open-descriptor
   anti-TOCTOU guarantees?
9. Is publisher-manifest and payload authentication still separate and
   mandatory, with no authority granted to directory inventory or
   `.manifest_verified`?
10. Does the CPU/real-source matrix cover arithmetic boundaries, parser
    mutations, source-fingerprint races, both real conventions, and
    metadata-only cold-start behavior before checkpoint admission?
11. Does r2 unambiguously supersede v1 so its old handoff/token cannot open
    implementation?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
answer every decision with an unqualified `YES` or `NO`. Only if every answer
is `YES`, attest the candidate and all nine exact input hashes, then end with
the requested token as the only bare acceptance line.

Acceptance opens only the CPU parser/accounting implementation and a separate
real metadata-only admission proof. It does not accept that implementation,
authenticate either checkpoint, authorize conversion or CUDA, or establish
checkpoint, quality, capacity, cold-start, or performance evidence.
