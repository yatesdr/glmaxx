# Offline serving foundation

Date: 2026-07-29

Status: implemented CPU/reference candidates; no new GPU evidence

This work advances serving capability without using cn4. It does not skip the
required SM120 microbenchmark, one-layer replay, checkpoint smoke, quality,
or matched end-to-end gates.

## Implemented contracts

| Area | Rust implementation | Contract now enforced |
|---|---|---|
| EXL3 source payload | `glm-format::exl3` | pinned three-bit MCG decode, component lengths, FP16/H128 reference path, deterministic metadata, corruption rejection |
| weight policy | `glm-engine::weight` | immutable `(layer,expert,role)` codec, target plus optional layer-78 inventory, protected allocations, per-rank physical fit, policy hash |
| process startup | `glm-engine::startup` | seven ordered gates, four-rank consensus, identical immutable digests, terminal failure |
| batching | `glm-scheduler` | captured-shape admission, multi-sequence prefill/decode/verify, configurable MTP0–6 cohorts, decode burst bound, weighted tenant ordering |
| cancellation | `glm-scheduler` | cancellation becomes visible only at the next collective-safe step boundary |
| sampling | `glm-reference::sampling` | sharded greedy, exact bounded top-k/top-p, unbounded distributed mass for `top_k=0,p=1`, speculative residual sampling |
| tokenizer/text | `glm-tokenizer` | exact pinned bundle hashes, fixed chat template, mapped-vocabulary mask, incremental UTF-8 and cross-token stops |
| prefix index | `glm-cache::prefix` | chained 64-token content keys, longest full-page match, reference counts, DCP-neutral namespace |
| DRAM/NVMe metadata | `glm-cache::tier` | exact target/indexer/draft piece geometry, alignment, checksums, durable-piece journal, publish-after-all, crash replay |
| FC2 and layer boundary | `glm-reference::routed_fc2` | activation ordering, rank-local down projection, route weight after FC2, shared/routed combination, fixed-rank TP reduction |
| sparse-layer descriptors | `glm-reference::routed_fc2` | target layers 3–77 and full attention-plus-MoE MTP layer 78, exact TP4 dimensions and operation order |

## EXL3 proof boundary

The CPU decoder accepts source components directly; it does not invent a
second packed representation. One real layer-3/expert-0/gate/rank-0 payload
was range-extracted from the pinned checkpoint into `/tmp`:

```text
source bytes:          1,192,964
source SHA-256:        68e96700af31debf63c42be271595df75c523f40177e6b6f48c0bab4b24a0ec4
reconstructed bytes:  6,291,456
reconstruction SHA:   a13c295c381993da35eaef392c412024e70dd3d80c28612f71fb24cd17a74d13
```

Rust and the independent audited NumPy reconstruction produced the same
FP16 byte digest. See `exl3-trellis-cpu-contract.md` for the full algorithm
and provenance. No source weight bytes are tracked.

## Weight and capacity behavior

A full serving policy enumerates every routed target tensor:

```text
75 target layers × 256 experts × 3 projection roles = 57,600 assignments
```

An MTP-enabled process adds:

```text
1 draft layer × 256 experts × 3 roles = 768 assignments
```

Each assignment declares EXL3 or NVFP4, exact rank-local physical bytes, and
a quality-evidence SHA-256. Protected tensors are separate immutable
allocations with role, precision, bytes, and payload digest. The builder
sorts canonical records, rejects omissions/duplicates, rejects all-NVFP4
serving, and fails before startup if the rank weight budget is exceeded.

## Concurrency posture

The scheduler does not serialize users into a llama.cpp-style single
request. It forms bounded multi-sequence batches and maintains request state
across steps. Prefill, MTP0 decode, and a common MTP depth are separately
batchable. Different MTP depths form deterministic cohorts because they use
different captured graph shapes. Decode cohorts also have one collective
sampling class. Greedy, bounded top-k, and distributed-mass requests cannot
share a step; the resulting batch, rather than a process-global option,
selects the collective route identically for all four ranks.

The first contract deliberately uses separate prefill and decode steps.
Decode receives priority until its configured burst bound, after which
waiting prefill is serviced. This matches the current reviewed spec posture:
mixed execution remains disabled until a dual-transport `StepPlan` or a
reviewed compound route exists.

The current weighted ordering is a deterministic reference policy, not a
tuned production SLO algorithm. It is sufficient to prove tenant isolation,
limits, cancellation safety, and absence of starvation in bounded
simulations. cn4 measurements will set the real token costs and decode
deadlines.

## Tier and prefix crash semantics

One logical 64-token page has four possible durable pieces:

| Piece | Bytes |
|---|---:|
| target KV, 78 layers | 1,837,056 |
| target indexer, 21 groups | 177,408 |
| draft KV, layer 78 | 23,552 |
| draft indexer, layer 78 | 8,448 |

MTP0 records require the first two. MTP-capable records require all four.
The journal writes a begin record, records each checksum-verified durable
piece, and publishes only after the required set is complete. Replay ignores
unpublished crash orphans and rejects a false or corrupt publication.

Prefix keys hash namespace, parent key, valid token count, and token IDs.
Only full 64-token pages publish. The namespace covers model, tokenizer,
template, weight policy, cache ABIs, and RoPE interpretation. It
intentionally has no writer rank or DCP posture field, so ownership-neutral
NVMe bytes survive attachment-layout changes.

## Sampling communication bounds

- Greedy exchanges one local maximum and token ID per rank.
- `top_k=1..256` exchanges at most `4*top_k` candidate pairs per row.
- `top_k=0, top_p=1` exchanges fixed-rank maximum/mass state and selects a
  rank interval before a rank-local vocabulary-order CDF.
- `top_p<1` with `top_k=0` fails closed as required by v0.
- Speculative residual sampling uses the same rank-ordered distributed-mass
  construction over `max(P_target-P_draft,0)`.
- Counter tickets explicitly bind request, position, draft step, purpose, and
  pre/post counter values for target, draft, acceptance, residual, and bonus
  draws.

No reference path requires a full-vocabulary logits gather.

## What cn4 must establish

The remaining gates are physical, not placeholders for missing CPU design:

1. compile the current CUDA/CUTLASS source with the pinned cn4 toolchain;
2. pass the NVFP4 actual-shape SM120 correctness matrix;
3. implement and qualify direct EXL3 source-payload consumption;
4. measure inclusive FC1+SwiGLU+FC2 and separate CUDA/framework/collective
   time;
5. replay one full sparse layer on TP4 with real routes and tensors;
6. replace synthetic graph/memory terms with measured per-rank values;
7. run the small-checkpoint Rust executor before any full conversion;
8. freeze a reviewed fit-capable policy, then run quality and 1M-context
   gates;
9. connect these control-plane contracts to device workers and the serving
   API;
10. run matched concurrency, prefix, MTP, topology, and fault matrices
    against the pinned general-purpose controls.

None of those steps is claimed complete by this offline foundation.
