# Native rank reader CPU proof v1

Date: 2026-07-29

Status: CPU/reference proof passed; device residency and checkpoint startup
remain open.

## Candidate

The proved implementation is commit
`ee9d16bc1d20d90aefaf1e45d0d5ecfbd8a99a23`.

| Input | SHA-256 |
|---|---|
| `crates/glm-format/src/native_reader.rs` | `ae3579593713d35f633fadd1fe326db0ba8bae6ffe3644643e73b3321a6a0b4c` |
| `crates/glm-format/src/container.rs` | `9445a87c37217abe3c1dcae4b4ebd637191d58a8045fb3c52f65407e8226a7d1` |
| `crates/glm-format/src/lib.rs` | `5b58dd408cb5160665ffe49d3628a54fcd08811727fce78502e670b61e4e7913` |
| `crates/glm-cli/src/main.rs` | `728ff73ebdb616b2e70ee9899a8206b696657e1584ac3d3d2adf3e52493c3a09` |
| `scripts/local-checks.sh` | `378a717bc9d592bac68ac999c6a91d4655b633177a9005529288096a3d2a029b` |

## Proved behavior

`NativeRankReader` opens an immutable rank image without reading its weight
payload into a `Vec`. Open-time validation covers:

- exclusive regular-file ownership, `O_NOFOLLOW`, and stable descriptor/path
  fingerprints;
- the fixed header, CRC32C, reserved fields, exact canonical control-region
  offsets, zero gaps, and rejection of trailing file bytes;
- all control-region SHA-256 values and writer-canonical JSON manifest bytes;
- exact descriptor, name, codec metadata, plain/NVFP4/EXL3 geometry, and
  canonical payload-plane placement;
- deterministic file UUIDs, four-rank conversion UUID derivation, rank order,
  common model/tokenizer/template/policy/kernel identities, and corresponding
  tensor semantics; and
- current-binary kernel ABI identity in the CLI proof.

Payload verification is one sequential pass in canonical tensor order. It
checks the aggregate payload hash, every primary/auxiliary hash, all alignment
padding, plain-tensor padding, NVFP4 scale encodings, and complete EXL3 source
planes. The streaming buffer is 8 MiB. EXL3 validation additionally retains
only the current projection's primary and auxiliary planes. The proof reports
the maximum payload-verification scratch observed per rank.

`RankTensorSink` is the future direct-upload boundary. Its bytes are
tentative: an implementation must keep the destination unreachable by
execution until the entire rank pass succeeds. A sink error aborts the proof.
The CLI verifies four ranks in parallel and emits a single result only after
all four payloads and the common rank-set contract pass.

The adversarial tests also prove fail-closed behavior for:

- payload corruption and appended bytes;
- re-signed noncanonical JSON and nonzero tensor padding;
- tensor-name and EXL3-projection divergence between ranks;
- hard-linked inputs; and
- an injected upload-sink failure.

The in-memory reference parser was tightened to reject nonzero header-reserved
bytes, noncanonical control-region starts, and trailing bytes as well.

## Commands and result

The complete local gate ran at the candidate commit:

```text
GLMAXX_TOKENIZER_DIR=/tmp/glmaxx-tokenizer.uQIinY \
  scripts/local-checks.sh
```

Result:

- 221 Rust tests passed;
- formatting and workspace Clippy with warnings denied passed;
- CUDA FFI type checking and Clippy passed using the local stub boundary;
- deterministic CPU, matrix, manifest, rank fixture, engine, serving, and
  tokenizer proofs matched their checked-in fixtures;
- all 20 candidate-based review handoffs hash-verified; and
- the local host had no CUDA compiler and no GPU was used.

For a published native rank set, the operational proof command is:

```text
cargo run -p glm-cli --release -- \
  native-rank-proof /external/native/capacity-exl3 \
  /external/evidence/native-rank-proof-v1.json
```

The directory must contain exactly `rank-0.g5n` through `rank-3.g5n`.

## Explicit exclusions and next gate

This is not a checkpoint smoke, device-load proof, or serving result. It did
not read an actual full native rank set, allocate CUDA memory, upload weights,
launch a kernel, or establish checkpoint quality or throughput. cn4 was not
accessed and no GPU work remains in flight.

Checkpoint item C11 therefore remains open. Its next implementation steps are:

1. run the proof over a complete externally stored rank set and record
   per-rank bytes, scratch, wall time, and storage throughput;
2. implement a quarantined pinned-host/CUDA upload sink whose arena becomes
   immutable only after the rank proof succeeds;
3. bind each verified rank image to its matching persistent rank worker and
   require four-rank startup consensus; and
4. pass a small-checkpoint device smoke before attempting full residency.
