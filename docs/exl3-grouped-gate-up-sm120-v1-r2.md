# SM120 EXL3 grouped paired gate/up decode v1 r2 amendment

Date: 2026-08-04

Status: corrective design candidate; adversarial acceptance required before
CPU or CUDA implementation

Base contract: `docs/exl3-grouped-gate-up-sm120-v1.md`

## Scope and precedence

This amendment closes ambiguities found during pre-review of the grouped
paired gate/up design. It is normative wherever it conflicts with or adds to
the base contract. The model geometry, K=3-only scope, descriptor field
layout, three computational kernels, numerical recurrence, traffic count,
gate sequence, and nonclaims remain unchanged.

No implementation or device launch is authorized by this amendment.

## Rank-local addresses and rank-common identity

The six device pointer tables contain rank-local virtual addresses. Their raw
bytes and their readback hashes are therefore rank-local receipts and MUST NOT
be compared across ranks.

Before upload, every pointer-table entry is derived from a rank-manifest span
whose logical identity is:

```text
(rank, layer, expert, projection, component,
 source_file_sha256, source_offset, source_length,
 resident_arena_generation, resident_offset, resident_length)
```

All four ranks instead agree on the canonical digest of those logical
bindings, the complete four-rank manifest-set digest, the target-program
digest, and the resident-generation digest. Each owner thread independently
uploads its rank-local table, reads back all 2,048 bytes, and verifies its
rank-local table hash against its own construction receipt. A table pointer
cannot be adopted from another rank or weight generation.

For every active K=3 expert, the adopted spans are exact:

| Component | Alignment | Bytes |
|---|---:|---:|
| gate/up trellis | 4 | 1,179,648 each |
| gate/up SUH | 2 | 12,288 each |
| gate/up SVH | 2 | 1,024 each |

Every active entry must be nonzero, aligned, and contained completely inside
the corresponding authenticated resident span. Inactive entries are zero.
Rust validates these facts from the immutable owner-thread allocation receipt
before entering the grouped FFI. The launcher never attempts to dereference a
device pointer table from the host.

## Exact route identity

The descriptor's four `route_digest_u64` words are the four consecutive
little-endian U64 decodings of a 32-byte SHA-256 digest. C and Rust compare
the same 32 in-memory bytes; numeric U64 formatting is not part of the ABI.

The digest preimage is exactly:

```text
"glmaxx.sm120.exl3.grouped_paired_gate_up.route.v1\0" ||
u32_le(rows) || u32_le(assignments) || u32_le(active_expert_count) ||
u64_le(sequence) ||
full_router_table_sha256 || backend_policy_sha256 ||
four_rank_manifest_set_sha256 || target_program_sha256 ||
resident_generation_sha256 ||
for i in 0..assignments:
  u16_le(route_experts[i]) || u32_le(route_tokens[i]) || u8(route_slots[i]) ||
for expert in 0..257: u32_le(expert_offsets[expert]) ||
for i in 0..active_expert_count: u16_le(active_experts[i])
```

`sequence` is nonzero and must equal the adopted step command. The full
router table and backend policy are already rank-common inputs; the filtered
arrays are recomputed independently on every rank and must reproduce this
digest before upload. The digest is checked against the argument-upload
receipt immediately before launch.

K=3 output index `i` remains keyed by
`(route_tokens[i], route_slots[i], route_experts[i])`. A later mixed-K
consumer scatters K=3 and K=4 results into the unique `(token, route_slot)`
destination. It may not concatenate filtered output arrays or infer the
original compacted ordinal from filtered position alone.

## Adopted spans and non-aliasing

The two scratch pointers describe one contiguous adopted workspace, not two
independent allocations:

```text
rotated_bytes   = 2 * assignments * 6144 * 2
projected_bytes = 2 * assignments *  512 * 2
projected_f16   = rotated_input_f16 + rotated_bytes
workspace_bytes = rotated_bytes + projected_bytes
                = 26,624 * assignments
```

All additions and multiplications are checked in U64 before any CUDA call.
The rotated pointer is the adopted workspace base, the projected pointer must
equal the derived address above, and the complete range must fit in the fixed
rank workspace receipt.

The launcher also validates these exact byte ranges:

```text
input_f16       rows * 6144 * 2
gate_output_f16 assignments * 512 * 2
up_output_f16   assignments * 512 * 2
route_experts   assignments * 2
route_tokens    assignments * 4
route_slots     assignments
expert_offsets  257 * 4
active_experts  active_expert_count * 2
pointer_table   256 * 8, for each of six tables
validation      4
```

