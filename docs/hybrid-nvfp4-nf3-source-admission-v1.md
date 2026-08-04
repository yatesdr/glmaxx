# Hybrid NVFP4/NF3 source admission v1

Date: 2026-08-04

Status: design candidate; adversarial acceptance required before implementation

## Problem

The real `GLM-5.2-hybrid` checkpoint has no publisher manifest. Metadata-only
hashes cannot authenticate 365.99 GB of safetensors. The completed cn4 audit
now supplies a stable digest for every top-level regular file, but that
operator-pinned evidence must not be mislabeled as publisher provenance or
accepted without an engine-owned structural verifier.

## Profile identity

Add an explicit source profile separate from every EXL3 profile:

| Field | Required value |
|---|---|
| Profile ID | `glm52-hybrid-nvfp4-nf3-local-a4d9-v1` |
| Identity basis | `operator-pinned-local-content` |
| Source manifest | `manifests/glm52-hybrid-source-v1.sha256` |
| Source-manifest SHA-256 | `a4d9cb546e8fdae5dd7e494228750ee0a2723904170a7c700e461704d5683ab7` |
| Source rows / bytes | 194 / 366,021,385,004 |
| Shard rows / file bytes | 184 / 365,987,273,208 |
| Shard-row manifest SHA-256 | `8d563757c4fa1f6cb5dedcbc04ad9572ab8f8d4967d3b44d5a2e94952bc7e121` |
| Non-shard rows | 10 |
| Non-shard-row manifest SHA-256 | `e0f0605162858e0e7792bcf800557b5211d92ca4333cbf1b6f52384855827f09` |
| Config SHA-256 | `254974797e9f455716a30ab5505ba68272181b20b58a3693e54f94fb8056f3ef` |
| Index SHA-256 | `6eb773222d932418dd0530c63aca498f86ef424da2a4526ccba76b59726da234` |
| Tier-map SHA-256 | `ebcd6087180033d4512fafa5f154f4fecfbc1ee5e5051448f34859cccc4430f0` |
| Index tensors / shards | 148,289 / 184 |
| Index `metadata.total_size` | 365,968,736,768 |

The profile is a caller-selected enum. Content sniffing, basename inference,
and rank-local selection are forbidden.

## Source-manifest contract

Compile the checked-in manifest into the binary. Require exactly 194
newline-terminated rows of `64 lowercase hex`, two ASCII spaces, and one safe
basename matching `[A-Za-z0-9._-]+`. Reject duplicates, reordering,
noncanonical hex, missing final newline, path separators, or any row not in
the compiled bytes.

The checkpoint root must contain exactly those 194 basenames as regular,
single-link, non-symlink files. The existing `.cache` directory may be present
only as a real directory named exactly `.cache`; it is untrusted, never
traversed, and excluded from identity. Any other top-level entry fails.

This profile has no exceptions. README, compose, tokenizer, config, index,
tier map, and every shard must match its row exactly. There is no
missing-file, publisher-metadata, extension, or optional-file exemption.

## Structural admission

1. Select the same profile on all four workers before filesystem access.
2. Open the index and every manifest file using no-symlink, regular-file,
   single-link, fingerprinted descriptors. Retain shard descriptors for the
   complete verification/load transaction.
3. Parse the index with bounded memory and checked arithmetic. Require exactly
   148,289 unique tensor names, exactly 184 unique canonical shard basenames,
   the exact manifest shard set, and declared total size 365,968,736,768.
4. Validate every safetensors header before payload hashing: canonical JSON,
   unique names across shards, exact index ownership, supported dtypes, ranks
   and shapes, monotonic nonoverlapping offsets, payload bounds, and no holes
   or trailing bytes. Checked-add the shard file lengths and require
   365,987,273,208.
5. Parse the exact config and tier map. Require GLM-5.2 architecture geometry,
   `quant_method=modelopt`, `quant_algo=NVFP4`, hybrid-map layers 3 through 77
   exactly once, 14,400 NF3 values equal to 3, 4,800 NVFP4 values equal to 4,
   no other value, and no layer-78 map entry. Layer-78 tensor admission remains
   explicit in the tensor inventory; absence from the map is not permission to
   drop MTP weights.
6. Classify every indexed tensor through a complete engine-owned inventory as
   ModelOpt NVFP4, NF3, protected/shared source precision, or MTP. Require
   exact component families, dtype, shape, byte length, projection-specific
   outer-scale membership, layer/expert ownership, and TP slicing policy.
   Unknown, duplicate, missing, or cross-family components fail.
7. Hash shards through their retained descriptors and all other rows through
   retained/fingerprinted descriptors. Require every per-file digest, exact
   194-row file count, exact 366,021,385,004-byte sum, and the compiled source
   manifest digest.
8. Revalidate fingerprints. Emit a typed receipt containing profile and
   identity basis, source/config/index/tier and shard/non-shard digests,
   counts/byte sums, tensor-inventory digest, codec-family counts, and exact
   ignored-cache posture.
9. Compare complete receipts on all four ranks before conversion, allocation,
   or upload. Any disagreement emits one common abort; no rank may substitute
   a different profile, map, or fallback.

The full engine-owned tensor inventory digest is derived and pinned during the
CPU proof. It cannot be invented from the source index alone: the validator
must independently encode allowed GLM-5.2 tensor semantics and TP ownership.

## CPU proof

The first implementation gate must prove:

1. Rust reproduces all 194 compiled rows plus the full, shard-only, and
   non-shard manifest digests;
2. every actual index name maps to one and only one engine-owned semantic
   contract, including protected/shared and layer-78 MTP tensors;
3. the exact NVFP4/NF3 layer/expert bitmap and projection-specific scale
   families are exhaustive;
4. a digest, name, ordering, final-newline, file type, hard-link, shard set,
   index ownership, offset, bound, dtype, shape, component, scale membership,
   tier value, layer, expert, TP owner, count, or byte-total mutation fails;
5. extra root files, directories other than `.cache`, traversal into `.cache`,
   symlinks, FIFO/device entries, fingerprint races, and arithmetic overflow
   fail;
6. this profile cannot admit EXL3, pure NVFP4, or another ModelOpt checkpoint;
   and
7. four receipts are byte-identical only for one identical source identity
   and one common semantic policy.

Synthetic sparse fixtures may cover mutation classes. Before conversion, cn4
must run the resulting Rust verifier against the real tree, reread every
payload through retained descriptors, and preserve per-file results outside
Git.

## Claim boundary

Acceptance authorizes only implementation and CPU proof of source admission.
It does not bless the checkpoint's publisher, accept a conversion or resident
format, establish ModelOpt/NF3 numeric correctness, authorize HBM upload or a
kernel, or prove checkpoint smoke, KLD, KV capacity, latency, or throughput.
