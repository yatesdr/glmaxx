# Native-rank load-plan proof v1

Date: 2026-07-30

Status: implementation candidate; adversarial review pending

GPU claim: none

## Candidate

The proved source commit is
`dfc1253c961c1d5aeaf7b490842d4d7528f69bb1`.

| Input | SHA-256 |
|---|---|
| `crates/glm-engine/src/checkpoint_load.rs` | `dc0947540fd4e7d692ff2fe1222040536c68aecfb1a0bd0a8043e4bc34554a9e` |
| `crates/glm-engine/src/lib.rs` | `538fb1aabb3354d7b2dde4256ecd46ca7bcb7856e7bd8e3896520834f71c4959` |
| `crates/glm-format/src/container.rs` | `2fbe22c55d481e40699a02be9929f9a964f50bc08199853772dd314c85ade47f` |
| `crates/glm-format/src/lib.rs` | `c37aa179cc9cf56f64c57f7e8b50ba6b226eac74af58066630a53c700bb0f60a` |
| `crates/glm-format/src/native_reader.rs` | `89e61aff8541ddbb48fd28a2458b10bb3e90dcf2d654a2fb134217521a6fdc5e` |
| `crates/glm-format/src/rank_manifest.rs` | `cb57a81ad3643a86992c4fd9c2166e9ee238cc7e66063732355d14e485f73410` |
| `crates/glm-format/src/stream.rs` | `b6d7dae8adf6fbb7ebd0f08c79c3d7f9dbba6269408f6b760fa43b18028a22fb` |
| `docs/checkpoint-load-transaction-v1.md` | `79d9c376201f3540f247344c24c37dd7d819d629f10459899a73c15f8b27015f` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

## Implemented boundary

`build_rank_set_load_plan` now accepts exactly four opened
`NativeRankReader` objects and first executes the existing fail-closed
rank-set validation. The public path therefore requires unchanged regular
files, ranks zero through three, a common conversion identity, exact native
headers and descriptors, authenticated manifests, canonical codec metadata,
and matching rank semantics before a load plan can exist.

The production manifest validator retains an exact normalized semantic
record for every tensor. Each record uses the reviewed 128-byte
`TensorSemanticEntry.v1` encoding, fixed source-kind, reconstruction,
collective, and source-dtype IDs, tensor-ID order, and the domain-separated
catalog hash. Unknown enumeration values fail. Rank-set consensus now
compares the complete normalized catalogs as well as their reviewed manifest
identities.

The physical arena is not sized from rank-file claims. For the only supported
production profile, `capacity-exl3`, the planner constructs a process-common
physical contract from the compiled pinned GLM-5.2 rank plan. Each reader
descriptor must match that contract's tensor ID, role, codec, descriptor
flags, metadata length, primary length, auxiliary length, and 256-byte
alignment. Only the compiled contract is then used to assign destination
offsets. A private authenticated-source projection makes this planning logic
positively CPU-testable without exposing a public way to bypass
`NativeRankReader::validate_rank_set`.

The complete pinned contract has 59,585 tensors per rank. Its four normalized
semantic catalogs are identical and have SHA-256:

```text
bb08eef4631a430a4fe3660087c349927f25eb69281bde76d85a5f6eef222deb
```

The deterministic per-rank physical allocation is:

| Arena | Exact bytes |
|---|---:|
| weight primary/auxiliary planes plus alignment gaps | 81,605,027,840 |
| immutable codec metadata plus alignment gaps | 14,942,048 |

The rank-0 arena-layout digest is:

```text
140274b8d69521115e82ffe72b83af4018dc55c6e7ac7f6bb8ce5af8f81df039
```

Rank is included in the arena-layout hash, so ranks one through three have
distinct digests even though the current physical entry tables are
byte-identical. Every rank entry additionally binds its native file UUID,
manifest, descriptor, payload and rank-local tensor-contract hashes, exact
file payload extent, device identity, and both arena sizes.

The builder rejects a missing production manifest, unsupported or
environment-mismatched profile, rank drift, process-identity drift, tensor
count drift, semantic-catalog drift, descriptor-to-contract drift, invalid
alignment, arithmetic overflow, interval overlap, and an unaccounted arena
tail.

## Tests

The new focused tests prove:

- the exact 128-byte semantic-record encoding and domain-separated catalog
  preimage;
- all 22 frozen safetensors source-dtype IDs plus the distinct EXL3 ID;
- identical full 59,585-entry semantic catalogs for all four compiled rank
  plans and the pinned catalog digest;
- a deterministic successful four-source plan, including all common and
  rank-local identities;
- rejection of common identity, semantic, and profile drift;
- descriptor-to-compiled-contract matching;
- exact primary, auxiliary, and metadata destination projection;
- invalid tensor order and non-power-of-two alignment rejection; and
- the full actual-shape arena byte arithmetic and pinned layout digest.

At exact candidate `dfc1253c961c1d5aeaf7b490842d4d7528f69bb1`,
`scripts/local-checks.sh` passed:

- 314 Rust unit/integration tests with zero failures;
- workspace formatting;
- workspace Clippy and CUDA-FFI Clippy with warnings denied;
- CUDA-FFI host type checking;
- deterministic CPU, 135-case matrix, manifest, native-rank, memory-budget,
  ABI, engine, serving, and cache-lifecycle proof regeneration and byte
  comparison; and
- all 79 then-current review handoffs, with 0/60 configured result artifacts.

The external tokenizer proof was skipped because `GLMAXX_TOKENIZER_DIR` was
unset. The local host has no `nvcc`; no CUDA source was compiled or launched.

## Explicit remaining work

This proof does not contain four complete production `.g5n` rank files and
does not claim that an 81.6 GB arena was allocated. It does not prove:

- opening and streaming the complete four-rank capacity checkpoint;
- the rank-by-rank injected reader/upload fault matrix;
- a pinned host staging ring or fixed CUDA event ring;
- H2D completion, asynchronous error observation, gap initialization, or
  device-memory read-back/content verification;
- persistent owner-thread CUDA allocation, adoption, or destruction;
- NVFP4-laboratory or hybrid-serving manifest/load-plan construction;
- a small-checkpoint or fit-capable checkpoint smoke;
- production `Healthy`, graph capture, KV initialization, or collectives; or
- SM120 correctness, capacity, quality, or performance.

The immediate CPU successor is the full four-rank fault coordinator. The
device successor remains the reviewed pinned-host/event-ring writer plus
full first-load device-content verification, owned by each persistent rank
thread.
