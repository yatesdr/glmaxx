# cn4 hybrid source content hash at fc4871d

Date: 2026-08-04

Status: complete local content identity; no publisher-provenance or admission
claim

## Result

The committed audit wrapper at
`fc4871df5af54542b81bbb5581ae8bd10299ef4b` hashed the complete top-level
regular-file set of `/home/claude/LLM/GLM-5.2-hybrid` read-only.

```text
HYBRID_SOURCE_CONTENT_HASH_PASS
```

| Property | Result |
|---|---:|
| Regular files | 194 |
| Regular-file bytes | 366,021,385,004 |
| Safetensors shards | 184 |
| Safetensors file bytes | 365,987,273,208 |
| Index tensor names | 148,289 |
| Index `metadata.total_size` | 365,968,736,768 |
| Hashing elapsed | 774,268,928,528 ns |
| Start | `2026-08-04T02:08:13Z` |
| End | `2026-08-04T02:21:07Z` |

The exact filename/SHA-256 map is checked in as
`manifests/glm52-hybrid-source-v1.sha256`. It contains hashes only—no model or
tensor bytes.

| Identity | SHA-256 |
|---|---|
| Complete 194-row source manifest | `a4d9cb546e8fdae5dd7e494228750ee0a2723904170a7c700e461704d5683ab7` |
| Canonical 184-row shard subset | `8d563757c4fa1f6cb5dedcbc04ad9572ab8f8d4967d3b44d5a2e94952bc7e121` |
| Canonical 10-row non-shard subset | `e0f0605162858e0e7792bcf800557b5211d92ca4333cbf1b6f52384855827f09` |
| Audit script | `ccbda911564db1d002b0748fa989755bbbfeb630ec34174b705ea266472202d2` |

Subset digests hash the newline-terminated rows selected from the complete
manifest in its existing byte order. Shard rows match
`^model-[0-9]+-of-[0-9]+\.safetensors$`; the metadata subset is its complement.

## Stability and isolation

Before the read, cn4 had no GPU compute process. All four SM120 GPUs reported
0% utilization and 97,232--97,249 MiB free. The audit used `nice -n 19` and
idle-class `ionice -c 3`, did not create a CUDA context, and did not access a
vLLM checkout, container, image, cache, port, or result.

Each file's name, device, inode, length, and nanosecond mtime were captured
before and after hashing. Both sorted fingerprint records have SHA-256
`92d9a8f4d55d1db52d616310759f2ffb6b643e4b8ffcb26ac868cfff35c57f44`.
The GLMAXX source remained clean at the exact detached commit. Postflight
again found no compute process, 0% utilization, and the same GPU memory
posture.

## Evidence

Authoritative evidence root:

```text
/home/derek/glmaxx/evidence/20260804T020700Z-hybrid-source-hash-fc4871d
```

| Record | SHA-256 |
|---|---|
| `evidence-sha256.txt` | `3f865381946b9377a7b336845b3837677af78750b1679454186261e81ce7f939` |
| `source-file-sha256.txt` | `a4d9cb546e8fdae5dd7e494228750ee0a2723904170a7c700e461704d5683ab7` |
| `source-manifest-sha256.txt` | `1fd331a8eaa5b294066bcf807176f4f6f8a8a8a5f1d2d795605e133ab006206d` |
| `source-binding.txt` | `d9cd86a4f63a10f5a57847586c6a5aa20246c3a252760504185be03b27f9b1ca` |
| `timing.txt` | `6c5f081c9fa7aeb92d05fd2936ed5100588f3785e8159c1fce11d8e6a73e36f5` |
| `verdict.txt` | `1949e3414c765445be3b44f4e97feff14524a29396916e06aa4857193574ad8e` |
| `start-utc.txt` | `c464e14e46e4c44679412768fa924f27ea5cf46072226c64755b23dbe751daf2` |
| `end-utc.txt` | `8b7816a2ffefced34fec8d2cfa3893c5bc7fd71b39c428b9e79b084568839ffa` |

The checkpoint resides on ext4 at `/dev/nvme0n1p2`. The source manifest is
relative-name based and therefore does not bind that mount path.

## Claim boundary

This closes the missing complete local content identity for the real
NVFP4/NF3 tree. The checkpoint has no publisher manifest, so this is an
operator-pinned local identity rather than publisher provenance. It does not
accept a Rust source verifier, safetensors/tier semantics, codec, conversion,
rank image, HBM load, kernel, smoke, KLD, capacity, or performance result.
