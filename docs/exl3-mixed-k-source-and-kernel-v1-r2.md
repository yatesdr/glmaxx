# EXL3 mixed-K source and kernel contract v1 r2

Date: 2026-08-03

Status: corrective design candidate; implementation is blocked on adversarial
acceptance

## Supersession

This document replaces the source-membership and byte-accounting authority in
`docs/exl3-mixed-k-source-and-kernel-v1.md`. The first candidate incorrectly
required one 256-entry `k` array for layer 78 and treated it as a 192:64 target
mixture. The authenticated source instead contains 75 mixed target sparse
layers plus one uniform-K3 recurrent draft layer. The original review handoff
must not be accepted or used to open implementation.

All unaffected v1 requirements remain mandatory: descriptor-derived width,
exact component validation, immutable authenticated source identity, checked
native accounting, compile-time K=3/K=4 specializations, deterministic TP4
binning, no inner-loop width branch, no dense reconstructed-weight cache, and
no rank-local fallback.

## Correct source membership

The complete MTP-capable source has two distinct domains:

```text
target layers                 0 through 77
target sparse layers          3 through 77   (75 layers)
recurrent draft layer            78   (one layer)
```

The content-authenticated tier map must have exactly the 76 top-level integer
keys 3 through 78.

For every target sparse layer 3 through 77:

- `k` must exist and contain exactly 256 integers;
- exactly 192 entries must equal 3 and 64 must equal 4;
- each entry must equal the width independently derived from gate, up, and
  down trellis descriptors on all four TP ranks; and
- any missing, extra, non-integral, unsupported, projection-inconsistent, or
  rank-inconsistent value fails complete checkpoint admission.

For recurrent draft layer 78:

- `k` must be absent;
- `keep_nvfp4` must be empty and `tail_tr3` must be exactly experts 0 through
  255;
- every gate, up, and down trellis descriptor on ranks 0 through 3 must derive
  K=3; and
- any K=4 descriptor, missing expert, extra expert, nonempty NVFP4 membership,
  or projection/rank disagreement fails complete checkpoint admission.

The tier-map schema is corroboration, not caller-controlled width authority.
Physical width always comes from the validated trellis shape
`[logical_k/16, logical_n/16, 16*bits]`, with `bits` exactly 3 or 4. The
publisher manifest, index, tier map, shard headers, and payload hashes remain
separate mandatory identities.

## Correct accounting

Per projection, expert, and rank:

| width | trellis bytes | rotation + marker bytes | source-plane bytes |
| --- | ---: | ---: | ---: |
| K=3 | 1,179,648 | 13,316 | 1,192,964 |
| K=4 | 1,572,864 | 13,316 | 1,586,180 |

Only the 75 target sparse layers contain K4 experts. Relative to a uniform-K3
target-plus-draft source, the exact per-rank delta is:

```text
393,216 * 75 * 64 * 3 = 5,662,310,400 bytes
```

The complete target-plus-draft routed source-plane total per rank is:

```text
1,192,964 * 76 * 256 * 3 + 5,662,310,400
  = 75,293,233,152 bytes
```

The planner must retain target and draft membership separately even when both
dispatch to the K3 specialization. It may not use 3.25 bpw, 76 mixed layers,
or an average-width estimate as allocation authority.

## CPU proof amendment

Before CUDA implementation, the CPU proof must add:

1. exact tier-map schema checks for target `k` presence and draft `k`
   absence;
2. a complete metadata census of all 233,472 trellis tensors across layers
   3 through 78, three projections, and four ranks;
3. exact target counts of 172,800 K3 and 57,600 K4 trellis tensors;
4. exact draft counts of 3,072 K3 and zero K4 trellis tensors;
5. target and draft descriptor/tier mutations, including adding a draft `k`,
   removing a target `k`, assigning draft K4, or moving one target expert
   between widths;
6. the corrected 5,662,310,400-byte delta and 75,293,233,152-byte routed
   source total, with checked overflow behavior; and
7. real reconstruction for target K3, target K4, and draft K3 representatives
   on both rank 0 and rank 3 for gate, up, and down.

The read-only census in `docs/cn4-tr3-tier-boundary-20260803.md` is discovery
evidence only. The implementation must reproduce it through reviewed Rust
parsers before it can become checkpoint admission evidence.

## CUDA and execution amendment

The v2 source-projection ABI still dispatches once, outside the weight loop,
to separate compile-time K=3 and K=4 kernels. Target routed execution creates
canonical K3 and K4 bins from the fully admitted target map. Draft execution
uses only the K3 specialization and rejects any K4 plan before launch.

All ranks hash one common target/draft partition plan. Canonical token/slot
order, collective routes, precision membership, and accumulation order remain
identical across ranks. A rank-local fallback, width substitution, or silent
draft treatment as a target layer is forbidden.

The first matched layer replay must report target K3, target K4, target
binning, recurrent-draft K3, collective, and framework time separately. A
target-only microbenchmark cannot qualify the MTP draft path.

## Gate sequence

After adversarial acceptance of this correction:

1. implement the target/draft tier parser, accounting, and CPU mutation proof;
2. obtain implementation review with pinned real-source evidence;
3. launch K3 and K4 controls on SM120, including a real draft K3 tensor;
4. benchmark specialized target bins and draft K3 separately;
5. run the mixed 192:64 target-layer replay and recurrent draft replay;
6. continue to authenticated checkpoint smoke, MTP0 quality, then MTP3.

Acceptance of this design authorizes no implementation result, conversion,
CUDA launch, layer, checkpoint, quality, capacity, or performance claim.
