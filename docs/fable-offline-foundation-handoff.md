# Fable handoff — offline serving foundation

Date: 2026-07-29

Base commit:
`8eca10a5b69581ce7da4316f9737de0de9c609e8`

Review scope: the CPU/offline workstreams requested after the Phase-A engine
contract. cn4 was used for a no-device compile and test reproduction; no GPU
kernel execution occurred.

## Candidate hashes

Hash these files again at review start and finish:

| File | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `spec/format-v0.md` | `ab6066d5a9ebab8fbc42966f4193f867b7c2c7cec10ec8a1e8be17fcf0f470f6` |
| `docs/native-engine-plan.md` | `9662c82bea15c7b336ac3efa41a12a1e627a511a5f8da603ba466d5bcb6ae036` |
| `docs/exl3-trellis-cpu-contract.md` | `7c5bfa0795aa6b78646b00b029f8d4edc7c15491d69ea19e423f770701b5cdc3` |
| `docs/offline-serving-foundation.md` | `576932d119332774df2a8a65d85a4f185e804504acca949561689f03b02f28ae` |
| `docs/checkpoint-ingest.md` | `cad2020ffe5e87c423cacd7aee5f3e3d251366126adad7cc13b7c8968ece1b79` |
| `crates/glm-format/src/exl3.rs` | `6ba46fa98979711f5ecaaccf9be150045de768648e2a31fb5fbf49d56d95bbe4` |
| `crates/glm-format/src/container.rs` | `c10d36c8a130647a22fc36a59d31337628f40f6e48bfacd916823d3a3f66772a` |
| `crates/glm-format/src/safetensors.rs` | `a0a026cd76dcd5597e34d44362fac96b44bd7715ea638c60b0db91503a7b6d75` |
| `crates/glm-format/src/stream.rs` | `3e496ebe852f453597bd022a74381d4a1a979a9711fbf216559e71f6fe02f4d2` |
| `crates/glm-engine/src/weight.rs` | `d658cefefc17757a28258bafd0e13f5309e8adcbf2b30c4d2bdc97be9899ca19` |
| `crates/glm-engine/src/startup.rs` | `9634f120a2e01f21aaa5778954053d9a06f1e8d2af6c5abe1f9c6e4cbbd31e87` |
| `crates/glm-scheduler/src/lib.rs` | `b6ec68d37d87ba64dead63444972b6334218d6a7fbeceb4e7f685162bf2b43e8` |
| `crates/glm-reference/src/sampling.rs` | `6d90f43dbf0d2865ef63c001601c9084d6370c168056f883f49ffe7c732d9d13` |
| `crates/glm-reference/src/routed_fc2.rs` | `4f34f5b89cd542f096269a7442da5289900d1e831e32fcf83462560dc410a40d` |
| `crates/glm-cache/src/prefix.rs` | `2334d68914bf01ce1432bd7e4d07500ea9fc374e027deedeeb71972cc514fe68` |
| `crates/glm-cache/src/tier.rs` | `2730d829c8538e7b10649e0fba6504ee3389adc21c2f557e474a93c6dbee4f97` |
| prior `fable-adversarial-v2.md` | `f0019b96d5b35bdca6d026691629b56fbeb0c3c4528e1ae4ff9c1aa06817953e` |

## What changed

1. The pinned EXL3 MCG source payload, bit extraction, lane scatter, FP16
   decode, and H128 path are implemented in Rust.
2. One real 1,192,964-byte checkpoint projection was reconstructed by Rust
   and an independent NumPy oracle to the same FP16 byte digest.
3. Full weight policies enumerate every `(layer, expert, projection role)`,
   optionally including draft layer 78, with immutable codec, bytes, and
   quality digest. Protected tensors are separate typed allocations.
4. A bounded-channel four-worker startup simulation fails the process on
   rank, stage, or immutable-digest disagreement.
5. A new scheduler crate batches multiple users for prefill, MTP0 decode, or
   one MTP depth; it validates captured shapes, tenant limits, fairness, and
   step-boundary cancellation.
6. Sampling covers greedy, bounded top-k/top-p, the unbounded distributed
   mass case, residual sampling, and explicit RNG counter tickets.
7. Prefix and tier code provides chained full-page keys, DCP-neutral
   namespaces, exact target/indexer/draft pieces, atomic multi-page insertion,
   and recoverable publish-after-durable journaling.
