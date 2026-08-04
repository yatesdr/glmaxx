# Hybrid NVFP4/NF3 source admission v1 r2

Date: 2026-08-04

Status: corrective design candidate; adversarial acceptance required before
implementation

## Supersession

This supersedes `hybrid-nvfp4-nf3-source-admission-v1.md`. V1 conservatively
described the source as an operator-pinned local identity because the
publisher repository was unresolved. The public proof at `e1dfb9e` now binds
all 194 local files to one immutable publisher revision. The semantic
admission algorithm is unchanged; its identity and receipt authority are
stronger.

Do not implement the V1 profile ID or issue its acceptance token.

## Corrected profile identity

| Field | Required value |
|---|---|
| Profile ID | `glm52-hybrid-nvfp4-nf3-68babde-v1` |
| Identity basis | `immutable-publisher-revision` |
| Repository | `madeby561/GLM-5.2-MXFP8-NVFP4-NF3-Hybrid` |
| Revision | `68babde27a97a4c980c2494e830dd424975cd5a3` |
| Source manifest | `manifests/glm52-hybrid-source-v1.sha256` |
| Source-manifest SHA-256 | `a4d9cb546e8fdae5dd7e494228750ee0a2723904170a7c700e461704d5683ab7` |
| Source rows / bytes | 194 / 366,021,385,004 |
| LFS / Git-backed rows | 186 / 8 |
| Shard rows / bytes | 184 / 365,987,273,208 |
| Shard-row manifest SHA-256 | `8d563757c4fa1f6cb5dedcbc04ad9572ab8f8d4967d3b44d5a2e94952bc7e121` |
| Non-shard-row manifest SHA-256 | `e0f0605162858e0e7792bcf800557b5211d92ca4333cbf1b6f52384855827f09` |
| Config SHA-256 | `254974797e9f455716a30ab5505ba68272181b20b58a3693e54f94fb8056f3ef` |
| Index SHA-256 | `6eb773222d932418dd0530c63aca498f86ef424da2a4526ccba76b59726da234` |
| Tier-map SHA-256 | `ebcd6087180033d4512fafa5f154f4fecfbc1ee5e5051448f34859cccc4430f0` |
| Index tensors / shards | 148,289 / 184 |
| Index `metadata.total_size` | 365,968,736,768 |

The public proof independently requires the API revision, exact sibling set,
per-object size, all 186 publisher LFS SHA-256 values, and full-body SHA-256
for all eight Git-backed files. It found no publisher/local exception.

## Normative admission

All source-manifest syntax, exact-root-entry policy, no-exception policy,
retained-descriptor rules, bounded safetensors checks, GLM-5.2 config and tier
requirements, complete tensor semantic classification, byte arithmetic,
fingerprint revalidation, receipt consensus, CPU mutation proof, and claim
boundaries from V1 remain normative without relaxation.

The following r2 rules replace V1 identity wording:

1. Runtime profile selection binds the exact repository and revision above in
   addition to the compiled 194-row source manifest. Runtime admission remains
   offline and does not contact Hugging Face.
2. Optional downloader markers are accepted only as a complete exact pair of
   regular single-link files named `glmaxx-source-repository.txt` and
   `glmaxx-source-revision.txt`, each containing its exact value plus one
   newline. A partial or different pair fails. Absence of both is allowed
   because the complete compiled content map is revision-authenticated.
3. The typed admission receipt includes repository, revision, identity basis,
   source-manifest digest, all structural/tier/inventory identities, and
   `source_markers_verified`. Four-rank consensus compares every field.
4. No cache metadata, directory name, README claim, branch name, moving tag,
   checkpoint basename, or live network response is runtime authority.
5. The V1 local-only profile ID is rejected. EXL3, pure NVFP4, another hybrid
   revision, a rewritten 194-file map, or any per-file byte change cannot
   cross-admit.

## Corrective CPU-proof additions

In addition to the complete V1 CPU proof, implementation must prove:

- repository and revision mutations alter the receipt and fail consensus;
- exact marker-pair success plus partial, malformed, symlink, hard-link, and
  wrong-revision failures;
- the compiled source map alone authenticates the marker-absent real tree;
- V1 profile bytes and identity basis are rejected; and
- an independently written parser reproduces the public API's 194 names,
  366,021,385,004 bytes, 186 LFS identities, eight body identities, and the
  exact source-manifest digest.

The public proof is provenance evidence, not part of serving startup. The
later cn4 real-tree CPU proof must still hash local bytes through retained
descriptors before conversion.

## Claim boundary

Acceptance authorizes implementation and CPU proof of this publisher-bound
source profile only. It does not accept the pending NF3/ModelOpt numerical
contract, native-rank manifest, conversion, HBM residency, CUDA kernels,
checkpoint output, KLD, capacity, cold start, hot reload, or performance.
