# Safetensors index `total_size` accounting v1 r2

Date: 2026-08-03

Status: corrective design candidate; implementation is blocked on adversarial
acceptance

## Supersession and real conventions

This document replaces `docs/safetensors-index-total-size-v1.md`. The original
arithmetic is correct, but its public directory semantics and source-stability
boundary are incomplete.

Read-only prefix and file-size enumeration independently reproduced both real
GLM-5.2 conventions:

| checkpoint | shards | declared | payload | complete shard files | prefix + headers | interpretation |
|---|---:|---:|---:|---:|---:|---|
| TR3 3.25 bpw | 81 | 339,069,245,936 | 338,954,037,248 | 339,069,245,936 | 115,208,688 | complete shard files |
| NVFP4/NF3 hybrid | 184 | 365,968,736,768 | 365,968,736,768 | 365,987,273,208 | 18,536,440 | tensor payload |

No checkpoint name, hash exception, tolerance, ratio, range, or wildcard is
needed to represent those two producer conventions.

## Stable structural accounting

For every unique shard named by an index, the reader must:

1. reject a symlink, non-regular file, hard link, unsafe path, two distinct
   shard names that resolve to one device/inode, or duplicate tensor key;
2. capture device, inode, length, modification time, and change time from both
   the pathname and opened descriptor before reading the prefix;
3. read only the eight-byte length prefix and padded JSON header;
4. validate supported dtype, checked shape/byte arithmetic, exact index/header
   tensor-name equality, in-bounds ranges, and complete contiguous data
   coverage; and
5. recapture the pathname and descriptor fingerprints after validation and
   require exact equality with both initial fingerprints.

The totals use checked `u64` arithmetic over those stable opened descriptors:

```text
actual_payload_bytes = sum(validated tensor descriptor byte lengths)
actual_file_bytes = sum(unique verified shard fingerprint lengths)
actual_container_overhead_bytes = actual_file_bytes - actual_payload_bytes
```

For each shard, contiguous coverage proves that payload bytes equal
`file_bytes - (8 + padded_header_bytes)`. Therefore aggregate container
overhead equals the sum of every eight-byte prefix and padded header. It is
strictly positive because every nonempty accepted shard has a prefix and a
header of at least two bytes. Payload and complete-file interpretations cannot
both match one declared value.

Any fingerprint change, subtraction/addition overflow, or mismatch fails
before an inventory is published. Production payload hashing later repeats
the existing descriptor/path fingerprint checks before and after each hash;
structural stability does not replace content authentication.

## Typed public result

Replace the misleading `declared_payload_bytes` field with one accounting
record:

```text
SafetensorsAccounting {
  declared_total_size: Option<u64>,
  actual_payload_bytes: u64,
  actual_file_bytes: u64,
  actual_container_overhead_bytes: u64,
  interpretation: Unspecified | TensorPayload | CompleteShardFiles,
}
```

For an indexed source:

- absent `metadata.total_size` yields `declared_total_size=None` and
  `Unspecified`;
- a present JSON value that is not an integer representable as `u64` fails;
- exact equality with `actual_payload_bytes` selects `TensorPayload`;
- exact equality with `actual_file_bytes` selects `CompleteShardFiles`; and
- every other value returns a typed `IndexTotalSize` error containing the
  declared, payload, file, and overhead totals.

Directory inventory has no producer index declaration. It must also return
`declared_total_size=None` and `Unspecified`; it may not fabricate a declared
payload value. It still reports the three actual totals after the same stable
shard validation. Directory inventory remains diagnostic and cannot become a
production checkpoint identity.

The index SHA-256, tensor inventory, and accounting record remain separate
public fields. Accounting totals and interpretation are key-order independent;
the raw index SHA-256 remains byte-sensitive and therefore changes when the
serialized index is reordered.

## Integrity and cold-start boundary

This contract proves stable structural byte accounting only. Production
admission separately authenticates the pinned index, tier map, configuration,
publisher manifest, and every manifest entry. A `.manifest_verified` marker
and directory inventory have no authority.

The accounting pass reads file metadata, prefixes, and headers only. It does
not read tensor payload bytes and adds no weight-payload I/O to cold start.

## CPU and real-source proof

Before any checkpoint or GPU rerun, tests must cover:

1. both interpretations over byte-identical synthetic shards;
2. exact typed reporting for indexed and directory sources;
3. absent, negative, floating, string, boolean, null, out-of-range, and
   duplicate `total_size` values;
4. one byte below/above both valid totals and every boundary between them;
5. checked payload, file, overhead, and aggregate overflow helpers;
6. duplicate index keys, duplicate shard aliases, extra/missing tensors,
   non-contiguous data, links, changed headers, and unsafe paths;
7. deterministic pre/post fingerprint mutation at each read boundary;
8. index-key and shard-order permutation invariance for accounting, while the
   raw index digest remains byte-sensitive; and
9. metadata-only admission of both real cn4 checkpoints from an immutable
   GLMAXX worktree with read-only mounts and no CUDA device passed.

The real proof must retain index identity, shard count, all four accounting
values, interpretation, per-shard prefix/header totals, command/tool/source
identity, before/after file fingerprints, and an artifact hash stream. It does
not authenticate contents or authorize conversion, CUDA, checkpoint smoke,
quality, capacity, or performance claims.
