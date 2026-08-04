# EXL3 mixed-K source and kernel contract v1 r3 amendment

Date: 2026-08-04

Status: corrective design candidate; implementation is blocked on adversarial
acceptance

Base contracts:

- `docs/exl3-mixed-k-source-and-kernel-v1.md`
- `docs/exl3-mixed-k-source-and-kernel-v1-r2.md`

## Scope and precedence

R2 corrected the target/draft layer boundary and all byte arithmetic, but it
required a rank-common partition hash without defining its preimage. It also
did not say whether non-`k` tier fields were admitted, how duplicate JSON keys
were handled, or which functional route executes K4 assignments while the
separate grouped K3 optimization remains gated. Those omissions make the r2
CPU implementation and TP4 consensus under-specified.

This amendment is normative for tier parsing, canonical tier identity,
per-step K3/K4 partition identity, and the minimum functional execution route.
It retains r2's 75 target layers plus one draft layer, descriptor counts,
source-plane byte arithmetic, descriptor-derived precision, native metadata,
outer K3/K4 dispatch, no-repack rule, replay matrix, and gate order.

The r2 handoff is superseded for implementation authorization. An r2 result
may be retained as review history but cannot open code or CUDA work.

## Strict TR3 tier-map view

The raw `tier_bitmap.json` bytes are authenticated before they are interpreted.
The Rust parser consumes UTF-8 bytes directly, rejects a BOM, trailing bytes,
and duplicate object keys at every depth, and never constructs a permissive
`serde_json::Value` first. JSON object order and number spelling remain part of
the authenticated raw-file digest but do not control the execution plan.

The root object has exactly the 76 decimal string keys `"3"` through `"78"`.
Leading zeroes, signs, whitespace in a key, missing layers, and extra keys are
invalid. Every expert-index array is in expert-ID order and has exactly 256
entries.

Target records for layers 3 through 77 have exactly these keys:

```text
expert_rel_rt_mse
expert_rel_rt_mse_donor_k3
k
keep_nvfp4
tail_tr3
```

`k` has exactly 192 integer `3` values and 64 integer `4` values.
`keep_nvfp4` is empty and `tail_tr3` is exactly integers 0 through 255.
Both error arrays contain exactly 256 finite JSON numbers. Their values are
authenticated diagnostic inputs but never select a runtime width.

Draft record 78 has exactly these keys:

```text
expert_rel_rt_mse
keep_nvfp4
tail_tr3
```

It has no `k` or donor array, its error array contains exactly 256 finite JSON
numbers, `keep_nvfp4` is empty, and `tail_tr3` is exactly integers 0 through
255. Its 256 execution widths are derived as K3 only after every projection
and rank descriptor agrees.

The exact key sets and array shapes above are checkpoint-profile admission
rules, not a general EXL3 parser. Any future tier schema requires a different
profile identity and review. A JSON number that cannot be represented as a
finite binary64 value is rejected even though the error arrays are not used
for dispatch.

## Canonical target/draft tier identity

After publisher authentication and the complete descriptor census, every
rank independently constructs the same address-free binary preimage:

```text
"glmaxx.exl3.tr3-tier-plan.v1\0" ||
u16_le(1) ||
source_profile_sha256 ||
publisher_manifest_sha256 ||
tier_file_sha256 ||
u16_le(76) ||
for layer in 3..=78:
  u8(layer) ||
  u8(layer <= 77 ? 1 : 2) ||       # 1 target, 2 recurrent draft
  u16_le(256) ||
  for expert in 0..256:
    u8(descriptor_derived_bits[layer, expert])
```

The SHA-256 of those bytes is `tr3_tier_plan_sha256`. Every width byte must
equal all twelve physical observations for that `(layer,expert)` tuple: three
projections times four ranks. Target width bytes must also equal `k`; draft
width bytes must all be 3. The input identities are nonzero and already
consensus-equal. No filename, JSON iteration order, float spelling, allocation
address, TP rank, or CUDA object appears in this digest.

This digest binds the exact 75-by-192/64 target membership and the uniform-K3
draft membership. The complete raw tier digest separately binds all diagnostic
fields, so canonicalizing the selection view does not discard source identity.

## Canonical per-step mixed-K partition

The target-layer router first produces its existing rank-common compacted
assignment table in ascending `(expert_id, token_row, route_slot)` order. Each
`(token_row, route_slot)` destination occurs exactly once. The source ordinal
is its zero-based index in that complete table.

One partition record is exactly 16 bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 4 | source ordinal |
| 4 | 4 | token-row ordinal |
| 8 | 2 | expert ID |
| 10 | 1 | route slot |
| 11 | 1 | descriptor-derived bits, 3 or 4 |
| 12 | 4 | reserved zero |

