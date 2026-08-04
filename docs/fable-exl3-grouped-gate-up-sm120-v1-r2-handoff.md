# Fable handoff: SM120 EXL3 grouped paired gate/up decode v1 r2

Date: 2026-08-04

Status: corrective adversarial design review requested; supersedes the v1
handoff for implementation authorization

GPU authorization conveyed by this handoff: none

Read-only cn4 artifact verification: permitted; do not launch CUDA or modify
the retained evidence tree

Review candidate commit:
`7b6a98630ed612d923d7656b8723d44e5aa67b8c`

Required result path:
`fable-exl3-grouped-gate-up-sm120-v1-r2.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`exl3-grouped-gate-up-sm120-v1-r2-design-accepted`

## Required provenance procedure

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Report a stale candidate and withhold the token if either
set differs from this table. Do not substitute current `main`, the later
handoff commit, or an untracked review-inbox file.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/exl3-grouped-gate-up-sm120-v1-r2.md` | `b39d36644bba8c25e1ddc154f84c1a573a05589d86689fdb78defe2550509754` |
| `docs/exl3-grouped-gate-up-sm120-v1.md` | `f618027ca386c1976052e9ad5259f94180ac1cd73d7e78a6f17ffd9126847547` |
| `docs/exl3-sm120-warp-decode-v2.md` | `67fb3bcb5b839cc50f3462990f1ef6056ca7c9d991851efc5e668fed9d0b3325` |
| `docs/exl3-warp-staging-cpu-proof-v2.md` | `5c77b5721885da708d0240e9eeb6537e9ed74a25a6940cf92e00bc79de494b31` |
| `docs/cn4-exl3-staged-k3-ncu-20260804.md` | `cc7592fd6da2b4bc589cefd664819c4d04c03dfb0e1a782582ad54fdefda0865` |
| `docs/target-layer-execution-v1.md` | `89c6cf7397a3dc6b0c01383e679dcc4b51e20e3c45057bef0928dc24b866a819` |
| `docs/target-layer-execution-v1-r2.md` | `808da35c2e54eb5692512996650839fb6f127cb91658603eb2fb5ce049c56ed2` |
| `docs/sm120-rank-executor-v1-r2.md` | `4f40ea7652858b4cebbe4093dc81149cb30aa26bedc69edef72fa627c987df89` |
| `crates/glm-format/src/rank_manifest.rs` | `cb57a81ad3643a86992c4fd9c2166e9ee238cc7e66063732355d14e485f73410` |
| `crates/glm-engine/src/native_worker.rs` | `bb054afd16ebe9043740383aad2190a985f9900a2c254d9d6e415b2da45647ba` |
| `crates/glm-cuda/src/abi.rs` | `28905e69300a3a8c8105752ee9aaeb4d718cbe4387cab548139c257242ec68a4` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

Run the complete CPU-only gate and record its exit status:

```text
./scripts/local-checks.sh
```

This candidate has no grouped implementation. A green repository gate does
not answer the design questions or authorize CUDA work.

## Decision 1: closure and precedence

Determine whether the r2 amendment unambiguously supersedes the base text for
rank identity, route hashing, adopted spans, validation, and staging while
retaining the base model geometry, descriptor offsets, K=3-only scope,
arithmetic, traffic, gate sequence, and nonclaims. Report any surviving
contradiction rather than silently choosing one interpretation.

## Decision 2: rank-local address binding

Prove that raw device addresses and their table-byte hashes remain rank-local
while all four ranks agree on logical manifest bindings, target program,
resident generation, and launch route. Attack cross-rank pointer-table reuse,
stale generations, table swaps, inactive nonzero entries, span truncation,
misalignment, wrong component/projection, and readback mismatch.

Re-derive each exact active K=3 span: 1,179,648 trellis bytes, 12,288 SUH
bytes, and 1,024 SVH bytes per gate or up projection. Determine whether the
logical consensus plus owner-thread rank-local receipt is sufficient to bind
the six pointer tables without comparing virtual addresses across GPUs.

## Decision 3: route digest and mixed-K restoration

