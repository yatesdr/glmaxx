# Fable adversarial review — manifest and cache ABI v0.2.2

Date: 2026-07-29

Reviewer: Fable (adversarial design-gate review)

Handoff: `docs/fable-manifest-abi-v022-handoff.md`

Verdict summary: **NOT ACCEPTED — no gate token is emitted.** One BLOCKER
(a cross-CTA data race in the FC2 CUTLASS control's in-place BF16-to-FP32
expansion) fails Decision 4, and one MAJOR spec/model divergence on the
combined draft sidecar qualifies Decision 2. Decisions 1 and 3 pass on their
own terms. Because the handoff requires four unqualified YES answers, the
acceptance token is withheld and the required hash lines are deliberately not
emitted in their gate format.

## Provenance

- Candidate base commit reviewed:
  `22d03fcce921483bbf71da5a51e80131326217b7`
  ("Add routed FC2 qualification smoke").
- Review venue: a dedicated git worktree at that commit under the reviewer
  scratchpad (`wt-manifest-22d03fc`). All review reads, tests, and derivations
  used only the pinned worktree bytes.
- Drift observed versus HEAD: at review start, HEAD of
  `/Users/derek/glm5-native` was `830c6c8` ("Fix exact SM120 capability
  match") and **11 of 33 pinned inputs mismatched at HEAD**
  (`crates/glm-cuda/src/abi.rs`, `ffi.rs`, `lib.rs`,
  `kernels/include/glmaxx_kernel.h`, `kernels/sm120/nvfp4_routed_fc1.cu`,
  `kernels/CMakeLists.txt`, `crates/glm-format/src/checkpoint.rs`,
  `crates/glm-cli/src/main.rs`, `docs/checkpoint-ingest.md`,
  `scripts/cn4-phase-b-prepare.sh`, `scripts/cn4-phase-b.sh`). All 11
  matched the pinned candidate commit exactly, so the review proceeded
  against the pinned bytes per the handoff procedure. During the review,
  HEAD advanced again to `336e52b` with further uncommitted modifications in
  the working tree — concurrent editing is active; nothing at HEAD was
  reviewed or relied upon.
- Read-only evidence tree: `../glm52-opt` at HEAD
  `d213925ee6701072f117aec59ca94f1bf00d5e7f` (matches the manifest source
  pin). It was read and hashed only; not modified.
- Workspace tests: `cargo test --workspace` at the pinned commit — 153
  tests, 0 failures.

### Verified input SHA-256 table

All 33 inputs were hashed inside the worktree at review start and re-hashed
at review finish; both passes matched this table (which equals the handoff
table) exactly.

| Input | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `docs/nvfp4-physical-abi.md` | `01939514efdd7f34045d64830b43b09647af600f8f5cf641e26a9a4d0cae2c23` |
| `docs/phase-a-proof.md` | `d38eea85efd96b07bbdbdb27c039a2d7848d348b499615ca21c59e0c29904a41` |
| `crates/glm-reference/src/manifest.rs` | `dc8076f90632ac556cf718053c82231ad2bd95d4871fc3ba23444a9574975403` |
| `crates/glm-reference/src/routed_fc1.rs` | `55709be01710d08f5b13de22afdb24884d233cdfab1c5de132ad22edd304f40b` |
| `crates/glm-reference/src/routed_fc2.rs` | `4f34f5b89cd542f096269a7442da5289900d1e831e32fcf83462560dc410a40d` |
| `crates/glm-cuda/src/abi.rs` | `3593a96f09319f0f1f7e2fef47f555d4fd47849790e2bdf60a4ce96a81f0996c` |
| `crates/glm-cuda/src/ffi.rs` | `b4bff008d1b262de9cf3032fbe3777e8e2bc1f62dd86b4dd1dbe11c0c1d55d4d` |
| `crates/glm-cuda/src/lib.rs` | `08eae48f2a60d30abc529ed299ba023d027b212fb958c8863cb27a217adc3073` |
| `kernels/include/glmaxx_kernel.h` | `e6a13f495362704f248a350bdfe941421bc8a2119109e3106dee3b42f2fc4470` |
| `kernels/sm120/nvfp4_routed_fc1.cu` | `a6ea3cd4cefd08ae2dcd98752e092fbbfa7d19bf799c00457052f2717a562f60` |
| `kernels/sm120/nvfp4_routed_fc2.cu` | `0c48e1dae810ab658bc2c565452f06e96026aed3c4b472e6036bd4ba3a49706d` |
| `kernels/sm120/cutlass_nvfp4_dense_control.cu` | `1ec4abf4fc307f709aa5d2576ac80c84825d75a7eb9db7b11ae036fd67a34541` |
| `kernels/sm120/cutlass_nvfp4_fc2_control.cu` | `26c74dfbcb7ea3f75cceb32021d2978271b3f36a1b3d87bab67284cd5d41ea63` |
| `kernels/CMakeLists.txt` | `872403cd1e67380476b91a01b60b612d2ef24d84f261e365c452e8a54a864416` |
| `crates/glm-cache/src/kv.rs` | `60701a0ec25dfac0345d3b088d8937a8adcc1107d8f2a3afa96c0b38379ec8b0` |
| `crates/glm-cache/src/tier.rs` | `2730d829c8538e7b10649e0fba6504ee3389adc21c2f557e474a93c6dbee4f97` |
| `crates/glm-cache/src/page.rs` | `d32d70b46f8e09c31923b6fb574db07ef6a8a7dfc7489392b39785dd563217ed` |
| `crates/glm-cache/src/budget.rs` | `14b563afbeea90fb2bc8897db1a73dab33c64f5427dacac83edd56a00e0eb8a7` |
| `fable-adversarial-v2.md` | `f0019b96d5b35bdca6d026691629b56fbeb0c3c4528e1ae4ff9c1aa06817953e` |
| `docs/fable-v2-disposition.md` | `fd60c89ec188fc6467507ad054f114a379625b0eec40b863cb61c5ace5b1783b` |
| `docs/cn4-preparation-result-20260729.md` | `427004e5bc1f6480bd62acbb11a5fab5146d8cd271c53b0e4b94595b7130e7f9` |
| `docs/manifest-source-audit-20260729.md` | `02d853aad455aa120efc88926c8dbe06841c621a2831067cf59fb4a5b78d4cad` |
| `profiles/profile-budget-v0.json` | `028516adc04d454317e1b76a3147be4807c3ed3ce371e1d43aead3396270400d` |
| `crates/glm-format/src/checkpoint.rs` | `eadf86769c220d42a419b2a9c5a78ff0377d98e85cff4d90c5b792894fa7f684` |
| `crates/glm-format/src/stream.rs` | `4cd4cb23d68ef4280a9a9a00270fc7dad4091ade058fd1165f353d6c95772c8f` |
| `crates/glm-format/src/safetensors.rs` | `a7f8ce1074e585106c2f44d05c2669518e5e3638732c0ba8ee0fdc882ac3a2d1` |
| `crates/glm-cli/src/main.rs` | `b0dbf5c3fcbff295fa1c685a3d82b234de9f67941ce25541f3cbbbf7d96ab93a` |
| `docs/checkpoint-ingest.md` | `b25ce1ba6d9c8406ed9570c95979ded52edb090d05d2f5770cf9eae57f62b6da` |
| `scripts/cn4-phase-b-prepare.sh` | `2e51621e6f9d8e74274ac1a4e89d53962620418c96c34f9c33a95cb6eb08ed4c` |
| `scripts/cn4-phase-b.sh` | `e96a1322f05eb0dc2f7ba5e978db2a2eafd7f8fcbec61251bea4bfc2e7d130cc` |

### Additional pinned sources verified against the read-only tree

| Source | SHA-256 | Result |
|---|---|---|
| `glm52-opt/.../transformers/models/glm_moe_dsa/modeling_glm_moe_dsa.py` | `adb8317a21716b01273046e46c807f14f0dbaf035af59b60d52bd6bc3007cf72` | matches manifest `transformers-modeling-sha256` |
| `glm52-opt/.../transformers/models/glm_moe_dsa/configuration_glm_moe_dsa.py` | `5a81164be746307431ad998f789b6b0bca20eb4c14a726552eb3730268413997` | matches manifest `transformers-configuration-sha256` |
| `glm52-opt/.../models/deepseek_mtp.py` | `3a8a0b30e5dc5eb8c1f0ddb2ce317c375dc094de5b5ba8ba78f71d5481deae6d` | matches pin |
| `glm52-opt/.../deepseek_v32/nvidia/mtp.py` | `8e09e33823d4a6feb5071eb4ef3a5822bf79c1fab7ab59b9e5220be67b5571ca` | matches pin |

## Findings

### BLOCKER

**B1 — Cross-CTA data race in the FC2 CUTLASS control's in-place
BF16-to-FP32 expansion.**
`kernels/sm120/cutlass_nvfp4_fc2_control.cu`, kernels
`expand_scaled_projection` (dense, lines 280–301) and
`expand_scaled_projection_grouped` (grouped, lines 132–155).

The control materializes the unscaled BF16 projection at
`assignment_down_f32 + PE * sizeof(uint16_t)` (where
`PE = assignments x 6144`) and then expands it in place: element `l` reads
BF16 at byte `2*PE + 2*l` and writes FP32 at byte `4*l` of the same
allocation. The FP32 write range `[0, 4*PE)` overlaps the BF16 read range
`[2*PE, 4*PE)`. For every writer element `l >= PE/2`, the 4-byte write at
`4*l` lands exactly on the unread BF16 source bytes of elements
`m = 2*l - PE` and `m+1` (derivation: `2*PE + 2*m = 4*l`), and `m < l` for
all valid `l < PE`, so the victims belong to other threads in other CTAs.
The kernel is a grid-stride launch of up to 4,096 CTAs with no inter-CTA
ordering, so whether a victim reads its source before it is clobbered
depends entirely on block scheduling. Sequential ascending execution would
be safe (`4*l <= 2*PE + 2*l`), which is presumably why the hazard was not
noticed; parallel execution is not sequential.

Consequences: the FC2 dense and grouped CUTLASS controls — the very
comparison baselines the gated M2 smoke depends on — can nondeterministically
produce corrupted `assignment_down` values, causing spurious mismatch
failures or, worse, unstable agreement evidence. This fails Decision 4
item 5/6 adversarial requirements (the aliasing of the materialized scratch
with the live assignment-major output is exactly an overlap of scratch reuse
with a live output plane). The FC1 control does not share the defect: it
materializes BF16 into `gate_up_accum_f32` (a 2x-larger separate buffer) and
writes SwiGLU output to the distinct `output_bf16` buffer.

Required correction: break the aliasing — e.g. materialize the BF16 GEMM
result in a non-overlapping scratch region (the FC1 pattern), or make the
epilogue emit the scaled FP32 directly, or expand via a second
non-overlapping buffer. `docs/nvfp4-physical-abi.md` documents the
token-output scratch reuse but is silent on this in-place expansion; the doc
must be corrected together with the code.

### MAJOR

**M1 — The pinned tier-journal model cannot represent the frozen combined
draft sidecar record.**
`crates/glm-cache/src/tier.rs` models the MTP sidecar as two independent
pieces — `TierPiece::DraftKv` (23,552 bytes) and `TierPiece::DraftIndexer`
(8,448 bytes) — each with its own `storage_offset` (tier-aligned) and its own
payload SHA-256. `spec/format-v0.md` section 22.1 freezes a single combined
draft record: one `G5MTPG00` header, one token-major 32,000-byte payload
`[token][draft_kv 0..368][draft_indexer 0..132]` whose interleaving exists
precisely so "a committed position's two required records cannot be split",
and one payload SHA-256 over the combined 32,000 bytes (header field at
offset 168, `token record bytes = 500`). Two contiguous pieces of 23,552 and
8,448 bytes at independent offsets cannot describe that interleaved payload,
and the two per-plane digests are not the digest the frozen header carries.
The journal's atomicity/pairing invariants themselves are correct (see the
positive results below), but the reviewed model and the frozen byte contract
disagree about the durable unit and its hash inputs. Either the journal
gains a combined-draft piece matching section 22.1 (with the 500-byte
token-major payload and single digest), or the spec is amended — as written,
an implementation cannot satisfy both. Note also that `TierRecord::validate`
does not check pieces for offset overlap or distinctness.

