# SM120 rank executor v1 corrective amendment r4

Date: 2026-08-04

Status: corrective design candidate; adversarial review required before
CPU/mock or native implementation

Base contracts:

- `docs/sm120-rank-executor-v1.md`
- `docs/sm120-rank-executor-v1-r2.md`
- `docs/sm120-rank-executor-v1-r3.md`
- `docs/sm120-rank-executor-native-abi-v1.h`

## Scope and precedence

This amendment closes one unimplementable native edge in r3. It is normative
over conflicting r1-r3 text. All other r3 requirements remain in force.

The ABI version remains one because no implementation or accepted executor
ABI exists. An executor implementation must use the header bytes pinned by
this r4 candidate. The r3 header and review handoff are superseded as
implementation authority.

## Explicit validation-module binding

R3 requires the dedicated validation-node entry to bind the unique adopted
`DEVICE_VALIDATION` module capability, but its function signature supplied no
module handle. A graph builder carries only a context and stream; the native
ABI has no module-set handle, module-adoption operation, or unambiguous
context-global module selection. Scanning all modules loaded in a context is
forbidden because compatible hot reload may keep old and candidate module
generations resident simultaneously.

The exact v1 signature is therefore:

```c
int32_t glmaxx_executor_validation_node_add_v1(
    glmaxx_executor_handle_v1 graph_builder,
    glmaxx_executor_handle_v1 validation_module,
    const struct glmaxx_executor_validation_desc_v1* descriptor,
    struct glmaxx_executor_error_v1* error) noexcept;
```

`validation_module` must be nonzero, owner-thread-local, loaded in the same
context as `graph_builder`, retained through graph instantiation, and one of
the exact module handles adopted by the current Rust module generation. Its
queried capability table must contain the unique
`GLMAXX_KERNEL_DEVICE_VALIDATION` record bound by the accepted module-set
capability digest. A zero handle, stale generation, foreign context, unloaded
module, unadopted module, missing/duplicate validation family, or target/MTP-
only module fails before capture or enqueue.

The native library resolves the one fixed validation entry family from this
exact module handle. It may not search the context, select the newest loaded
module, reuse a target/MTP capability, or fall back to a built-in validation
kernel.

The validation descriptor's final field is
`program_set_sha256[32]`, not a single-program digest. Its exact preimage is:

```text
SHA256(
  "glmaxx.executor-graph-program-set.v1\0" ||
  u32_le(graph_kind) ||
  target_program_sha256 ||
  u8(mtp_program_present) || seven_zero_bytes ||
  (mtp_program_present ? mtp_program_sha256 : 32 zero bytes)
)
```

PREFILL and MTP0 DECODE require `mtp_program_present=0`. A VERIFY graph with
nonzero `mtp_depth`, including bootstrap/replacement draft work, requires
`mtp_program_present=1` and the exact resident MTP-program digest. A graph
with no MTP node rejects a present MTP digest; a graph with any MTP node
rejects an absent or zero one. Each TARGET/MTP graph-node record still carries
its individual program digest, and graph finalization proves that every node
matches the corresponding member of this set. Thus one validation node binds
the complete model-program membership without adding a second validation
node, changing the 192-byte descriptor, or allowing a target/MTP generation
mix. The program-set digest is not a substitute for validation-module
identity.

The generic `glmaxx_executor_graph_node_add_v1` still rejects
`GLMAXX_NODE_DEVICE_VALIDATE`. Target and MTP graph nodes still carry their
module handles in `native_object`; no other native-object meaning changes.

## Corrected CPU/mock gate

In addition to the full r1-r3 matrix, the proof must:

1. compile and compare the four-argument validation-node signature in C11,
   C++17, and Rust;
2. prove a graph can bind the exact adopted validation module without any
   context-global module lookup;
3. reject zero, stale, foreign-context, unloaded, unadopted, duplicate-family,
   and wrong-family module handles before capture or enqueue;
4. keep old and candidate hot-reload module generations resident together and
   prove each graph binds only the explicitly supplied generation; and
5. enumerate target-only and target-plus-MTP program-set preimages, reject
   absent, extra, zero, stale, and cross-generation MTP membership, and prove
   every target/MTP node matches its set member; and
6. prove module unload remains ordered after destruction of every graph that
   borrowed the module.

Only unqualified adversarial acceptance of r1+r2+r3+r4 and the corrected
header permits the coordinated CPU/mock executor implementation to begin.
This amendment accepts no current Rust worker, native library, cn4 execution,
checkpoint loading, graph capture, collective, target/MTP execution, quality,
capacity, concurrency, or performance result.
