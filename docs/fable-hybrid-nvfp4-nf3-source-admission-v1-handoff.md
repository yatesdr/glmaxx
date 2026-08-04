# Fable handoff: hybrid NVFP4/NF3 source admission v1

Date: 2026-08-04

Status: superseded by the r2 handoff; do not review or issue this token

GPU authorization conveyed by this handoff: none

Review candidate commit:
`dfdfc9770ada487d629847f84a2ea2a04e640b16`

Review the candidate in a detached worktree. Current HEAD and untracked review
drops are not authority. Verify every pinned input at review start and finish.

Required result path:
`fable-hybrid-nvfp4-nf3-source-admission-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`hybrid-nvfp4-nf3-source-admission-v1-design-accepted`

Emit the token only if all five questions receive an unqualified **YES**.
Otherwise withhold it and identify the smallest corrective scope.

## Pinned inputs

| Path | SHA-256 |
|---|---|
| `docs/hybrid-nvfp4-nf3-source-admission-v1.md` | `c30770d095a175facb8c96db39c3b3d660d63eb45c13a64df8b43a5b64a3471f` |
| `docs/cn4-hybrid-source-content-hash-fc4871d-20260804.md` | `dc47043402211e04afe6fbeb39e29e4e5dea8bef008ca37987a62c7949fe1bce` |
| `manifests/glm52-hybrid-source-v1.sha256` | `a4d9cb546e8fdae5dd7e494228750ee0a2723904170a7c700e461704d5683ab7` |
| `docs/cn4-checkpoint-runtime-identity-20260804.md` | `0362742d599692a68ddf8b62bc3bc1b401fe84d91f91f69ff6b7aba5e5b0487f` |
| `docs/cn4-hybrid-r2-contract-audit-20260803.md` | `2e80f773468ffc89972ea8dbb6dee82b51fec6c0b3f49b319cc2ebf913698573` |
| `docs/nf3-nvfp4-hybrid-source-and-kernel-v1-r2.md` | `80a055c354971e7ff82b0eeeb54413cd6acafb7ee31611426f3154accc35b2aa` |
| `docs/nf3-nvfp4-native-rank-manifest-v1-r2.md` | `ee81b6fc50a9a948af48aaf60d992fceb4c1bcb0c1687ac8cbe6f133a15baf9a` |
| `crates/glm-format/src/safetensors.rs` | `4a7d8d4a2121a2257a5e8b7ec531c98b4b83bddb6ea140ade697088a05009594` |
| `crates/glm-format/src/checkpoint.rs` | `08450f0cb33e592ec76dfbe655b06580ba1743e60f3109a65218d052dbea406c` |
| `scripts/cn4-hybrid-source-hash.sh` | `ccbda911564db1d002b0748fa989755bbbfeb630ec34174b705ea266472202d2` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `scripts/local-checks.sh` | `2d1882be9afd91f4a54c1d3ff9b9f02cd5087357eeb5668d4094c2114c3003ce` |

Raw evidence remains outside Git at:

```text
/home/derek/glmaxx/evidence/20260804T020700Z-hybrid-source-hash-fc4871d
```

Its concise hashes are pinned in the result input. The checked-in source
manifest contains only filenames and SHA-256 values. This review requires no
cn4 access and may not infer publisher provenance from that local identity.

## Independent work

From `manifests/glm52-hybrid-source-v1.sha256`, independently verify:

- canonical syntax, uniqueness, final newline, 194 total rows, 184 exact shard
  names, and 10 non-shard rows;
- full-manifest SHA-256 `a4d9cb54...5683ab7`;
- shard-row digest `8d563757...bc7e121` and non-shard-row digest
  `e0f06051...827f09`; and
- exact config, index, tier-map, tokenizer, and shard-set membership.

Independently audit the current safetensors reader and the two corrective r2
hybrid contracts for any semantic class, source component, MTP tensor, scale,
or race that the new source gate leaves unauthenticated.

## Questions

1. Does the cn4 evidence plus the checked-in 194-row map establish a complete,
   stable operator-pinned local content identity while accurately withholding
   any publisher-provenance claim?
2. Is the compiled-manifest parser and exact-root-entry policy canonical and
   fail-closed, including no exceptions, one ignored-but-never-traversed real
   `.cache` directory, retained descriptors, hard-link/symlink rejection,
   exact byte sums, and post-hash fingerprints?
3. Do the index, safetensors, config, tier-map, component-family, scale,
   protected/shared, TP-ownership, and layer-78 requirements provide a
   complete semantic admission boundary rather than merely hashing opaque
   shards?
4. Does the typed receipt and four-rank comparison prevent rank-local profile,
   tier, source-map, or fallback divergence and close verify-then-reopen
   exposure before conversion/allocation/upload?
5. Is the CPU mutation proof sufficient and correctly sequenced, with clean
   separation from the pending r2 codec/kernel and native-rank-manifest gates
   and from every CUDA, quality, capacity, and performance claim?

Classify findings as blocker, major, or minor and explicitly report candidate
and input-hash checks at review start and finish.
