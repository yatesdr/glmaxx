# Safetensors index `total_size` accounting v1

Date: 2026-08-03

Status: design candidate; implementation is blocked on adversarial acceptance.

## Problem

The Rust sharded-safetensors reader currently interprets
`metadata.total_size` only as the sum of tensor payload bytes. Real GLM-5.2
checkpoints on cn4 use two conventions:

| Checkpoint | Declared `total_size` | Tensor payload | Shard file bytes | Convention |
|---|---:|---:|---:|---|
| TR3 3.25 bpw | 339,069,245,936 | 338,954,037,248 | 339,069,245,936 | complete files |
| NVFP4/NF3 hybrid | 365,968,736,768 | 365,968,736,768 | 365,987,273,208 | tensor payload |

For TR3, the 115,208,688-byte difference is exactly the sum of every shard's
8-byte length prefix and padded JSON header. The index and all shard
inventories are otherwise internally consistent. Treating the value as an
unexplained mismatch rejects the real checkpoint; ignoring it would weaken
admission.

## Contract

For an indexed shard set, the reader computes with checked `u64` arithmetic:

- `actual_payload_bytes`: sum of every validated tensor descriptor's byte
  length;
- `actual_file_bytes`: sum of every uniquely named, already-open shard's file
  length; and
- `actual_container_overhead_bytes = actual_file_bytes -
  actual_payload_bytes`.

Every shard must first pass the existing safetensors rules: regular non-link
file, exact index/header tensor-name equality, supported dtypes, checked shape
arithmetic, in-bounds byte ranges, and complete contiguous data coverage. This
proves each shard length is its 8-byte prefix plus padded header plus tensor
payload.

If `metadata.total_size` exists, exactly one of these interpretations must
hold:

1. `tensor-payload`: declared value equals `actual_payload_bytes`; or
2. `complete-shard-files`: declared value equals `actual_file_bytes`.

Any other value fails closed with an error containing the declared, payload,
and complete-file totals. This is an accounting rule, not a checkpoint-name or
hash exception. It admits no tolerance, ratio, range, wildcard, or unchecked
producer metadata. When the field is absent, the reader records `unspecified`
and retains all structural validation.

The public inventory report records the declared value, both actual totals,
container overhead, and the selected interpretation. The existing
`declared_payload_bytes` name is removed rather than returning a file-total
under a false name.

## Integrity boundary

This contract proves structural byte accounting; it does not authenticate
payload contents. Production admission must separately verify the pinned
index, tier map, configuration, manifest, and every manifest entry. A stale
`.manifest_verified` marker has no authority. Directory inventory remains a
diagnostic path and cannot replace the pinned indexed source proof.

The accounting pass reads shard metadata and headers only. It must not reread
weight payloads or add weight I/O to cold start.

## CPU proof matrix

Tests must cover:

1. a payload-total index and a complete-file-total index over the same shards;
2. exact reporting of declared, payload, file, overhead, and interpretation;
3. values one byte below and above either valid total;
4. a value between the two totals and an overflow in aggregate accounting;
5. missing `total_size`, a non-`u64` value, duplicate index keys, extra or
   missing shard tensors, non-contiguous data, changed headers, and links;
6. deterministic results independent of index key order; and
7. real cn4 inventory of both named checkpoints from an immutable GLMAXX
   worktree, with the checkpoints mounted read-only and no CUDA device passed.

## Evidence provenance

The discovery run used source commit
`7ebc39cad3d26a4f0a41c029d192af0df48acc52`, container image
`sha256:2e401388dcc9c180401cb9997e3e8394c2db695c7bf4e3139ff8a9a517940719`,
and binary SHA-256
`14fec4f66c633ffac761b948f83bf82cc60025344eeed4f6fa794e75d7847f66`.
Raw JSON remains outside Git at
`/home/derek/glmaxx/evidence/20260803T124100Z-checkpoint-inventory-7ebc39c`.
Its non-empty records hash as:

- TR3 directory inventory:
  `9c431b33569d7c83edc5212bf8376324690f12fb111bd8b25c62225be9b349d5`;
- hybrid indexed inventory:
  `e9d23e55b59123115faac2c749164b6d6ed8a5b63cb4efa0546b9e4cb2a22f8d`.

The run was CPU-only, used only `~/glmaxx/`, launched no CUDA work, and did not
touch any vLLM worktree, process, container, image, cache, volume, or result.