### MINOR

**m1 — C-language compilation of `glmaxx_kernel.h` silently yields a
216-byte FC1 descriptor.** The `alignas(16)` and both `static_assert`
guards are inside `#if defined(__cplusplus)`. A pure-C translation unit sees
a naturally aligned struct: FC1 = 216 bytes (12 x u32 + 17 x u64 + 32
reserved, 8-byte alignment), FC2 happens to remain 224. No C consumer exists
today and the runtime `struct_bytes` check would reject a mismatched caller,
but the "Rust and C independently freeze" claim should be backed by C11
`_Static_assert`/`_Alignas` in the C branch.

**m2 — FC1 grouped quantize kernel lacks the device-side expert
membership/bounds guard FC2 has.** In `nvfp4_routed_fc1.cu::
quantize_compacted_rows`, the `expert_local_sfa` path indexes
`expert_offsets[expert]` and `expert_sfa_offsets[expert]` with no
`expert < 256` or `begin <= assignment < end` check; `nvfp4_routed_fc2.cu::
quantize_fc2_rows` performs both. Host-side validation (`ffi.rs::
validate_case` / `active_experts_for_grouped`) is currently the only defense
against out-of-bounds `scale_base` arithmetic from a malformed route table
passed through the raw C ABI.

**m3 — Device route validation does not reject negative route weights.**
`build_slot_assignment` flags only non-finite weights
(`kRouteWeightNotFinite`); negativity is enforced host-side only
(`ffi.rs::prepare_case`, `routed_fc1.rs::compact_routes`). Through the
Rust boundary the Decision 4 item-4 property holds (rejection before launch,
plus device flag checked before output is observable), but a raw
`glmaxx_nvfp4_routed_fc2_launch` caller can pass negative weights that the
device accepts. Add a sign check to the device pass or document the C ABI as
Rust-boundary-only.

