# cn4 checkpoint runtime identity

Date: 2026-08-04

Status: immutable TR3 publisher identity and hybrid local content identity
resolved

## Scope

This record pins the two checkpoint trees currently intended for GLMAXX. The
cn4 recheck at `2026-08-04T01:48:13Z` was read-only: it hashed metadata and
counted shard file sizes without creating a CUDA context or changing either
tree. The earlier full TR3 manifest run remains the payload-content evidence.

No checkpoint is admitted by this record alone.

## TR3 3.25-bpw

Checkpoint root:
`/home/derek/models/GLM-5.2-EXL3-TR3-3.25bpw`

| Property | Observed value |
|---|---:|
| Safetensors shards | 81 |
| Safetensors file bytes | 339,069,245,936 |
| Index `metadata.total_size` | 339,069,245,936 |
| Index tensor names | 935,105 |
| Manifest entries | 97 |
| Present manifest entries | 96 |
| Present manifest-entry bytes | 339,173,280,736 |

| Runtime file | SHA-256 |
|---|---|
| `config.json` | `d83e2d8d96b6f36e94d896a05a104200d6674673daa02f788de710c1c0f94ba4` |
| `generation_config.json` | `ac76b43d8683d3b930126870fc8be73d8679308fe752fa1f381096d8354f6a55` |
| `chat_template.jinja` | `172dc74a35e1752df75ecfb2b2cf9326d2852bb1379868ebeec9571654489679` |
| `tokenizer.json` | `19e773648cb4e65de8660ea6365e10acca112d42a854923df93db4a6f333a82d` |
| `tokenizer_config.json` | `98b1271574f41abf89427ae2dda030d94dc9478f0edc5a8bd240db213c6fd5fc` |
| `model.safetensors.index.json` | `f5dcd976a64ca70808dd4d8bd3ad07e9610c8ca6c30e3a6ed77ddefdac4c1d21` |
| `tier_bitmap.json` | `a287ffe816de5998fbc35a56a1ec05f69eb71087d5bbdfe631242c6b296b2a3d` |
| `MANIFEST.sha256` | `db01ba5885fbb39370746e78e7bcb4205ea4e639b20f8950b71f94038f9f992e` |
| `.manifest_verified` | `aa71c9791ae8c22cbad1cd7ed940733b0144a2f8a25d0e0bc571173065782514` |

The marker is stale and non-authoritative. Admission must neither trust nor
derive identity from it.

### Immutable publisher resolution

The actual publisher repository is
`willfalco/GLM-5.2-EXL3-TR3-3.25bpw`. The local tree's download metadata names
revision `e2b03576cd103e6ad322a1e091e5d0e2d0529073`; the prior R14 serving record
names later revision `d7d79c2d14599dfce7a5d12b85f7ad73f40e623d`.

Direct downloads from both immutable revisions reproduced the local hashes
for config, generation config, chat template, tokenizer files, index, tier
map, and manifest. The only publisher change among the checked files was the
README:

| File | `e2b035...` | `d7d79c...` | Local |
|---|---|---|---|
| `README.md` | `5e02ff95713b019267f0ad298f59b542c4fe1c61230b51437a00906d25e6b8d5` | `2fa175e90fac45e9559eba749f2a2491d70f7e736abcfe8942862e9312dd9204` | `5e02ff95713b019267f0ad298f59b542c4fe1c61230b51437a00906d25e6b8d5` |
| `docker-compose.yaml` | `eeebf4a0ef7639f842bcb072c47babb8cf1439685f930c0d50801335cda9a83f` | same | same |
| `docker-compose.yml` | HTTP 404 | HTTP 404 | absent |

Therefore `e2b035...` is the exact immutable publisher state represented by
the local tree. `d7d79c...` remains useful historical runtime provenance but
is not the local source revision because its README differs.