For one target or draft layer, the common receipt is:

```text
SHA256(
  "glmaxx.exl3.mixed-k-partition.v1\0" ||
  u16_le(1) ||
  u8(layer_id) ||
  u8(layer_id <= 77 ? 1 : 2) ||
  u64_le(step_sequence) ||
  u32_le(real_rows) ||
  u32_le(assignment_count) ||
  step_input_sha256 ||
  router_table_sha256 ||
  tr3_tier_plan_sha256 ||
  target_or_mtp_program_sha256 ||
  backend_policy_sha256 ||
  u32_le(k3_count) ||
  K3 records in increasing source-ordinal order ||
  u32_le(k4_count) ||
  K4 records in increasing source-ordinal order
)
```

All digests and `step_sequence` are nonzero. `assignment_count` is exactly
`real_rows * 8` for the pinned top-8 route and equals `k3_count + k4_count`.
Every source ordinal from zero through `assignment_count - 1` occurs once
across the two bins;
token rows are below `real_rows`, route slots are `0..7`, experts are
`0..255`, and each bits byte equals the admitted tier plan. Draft layer 78
requires `k4_count == 0`.

All four ranks derive and compare this address-free receipt before argument
upload. Rank-local pointer arrays and their readback digests are separate
owner-thread receipts and are never compared across GPUs. Filtering never
renumbers assignments: every output scatters through its retained source
ordinal and unique `(token_row,route_slot)` destination. Concatenating K3 and
K4 output positions is invalid.

Mutation tests must reject changed bin order, changed source ordinal, duplicate
or missing destination, cross-bin duplication, stale step/tier/program/policy
identity, K4 draft membership, rank-local width choice, and a partition whose
counts are arithmetically consistent but whose records do not reproduce the
full router table.

## Minimum functional K4 route

The first functional mixed-K route does not depend on the separately reviewed
grouped K3 optimization. It uses source-projection ABI v2 with one outer
dispatch into compile-time-specialized K3 or K4 controls for gate, up, and
down. Assignments may be batched by `(bits,expert)` while preserving the
partition record's source ordinals. The K3 and K4 controls consume the same
source-order planes and emit the same FP16 projection boundary required by the
target-layer contract; SwiGLU, route weighting, scatter-add order, and the TP4
collective are unchanged.

The backend policy binds both specialization capability digests and the exact
functional route before graph construction. Startup fails if either target
specialization is absent. A module load failure, invalid descriptor, or launch
failure is fatal to the common step; it cannot switch one rank, expert, or
projection to another route.

After its own acceptance, the grouped paired K3 gate/up path may replace only
the K3 gate/up portion under a new backend-policy digest. K4 gate/up and both
down widths remain on an independently qualified source-projection route until
separate optimized kernels pass their gates. The matched control retains the
same partition bytes, projections, precision, accumulation order, and output
scatter. A grouped-K3 benchmark is never presented as a complete mixed-K
layer result.

## Corrected proof and gate sequence

After adversarial acceptance of r1+r2+r3, the CPU implementation must add to
r2's proof matrix:

1. duplicate-key, exact-key-set, key-spelling, array-shape, integer, finite-
   number, tail-membership, and unknown-field mutations;
2. byte-exact tier-plan encode/hash vectors and mutations of each identity,
   layer-kind byte, expert width, and ordering rule;
3. exhaustive partition construction for rows 1 through 8 and targeted
   3,072-row arithmetic, including all-K3, all-K4, 192:64, empty-expert, and
   maximally skewed router distributions;
4. independent reconstruction of the complete router table from both bins;
5. four-rank consensus simulation with rank-local pointer/address variation;
   and
6. functional gate/up/down K3 and K4 CPU projection controls whose scatter
   matches the unpartitioned oracle bit-for-bit.

Only after a separate implementation review may the K3/K4 source-projection
controls launch on SM120. The required hardware order remains K3 regression,
K4 control, real target K3/K4 and draft K3 tensors, matched mixed target-layer
replay, recurrent-draft replay, authenticated checkpoint smoke, MTP0 quality,
then MTP3. Grouped K3 is an optional measured optimization inside that order,
not a prerequisite for functional K4 execution.

Acceptance of this amendment authorizes only the coordinated Rust parser,
tier-plan, partition, accounting, and CPU proof. It does not accept those
bytes, authenticate the real checkpoint, authorize CUDA, accept a layer or
model output, or establish quality, capacity, reload, concurrency, or speed.
