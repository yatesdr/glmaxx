# SM120 W4A16/NF3 execution integration audit

Date: 2026-08-03

Status: static cross-contract audit; no implementation or GPU evidence

## Scope

This audit re-derived candidate `fc5786dde5f88bc1f99efa8dd4c883f35b750c7e`
against the current target-layer, MTP, resident-generation, resident-binding,
and rank-executor contracts. It used only tracked repository bytes. It did not
connect to cn4, read a checkpoint, compile CUDA, or launch a GPU.

The first W4A16/NF3 execution design is not implementable as written. Its
weight arithmetic and workspace totals remain useful, but six integration
findings require a corrective r2 before adversarial acceptance.

## BLOCKER 1: target-program identity loses physical meaning

`target-layer-execution-v1-r2.md` still hashes ten-byte bindings containing
only tensor ID, role, expert, and codec. The W4A16/NF3 source contract requires
every graph to bind projection plus value/scale layouts. The real hybrid rank
contains combined gate/up records for both codec `0x0102` and codec `0x0300`;
neither can be distinguished completely by the ten-byte target record.

The older projection amendment adds a discriminator, and fused-NVFP4 r3 adds
layout IDs, but neither identity is incorporated by target-layer r2. Candidate
v1 merely stores `target_program_sha256`; it never defines a layout-bound
successor. An old layout-free target program could therefore name the new
kernel plan.

Required correction: one 16-byte physical binding record and a new target
program domain must bind projection and both layout IDs. Layer 78 needs the
same extension under a distinct MTP-program digest.

## BLOCKER 2: the alleged common plan digest is rank-local

Candidate v1 hashes its complete plan descriptor while that descriptor
contains TP rank and CUDA virtual addresses for the layer binding and
workspace. Those bytes differ across ranks and processes. The step receipt
then binds that plan digest while claiming one rank-common receipt.

Required correction: split an address-free common plan from one owner-created
rank materialization. Only the common digest participates in TP4 consensus;
the ordered rank receipts attest local addresses separately.

## BLOCKER 3: persistent weights incorrectly depend on a module generation

The binding-table header and logical receipt in candidate v1 include the
active module generation. A compatible hot reload changes modules and graphs
while retaining weights. Updating an allegedly immutable binding table on
every tuning generation either violates its lifetime or creates hidden H2D
traffic.

Required correction: the persistent table binds the resident-weight identity,
owner allocation generation, target program, and MTP program only. Runtime and
module generations belong solely to replaceable graph materialization.

## MAJOR 1: production launch bypasses the native executor contract

The rank-executor ABI accepts arena-relative spans and owner-thread native
handles. Graph nodes consume a descriptor span through
`glmaxx_executor_graph_node_add_v1`. Candidate v1 instead describes direct
prepare/execute symbols whose records contain raw pointers without defining an
executor-span materialization boundary. It neither proves those values are
owner-derived nor says how the calls enter the captured target-program node.

Required correction: the common plan must use fixed graph-slot-relative
offsets. Only the persistent rank owner may resolve those offsets and binding
tables to CUDA addresses. Production capture remains behind the existing
native graph-node entry; standalone direct launches are diagnostic-only.

## MAJOR 2: layer 78 has no program identity

The binding table includes sparse layers 3 through 78, but its header binds
only `target_program_sha256`. The target program covers layers 0 through 77;
layer 78 belongs to `MtpProgram`. Thus the draft weights could be materialized
without a hash-covered draft operator program.

Required correction: bind nonzero target and MTP program digests in every
serving-profile table and plan. MTP0 may skip draft execution but may not
reinterpret or anonymously admit the resident layer-78 records.

## MAJOR 3: routed output is not connected to target-layer lifetimes

Candidate v1 correctly retains FP32 slot-ordered routed reduction, but it does
not map work, intermediate, slot, and output planes to target-layer buffer
classes 19, 21, and 22. It also stops at an FP32 routed partial without fixing
the shared-expert combination and final BF16 rank-partial boundary required
before the one MLP TP4 reduction.

Required correction: graph-profile spans own every plane. The fused FC1 output
is class 21; compaction/status is class 19; slot tile and routed FP32 result are
class 22. The following target operation combines routed FP32 with the
protected shared projection, rounds the one rank partial to BF16, writes class
24, and performs exactly one BF16 TP4 sum.

## MAJOR 4: prefill compaction changes a target-layer identity

Target-layer v1 fixes logical compaction as global expert ID, token row, then
route slot. Candidate v1 instead states codec, global expert, token, slot.
Although disjoint slot projection can make those schedules numerically
equivalent, their compacted bytes and receipt differ, so the new order cannot
silently replace the target-program identity.

Required correction: retain one canonical expert/token/slot work stream and
global-expert count/prefix table. A separate active-expert execution list may
be stably partitioned by codec while each entry continues to address its
canonical contiguous range.

## Retained arithmetic

The following v1 derivations were independently recomputed and remain exact:

```text
76 * (1,024 + 256*128) + (256 + 76*64) = 2,573,312
M8 direct workspace                              = 1,839,104
M3072 tiled prefill workspace                  = 126,225,408
M3072 untiled slot plane                       = 603,979,776
```

Those are format/workspace charges only. They do not prove allocation, fit,
correctness, or performance.

## Consequence

`sm120-w4a16-nf3-fused-moe-v1.md` and its handoff must be superseded. No token
for v1 may authorize CPU or CUDA implementation. A corrective r2 must preserve
the valid codec, route, reduction, and tiling work while closing all seven
findings above.