**m4 — Inconsistent tail-slack policy in `profiles/profile-budget-v0.json`.**
Draft-KV and draft-indexer terms carry exactly 7 pages of slack each
(96,633,856 = 385,875,968/4 + 7 x 23,552; 34,662,144 = 138,412,032/4 +
7 x 8,448), while the target-KV and target-indexer terms equal the bare
committed quarter with zero slack (7,524,581,376; 726,663,168), despite
engine spec section 8 requiring tail slack to be additional for target
slots too. Immaterial to the blocked posture, but it must be reconciled when
the measured terms replace the assumptions.

**m5 — Non-Linux publication fallback is not atomic.**
`stream.rs::rename_directory_no_replace` uses `renameat2(...,
RENAME_NOREPLACE)` on Linux (correct, TOCTOU-free) but a check-then-`rename`
fallback elsewhere, which can replace an empty destination created in the
window. Conversion targets Linux cn4, so this is hygiene, not exposure.

**m6 — Undocumented descriptor-field repurposing in the kernel ABI.**
In grouped modes, `Fc1Descriptor::compacted_input_bf16` carries a
`u64[257]` expert-SFA offset table (never compacted BF16 rows — no kernel
materializes them), and `Fc2Descriptor::token_output_f32` doubles as the
grouped scratch/argument arena before the reducer overwrites it. The header
declares neither; only prose in `docs/nvfp4-physical-abi.md` covers the FC2
case. Additionally, the FC1 *dense* control places its CUTLASS workspace at
`compacted_input_bf16` offset 0, clobbering the expert-SFA table that the
grouped paths need — safe today only because every control run re-prepares
its own case.

