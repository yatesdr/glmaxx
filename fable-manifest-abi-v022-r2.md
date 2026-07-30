# Fable adversarial re-review — manifest and cache ABI (r2)

Date: 2026-07-30
Reviewer: Fable (Claude), independent gate re-review
Handoff: `docs/fable-manifest-abi-v022-r2-handoff.md` (queue row 33)

Location note: the handoff declares the required result path at the
repository root; the operator directed all queue reviews into
`docs/reviews/`. The Phase-B script gates on a Git-tracked root
`fable-manifest-abi-v022-r2.md` containing the exact token and the four
attestation lines — move this file there (unmodified) when consuming the
acceptance.

## Verdict

All four required answers are unqualified **YES**. The attestation lines
and gate token are included at the end. Acceptance closes only the
independent-review half of the M2 gate; it does not authorize a GPU
launch, approve full conversion, or establish performance.

Gate-name contract acknowledged: `manifest-abi-v0.2.2-accepted` is the
stable M2 gate name; this acceptance binds that name to the exact v0.2.3
format hash attested below, not to a claim that the bytes are v0.2.2.

## Provenance

- Candidate `0edfc8d796aeaeb969668005149bcb6286aa1e85` reviewed in a
  detached worktree. All 16 handoff input hashes verified at review start
  and re-verified at finish; all matched.
- The pinned prior review `fable-manifest-abi-v022.md` is byte-identical to
  the committed 2026-07-29 artifact.
- cn4 evidence `/home/derek/glmaxx/evidence/prepare-c25e558-r1`: all 19
  raw-record hashes verified over ssh (done under the source-projection r2
  review, same evidence root). Independently confirmed the handoff's
  byte-identity statement rather than trusting it:
  `kernels/sm120/cutlass_nvfp4_fc2_control.cu` and
  `crates/glm-cuda/src/ffi.rs` at the evidence commit `c25e558` hash
  exactly to the r2-pinned values, so the five `sm_120f` cubins, the
  retained 256 block-scaled OMMA instructions, and the SFA/SFB probe passes
  (42,564,864 + 393,216 comparisons) cover the exact corrected FC2 bytes.
- Carry-over integrity: all four attestation hashes (engine-v0, format-v0,
  operation manifest, profile budget) appear in the first review's verified
  input table, so the Decision 1 and Decision 3 carry-over rests on
  byte-identical inputs.
- `cargo test -p glm-cache` at the candidate: 33 passed, 0 failed.

## Decision 2 verification — one combined draft sidecar

All seven items verified directly in `crates/glm-cache/src/tier.rs`,
`store.rs`, `lib.rs`:

1. `TierPiece` has exactly `TargetKv`, `TargetIndexer`, `DraftSidecar`.
2. `TierRecord::validate` requires exactly {TargetKv, TargetIndexer,
   DraftSidecar} for MTP (exactly two pieces otherwise), each piece unique,
   each `byte_length` equal to its expected size — the sidecar exactly
   `PAGE_TOKENS × DRAFT_COMMITTED_RECORD_BYTES = 64 × 500 = 32,000` bytes —
   with one `storage_offset` and one non-zero `sha256` per piece.
3. `encode_draft_sidecar_payload` is token-major: per token, 368 draft-KV
   bytes (`KV_RECORD_BYTES`) immediately followed by 132 draft-indexer
   bytes (`INDEXER_RECORD_BYTES`); `500 = 368 + 132` is pinned in `lib.rs`.
4. `decode_draft_sidecar_payload` splits each 500-byte token record back
   into the two logical planes; the round-trip test
   (`draft_sidecar_is_one_token_major_round_trip_payload`) passes and the
   durable representation stays the single combined payload.
5. Piece ranges use `checked_add` (overflow → `TierError::Overflow`) and a
   correct pairwise interval-intersection rejection
   (`offset < prior_end && start < end`), covered by
   `tier_piece_ranges_must_not_overlap`.
6. `TierJournal::piece_durable` verifies the observed digest against the
   record's expected digest and rejects duplicates; `publish` refuses until
   every declared piece (including the sidecar) is durable; `recover`
   replays only fully durable, published transactions — begun or partial
   records remain invisible.
