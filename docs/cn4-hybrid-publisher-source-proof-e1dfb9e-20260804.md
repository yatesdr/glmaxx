# cn4 hybrid immutable publisher source proof at e1dfb9e

Date: 2026-08-04

Status: immutable publisher bytes authenticated; semantic admission remains
unimplemented

## Result

The clean isolated cn4 checkout at
`e1dfb9ec67550ba5a2c97bf3aabb28a729e48f50` ran the committed public-source
proof against:

```text
repository = madeby561/GLM-5.2-MXFP8-NVFP4-NF3-Hybrid
revision   = 68babde27a97a4c980c2494e830dd424975cd5a3
```

The result was:

```text
HYBRID_IMMUTABLE_PUBLISHER_SOURCE_PASS
```

The immutable Hugging Face API returned exactly 194 siblings. All 186 LFS
records exposed SHA-256 and size values identical to
`manifests/glm52-hybrid-source-v1.sha256`. That set includes all 184
safetensors shards, the 13.72-MB index, and the 20.22-MB tokenizer. The eight
Git-backed files were downloaded from the same immutable revision and matched
their full-body SHA-256 and size exactly.

| Property | Result |
|---|---:|
| Public revision | `68babde27a97a4c980c2494e830dd424975cd5a3` |
| Public/local files | 194 / 194 |
| Public/local file bytes | 366,021,385,004 / 366,021,385,004 |
| LFS identities matched | 186 |
| Git-backed bodies matched | 8 |
| Shard identities matched | 184 |
| Shard bytes matched | 365,987,273,208 |
| Source-manifest SHA-256 | `a4d9cb546e8fdae5dd7e494228750ee0a2723904170a7c700e461704d5683ab7` |
| Start | `2026-08-04T02:34:58Z` |
| End | `2026-08-04T02:34:59Z` |

Public revision:
`https://huggingface.co/madeby561/GLM-5.2-MXFP8-NVFP4-NF3-Hybrid/tree/68babde27a97a4c980c2494e830dd424975cd5a3`

## Evidence

Authoritative evidence root:

```text
/home/derek/glmaxx/evidence/20260804T024000Z-hybrid-publisher-e1dfb9e
```

| Record | SHA-256 |
|---|---|
| `evidence-sha256.txt` | `3d6127f3a85b9c3c9eef6c1db5b7f8fba0a92586a553c5a66f16d554cbdf0fe3` |
| `model-api.json` | `b0197ac45f4eb60681d30d8c4da6bf38a4e7a6bce6ac753389c4c2075e247065` |
| `publisher-lfs-identities.tsv` | `e0e7fc632b9c5dbe4947f700969c15c6e91a1e470baf87f6a44d3418afc32264` |
| `publisher-plain-identities.tsv` | `7b9401bd166f5b3642b2b91e8c6a7da2c1df05607a85735f1b74bd9477c31021` |
| `publisher-plain-body-sha256.txt` | `7b9401bd166f5b3642b2b91e8c6a7da2c1df05607a85735f1b74bd9477c31021` |
| `publisher-summary.json` | `a413a96ef17bc9d0d9d952e9c34bf1869d89d8909beac04bc074ca3fd1c7c1f2` |
| `input-sha256.txt` | `e21171431354964c9924c9951dd42379a182b6dd66403284011def7968f82a01` |
| `verdict.txt` | `8e351efd0957e33c38c70af263b436d197b33a7558ca64a67dc59a1a9e7af6be` |
| `start-utc.txt` | `60b9d3ea85efb26e7cb25b450e7c18ea7c529cef68f878e2100235a7f96a3b7c` |
| `end-utc.txt` | `b37e2d7c6a76b3acb218077745375d822b8b16c1224ab9550a6f366d20ae7089` |

The proof script SHA-256 was
`7433792cb18eab5bbe9cac7c4fa21a1d164fcd060dd082ef1c2131de46ba7c0e`.
The repository was clean before and after. This proof downloaded only the
public API record and eight small public files; it did not reread the local
checkpoint or create a CUDA context.

## Claim boundary

This upgrades the hybrid checkpoint from operator-local content identity to
an exact immutable publisher revision. It authenticates bytes, not their
meaning. Rust must still validate every safetensors header, tensor semantic,
hybrid assignment, component family, scale, TP owner, MTP tensor, and checked
byte range before conversion or upload. No codec, kernel, checkpoint smoke,
quality, capacity, latency, or throughput result is accepted here.