**m7 — `IndexerKeyRecord::encode` derives the power-of-two scale via
`f32::powf(log2().ceil())`.** `powf` is not guaranteed correctly rounded;
an off-by-ULP result would produce a non-power-of-two scale. The decoder's
`log2().fract() != 0.0` check fail-closes, so the worst case is a spurious
encode/decode failure, not silent corruption. An exponent-manipulation
construction would remove the risk.

### QUESTION

**Q-a —** Is the intended reconciliation for M1 that the journal gains a
single combined-draft piece (500-byte token-major payload, single digest per
section 22.1), or is the spec to be amended to two separately-hashed planes?
The answer changes the NVMe header contract at format section 23.

**Q-b —** The FC2 grouped scratch carve-out
(`grouped_scratch` in `cutlass_nvfp4_fc2_control.cu`) reserves the first
`257 x 8` bytes of `token_output_f32` for the expert-SFA table and checks
`metadata_bytes < rows x 6144 x 4`, but for very small `rows` with many
active groups the CUTLASS `get_workspace_size` check is the only backstop.
Is a minimum-`rows` posture (or explicit scratch sizing independent of the
output buffer) intended before production shapes are widened?

**Q-c —** `spec/format-v0.md` carries revision 0.2.3 while the handoff and
gate name the review v0.2.2 and the engine spec is 0.2.2. The handoff
explains the format hash change; confirm the version string offset is
intentional and that the gate token naming ("v0.2.2") is meant to cover
format 0.2.3 bytes, since the Phase-B script binds to the token string, not
to the spec revision fields.