Independently serialize the complete route digest preimage byte-for-byte,
including domain, scalar fields, sequence, five 32-byte identity digests,
filtered assignment triples, all 257 offsets, and the active expert list.
Confirm the four U64 fields preserve the exact SHA-256 bytes on the pinned
little-endian platform and reject numeric/text reinterpretation.

Attack reordered filters, duplicate token/slot destinations, K=4 admission,
rank-local backend policy, stale full-router identity, and mixed-K consumers
that concatenate filtered buffers. Determine whether scattering by the unique
`(token, route_slot)` key safely restores K=3 and K=4 results without relying
on filtered position.

## Decision 4: span arithmetic and alias safety

Re-derive the contiguous workspace layout and all route/input/output/table
lengths. Confirm `projected_f16` is exactly the derived suffix of
`rotated_input_f16`, all U64 arithmetic is checked, and the maximum A=64
workspace remains 1,703,936 bytes. Mutation-test partial receipts, one-past
ends, wrapping ranges, overlapping gate/up outputs, workspace/output aliases,
validation aliases, and read-only/write overlap.

Determine whether synchronous adopted-allocation validation is an adequate
FFI boundary even though the 256-byte device descriptor contains addresses
rather than lengths for every individual span.

## Decision 5: validation and stream safety

Model every failure point in the exact memset/input/projection/output stream
sequence. Confirm `atomicOr` yields a deterministic final error mask, a prior
kernel's error uniformly suppresses every successor, locally invalid CTAs or
warps issue no invalid access, all projection warps still reach every required
barrier, and no output is consumed before the final mask is read.

Pay special attention to a projection warp discovering bad assignment data
after the CTA has entered the staged loop, and to one projection CTA setting
an error while other CTAs continue. Withhold acceptance for any early return
that can strand a barrier or any path that consumes uninitialized scratch.

## Decision 6: complete 48-stage recurrence

Exhaustively simulate the r2 source-address pseudocode for all 48 stage groups,
both projections, all eight stage tiles, all 24 words, and all 256 threads.
Prove every stage writes exactly 384 distinct words, threads 0 through 127
write two and 128 through 255 write one, and gate/up stages never alias.

Prove the source address covers each `(k_tile,n_tile,word)` exactly once for
K=6,144 and N=512. Compare both accumulator sequences expression-by-expression
with two isolated accepted staged projections: 6,144 ascending K positions,
explicit RN multiply then add, independent gate/up FP32 accumulators, and one
final FP16 rounding each.

## Decision 7: measured premise and gates

Hash-verify the retained cn4 evidence and keep profiled duration distinct from
unprofiled latency. Confirm that the 32-CTA/188-SM gate result supports only
the under-parallelism hypothesis and that the proposed all-K3 M1 projection
grid has 256 CTAs. A real mixed K=3/K=4 route is not guaranteed to expose that
grid.

Confirm that acceptance opens only an independent Rust CPU proof. It does not
accept a grouped implementation, K=4, down projection, SwiGLU, target layer,
TP4 replay, checkpoint smoke, quality result, KV result, or serving speed.

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
answer separately and unambiguously:

1. Does r2 close the base ambiguities without changing its intended scope?
2. Are rank-local addresses bound safely without illegal cross-rank hashes?
3. Is route serialization exact and mixed-K restoration unambiguous?
4. Are all adopted spans, arithmetic, and alias rules safe?
5. Is every validation and stream-order failure fail-closed and barrier-safe?
6. Is the 48-stage paired recurrence bitwise equivalent to two isolated paths?
7. Is the measured premise narrow and the next gate honest?
8. Is the amended design accepted for an independent Rust CPU proof?

Only if every answer is an unqualified `YES`, include the candidate commit and
all twelve exact input SHA-256 values from the provenance table in the result,
then end with the requested acceptance token named in the header as the only
bare acceptance line.

Withhold the token for stale bytes, rank-common address comparison, ambiguous
route serialization, unsafe aliasing, successor scratch consumption after an
error, divergent barrier behavior, incomplete stage coverage, changed
accumulation order, K=4 scope leakage, or unsupported performance claims.

