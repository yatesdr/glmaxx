# W4A16/NF3 resident-loader readiness audit

Date: 2026-08-03

Status: static implementation-readiness audit; no design acceptance, code
implementation, checkpoint conversion, GPU launch, or performance evidence

## Verdict

The corrected W4A16/NF3 execution design can reuse the existing immutable
rank-reader, CUDA arena, and persistent-owner machinery. It does not require
adding projection or layout fields to the existing 64-byte `TensorArenaEntry`:
the owner retains the authenticated `NativeRankReader`, and
`validate_resident_tensor_bindings` already joins each descriptor and manifest
semantic with its adopted device spans.

The current path nevertheless admits only the capacity-EXL3 profile. It cannot
open, plan, allocate, or execute the r2 hybrid image. The required work is a
versioned hybrid admission path plus a composite pre-publication typestate;
changing the existing EXL3 meanings in place is forbidden.

No new arithmetic contradiction was found in:

- `nf3-nvfp4-hybrid-source-and-kernel-v1-r2.md`;
- `nf3-nvfp4-native-rank-manifest-v1-r2.md`; or
- `sm120-w4a16-nf3-fused-moe-v1-r2.md`.

Their acceptance tokens still gate the corresponding CPU implementations.

## Reusable ownership boundary

The following current properties are sufficient and must be preserved:

1. `NativeRankReader` opens a no-follow, single-link regular file, authenticates control
   regions, retains descriptors, names, codec metadata, and the validated
   manifest, and checks that the file did not change.
2. `PreparedCudaRank` streams authenticated planes into quarantined allocations
   owned by the rank thread.
3. `CudaWeightArena::tensor_binding` resolves an authenticated tensor ID to
   bounded metadata, primary, and auxiliary device spans without a caller
   offset or name lookup.
4. `NativeCheckpointRankExecutor` retains both the reader and arena on the
   owner thread. Its current finalization already cross-checks descriptor,
   manifest semantic, arena entry, byte length, codec, role, flags, and
   alignment.
5. `execute_bound` fails closed with `NATIVE_PROGRAM_NOT_IMPLEMENTED`; there is
   no CPU fallback or false checkpoint-smoke path.

The hybrid compiler can therefore derive projection, expert, layouts, plane
lengths, hashes, and scalar bits from authenticated host records, then resolve
only device addresses through the quarantined arena. It must not accept a
detached caller descriptor, metadata blob, tier map, or pointer.

## Exact gaps in the current tree

| Area | Current fail-closed state | Required versioned hybrid state |
|---|---|---|
| Container header | `container.rs` and `native_reader.rs` require format minor 2 and reject header flag bit 5 | Admit only the reviewed minor-3 header with exact flags 59 and a profile-4 manifest; retain the minor-2 path unchanged |
| Codec vocabulary | Only plain, `0x0100`, `0x0101`, and `0x0200` exist | Add distinct `0x0102` ModelOpt-W4A16 and `0x0300` NF3 codecs; never alias either to an old NVFP4 codec |
| Codec metadata | The reader knows old `Nvfp4Metadata` and `Exl3Metadata` | Decode and mutation-test both exact 192-byte r2 records, including CRC, layouts, dimensions, rank, role, plane bytes, source digests, scalar arity, finite-positive scalar rules, and reserved zeros |
| Rank manifest | `RankWeightProfile` has only `CapacityExl3`; the validator accepts only schema v0.2.2 and its 128-byte semantics | Add profile enum 4 and the reviewed hybrid schemas/domains with the 224-byte common semantic catalog and exact 40,129-entry inventory |
| Load profile | `LoadProfile::HybridServe` currently serializes as 3 and is never mapped from a manifest | Add a distinct value-4 hybrid profile/domain; do not silently reinterpret value 3 |
| Load contract | `build_rank_set_load_plan` always uses `pinned_exl3_rank_plan` | Build the hybrid contract from the accepted, authenticated hybrid catalog and descriptors; reproduce 94,006,274,048 weight bytes and 9,961,408 device-metadata bytes per rank |
| Startup | `load_native_checkpoint` requires four capacity-EXL3 memory plans and `pinned_exl3_weight_policy_sha256` | Select the policy and plan builder from the authenticated profile before allocation and require one common profile on all ranks |
| Memory ledger | The executable JSON budget is capacity-EXL3-only; `model_metadata` is an undifferentiated term | Add an exact hybrid ledger and explicitly charge the 2,573,312-byte device binding table, retained host catalog/metadata, graph/status spans, and maximum 126,225,408-byte tiled workspace once each |
| Resident program | `NativeWeightState::Resident` owns only `CudaWeightArena` | Own an immutable composite of weight arena, metadata arena, binding table, target/MTP identities, and four-rank adoption receipt |
| Execution | `execute_bound` returns backend `-1` | Materialize accepted graph plans from executor spans and submit only the target/MTP graph-node ABI; direct kernel calls remain diagnostics |

## Pre-publication transaction requirement