Input, both output planes, route arrays, offset/list arrays, all pointer
tables, validation word, and workspace must be contained by their adopted
allocation receipts. Every writable range is disjoint from every other range
and from every read-only range. Gate and up outputs are mutually disjoint.
No arithmetic wrap, partial span, or unproved alias reaches the FFI.

## Deterministic validation protocol

The validation word is a deterministic bit mask. Each error class owns one
fixed bit and device code reports failures only with `atomicOr`; it is never
used as a first-writer-wins integer code.

After all synchronous descriptor, device, receipt, and alias checks pass, the
grouped entry point enqueues this exact sequence on the caller-owned stream:

1. `cuMemsetD32Async(validation_error_u32, 0, 1, stream)`;
2. grouped input rotation kernel;
3. paired staged projection kernel; and
4. grouped output rotation kernel.

The memset is an asynchronous memory operation, not a fourth computational
kernel. Stream order guarantees that each successor observes every validation
bit set by its predecessor.

Every CTA proves its own indices before the first potentially invalid load.
Input/output rotation CTAs use only CTA-common assignment, projection, and
H128-block identities, so an invalid CTA takes one uniform no-access return.

The projection CTA validates its active expert and both adjacent offsets in
thread zero, publishes the result through shared memory, executes one CTA
barrier, and then either returns uniformly or proceeds. Each warp validates
its optional assignment in one lane and broadcasts the result. An invalid
warp sets its error bit and remains present at every stage barrier while
issuing no assignment-dependent load, decode, accumulation, or store. Thus a
bad route in one warp cannot strand another warp at `__syncthreads`.

At entry, the projection and output-rotation kernels uniformly no-op if a
prior kernel left the validation mask nonzero. A validation failure discovered
inside the projection kernel may race only with other locally validated
projection CTAs; each CTA is independently memory-safe, the output rotation
observes the completed projection mask and no-ops, and the caller discards all
outputs. Rust reads the validation word after stream completion and converts
any nonzero mask into a fatal step error before an output buffer is consumed.

Host-side validation failure enqueues nothing. CUDA API failure stops further
enqueue, poisons the step generation, and follows the rank executor's
collective failure protocol.

## Exact staged source schedule

The base contract's load mapping applies once for each of 48 consecutive
eight-K-tile stages. The complete paired projection loop is:

```text
gate_accumulator = +0.0f
up_accumulator   = +0.0f

for stage_group in 0..48:
  for linear = threadIdx.x; linear < 384; linear += 256:
    projection = linear / 192
    within      = linear % 192
    stage_tile  = within / 24
    word        = within % 24
    k_tile      = 8*stage_group + stage_tile
    source_u32  = ((k_tile * 32 + blockIdx.x) * 24) + word
    stage[projection][stage_tile][word] =
      trellis_table[projection][active_expert][source_u32]

  __syncthreads()

  for stage_tile in 0..8:
    k_tile = 8*stage_group + stage_tile
    for k_local in 0..16:
      k = 16*k_tile + k_local
      accumulator[projection] = __fadd_rn(
        accumulator[projection],
        __fmul_rn(rotated[projection,assignment,k],
                  decode(stage[projection][stage_tile],
                         k_local,n_local)))

  __syncthreads()
```

Both accumulators live across all 48 stages and are rounded to FP16 exactly
once after all 6,144 K positions. The stage is never read before the first
barrier or overwritten before the second. All 256 threads execute both
barriers for every valid CTA, including inactive or invalid assignment warps.

The load-loop bijection remains 384 distinct words: threads 0 through 127
load two words and threads 128 through 255 load one. `blockIdx.x` is the
16-column N tile and is identical for the gate and up halves; only the
projection-selected resident pointer differs.

## Corrected review gate

Adversarial review must additionally prove:

1. raw device pointer hashes are never compared across ranks;
2. logical binding consensus plus rank-local readback receipts bind the exact
   resident generation and spans;
3. route digest serialization is unambiguous and mixed-K scatter cannot
   confuse filtered positions;
4. every adopted range is checked, contained, and nonaliasing;
5. every predecessor failure prevents successor scratch consumption;
6. no validation path can diverge around a CTA barrier; and
7. all 48 stage groups load and consume exactly eight consecutive K tiles
   while preserving one ascending-K accumulator per projection.

Only after the base contract plus this amendment receive one unqualified
design acceptance may the independent Rust CPU proof begin.

