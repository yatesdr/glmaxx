# Safetensors index `total_size` accounting v1 r3 amendment

Date: 2026-08-04

Status: corrective design candidate; implementation is blocked on adversarial
acceptance

Base contracts:

- `docs/safetensors-index-total-size-v1.md`
- `docs/safetensors-index-total-size-v1-r2.md`

## Scope and precedence

R2 correctly defines the two real `metadata.total_size` conventions and the
typed accounting result, but its statement that any source change fails before
inventory publication is stronger than its per-shard transaction. A shard can
change after its own post-header check while another shard is being opened.
Directory mode also has no final exact-membership check. Arbitrary mutable
regular files cannot be made into one atomic multi-file snapshot by sequential
`stat` calls.

This amendment makes the boundary executable. It retains every r2 arithmetic,
parser, inventory, API, cold-start, and real-source rule, and adds an exact
publication sweep plus a revocable retained-source lease. It does not claim an
instantaneous cross-file snapshot. R3 supersedes the r1 and r2 handoffs for
implementation authority; an older result is review history only.

## Exact parser boundary

The index is UTF-8 JSON with no BOM or trailing non-whitespace bytes. A custom
visitor rejects duplicate top-level keys, duplicate metadata keys including
`total_size`, duplicate weight-map keys, unknown top-level keys, and a missing
or empty `weight_map`. `metadata` may be absent. If present, it must be an
object; its non-`total_size` values remain ignored metadata, but duplicate keys
are never collapsed before validation.

A present `total_size` must be a JSON integer representable as `u64`. Negative,
fractional, exponent-spelled noninteger, string, boolean, null, and out-of-range
values fail. Equality with exactly one of the checked actual payload or complete
shard-file totals selects the r2 interpretation. Absence selects
`None/Unspecified`; no producer or path identity participates.

## Retained source transaction

The indexed transaction is:

1. reject an index symlink, non-regular file, or link count other than one;
2. open the index read-only with close-on-exec and no-follow semantics, capture
   pathname and descriptor fingerprints, and require exact equality;
3. read exactly the captured index length, parse it under the rules above, and
   retain the descriptor and fingerprint;
4. resolve only safe relative shard names beneath the index parent;
5. open every unique shard, possibly in parallel, with the same no-follow,
   pathname/descriptor, regular-file, and single-link rules;
6. for each shard, capture the matching pre-read fingerprint, read only the
   eight-byte prefix and padded header, validate the complete r2 descriptor and
   coverage rules, recapture pathname and descriptor fingerprints, and require
   all observations to equal the retained fingerprint;
7. reject two different relative shard names that name the same device/inode,
   then compute the checked accounting record; and
8. run the publication sweep below before returning the inventory.

Every accepted source remains represented by the already-open descriptor, its
safe relative path, and its complete `(device,inode,length,mtime,ctime)`
fingerprint. No later payload operation reopens a shard as its data source.

## Publication sweep and lease semantics

Immediately before returning an indexed inventory, validate the retained index
and every retained shard in canonical relative-path order. Each validation
requires the current pathname and descriptor to remain regular, single-linked,
and fingerprint-equal to the retained observation. Any failure discards the
whole candidate inventory.

This sweep detects a source changed after its local header transaction but
before its position in the sweep. It is deliberately not described as an
atomic snapshot: a source can change after its sweep position. Instead, the
returned `ShardedSafetensors` is a revocable retained-source lease:

- `revalidate_sources()` checks the index and complete shard set in the same
  canonical order;
- every tensor read, tensor-reader construction, shard hash, and index hash
  validates its retained source before access and after the complete access;
- a streaming reader validates its retained descriptor at construction, first
  read, and terminal read; it reads only that retained descriptor, so pathname
  replacement cannot redirect bytes to a new inode;
- conversion/load admission calls `revalidate_sources()` immediately before
  publishing any native rank set or resident arena; and
- the first mismatch permanently fails that operation. No retry, path reopen,
  partial inventory, or rank-local continuation is permitted.