## Positive verification results (evidence for the answers)

Decision 1 — generated operation manifest, verified from pinned bytes, not
prose:

- Both transformers source pins reproduce byte-identically from the
  read-only `../glm52-opt` tree (hashes above). Every constant in the
  manifest matches the pinned configuration source directly (vocab 154,880;
  hidden 6,144; dense 12,288; MoE 2,048; 78 layers; heads 64; 1 shared;
  256 routed; scaling 2.5; kv-lora 512; q-lora 2,048; rope 64 / nope 192 /
  v 256; top-8; rms 1e-5; index topk 2,048 / dim 128 / heads 32;
  `first_k_dense_replace` 3). Sparse layers 3..77 (75) confirmed.
- The pinned `indexer_types` generation formula
  (`"full" if max(i - offset + 1, 0) % freq == 0`) re-derives exactly the
  21 full layers 0, 1, 2, 6, 10, ..., 74; the manifest's 21 groups cover
  each consumer layer 0..77 exactly once (verified by enumeration), with key
  production on the full layer and `prev_topk_indices` IndexShare reuse
  visible in the pinned attention source.
- Router facts verified from `GlmMoeDsaTopkRouter.forward`: FP32 linear,
  sigmoid, correction bias used for choice only, group-limited top-8,
  normalized gathered weights, x2.5.
- Gate/up stored `[experts, 2 x 2048, 6144]` gate-first; SwiGLU
  `SiLU(gate) x up`; route weight applied after down projection; shared
  expert (intermediate 2,048 x 1) added before the sparse residual — all
  read directly from `GlmMoeDsaExperts`/`GlmMoeDsaMoE`/decoder-layer source.
- MTP: draft checkpoint layer 78, one recurrent layer
  (`num_nextn_predict_layers = 1`), zero-embedding at logical position 0,
  enorm/hnorm/concat/`eh_proj[6144,12288]`, shared-head RMSNorm recycle and
  shared vocabulary head, recurrence-zero top-2,048 with transient reuse —
  verified against the two pinned MTP sources (hashes above).
- The manifest's stable compaction order (expert, token, slot ascending) and
  no hidden materialization/collective between FC1 and FC2 are contract
  choices implemented consistently in `routed_fc1.rs::compact_routes`
  (exact `sort_by_key((expert, token, slot))`) and the single TP4 all-reduce
  placement at ordinal 5. The pinned eager source's intra-expert iteration
  order differs (slot-major), which is immaterial as declared contract; no
  manifest fact was found that is plausible-but-not-source-derived.

Decision 2 — v0.2.2 combined draft sidecar arithmetic, independently
re-derived (all exact):

- 368-byte KV map re-derived from `kv.rs` byte-for-byte, including the
  all-zero canonical form, the decoded-stored-scale encoding rule, dynamic
  `s_t` at `[292,296)`, and padding rejection; 132-byte indexer record with
  ue8m0 ceil-power-of-two scale and fail-closed decode.
