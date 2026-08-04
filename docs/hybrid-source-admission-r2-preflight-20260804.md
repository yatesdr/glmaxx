# Hybrid NVFP4/NF3 source-admission r2 preflight

Date: 2026-08-04

Status: exact design candidate and immutable-publisher preflight passed;
adversarial acceptance and implementation remain pending

## Candidate and local gate

The exact candidate
`99ab1351ba71982f69c5855a9dc2adb394f8be11` was checked in detached worktree
`/tmp/glmaxx-hybrid-source-preflight-99ab`. All twelve hashes pinned by
`docs/fable-hybrid-nvfp4-nf3-source-admission-v1-r2-handoff.md` matched at the
start and finish, and the worktree remained clean.

`./scripts/local-checks.sh` returned zero. Its release matrix passed 417 tests:
122 cache, 17 CLI, 11 CUDA-ABI, 99 engine, 72 format, three NVFP4 proof, 23
reference, 16 scheduler, 44 serving, and ten tokenizer tests. Clippy, release
proofs, deterministic profile validation, review provenance, engine, serving,
and cache-lifecycle proofs also passed. The candidate review proof verified
149 handoffs and 39/131 configured results, with 39 accepted and none withheld
at that historical commit.

## Fresh immutable-publisher proof

The committed proof script ran from the clean detached candidate against
repository `madeby561/GLM-5.2-MXFP8-NVFP4-NF3-Hybrid` at exact revision
`68babde27a97a4c980c2494e830dd424975cd5a3`. It returned
`HYBRID_IMMUTABLE_PUBLISHER_SOURCE_PASS` and reproduced:

| Property | Observation |
|---|---:|
| Publisher/local names | 194 / 194 |
| Total bytes | 366,021,385,004 |
| LFS SHA-256 and size identities | 186 |
| Git-backed full-body identities | 8 |
| Safetensors shards | 184 |
| Safetensors shard bytes | 365,987,273,208 |
| Compiled source-manifest rows/bytes | 194 / 19,063 |
| Source-manifest SHA-256 | `a4d9cb546e8fdae5dd7e494228750ee0a2723904170a7c700e461704d5683ab7` |
| Shard-row manifest SHA-256 | `8d563757c4fa1f6cb5dedcbc04ad9572ab8f8d4967d3b44d5a2e94952bc7e121` |
| Non-shard-row manifest SHA-256 | `e0f0605162858e0e7792bcf800557b5211d92ca4333cbf1b6f52384855827f09` |

A separately written Ruby check re-parsed the public API and compiled
manifest, compared the complete name set, verified every LFS digest/size and
all eight downloaded body digests/sizes, and independently reproduced the
same counts and byte totals.

The fresh public-proof records are under
`/tmp/glmaxx-hybrid-public-proof-99ab-r1`; their `evidence-sha256.txt` hashes
to `f9c996c53be3daa84b3064adb074a1661513c028cede0affa4e9c6cd9f46a59c`.
The raw public API envelope now hashes to
`980c2cebbc7508da7954dc8a8277f269a77165965711c32f277845ec14c429ef`,
not the historical envelope hash. This does not indicate content drift: the
exact revision, sibling set, sizes, LFS object identities, and eight Git-backed
bodies are unchanged. The design correctly uses those semantic identities and
does not make the mutable API-envelope serialization a runtime authority.

## Gate boundary

No cn4 access, local checkpoint read, runtime admission, conversion, CUDA
context, or GPU launch occurred. This preflight found no new contradiction in
the r2 source-profile design, but it is not an adversarial review. The exact
token `hybrid-nvfp4-nf3-source-admission-v1-r2-design-accepted` remains
mandatory before Rust implementation and CPU mutation proof. The later real
proof must still hash the local 366-GB tree through retained descriptors and
reach four-rank receipt consensus before conversion.