The full 316-GiB audit in `docs/cn4-tr3-source-audit-20260803.md` verified 94
of 97 manifest rows, including all 81 weight shards and every runtime file.
The remaining three rows exactly reproduce the immutable `e2b035...`
publisher state:

| File | Manifest SHA-256 | Immutable/local state |
|---|---|---|
| `README.md` | `69523e1a1af7e34165678f0b05040aab9cd13d1894370e12c2354994646be6e6` | present as `5e02ff95713b019267f0ad298f59b542c4fe1c61230b51437a00906d25e6b8d5` |
| `docker-compose.yaml` | `766c8c7a851612063e314df419227ceb17de4b311b2beb9c042b2d9c592acb39` | present as `eeebf4a0ef7639f842bcb072c47babb8cf1439685f930c0d50801335cda9a83f` |
| `docker-compose.yml` | `504e02f85352cc21f8e902a9fbf44600e70c18fdc998d4d2c733e90c91697145` | absent |

Canonical compact JSON with top-level `absent` and `present` maps is 9,263
bytes and has SHA-256
`4ae8fb4b6e8076ba9db6ce3b1f300ec163a60511c2235b2b8c99e1de77395d73`.
Python and Ruby derivations independently agreed. The exact construction is
specified by `docs/tr3-325-publisher-manifest-reconciliation-v1.md`.

Publisher reference:
`https://huggingface.co/willfalco/GLM-5.2-EXL3-TR3-3.25bpw/tree/e2b03576cd103e6ad322a1e091e5d0e2d0529073`

## NVFP4/NF3 hybrid

Checkpoint root: `/home/claude/LLM/GLM-5.2-hybrid`

| Property | Observed value |
|---|---:|
| Safetensors shards | 184 |
| Safetensors file bytes | 365,987,273,208 |
| Index `metadata.total_size` | 365,968,736,768 |
| Index tensor names | 148,289 |
| `MANIFEST.sha256` | absent |

| Runtime file | SHA-256 |
|---|---|
| `config.json` | `254974797e9f455716a30ab5505ba68272181b20b58a3693e54f94fb8056f3ef` |
| `generation_config.json` | `ac76b43d8683d3b930126870fc8be73d8679308fe752fa1f381096d8354f6a55` |
| `chat_template.jinja` | `172dc74a35e1752df75ecfb2b2cf9326d2852bb1379868ebeec9571654489679` |
| `tokenizer.json` | `19e773648cb4e65de8660ea6365e10acca112d42a854923df93db4a6f333a82d` |
| `tokenizer_config.json` | `98b1271574f41abf89427ae2dda030d94dc9478f0edc5a8bd240db213c6fd5fc` |
| `model.safetensors.index.json` | `6eb773222d932418dd0530c63aca498f86ef424da2a4526ccba76b59726da234` |
| `mxfp8_tier_nokvb.json` | `ebcd6087180033d4512fafa5f154f4fecfbc1ee5e5051448f34859cccc4430f0` |

The shared tokenizer/template hashes remove one source of cross-profile KLD
drift. This tree has no publisher source manifest. A follow-up complete
read-only audit at `fc4871d` hashed all 194 top-level regular files, including
all 184 shards, and checked in the filename/digest map as
`manifests/glm52-hybrid-source-v1.sha256`. Its SHA-256 is
`a4d9cb546e8fdae5dd7e494228750ee0a2723904170a7c700e461704d5683ab7`.
The detailed result is
`docs/cn4-hybrid-source-content-hash-fc4871d-20260804.md`.

This resolves a complete operator-pinned local content identity, not
publisher provenance. Native admission still requires a reviewed verifier
that binds every filename and digest, the safetensors inventory, and the
hybrid tier semantics.

## Claim boundary

This record corrects the repository inference in the earlier TR3 audit,
authorizes a narrow TR3 manifest-reconciliation design review, and now points
to the complete hybrid local-content identity. It does not accept a source
verifier, converter, rank image, kernel, checkpoint smoke, KLD, capacity, or
performance result.
