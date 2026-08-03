# EXL3 mixed-K r2 implementation readiness audit

Date: 2026-08-03

Status: static implementation map; no design token, source change, CPU proof,
CUDA result, or checkpoint admission

## Scope

This audit maps the committed source at
`691621c0a7b32dda3b57eb3302e08f97702f0e6a` to the corrective contract in
`docs/exl3-mixed-k-source-and-kernel-v1-r2.md`. That contract still lacks the
required Fable token, so this audit does not implement it or authorize cn4.
The concurrent FC2 worktree changes were neither read as EXL3 authority nor
modified.

The r2 source pins in the handoff still match the committed implementation
bytes:

```text
crates/glm-format/src/exl3.rs                       f6fa1b25311d78e13e22a0c7c908da7abca636948218fef1987c89850e974edb
crates/glm-format/src/safetensors.rs                4a7d8d4a2121a2257a5e8b7ec531c98b4b83bddb6ea140ade697088a05009594
crates/glm-format/src/checkpoint.rs                 08450f0cb33e592ec76dfbe655b06580ba1743e60f3109a65218d052dbea406c
crates/glm-format/src/stream.rs                     b6d7dae8adf6fbb7ebd0f08c79c3d7f9dbba6269408f6b760fa43b18028a22fb
crates/glm-format/src/native_reader.rs              953a56702ba1ee000f508fe24cbbae7c6137d6496d104720f658fede5572699c
crates/glm-cuda/src/abi.rs                          28905e69300a3a8c8105752ee9aaeb4d718cbe4387cab548139c257242ec68a4
crates/glm-cuda/src/ffi.rs                          2a76ad51cb1c9b28a508dc4734bfeb6b6ad46103c3b437ec8e8ff8f6a6ff2f31
kernels/sm120/exl3_projection_control.cu            241730ceaf629d01101629cb3f107e8d13fe92019444f4b635f9aa1d8cbc819d
kernels/include/glmaxx_kernel.h                     c5f5ceed453c901a63dfeecea0ec83a53b6485e98c32763650c708c699b56406
```

## Reusable implementation

The retained CPU source representation already carries most of the physical
information needed by r2:

- the 96-byte `Exl3Metadata` wire record already serializes `bits`, logical
  shape, trellis word count, and CRC;
- `trellis_word_count` already uses checked descriptor-derived width;
- the source decoder indexes tile halves and cyclic words from a runtime
  `bits` value, so its address arithmetic is structurally width-generic;
- safetensors component validation already checks the third trellis dimension
  against `16 * metadata.bits`; and
- the 144-byte native descriptor already contains a `bits` field.

These are useful building blocks, not K4 acceptance. Every public validator
still rejects K4 or obtains width from a caller-supplied metadata object.

## Required CPU/source cut

The first accepted implementation should be one atomic CPU/source change,
not a CUDA-first patch:

1. Change `Exl3Metadata::validate` from `bits == 3` to exactly `{3,4}` while
   preserving wire version 1 and rejecting every other width.
2. Add independent forward-scatter reconstruction and boundary-window tests
   for K4 rather than trusting the width-generic inverse decoder alone.
3. Replace caller-authoritative width admission with a reader that derives
   width from the validated trellis third dimension and only then constructs
   metadata. The current `load_exl3_projection[_sharded]` accepts
   `Exl3Metadata` from its caller and therefore cannot be the production
   authority unchanged.
4. Parse the authenticated tier map into separate target and draft domains:
   target layers 3 through 77 require 256-entry `k` arrays with 192 K3 and 64
   K4 experts; draft layer 78 requires absent `k`, empty `keep_nvfp4`, complete
   `tail_tr3`, and only K3 descriptors.
5. Compare gate/up/down and all four rank-derived widths for every expert
   before creating a plan. The tier map corroborates physical descriptors; it
   never selects their precision.
6. Bind target/draft role, bits, source bytes, metadata digest, tensor IDs,
   and rank ownership into native descriptors and operation-manifest rows.
7. Emit one common target/draft partition-plan digest and reject any
   rank-local difference before allocation or launch.

No committed crate currently contains a `tier_bitmap`, mixed-K partition, or
target/draft partition implementation. A repository search at the pinned
commit returns no such runtime symbol.

## Exact CPU proof obligations

The implementation proof must reproduce the complete real census rather than
testing only representatives:

```text
target sparse layers       75 (3..77)
draft layers                1 (78)
all trellis descriptors     233,472
target K3                   172,800
target K4                    57,600
draft K3                      3,072
draft K4                          0
K4 delta bytes/rank       5,662,310,400
routed source bytes/rank 75,293,233,152
```

Synthetic mutations must include target missing `k`, draft added `k`, draft
K4, incomplete `tail_tr3`, nonempty draft NVFP4 membership, projection or
rank disagreement, width 2/5, third-dimension 47/49/63/65, and checked byte
overflow. Real reconstruction must cover target K3, target K4, and draft K3
on ranks 0 and 3 for every projection.

## Required native/CUDA cut after CPU acceptance

The current native boundary is unambiguously K3-only:

```text
EXL3_ABI_VERSION = 1
EXL3_BITS = 3
EXL3_KERNEL_ABI = glmaxx.sm120.exl3.source_projection.v1
CUDA kBits = 3
```

`Exl3Descriptor::new` hard-codes three bits, and `NativeExl3Fixture` does not
retain the tensor width when it creates a descriptor. The CUDA decoder uses
global K3 constants for tile halves, words, and bit positions.

After the CPU implementation and its separate review, the native cut must:

1. advance the ABI identifier and version to source projection v2 while
   preserving the 144-byte descriptor layout;
2. make descriptor construction require the admitted tensor width and retain
   it in `NativeExl3Fixture`;
3. validate exactly K3/K4 in Rust and CUDA with matching byte arithmetic;
4. compile two decoder specializations and dispatch once on `descriptor.bits`
   before the projection loop;
5. keep the existing K3 launcher as the matched control during qualification;
6. create canonical stable K3 and K4 target bins without changing token/slot
   order; and
7. reject any K4 draft plan before launch.

The width may not become an inner-loop branch, a reconstructed dense cache, a
rank-local fallback, or an average 3.25-bpw allocation shortcut.

## Qualification order

The shortest valid path after the r2 token is:

```text
target/draft parser + accounting
-> exhaustive CPU/source proof
-> implementation review
-> K3 regression and K4 synthetic SM120 control
-> real target K3/K4 and draft K3 projection controls
-> specialized bin timing
-> mixed 192:64 target-layer plus recurrent-draft replay
-> authenticated checkpoint smoke
```

This audit intentionally stops before implementation because the governing
r2 design has not yet been adversarially accepted.
