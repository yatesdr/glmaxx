# Deterministic small-checkpoint runner v1

Date: 2026-07-30

Status: superseded by `docs/small-checkpoint-runner-v1-r2.md`; do not
implement this base document without the corrective amendment

GPU claim: none

## Purpose and gate position

This contract defines M4: the first deterministic checkpoint-to-logit smoke
of the Rust engine. It comes only after:

1. the NVFP4 CPU/format gate;
2. accepted actual-shape SM120 operator correctness;
3. accepted TP4/DCP4 collective routes; and
4. the complete layer-6 M3 replay.

It is deliberately later than a kernel smoke and earlier than a full
fit-capable checkpoint. Passing M4 proves that a bounded real GLM-5.2 tensor
subset can traverse native files, four-rank load/adoption, persistent rank
workers, production descriptors, graphs, collectives, MTP0 execution,
distributed greedy sampling, and cleanup. It does not prove model quality,
full-model service, capacity, or performance.

The runner is a dedicated CLI/evidence path. It must not bind the HTTP
server, report production `Healthy`, register a serving model name, populate
the production prefix namespace, or treat this subset as
`capacity-exl3`/`hybrid-serve`.

## Immutable source identity

M4 consumes the exact GLM-5.2 NVFP4 source checkpoint and codec policy
accepted by M2. The M4 fixture manifest embeds:

```text
source repository and revision
complete source checkpoint identity
M2 result SHA-256 and acceptance token
source tensor hashes for every selected tensor
NVFP4 quant-policy SHA-256
format-v0 SHA-256
engine-v0 SHA-256
operation-manifest SHA-256
kernel ABI SHA-256
collective-route SHA-256
graph-profile SHA-256
memory-plan SHA-256
```

There is no rank-local source choice or fallback. If the existing NVFP4
checkpoint does not expose bytes with the reviewed quantization semantics,
M2 must produce one process-common deterministic repack and pin its complete
identity before this design can be implemented. Requantizing the EXL3
checkpoint is not an implicit fallback and cannot be called an NVFP4 quality
control.

Model bytes, captured activations, cache state, logits, and raw timings stay
outside Git. Git contains only contracts, generators, commands, small
manifests, and hashes.

## Exact tensor subset

The subset is layer 6 plus the actual final norm and vocabulary head. Layer 6
is fixed because it exercises a full indexer, absorbed MLA, routing,
256 routed experts, shared experts, both TP reductions, and DCP4 attention.
It is also the layer fixed by the first M3 replay.

Each rank file contains exactly 533 tensors:

| Membership | Tensors/rank | Payload bytes/rank |
|---|---:|---:|
| layer-6 protected attention/norm/indexer/router/shared-expert tensors | 19 | 147,487,232 |
| layer-6 routed experts: 256 combined gate/up plus 256 down | 512 | 1,358,954,496 |
| final norm plus rank-sharded vocabulary head | 2 | 475,803,648 |
| **total** | **533** | **1,982,245,376** |

The routed-expert arithmetic is:

| Tensor | Logical rank shape | Values | Scales | Planes total |
|---|---:|---:|---:|---:|
| combined gate/up | `[1024,6144]` | 3,145,728 | 393,216 | 3,538,944 |
| down | `[6144,512]` | 1,572,864 | 196,608 | 1,769,472 |

One expert therefore owns 5,308,416 payload bytes, and 256 experts own
1,358,954,496 bytes. The 512 NVFP4 tensors carry 65,536 codec-metadata bytes
per rank. Every listed plane is already a multiple of the 256-byte payload
alignment; the CPU implementation must independently rederive and test that
the subset has zero internal payload-alignment slack rather than assuming it.

The protected layer-6 membership is exactly:

```text
input_layernorm
post_attention_layernorm
q_a_proj
q_a_layernorm
q_b_proj
kv_a_proj_with_mqa
kv_a_layernorm
kv_b_proj
o_proj
indexer.wq_b
indexer.wk
indexer.weights_proj
indexer.k_norm.weight
indexer.k_norm.bias
router weight
router correction bias
shared-expert gate
shared-expert up
shared-expert down
```

The builder projects this set from the compiled GLM-5.2 tensor catalog. It
must not use string-prefix filtering as the authority. Tensor IDs are
canonical name order within the subset, and every rank derives an identical
rank-invariant semantic catalog while retaining rank-local physical/source
bindings.

