# TR3 3.25-bpw publisher-manifest reconciliation v1

Date: 2026-08-04

Status: design candidate; adversarial acceptance required before implementation

## Problem

The real TR3 3.25-bpw checkpoint is payload-authenticated by a 97-row
publisher manifest, but that manifest is self-inconsistent for three
deployment-only files. A strict generic verifier correctly fails. The
existing GLMAXX verifier is bound to a different 3.0-bpw repository and must
not be relabeled or widened for this checkpoint.

The 316-GiB cn4 audit proved all 81 shards and 13 other rows byte-exact to the
manifest. Independent immutable-revision retrieval proved that the remaining
two replacement bytes and one absence are the publisher's exact state, not a
local edit.

## Profile identity

Implement a separate source profile with these exact anchors:

| Field | Required value |
|---|---|
| Profile ID | `glm52-exl3-tr3-325-e2b03576-v1` |
| Repository | `willfalco/GLM-5.2-EXL3-TR3-3.25bpw` |
| Revision | `e2b03576cd103e6ad322a1e091e5d0e2d0529073` |
| Manifest SHA-256 | `db01ba5885fbb39370746e78e7bcb4205ea4e639b20f8950b71f94038f9f992e` |
| Manifest bytes / rows | 8,948 / 97 |
| Index SHA-256 | `f5dcd976a64ca70808dd4d8bd3ad07e9610c8ca6c30e3a6ed77ddefdac4c1d21` |
| Tier-map SHA-256 | `a287ffe816de5998fbc35a56a1ec05f69eb71087d5bbdfe631242c6b296b2a3d` |
| Tensor names / shards | 935,105 / 81 |
| Index total bytes | 339,069,245,936 |
| Present manifest bytes | 339,173,280,736 |
| Canonical source identity SHA-256 | `4ae8fb4b6e8076ba9db6ce3b1f300ec163a60511c2235b2b8c99e1de77395d73` |

The runtime profile selection must be explicit. No content sniffing may route
between this profile and the existing 3.0-bpw profile.

## Exact exceptions

The exception representation is a tagged enum, not a generic ignored-file
list:

```text
PresentReplacement {
  name,
  manifest_sha256,
  revision_sha256,
}

AbsentAtRevision {
  name,
  manifest_sha256,
}
```

The only permitted instances are:

| Kind | Name | Manifest SHA-256 | Required immutable/local state |
|---|---|---|---|
| PresentReplacement | `README.md` | `69523e1a1af7e34165678f0b05040aab9cd13d1894370e12c2354994646be6e6` | SHA-256 `5e02ff95713b019267f0ad298f59b542c4fe1c61230b51437a00906d25e6b8d5` |
| PresentReplacement | `docker-compose.yaml` | `766c8c7a851612063e314df419227ceb17de4b311b2beb9c042b2d9c592acb39` | SHA-256 `eeebf4a0ef7639f842bcb072c47babb8cf1439685f930c0d50801335cda9a83f` |
| AbsentAtRevision | `docker-compose.yml` | `504e02f85352cc21f8e902a9fbf44600e70c18fdc998d4d2c733e90c91697145` | path absent; publisher revision returns HTTP 404 |

No weight, index, tier-map, tokenizer, configuration, calibration, or script
file may be excepted. A present `docker-compose.yml` fails even if its digest
equals the manifest. A missing present-replacement file fails. Filename-only,
extension-wide, directory-wide, or arbitrary metadata exceptions are
forbidden.

## Admission algorithm

1. Select the profile identically on all ranks before filesystem work.
2. Open `model.safetensors.index.json` and all 81 unique shards using the
   existing no-symlink, regular-file, single-link, fingerprinted descriptor
   rules. Validate the complete safetensors structure, tensor inventory,
   offsets, bounds, dtype/shape contracts, tier map, and checked byte sums.
3. Open `MANIFEST.sha256` as a small regular file; require its exact byte
   count and SHA-256. Parse exactly 97 canonical newline-terminated rows,
   reject duplicate or unsafe names, and require the exact index row and exact
   81-shard basename set.
4. Hash each shard through the already-open descriptor. Hash every other
   present row through a fingerprinted regular-file descriptor. Default
   behavior is exact manifest equality.
5. Apply a present replacement only when name, manifest hash, and observed
   hash equal one complete tuple above. Apply the absent exception only when
   the name and manifest hash match and `symlink_metadata` returns
   `NotFound`; any file, directory, symlink, broken symlink, or other error
   fails closed.
6. Revalidate every path fingerprint after hashing. Checked-add the 96
   present lengths and require exactly 339,173,280,736 bytes.
7. Construct two `BTreeMap<String,String>` values. `present` maps all 96 names
   to their lowercase observed SHA-256; `absent` maps the one absent name to
   its manifest-claimed SHA-256. Serialize the struct fields in the order
   `absent`, `present` as compact UTF-8 JSON. Require 9,263 bytes and SHA-256
   `4ae8fb4b6e8076ba9db6ce3b1f300ec163a60511c2235b2b8c99e1de77395d73`.
8. Emit an admission receipt containing profile ID, repository, revision,
   manifest/index/tier/source-identity hashes, counts, byte totals, the two
   present replacements, the explicit absence, and whether an exact complete
   repository/revision marker pair was verified.

Downloader markers remain optional only as an exact pair. If either marker is
present, both must be regular single-link files containing the profile's exact
repository and revision plus one newline. `.manifest_verified` is ignored as
untrusted downloader state and may never satisfy a gate.

The receipt is not rank-local policy. All four workers must compare the full
receipt before conversion or upload; any disagreement aborts all ranks.

## CPU proof

The implementation gate must include independently constructed fixtures that
prove:

1. all 97 manifest names and the exact canonical identity digest;
2. both present replacements and the absence pass only as complete tuples;
3. changing any tuple field, making the absent path exist in any form, or
   removing a required present path fails;
4. every payload, runtime, tokenizer, tier, calibration, and script digest
   mutation fails;
5. duplicate names, noncanonical syntax, unsafe paths, symlinks, hard links,
   fingerprint races, size overflow, wrong shard sets, and wrong byte totals
   fail;
6. the existing 3.0-bpw profile cannot admit this tree and this profile cannot
   admit a 3.0-bpw tree; and
7. Python, Ruby, and Rust independently reproduce the 9,263-byte canonical
   JSON and its pinned SHA-256.

The CPU proof may use synthetic sparse files and tiny manifest fixtures for
mutation coverage. Before any conversion, cn4 must rerun the full real-tree
proof and preserve per-file outcomes and hashes in a unique GLMAXX evidence
directory.

## Claim boundary

Acceptance authorizes only implementation and CPU proof of this source
profile. It does not accept conversion output, EXL3 reconstruction, a native
rank manifest, HBM upload, kernel execution, checkpoint text, KLD, capacity,
or performance. The hybrid NVFP4/NF3 tree requires its own complete source or
converted-image identity and cannot inherit this profile.