7. The incompatible development journal change is explicit:
   `JOURNAL_MAGIC = "GLTJRNL2"`, `JOURNAL_VERSION = 2`, and the test
   `v1_journal_fails_closed_after_unified_draft_sidecar_change` proves a
   syntactically valid `GLTJRNL1` record fails closed.

As instructed, no inference is made that `FileTierStore` serializes the
format document's complete 4,096-byte sealed page header; this acceptance
covers the durable tier model and journal only.

## Decision 4 verification — non-overlapping FC2 materialization

For `P = assignments × 6144` elements, verified in
`cutlass_nvfp4_fc2_control.cu`, `abi.rs`, `ffi.rs`, `nvfp4_routed_fc2.cu`:

1. The live FP32 assignment output owns bytes `[0, 4P)` of
   `assignment_down_f32`.
2. The BF16 materialization plane starts at `+ P × sizeof(float)` = byte
   `4P` and occupies `[4P, 6P)`.
3. Dense `ptr_d` is exactly `base + 4P`; grouped `ptr_d[group]` is
   `base + 4P + begin × 6144 × sizeof(uint16_t)` — per-group disjoint
   sub-ranges of the BF16 plane only.
4. Both expansion kernels (`expand_scaled_projection`,
   `_grouped`) read only `materialized[linear]` (a pointer into `[4P,6P)`)
   and write only `output[linear]` in `[0, 4P)` via a grid-stride loop that
   touches each element exactly once. Source and destination are fully
   disjoint, so no inter-CTA ordering is required and no CTA can clobber
   another's unread source — the r1 BLOCKER is structurally eliminated, not
   just reordered.
5. Rust allocates the combined region as
   `assignments × HIDDEN × (4 + 2)` = `6P` bytes.
6. Workspace formulas match term-for-term: Rust adds
   `materialized_down_bf16 = assignments × HIDDEN × 2` alongside
   `assignment_down = assignments × HIDDEN × 4`; C adds
   `assignments × kHidden × sizeof(uint16_t)` alongside
   `× sizeof(float)`. Exact parity across all eight terms.
7. The corrected source compiles into the pinned `sm_120f` library with the
   expected OMMA count (cn4 evidence, byte-identity proven above).

Phase-B script: requires the dedicated root `fable-manifest-abi-v022-r2.md`
tracked by Git, the exact token via `grep -Fxq`, and all four
`require_hash` contract attestations — all before any GPU inventory or
launch step.

## Findings

### BLOCKER / MAJOR

None.

### MINOR

**MINOR-1 (observation, non-blocking).** `expand_scaled_projection` reads
`activation_globals[assignment]` with `assignment = linear / kHidden`
recomputed per element — an integer divide per element on the expansion
path. Correct, and irrelevant for a development control; hoist per-row if
this pattern reaches a performance path.

**MINOR-2 (carried context).** The first review's five other MINORs on
unchanged files (r1 M5–M7 etc.) remain as recorded there; none touches the
r2 scope or blocks M2.

### QUESTION

None.

## Required answers

1. **Does the first review's Decision 1 answer remain an unqualified
   YES?** YES — every Decision 1 input is byte-identical to the first
   review's verified bytes (attestation hashes cross-checked against its
   table); nothing in the r2 diff touches the operation manifest facts.
2. **Is the corrected combined draft-sidecar durable model accepted for
   M2?** YES — all seven items verified directly; the r1 MAJOR M1 is
   closed with a faithful single-piece token-major representation, atomic
   publication, and an explicit fail-closed journal version break.
3. **Does the first review's Decision 3 answer remain an unqualified
   YES?** YES — conversion-path inputs are byte-identical; the budget
   remains `conversion_allowed=false` in its non-authorizing posture.
4. **Is the FC2 data race eliminated and the corrected routed-MoE
   development control accepted for M2?** YES — the expansion is now a
   pure disjoint-plane copy-scale with proven 4P/2P partition, exact
   Rust/C workspace parity, and compile evidence for the exact bytes.

## Acceptance

engine-v0-sha256=efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
format-v0-sha256=619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a
operation-manifest-sha256=8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6
profile-budget-v0-sha256=028516adc04d454317e1b76a3147be4807c3ed3ce371e1d43aead3396270400d

manifest-abi-v0.2.2-accepted
