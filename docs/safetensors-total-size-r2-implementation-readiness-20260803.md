# Safetensors `total_size` r2 implementation readiness audit

Date: 2026-08-03

Status: static implementation map; no design token, parser change,
checkpoint admission, payload read, or GPU result

## Scope

This audit maps committed source at
`1f2fd17361a68adbd58efe47afd940da6978c460` to
`docs/safetensors-index-total-size-v1-r2.md`. The r2 design still lacks its
required Fable token, so no implementation is authorized by this record.

The relevant committed source bytes remain the exact handoff inputs:

```text
crates/glm-format/src/safetensors.rs  4a7d8d4a2121a2257a5e8b7ec531c98b4b83bddb6ea140ade697088a05009594
crates/glm-format/src/lib.rs          27aa8052ce18423b66bebe86ddbaafecfbaab989be661ab58c823e692b5d6c3d
crates/glm-format/src/checkpoint.rs   08450f0cb33e592ec76dfbe655b06580ba1743e60f3109a65218d052dbea406c
crates/glm-cli/src/main.rs            381a74d0ef7311a95a2c5996be80b39eb76442489edcaf8a2f934beaf00cf518
```

## Exact current failure

`ShardedSafetensors::open` currently parses `metadata.total_size` into
`declared_payload_bytes` and requires it to equal the sum of tensor descriptor
bytes. That accepts the hybrid checkpoint's tensor-payload convention and
rejects the real TR3 checkpoint, whose declaration is the sum of complete
shard-file bytes.

```text
TR3 declared/file     339,069,245,936
TR3 payload           338,954,037,248
TR3 overhead              115,208,688

hybrid declared/payload 365,968,736,768
hybrid files            365,987,273,208
hybrid overhead              18,536,440
```

This is why the real TR3 admission diagnostic reaches `glmaxx: Index` before
tensor upload or CUDA launch.

## Public API cut

The committed public state is concentrated in one field and accessor:

```text
ShardedSafetensors::declared_payload_bytes: Option<u64>
ShardedSafetensors::declared_payload_bytes() -> Option<u64>
```

No production caller reads that accessor today; the only direct use outside
the type is a unit-test assertion. The accepted implementation can therefore
replace it atomically with:

```text
SafetensorsAccounting {
  declared_total_size: Option<u64>,
  actual_payload_bytes: u64,
  actual_file_bytes: u64,
  actual_container_overhead_bytes: u64,
  interpretation: Unspecified | TensorPayload | CompleteShardFiles,
}
```

The new types must be re-exported from `glm-format`, and the generic `Index`
failure must gain the contract's typed `IndexTotalSize` diagnostic containing
all four totals.

## File-opening transaction

The accounting implementation cannot be a final comparison added to the
current loop. It must refactor shard opening into one stable transaction:

1. resolve and validate the safe relative shard path;
2. collect pathname metadata and reject symlink, non-regular file, hard link,
   or an inode already owned by a different shard name;
3. open without following a symlink and immediately collect descriptor
   metadata;
4. require the initial pathname and descriptor fingerprints to match before
   reading the eight-byte prefix;
5. read and validate only the prefix and padded header, including exact
   contiguous tensor coverage and index/header inventory equality;
6. recollect both pathname and descriptor fingerprints after header
   validation and require all four fingerprints to be identical; and
7. retain the opened descriptor and fingerprint for later authenticated
   payload operations.

The current path precheck is separate from `SafeTensorFile::open`, while the
descriptor fingerprint is captured only after that function has read and
validated the header. That ordering does not implement the required
before-read equality boundary unchanged.

The direct index path also deserves explicit treatment during implementation:
`ShardedSafetensors::open` currently calls `File::open` directly, while only
`open_auto` performs a prior symlink check. The r2 change must not accidentally
make direct indexed admission weaker than the safe shard transaction. Raw
index hashing and production pinning remain separate from `total_size`
interpretation.

## Parsing and arithmetic cut

`RawIndex.metadata` currently uses
`BTreeMap<String, serde_json::Value>`. A dedicated unique-key map or custom
visitor is required so duplicate `total_size` keys are rejected instead of
being accepted according to generic map behavior.

After every shard transaction succeeds, checked helpers must compute:

```text
payload = sum(tensor descriptor bytes)
files   = sum(unique opened-descriptor lengths)
overhead = files - payload
```

Per-shard contiguous coverage must independently prove
`payload == file_bytes - (8 + padded_header_bytes)`. Aggregate overhead must
equal the sum of those prefix/header extents and be positive. Only exact
equality with payload or files selects an interpretation; neither a tolerance
nor checkpoint identity may participate.

Directory inventory has no producer declaration. The committed implementation
currently fabricates `Some(sum(payload))`; r2 must instead return
`None/Unspecified` while retaining all actual totals.

## Required CPU proof

The first implementation review needs evidence for all of the following, not
only the two positive interpretations:

- absent, duplicate, negative, fractional, string, boolean, null, and
  out-of-range declarations;
- one byte below and above payload and file totals and every boundary between
  them;
- checked addition/subtraction overflow for per-shard and aggregate helpers;
- alias, hard-link, symlink, unsafe path, extra/missing tensor, duplicate
  tensor, and non-contiguous data rejection;
- deterministic replacement, resize, mtime, ctime, and header mutation at
  every pre/post fingerprint boundary;
- index-key and shard-order permutation invariance for accounting while raw
  index SHA remains byte-sensitive; and
- directory `None/Unspecified` versus indexed typed reporting.

The final CPU gate is metadata-only admission of both real read-only cn4
sources. It must retain per-shard prefix/header totals and fingerprints and
must prove that no tensor payload byte was read.

## Claim boundary

Correct structural accounting will remove the current TR3 `Index` failure,
but it does not authenticate the publisher manifest, tier map, configuration,
or tensor contents. It cannot by itself authorize conversion, device upload,
checkpoint smoke, quality, capacity, cold-start, or throughput claims.