- `368 x 78 x 2^20 = 30,098,325,504`; `132 x 21 x 2^20 = 2,906,652,672`;
  `368 x 2^20 = 385,875,968`; `132 x 2^20 = 138,412,032`;
  `368 + 132 = 500`; `500 x 64 = 32,000`;
  `4,096 + 32,000 = 36,096 -> 36,864` (9 x 4,096). Page terms: 23,552 /
  1,837,056 / 177,408 / 8,448; 16,384 pages, 4,096/rank, 262,144
  slots/rank. All reproduced computationally and matched by the frozen
  tests in `budget.rs`/`tier.rs`.
- Atomicity: `TierJournal::publish` requires every declared piece durable
  with matching digests; MTP records require all four pieces; recovery
  replays only fully published transactions and rejects false publication,
  duplicate durability, and checksum mismatch (tests exercised). MTP0
  degradation exists as the mtp=false record type; an orphan draft cannot
  publish. `PageAttachments::validate` enforces single-generation
  target/indexer/draft atomicity; the page state machine's frozen transition
  table (verified exhaustively by test) forces
  Tentative -> Mutable/Invalid -> Free before reuse, and `mtp.rs` clamps
  depth at the 1,048,576 limit. Tier records carry no owner field, so the
  durable bytes are posture-neutral. The M1 finding above is the one
  divergence.

Decision 3 — conversion and profile-budget boundary:

- All 92 source-manifest files are hashed before any write
  (`verify_pinned_source_files`; the parser requires exactly 92 entries,
  canonical syntax, and the pinned MANIFEST digest); shard files are hashed
  through the already-open validated descriptors
  (`ShardedSafetensors.open_shards` + fingerprint re-verification before and
  after hashing).
- 59,585 tensor contracts per rank independently re-derived:
  76 routed layers x 768 + 79 x 7 attention + 22 x 5 indexer + 76 x 2
  router + 3 x 3 dense + 76 x 3 shared + 79 x 2 norms + 4 MTP + 3 global
  = 59,585. `source_payload_bytes() == 81,590,319,104` is asserted per rank
  by passing tests and is consistent with the 326,361,276,416-byte
  four-rank total.
- Deferred tensor groups preserve payload-before-descriptor durability
  (`write_tensor_deferred`/`commit_pending`: data sync, then descriptor
  writes, then sync; crash leaves zero descriptors).
- The aggregate output payload SHA-256 is sealed into a pre-reserved 64-hex
  slot without changing layout (`seal_output_payload_sha256`), is idempotent
  on resume, and fail-closes if the slot holds a different non-zero value.
- All rank headers derive one conversion UUID exactly per format 5.1;
  publication is `renameat2(RENAME_NOREPLACE)` and cannot replace an
  existing destination (see m5 for the non-Linux fallback).
- The converter rejects any profile budget that is not
  `measurement_status == "complete"` with `conversion_allowed == true`, and
  additionally requires a review artifact containing the exact token line
  and all four pinned hash lines — both gates fail closed against the
  current artifact and against this review.
- `profiles/profile-budget-v0.json`: `conversion_allowed = false`,
  measurement pending, all five unmeasured blockers listed; the arithmetic
  is exact (term sum = 93,865,173,760 per rank; floor headroom
  8,089,967,872; target-KV and indexer terms equal the committed quarters).
  It validates only as a blocked arithmetic candidate (m4 noted). This
  review does not call the budget complete and does not authorize the
  326-GB conversion.

Decision 4 — SM120 routed-MoE physical ABI:

- Both descriptors independently re-derived at 224 bytes / 16-byte
  alignment: FC1 = 12 x u32 + 17 x u64 + 32 reserved = 216 -> 224 with
  align(16); FC2 = 12 x u32 + 18 x u64 + 32 = exactly 224. Field order
  matches between `abi.rs` and `glmaxx_kernel.h` one-for-one; C++
  static_asserts and the Rust layout test freeze them; version,
  `struct_bytes`, flags, reserved, geometry, pointer, alignment, and
  checked-overflow workspace validation exist on both sides and the FFI
  cross-checks the ABI string and all four workspace formulas at runtime
  (m1 caveat for pure C).