The laboratory rank-manifest schema and profile identity are distinct from
the complete production manifest. The strict production validator must
continue rejecting 533-tensor rank files as incomplete.

## Input, cache, and reference fixture

M4 reuses the exact M3 layer-6 source/checkpoint/input/cache/prompt and route
hashes. It contains:

- one M=1 decode row;
- one prefill bucket selected and accepted at M2/M3;
- context positions spanning all four DCP owners;
- target KV and indexer state required by those rows;
- the exact router IDs/weights and winner positions produced by the
  independent reference;
- the layer-6 output reference;
- final-norm output and four sharded vocabulary-logit references; and
- the distributed-greedy token reference.

The input is a captured real layer-6 hidden-state boundary, not an embedding
or a synthetic random vector. M4 does not pretend that a one-layer subset is
an autoregressive full model. “Prefill” and “decode” name the real production
operator/graph/collective paths at those shapes; they do not claim a
token-feedback loop through omitted layers.

The reference consumes the exact packed NVFP4 planes used by the device, not
an unquantized source weight. Per-phase tolerances and stable/tie greedy
classification are inherited from the accepted M2/M3 numerical contract and
copied verbatim into the fixture manifest before the device run.

## Load and execution transaction

The runner performs:

1. open and strictly validate four native rank files;
2. derive one immutable `RankSetLoadPlan.v1`;
3. prove the laboratory profile, tensor catalog, device identities, codec
   capabilities, graph profile, collective routes, and per-rank memory plan;
4. allocate four non-executable quarantined weight/metadata arenas;
5. stream, hash, semantically validate, and upload every planned interval;
6. drain and seal all four arenas;
7. validate four prepared receipts and issue one identical adoption command;
8. instantiate persistent owner-thread rank workers;
9. capture or instantiate the exact accepted prefill/decode graph buckets;
10. restore the fixture cache state;
11. execute prefill, M=1 decode, final norm/head, and distributed greedy;
12. compare every required phase and sharded logit record with the reference;
13. release graph, cache, collective, stream, module, and arena ownership; and
14. prove every device and host allocation returned to its initial bound.

No executor handle exists before four-rank adoption. No output becomes
accepted after a rank, graph, collective, hash, semantic, reference, or
cleanup failure. A laboratory runner success never advances the production
startup coordinator to `Healthy`.

## Required deterministic matrix

The accepted M4 result contains:

- one cold prefill and one cold decode run;
- 100 warm repetitions of each exact input with byte-stable plans, routes,
  receipts, greedy tokens, and evidence schemas;
- five complete load/run/destroy cycles proving no monotonic resource growth;
- eager and captured controls for each row shape;
- one forced file/hash/semantic failure on each rank;
- one forced upload, final-drain, prepared-receipt, and adoption failure on
  each rank;
- adoption failure after one prior acknowledgment;
- graph-key/profile mismatch;
- memory-plan and codec-capability mismatch;
- collective-route divergence and one rank-thread exit;
- malformed/nonfinite DCP partial state;
- malformed router/compaction output;
- sharded-logit padding and globally all-masked rejection; and
- cancellation only at a collective-safe boundary.

Every fault must end with zero published output and exactly-once cleanup of
all four rank generations. Tests may use mock allocations before cn4, but
the M4 token requires the real SM120 path.

## Evidence record

The result schema retains:

```text
all source/contract/build/container/toolchain/device/topology hashes
rank-file and load-plan hashes
prepared and rank-set receipt hashes
graph and collective route hashes
input/cache/prompt/route hashes
per-phase reference/device hashes and error metrics
all four sharded-logit comparisons
greedy local candidates, winning rank, and global token
per-rank verified/uploaded/allocated bytes
kernel, launch, collective, transfer, framework, and end-to-end times
cold/warm and eager/captured labels
fault ID and cleanup accounting
```

Timing is diagnostic at M4. A speed claim still requires the later matched
benchmark gate.

## Exit and explicit non-claims

M4 passes only when the real four-rank runner loads all 533 tensors per rank,
executes both fixed row shapes, matches the accepted reference contract, and
passes the repetition/fault/cleanup matrix.

M4 does not prove:

- layers other than layer 6;
- embedding or token feedback through a full target model;
- EXL3 execution;
- a fit-capable checkpoint;
- model quality or task accuracy;
- MTP depth above zero;
- multi-user serving;
- prefix reuse or DRAM/NVMe tiering;
- 1,048,576-token execution; or
- any performance advantage.
