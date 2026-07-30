# Resident tensor device binding proof v1

Date: 2026-07-30

Implementation candidate:
`843f4ae4cef2a59b7b110efc208e4290d8d99255`

Status: `HOST_PROOF_COMPLETE_REVIEW_AND_SM120_EXECUTION_PENDING`

## Purpose

The checkpoint loader previously ended with two globally adopted raw arena
bases. A target-layer launcher needs tensor-specific pointers, but accepting
an offset, name, or codec from the launcher would reopen the manifest boundary
after authentication.

This candidate attaches the exact plan-owned tensor layout to each adopted
rank arena and introduces one checked resolution:

```text
authenticated tensor_id
  -> fixed plan entry
  -> checked metadata / primary / auxiliary relative spans
  -> absolute rank-local device spans
```

This is a weight-address bridge. It does not compile a target program or
launch a model kernel.

## Ownership and identity

`RankSetLoadPlan` stores each rank layout as `Arc<[TensorArenaEntry]>`.
`PreparedCudaRank`, `AcknowledgedCudaRank`, and `CudaWeightArena` move clones
of that same allocation. No 59,585-entry vector is copied after the physical
checkpoint upload, and the globally adopted arena cannot be paired with a
layout supplied by a later caller.

The plan header, rank entries, and layout slices are crate-private after
construction. External callers receive only copy/read-only accessors, so safe
Rust cannot mutate a layout after `plan_sha256` is computed and then submit
that detached layout for adoption.

The binding remains behind `WeightArenaExecutionPermit`. Adoption requires:

- exact rank;
- exact rank-set plan SHA-256;
- exact owner-allocation generation;
- sealed and full-readback-verified device contents; and
- a nonempty, dense tensor-ID layout with nonzero roles/codecs, mandatory
  primary planes, power-of-two alignment, and bounded spans.

The arena records the plan digest and owner generation from the permit. Its
layout slice is immutable and shared with the plan whose canonical hash
already covers every 64-byte `TensorArenaEntry`.

Immediately after global adoption and before the rank publishes finalize
success, its persistent owner thread resolves all 59,585 tensor IDs and
cross-checks every binding against both the native descriptor and validated
manifest semantic. This covers tensor ID, role, codec, flags, required
alignment, and metadata/primary/auxiliary byte counts. A mismatch releases
the arena and fails the common finalize transaction; a cleanup failure is
terminal and cannot become a finalize acknowledgement.

## Checked span construction

`tensor_binding(tensor_id)` indexes the immutable slice and requires the
stored ID to equal the requested ID. It derives:

```text
absolute_pointer = adopted_arena_base + authenticated_destination_offset
```

with checked addition. Every nonempty span must:

- begin at an offset divisible by its required device alignment;
- end at or before the matching weight/metadata arena capacity; and
- produce an absolute pointer with the same alignment.

The primary span is mandatory. A zero-byte metadata or auxiliary plane
returns `None`; no zero-byte plane exposes an arena base as though it were
usable storage. The returned binding also retains the authenticated role ID,
codec ID, descriptor flags, and alignment.

Only the globally adopted type can perform this resolution. Quarantined and
acknowledged types expose neither raw arena bases nor tensor bindings. The
device-span types and resolver are crate-private; no downstream crate can
retain or manufacture one through the public engine API.

## CPU fault proof

The deterministic fake CUDA backend proves:

- one adopted EXL3-like tensor resolves the exact metadata, primary, and
  auxiliary absolute addresses and byte counts;
- tensor count, plan digest, owner generation, role, codec, flags, and
  alignment remain exact;
- an out-of-range tensor ID fails closed;
- wrong dense tensor ID, zero role, an overrun, and non-power-of-two
  alignment each prevent adoption;
- absent metadata and auxiliary planes resolve to `None`; and
- the resident arena shares the same layout allocation as the load plan via
  `Arc::ptr_eq`; and
- the post-construction plan API exposes no external mutable header, rank, or
  tensor-layout field.

The `cuda-ffi` build also compiles the owner-thread all-tensor validation path
with warnings denied.

At the candidate above:

```text
cargo test --offline -p glm-engine
```

passes 92 tests with zero failures, and:

```text
cargo clippy --offline -p glm-engine --all-targets -- -D warnings
GLMAXX_KERNEL_LIB_DIR=/tmp cargo clippy --offline -p glm-engine \
  --features cuda-ffi --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

all pass.

## Discovery for the target-program contract

The numeric tensor-ID boundary is intentional. A `(layer_id, role_id,
expert_id)` tuple is not sufficient to select every current capacity-EXL3
tensor: routed gate and routed up are separate authenticated source
descriptors with the same role and expert IDs. The future target-program
compiler must bind their distinct canonical tensor IDs using a reviewed
projection discriminator (for example, an explicit semantic projection
field or an exact startup-only name/metadata projection proof). It must not
silently assume that the existing manifest already contains one combined
gate/up descriptor.

This finding blocks claiming the complete target-program mapping, but it does
not weaken tensor-ID-to-device-span resolution.

## Explicit exclusions

No cn4 access, `nvcc`, CUDA context, or GPU operation was used. This candidate
does not prove:

- a complete target-program mapping;
- target tensor geometry or arithmetic;
- an NVFP4 or EXL3 kernel launch;
- a sparse-layer replay;
- graph capture or collectives;
- checkpoint execution, logits, quality, serving, or performance.