- FC1/FC2 workspace formulas re-derived and equal on both sides (spot value
  `fc2_workspace_bytes(1,8) = 258,116` reproduced by hand).
- The SFB/SFA swizzle formula was verified bijective by enumeration for all
  four operand shapes (1024 x 384, 6144 x 32, and the 128-row slabs);
  grouped expert-local slab arithmetic (`grouped_sfa_plan`,
  `build_expert_sfa_offsets`) matches per-expert 128-row padding with
  checked arithmetic and a capacity bound covering every accepted posture.
- FC1 quantizes each BF16 assignment row once (amax -> dynamic global scale
  -> E4M3 block scales encoded against the decoded stored scale -> packed
  E2M1, canonical zeros), reuses the packed row for gate and up, and applies
  SwiGLU only after independent FP32 gate/up accumulation.
- FC2 consumes `[assignments,512]`, produces assignment-major FP32
  `[assignments,6144]`, applies route weights after projection, and reduces
  slots in fixed 0..7 order via the `(token,slot) -> assignment` table with
  no floating-point atomics; through the Rust boundary, unsorted routes,
  malformed offsets, duplicates, out-of-range values, and
  non-finite/negative weights are all rejected before launch, and the device
  validation word is checked before any output is observable (m2/m3 caveats
  for raw C callers).
- The Phase-B script requires the operator-authorization variable, the
  exact gate token as a full line of the tracked root artifact
  `fable-manifest-abi-v022.md`, the pinned CUTLASS revision, a clean
  committed tree, idle-device checks before every launch, fresh evidence
  directories, and an unchanged artifact and tree afterward. This review
  intentionally satisfies none of the token conditions.
- The preparation documents present compile/SASS results strictly as
  preparation evidence with no device launch, and the BF16 boundaries are
  labeled development controls, not fused production kernels — but see B1:
  one of those controls is not currently sound.

## Required answers

1. **Is the generated GLM-5.2 operation manifest accepted for M2?**
   **YES.** Every fact checked was derived from the pinned source bytes
   (hashes reproduced in this environment), the group/consumer structure was
   re-derived independently, and no plausible-but-unsourced fact was found.

2. **Is the v0.2.2 combined draft-KV/draft-indexer cache ABI accepted for
   M2?** **NO — conditional on M1.** All arithmetic, atomicity, rollback,
   and posture-neutrality properties verified, but the pinned tier-journal
   model and the frozen combined draft record (format 22.1) are mutually
   unimplementable as written. Resolving M1 (one-line contract decision plus
   a small model change) should make this a YES on re-review.

3. **Is the conversion path and blocked profile-budget candidate accepted
   exactly in its stated, non-conversion-authorizing posture?** **YES.**
   The arithmetic is exact, the fail-closed gates are real and layered, and
   nothing in this acceptance authorizes conversion or treats the budget as
   complete. m4/m5 are hygiene items for the measurement revision.

4. **Is the routed-MoE v2 FC1/FC2 physical ABI and gated SM120 correctness
   procedure accepted for its development-control posture?**
   **NO — blocked by B1.** The descriptor ABI, workspace arithmetic,
   swizzle, quantization, route validation, and script gating all verified,
   but the FC2 CUTLASS control — a required comparison baseline of the gated
   correctness procedure — contains a genuine cross-CTA data race that can
   corrupt its output nondeterministically on real hardware. The gate must
   not run until the control is race-free.

## Token disposition

Two of the four required answers are not an unqualified YES. Per the
handoff, the acceptance token (the string beginning `manifest-abi-v0.2.2-`
and ending `-accepted`) is **not** emitted, and the four gate hash lines are
not emitted in their required `name=hash` line format. This file must not
satisfy `scripts/cn4-phase-b.sh`'s token check, and it does not: no line of
this document consists solely of the token.

Expected re-review scope after correction: B1 (one kernel file plus the
physical-ABI doc paragraph) and M1 (tier model or format 22.1). Decisions 1
and 3 need no rework; their acceptance here stands for the reviewed pinned
bytes only and does not transfer to the drifted HEAD content.