8. FC2 and sparse-layer code fixes activation order, route weighting after
   down projection, shared/routed combination, TP4 reduction order, target
   descriptors, and the full layer-78 draft descriptor.
9. Rank containers now preserve the pinned EXL3 trellis as the primary I16
   plane and marker/rotations as aux, validate all source arithmetic, reject
   unknown codecs and false direct-layout flags, and expose only an
   inspection/CPU-proof API.
10. A strict Rust safetensors reader validates single-file and standard
    sharded checkpoint structure, exposes bounded positional reads and
    content hashing, and imports the canonical four EXL3 components at actual
    GLM-5.2 shapes without a framework or intermediate representation.
11. The native container now carries protected BF16, FP16, and FP32 tensors
    with exact multidimensional shapes, canonical zero padding, and absent
    auxiliary planes.
12. The production writer streams every plane through an 8 MiB buffer,
    durably checkpoints completed descriptors, validates hashes on resume,
    rejects changed or malformed partial outputs, and emits byte-identical
    files to the in-memory oracle.
13. Safetensors discovery also supports strict flat per-layer shard
    directories, rejects duplicate tensor names and symlinks, and computes a
    deterministic structure digest.

## Reproduction

The complete local gate passes:

```text
./scripts/local-checks.sh
```

The workspace currently has 138 passing tests. Clippy passes with warnings
denied. The real EXL3 source proof is external-data dependent:

```text
cargo run -p glm-cli -- exl3-proof /external/path/source.payload
cargo run -p glm-cli --release -- \
  safetensors-inventory /external/model
cargo run -p glm-cli --release -- \
  exl3-safetensors-proof \
  /external/model 3 0 0 gate
```

Pinned real-payload result:

```text
payload SHA-256:
68e96700af31debf63c42be271595df75c523f40177e6b6f48c0bab4b24a0ec4

reconstructed FP16 SHA-256:
a13c295c381993da35eaef392c412024e70dd3d80c28612f71fb24cd17a74d13
```

No weights or raw benchmark evidence are in Git.

## Requested adversarial questions

1. Is the Rust MCG extraction/scatter and FP16/H128 execution definition
   sufficient to close format section 17 items 1–5 for this pinned source?
2. Does the 96-byte deterministic native EXL3 metadata omit any field needed
   to prevent interpreting the same payload under a different codebook,
   shape, rank, or projection?
3. Does the policy inventory correctly cover target layers 3–77 and optional
   full attention-plus-MoE draft layer 78, with the required
   `(layer,expert,tensor_role)` identity?
4. Can any startup failure or digest disagreement leave a subset of ranks
   healthy or able to enter a collective?
5. Can any scheduler path starve a tenant, mutate a cancelled shared prefix,
   or admit an uncaptured MTP shape?
6. Are top-k/top-p filtering, FP32 normalization, rank ordering, exact ties,
   residual mass, and counter advancement consistent with engine section
   15.5?
7. Can a partial/duplicate/corrupt DRAM or NVMe record become visible after
   crash replay, or can a late insert failure partially mutate prefix
   reference counts?
8. Are the FC2 activation ordering, route-weight placement, shared-expert
   addition, two TP4 reductions, and residual boundaries faithful to the
   pinned model?
9. Which of these candidates may enter the SM120 qualification path, and
   which require another CPU correction first?
10. Does the source-container split preserve exact EXL3 component semantics
    while remaining fail-closed against serving/GPU health claims?
11. Can the safetensors parser, index reconciliation, shape/dtype checks, or
    positional read path accept an ambiguous, truncated, overlapping,
    unmapped, path-traversing, or same-header/wrong-data EXL3 source while
    reporting it as proven?
12. Can the resumable writer publish a descriptor before its payload is
    durable, accept changed completed bytes, conceal nonzero padding, or emit
    bytes different from the deterministic in-memory oracle?
13. Do plain protected tensors or flat shard-directory discovery introduce
    an ambiguous dtype, rank, shape, empty-plane, duplicate-name, or symlink
    interpretation?

## Explicit non-claims

- no EXL3 or new NVFP4 SM120 execution;
- no CUDA graph capture or real four-process worker;
- no measured fairness/SLO tuning;
- no filesystem I/O, fsync, io_uring, or GDS implementation;
- no complete-model conversion or atomic four-rank directory publication;
- no model logit/quality result;
- no serving API or end-to-end throughput claim.
