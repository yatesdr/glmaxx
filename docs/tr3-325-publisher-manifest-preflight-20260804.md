# TR3 3.25-bpw publisher-manifest preflight

Date: 2026-08-04

Status: exact design-candidate and public-anchor preflight passed; adversarial
acceptance and implementation remain pending

## Candidate and local gate

The exact candidate
`f758c1ab82557b1a44fd746d9b36d76faeaa28d8` was checked in detached worktree
`/tmp/glmaxx-tr3-manifest-preflight-f758`. All nine hashes pinned by
`docs/fable-tr3-325-publisher-manifest-reconciliation-v1-handoff.md` matched
at the start and finish, and the worktree remained clean.

`./scripts/local-checks.sh` returned zero. Its release test matrix passed 417
tests: 122 cache, 17 CLI, 11 CUDA-ABI, 99 engine, 72 format, three NVFP4 proof,
23 reference, 16 scheduler, 44 serving, and ten tokenizer tests. Clippy,
release proofs, deterministic profile validation, review provenance, engine,
serving, and cache-lifecycle proofs also passed. The candidate review proof
verified 147 handoffs and 39/129 configured results; it found 39 accepted and
none withheld at that historical commit.

## Independent immutable-publisher check

Fresh HTTPS reads used the exact publisher revision
`e2b03576cd103e6ad322a1e091e5d0e2d0529073`. They reproduced:

| Path | Observation |
|---|---|
| `MANIFEST.sha256` | 8,948 bytes, 97 newline-terminated rows, SHA-256 `db01ba5885fbb39370746e78e7bcb4205ea4e639b20f8950b71f94038f9f992e` |
| `README.md` | SHA-256 `5e02ff95713b019267f0ad298f59b542c4fe1c61230b51437a00906d25e6b8d5` |
| `docker-compose.yaml` | SHA-256 `eeebf4a0ef7639f842bcb072c47babb8cf1439685f930c0d50801335cda9a83f` |
| `docker-compose.yml` | HTTP 404 |

Independent Ruby and Python derivations parsed 97 unique canonical rows,
replaced exactly the two observed publisher-revision digests, moved exactly
`docker-compose.yml` to the absent map, and produced 96 present plus one absent
entry. Both emitted the same 9,263-byte compact JSON with SHA-256
`4ae8fb4b6e8076ba9db6ce3b1f300ec163a60511c2235b2b8c99e1de77395d73`.

## Gate boundary

No cn4 access, checkpoint payload read, source admission, parser change,
conversion, CUDA context, or GPU launch occurred. This preflight found no new
contradiction in the source-profile design, but it is not an adversarial
review. The exact token
`tr3-325-publisher-manifest-reconciliation-v1-design-accepted` remains
mandatory before Rust implementation and CPU mutation proof. The later Rust
proof must independently reproduce the canonical identity and run the full
real-tree hash gate before conversion.
