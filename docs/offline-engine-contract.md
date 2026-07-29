# Offline engine-contract candidate

Date: 2026-07-28

Status: CPU implementation candidate; independent review pending

GPU evidence: none

## Purpose

`glm-engine` advances serving-critical control-plane work that does not
require cn4. It defines three fail-closed contracts:

1. one immutable, rank-invariant `StepPlan`;
2. admission against a process-immutable graph profile;
3. per-rank static HBM accounting using physical bytes.

This is not M3 execution, a CUDA graph implementation, a scheduler, or
serving evidence. It is the CPU-verifiable contract those later components
must consume.

## Canonical `StepPlan` candidate

ABI identity: `glmaxx.step-plan.v1`

The hash input is an explicit 85-byte little-endian encoding with no Rust
padding or Serde dependency:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | epoch |
| 8 | 8 | step ID |
| 16 | 1 | mode |
| 17 | 2 | active sequences |
| 19 | 2 | sequence bucket |
| 21 | 4 | scheduled prompt tokens |
| 25 | 4 | query rows |
| 29 | 4 | verifier-row bucket |
| 33 | 1 | MTP depth |
| 34 | 4 | graph ID |
| 38 | 2 | TP route ID |
| 40 | 2 | DCP route ID |
| 42 | 1 | attention transport |
| 43 | 2 | sampling route ID |
| 45 | 8 | sequence-table generation |
| 53 | 32 | collective-schedule SHA-256 |

The stored proof record appends the 32-byte plan hash for 117 bytes total.
Hashes use domain-separated SHA-256. This canonical record is currently a
coordinator/worker proof representation, not a CUDA kernel descriptor.

The collective schedule is ordered and hashes each operation's ordinal,
kind, globally selected route, payload bytes, and participant mask. Ordinals
must be contiguous and every participant mask must be a nonempty subset of
the four fixed ranks. A rank-local route or schedule change therefore changes
the digest and is rejected before the step enters a collective.

The implementation enforces:

- sequence buckets `1,2,4,8,16,32,64`;
- no more than 64 active sequences or 448 target/verifier rows;
- MTP depths zero through six;
- verifier rows exactly `active_sequences * (depth + 1)`;
- zeroed unused fields for `CACHE_ONLY`;
- an empty GPU collective schedule for `CACHE_ONLY`;
- mode-specific attention and sampling routes;
- a nonzero immutable sequence-table generation for compute steps.

## Deliberate mixed-mode stop

The specification's current `StepPlan` has one `attention_transport` field.
A `MIXED` step needs both a prefill transport and decode query/LSE transport.
Choosing one field's meaning during implementation would silently freeze an
ambiguous ABI.

`glm-engine` therefore returns `MixedContractUnreviewed` for every mixed
plan. Before mixed prefill/decode is enabled, review must choose either:

- separate prefill and decode transport fields; or
- a reviewed compound route ID whose route-table entry contains both.

This does not block separate prefill, decode, verify, or cache-only steps.

## Graph-profile candidate

`GraphProfile` deterministically sorts entries by graph ID, rejects duplicate
IDs and keys, and hashes:

- the `StepPlan` ABI;
- the kernel ABI;
- the complete graph key;
- real shape limits;
- compatible TP, DCP, and sampling route sets;
- scratch, argument, graph-object, and resident-module bytes;
- the admission/SLO class.

Admission fails if the graph is absent, shapes exceed the entry, or any
globally chosen route is incompatible. `CACHE_ONLY` has no CUDA graph.
Uncaptured correctness execution remains outside this production admission
contract.

## Static memory planner candidate

The planner validates four distinct ranks and rejects profile or MTP posture
differences. Every rank must independently fit; aggregate free memory cannot
rescue an overcommitted rank.

Each inequality explicitly contains:

- weights;
- CUDA modules and contexts;
- graph residency;
- the larger of prefill and verifier workspace;
- collective and tier-staging slabs;
- target KV and target indexer keys;
- draft KV and draft indexer keys;
- model metadata and page tables;
- allocator padding;
- emergency escrow.

Serving profiles require at least 262,144 committed target slots per rank and
1 GiB escrow. MTP-enabled serving additionally requires 262,144 committed
draft slots per rank. Slack and tentative slots are accounted separately.

The committed one-million-position per-rank byte terms remain:

```text
target KV:          7,524,581,376
target indexer:       726,663,168
draft KV:              96,468,992
draft indexer:         34,603,008
```

The deterministic proof adds 448 draft tentative slots to demonstrate that
transactional verifier capacity cannot hide inside the committed floor. Its
weight and usable-HBM figures are explicitly synthetic and are not a serving
admission result.

## CPU evidence

The workspace now has 58 tests. The 17 `glm-engine` tests cover:

- stable plan bytes and hashes;
- rank-local schedule divergence;
- tampered plan/profile rejection;
- noncanonical cache-only fields;
- exact MTP verifier rows;
- the mixed-mode fail-closed posture;
- duplicate graphs and incompatible routes;
- per-rank fit failure;
- serving KV and escrow floors;
- full-context cache arithmetic;
- profile mismatch and integer overflow.

`glmaxx engine-proof` regenerates
`fixtures/engine-contract-proof-v1.json`, and `scripts/local-checks.sh`
requires byte identity.

## Review and implementation boundary

Independent review should decide:

1. the mixed-mode dual-transport representation;
2. whether the 85-byte canonical record becomes the queue/wire ABI or remains
   only the hash input;
3. whether every collective operation needs explicit layer/group identity in
   addition to its ordinal;
4. the final graph-key context-band and pointer-table attachment contract;
5. the measured cn4 inputs that replace the synthetic memory fixture.

No CUDA graph has been captured, no GPU worker has consumed a plan, and no
memory result uses measured cn4 free bytes.
