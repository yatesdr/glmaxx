# Fable handoff: NF3/NVFP4 native rank manifest v1

Date: 2026-08-03

Status: adversarial design review requested

GPU authorization conveyed by this handoff: none

Read-only cn4 checkpoint-metadata inspection is authorized solely to
independently verify the pinned config, index, and final tier assignment. Do
not create a CUDA context, start/stop a process, read weight payload bytes, or
write anywhere on cn4.

Review candidate commit:
`b1584a989f5878c1b433ea54ffb0dc2925b03f9e`

Required result path: `fable-nf3-nvfp4-native-rank-manifest-v1.md` at the
repository root.

Requested acceptance token, only if every blocker and major is resolved:
`nf3-nvfp4-native-rank-manifest-v1-design-accepted`

## Provenance

Review the exact candidate in a detached worktree. Hash every checked-in input
at start and finish and withhold the token for any mismatch. If cn4 metadata is
used, independently verify the complete source hashes before trusting a query.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/nf3-nvfp4-native-rank-manifest-v1.md` | `72cb1f1aab8ab0a58663ea31080c9d23c9ac40950f2479682739388aea7e69c2` |
| `docs/hybrid-mtp3-capacity-ledger-v1.md` | `465af873bd388166beee5e84e3d2d4272f9501813bdfe95c1e5a3c05b062c2a8` |
| `docs/nf3-nvfp4-hybrid-source-and-kernel-v1.md` | `bd4606d4335f4450b6f5611f54c95a06077ef4ed644812b4dac111aca1c7e01b` |
| `docs/cn4-hybrid-source-inventory-20260803.md` | `caa270096611f2acfdfdef5f8cafd743b49d3b675e9779813dc5aeb7c400e247` |
| `docs/hybrid-serving-manifest-v1.md` | `934787ea37a5dbd9b6778844adbeb0b40fd365d4653991fc7cbfe77df3c685cf` |
| `docs/nvfp4-fused-routed-moe-v1-r3.md` | `f60d3adb777321ed715ae465abcf20895dbbf470c20970855636ed9b2b3a4db0` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `crates/glm-format/src/container.rs` | `2fbe22c55d481e40699a02be9929f9a964f50bc08199853772dd314c85ade47f` |
| `crates/glm-format/src/checkpoint.rs` | `08450f0cb33e592ec76dfbe655b06580ba1743e60f3109a65218d052dbea406c` |
| `crates/glm-engine/src/weight.rs` | `d658cefefc17757a28258bafd0e13f5309e8adcbf2b30c4d2bdc97be9899ca19` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

Run the CPU-only local gate and retain its exit status:

```text
./scripts/local-checks.sh
```

## Required independent work

1. Re-derive 14,400 target NF3, 4,800 target NVFP4, 256 draft NVFP4,
   19,456 experts, 38,912 routed descriptors, 1,217 protected descriptors,
   and 40,129 total tensors. Confirm that the config's
   `hybrid_bit_map`—not the module-prefix JSON—is expert assignment authority.
2. Independently generate all physical names, sort by unsigned UTF-8 bytes,
   and prove unique contiguous tensor IDs. Do not numerically sort layer or
   expert spellings.
3. From source bytes, prove that the final metadata-bearing name is
   `model.layers.9.mlp.experts.99.gate_up_proj.weight`, config entry
   `hybrid_bit_map["9"][99]` is 3, and the index exposes the exact NF3 gate/up
   component family. Reject a 128-byte final-tail premise.
4. Re-derive every NF3 and NVFP4 plane length from the TP4 shapes. Prove each
   plane is divisible by 256 and reproduce 55,207,526,400 NF3,
   26,839,351,296 NVFP4, and 94,006,274,048 total payload bytes.
5. Use the compiled protected inventory independently to simulate the complete
   UTF-8-ordered payload planner. Confirm 11,959,396,352 protected bytes, zero
   payload gap, final protected tensor `model.norm.weight`, and an unrounded
   94,006,274,048-byte end.
6. Reproduce 28,800 NF3 and 10,112 NVFP4 metadata records, 6,823,936 raw file
   metadata bytes, and `(38,912-1)*256+192 = 9,961,408` device bytes. Account
   for exactly 3,137,472 alignment bytes.
7. Audit the proposed format minor 3, profile 4, NF3 flag bit 5, exact flag
   value 59, codec/layout IDs, schemas, and hash domains. Look for any route by
   which a minor-2, EXL3/NVFP4, laboratory, or capacity artifact can alias it.
8. Recompute the 10,273,024-byte descriptor array and 8,988,896-byte common
   catalog. Identify every file/host/device metadata copy that the later HBM
   budget must charge separately.
9. Recompute 94,016,235,456 immutable arena bytes, 98,346,553,280 weight plus
   cache, 3,021,189,184 residual, and 471,052,352 provisional sensitivity
   margin. Confirm that none is presented as physical fit evidence.
10. Attack the planner and mutation matrix for missing offset, alignment,
    arithmetic, rank-consensus, source-binding, flag, final-tail, or
    profile-domain failures.

## Decisions

Answer each `YES` or `NO`:

1. Are the profile and source authorities distinct, complete, and fail-closed?
2. Are the expert, routed, protected, and total descriptor counts exact?
3. Does unsigned UTF-8 ordering uniquely determine every tensor ID and the
   stated NF3 final metadata record?
4. Are all payload plane, codec total, protected, and full weight-arena values
   exact with zero hidden payload alignment?
5. Are raw file metadata, device metadata, final-tail, and alignment-padding
   values exact?
6. Do the new minor, profile, flags, codec/layout IDs, schemas, and domains
   prevent every old-profile alias?
7. Can the existing 256-byte descriptor and new 224-byte-domain catalog
   represent all required NF3/NVFP4/protected semantics without reinterpreting
   a reserved field?
8. Does the design keep file/host/runtime metadata outside weight arenas while
   requiring every resident copy to be charged explicitly?
9. Are the immutable-arena and capacity sensitivity values exact and
   explicitly not a fit, allocation, checkpoint, quality, or speed claim?
10. Is the required CPU proof sufficient to implement this manifest next,
    followed by separate implementation review before any cn4 allocation?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, followed
by independent derivations and all ten decisions. Only if every decision is an
unqualified `YES`, attest all twelve checked-in hashes plus any cn4 source
hashes used, then end with the requested token as the only bare acceptance
line.