The binding table cannot be constructed after the current `adopt` transition.
At that point another rank may already have published its weight arena, while
a local table allocation, upload, validation, or digest check can still fail.
That would leave the four ranks in different resident states.

The hybrid path must introduce a composite quarantined typestate:

```text
authenticated reader + accepted common semantics
  -> prepared weight/metadata arenas
  -> owner-resolved 2,573,312-byte binding table
  -> local materialization receipt
  -> four-rank common-semantic and ordered-local-receipt consensus
  -> one composite adoption generation
  -> executable resident program
```

Before consensus, any failure releases only GLMAXX-owned quarantined resources.
After consensus, all four ranks publish the same resident generation. A rank
cannot publish an arena without its table or publish a table with different
target, MTP, numerical-policy, or common-semantic identities.

The current `PreparedRankReceipt` and `AdoptedRankSetReceipt` may remain the
EXL3 v1 records. The hybrid path should use successor receipts rather than
changing their canonical bytes. The local receipt binds the existing load-plan
identity plus the rank-local table materialization digest; the common receipt
binds `HybridResidentWeightSetIdentityV2` and the common binding-semantic
digest. CUDA addresses occur only in ordered rank-local receipts.

## Binding compiler data flow

For each physical routed pair, the owner must:

1. select the two tensor IDs from the accepted 40,129-entry UTF-8-ordered
   catalog, never from request input or a runtime name scan;
2. cross-check layer, global expert, role, projection, codec, value layout,
   scale layout, logical and padded shape, metadata hash, and plane hashes;
3. decode scalar bits from the reader-retained 192-byte metadata and enforce
   codec/projection arity;
4. obtain primary, auxiliary, and metadata addresses only through checked
   quarantined-arena span resolution;
5. encode the address-free 88-byte semantic record and compare its digest on
   all ranks;
6. encode the rank-local 128-byte binding and 64-byte directory records,
   proving every address and extent lies in its owner allocation;
7. upload and read back the complete table, then hash the exact readback bytes;
   and
8. include target layer 78 in the MTP-program identity even when the active
   serving posture is MTP0.

Protected records remain part of target/MTP program identity but do not enter
the routed 19,456-entry table. Each binding's metadata address must refer to
the authenticated device metadata record, while its scalar-bit fields are
copied exactly from host-validated metadata rather than a mutable side
allocation. NF3 scalar fields remain exact positive zero.

## Implementation cut after design acceptance

The smallest reviewable sequence is:

1. **Hybrid metadata and catalog proof (`glm-format`).** Add standalone r2
   metadata codecs and 224-byte semantic encoding, with golden vectors,
   exhaustive layout checks, overflow tests, and mutation tests. No file
   publication.
2. **Minor-3 reader and manifest proof (`glm-format`).** Add a profile-dispatched
   header/manifest validator and exact inventory/range planner. Keep minor 2
   byte-for-byte compatible and fail unknown combinations closed.
3. **Hybrid load-plan proof (`glm-engine`).** Add the value-4 load domain,
   descriptor-derived arena plan, exact HBM ledger, and four-rank consensus
   tests. Test the first and last metadata tail and one-byte-short arenas.
4. **Composite quarantine proof (`glm-engine`).** Add owner-only prepared table
   materialization, successor receipts, rollback, and atomic TP4 adoption with
   injected failure at every transition.
5. **Binding-table CPU proof (`glm-engine`/`glm-reference`).** Reproduce all
   19,456 semantics, 76 directories, exact table bytes and digests; mutate every
   field class and prove rank-common versus rank-local separation.
6. **CUDA ABI implementation (`glm-cuda` and `kernels/sm120`).** Only after its
   implementation review, add the exact records, graph-node integration, and
   codec-specialized diagnostic launchers.
7. **SM120 gates.** Run actual-shape microbenchmarks, then one sparse-layer TP4
   replay, checkpoint smoke, KLD, and matched end-to-end decode in that order.

Each step gets its own adversarial implementation/proof review. A later step
cannot be used as evidence for an earlier gate.

## Cold-start and hot-reload consequence

The current startup opens and validates each rank control plane once in the
coordinator and again in its persistent worker. That duplicates only control
work, not the full payload stream, but it should be removed after the first
correct hybrid smoke by transferring an authenticated plan/identity into the
owner open rather than reparsing it. Timing must keep identity/index, storage
read, staging/H2D, module setup, arena validation, collectives, graphs, KV, and
health publication separate.

The composite resident object is the required hot-weight boundary. Compatible
module/config generations may replace graphs only after matching
`HybridResidentWeightSetIdentityV2`; they must neither rebuild the binding
table nor read or upload any weight or codec-metadata byte.

## Immediate gate state

As of this audit, the critical acceptance tokens for the r2 source/kernel,
r2 native manifest, corrected W4A16/NF3 execution design, target-layer
execution, and MTP-layer execution are absent. This document authorizes no
implementation or cn4 allocation. The native worker remains deliberately
nonfunctional until those reviewed contracts open the corresponding CPU and
device phases.