Production checkpoint admission additionally requires the contract's pinned
content hashes and a read-only source mount for the entire admission/load
interval. The read-only requirement supplies the immutability that metadata
sampling alone cannot. The runner records the mount namespace, effective
capabilities, and matching `/proc/self/mountinfo` entry, and exposes no
`CAP_SYS_ADMIN`; the loader requires `fstatvfs(..., ST_RDONLY)` on every
retained source descriptor before admission and after load. An unsupported
query, writable descriptor filesystem, namespace change, or flag change fails
the common load. Read-only Unix permission bits are not a substitute. The
general structural reader does not silently assume or require this production
posture.

## Directory diagnostic transaction

Directory inventory remains diagnostic and has no producer declaration or
production identity. It returns `None/Unspecified`, but its result must still
describe a stable enumerated set:

1. reject a symlink or non-directory path; retain a read-only no-follow
   directory descriptor and matching pathname/descriptor fingerprint;
2. enumerate the exact sorted set of direct-child names ending in
   `.safetensors`; reject non-UTF-8 names, symlinks, hard links, unsafe names,
   aliases, and an empty set;
3. open and validate that exact set under the shard transaction above;
4. after accounting, revalidate the retained directory identity, enumerate the
   exact set again, require byte-identical membership, and revalidate the
   directory identity again; and
5. run the canonical shard publication sweep before returning.

Adding, removing, renaming, relinking, or replacing a member between the two
enumerations fails. Changes after the final check revoke the lease when a
source or directory revalidation observes them. Directory mode never hashes
unlisted files into checkpoint identity and cannot authorize conversion.

The directory membership digest is internal diagnostic identity only:

```text
SHA256(
  "glmaxx.safetensors-directory-members.v1\0" ||
  u32_le(member_count) ||
  for name in sorted members:
    u32_le(utf8_name_bytes) || utf8_name
)
```

## Accounting retained from r2

Checked `u64` arithmetic over unique retained descriptors computes:

```text
actual_payload_bytes = sum(validated tensor descriptor byte lengths)
actual_file_bytes = sum(retained shard fingerprint lengths)
actual_container_overhead_bytes = actual_file_bytes - actual_payload_bytes
```

Complete contiguous coverage in each shard proves payload bytes equal file
bytes minus its eight-byte prefix and padded header. Aggregate overhead must be
positive and equal the checked sum of those prefix/header extents.

The public result remains:

```text
SafetensorsAccounting {
  declared_total_size: Option<u64>,
  actual_payload_bytes: u64,
  actual_file_bytes: u64,
  actual_container_overhead_bytes: u64,
  interpretation: Unspecified | TensorPayload | CompleteShardFiles,
}
```

The real totals remain exactly:

| checkpoint | shards | declared | payload | files | overhead | interpretation |
|---|---:|---:|---:|---:|---:|---|
| TR3 3.25 bpw | 81 | 339,069,245,936 | 338,954,037,248 | 339,069,245,936 | 115,208,688 | complete files |
| NVFP4/NF3 hybrid | 184 | 365,968,736,768 | 365,968,736,768 | 365,987,273,208 | 18,536,440 | tensor payload |

## Corrected CPU and real-source gate

After adversarial acceptance, the r2 implementation matrix must also prove:

1. duplicate rejection for every index object boundary, BOM/trailing handling,
   and exact `u64` integer acceptance;
2. deterministic mutation of an early shard after its local validation but
   before the publication sweep, with whole-inventory rejection;
3. index mutation during shard opening and at the final sweep;
4. directory add, remove, rename, replacement, and membership reorder attempts
   between enumerations;
5. exact directory-membership digest vectors and byte-level mutations;
6. post-return mutation followed by deterministic failure of
   `revalidate_sources()` and every relevant retained-source operation;
7. proof that a pathname replacement cannot redirect a retained reader; and
8. both real cn4 metadata-only rows from an immutable worktree with checkpoint
   mounts read-only and no CUDA device exposed.

Race tests use explicit test-only barriers rather than probabilistic timing.
The real proof retains the r2 per-shard fingerprints, prefix/header totals, and
artifact hash stream plus the publication-sweep and read-only-mount receipts.
It reads no tensor payload and makes no content-authentication claim.

Acceptance of r1+r2+r3 authorizes only the Rust parser/accounting/retained-
source CPU implementation and a separate metadata-only proof. It does not
accept those implementation bytes, authenticate either checkpoint, authorize
CUDA or conversion, or establish a checkpoint, quality, capacity, reload,
concurrency, cold-start, or speed result.
