# FC2 grouped-control scratch r2 implementation audit

Date: 2026-08-03

Status: read-only audit of an uncommitted development candidate; not design
acceptance, implementation acceptance, or device evidence

## Scope and ownership

The main worktree contains concurrent, uncommitted FC2 scratch changes. This
audit did not edit, stage, commit, copy to cn4, compile with CUDA, or launch
those changes. It compares their bytes to
`docs/fc2-grouped-control-scratch-r2.md`, whose Fable acceptance token remains
absent.

The source base was `24c10286c34f6cc5e45469374ba19b17d28c0d42`. The
SHA-256 of the relevant six-file Git diff was:

```text
966a4da123322a2b3c173e104bdc7a2a1dc402216bc9effc94723d0775d173bd
```

The audited working bytes were:

```text
crates/glm-cuda/src/abi.rs                         e06ff270547c12ed485f51cd1adeff279bc78a5303691ff049b1f85bef98a0b0
crates/glm-cuda/src/ffi.rs                         a236ac9772d4243857d0ce66a3d5a0b0ff14f6a03c141ce30b51d0219b88af5d
crates/glm-cuda/src/lib.rs                         a8cbdac4f63d171cf455123ee2ec2d383273c8be0b2a1195c0bfe176ed295571
kernels/include/glmaxx_kernel.h                    647b1d503613068634e6df259b9d969fb5719cc890fa135c78341a9fbc0ea55b
kernels/sm120/cutlass_nvfp4_fc2_control.cu         8474c7466fbc45f093e7947f804eac28e4625031c54c8868aa0b448d5ec75e96
kernels/sm120/nvfp4_routed_fc2.cu                  b7477c17caba6764ba6df0c3eef8f27812c496134cb3103574f80e3cd0d05bba
```

## Findings

### BLOCKER 1: the frozen helper domain is not implemented

The contract requires both helpers to accept only:

```text
1 <= rows <= 65,536
1 <= assignments <= min(rows * 8, 65,535)
```

Neither helper checks `rows > 65_536`. Consequently both Rust and C accept
`rows=65_537, assignments=1`, contrary to the contract. At `rows=u32::MAX`,
Rust evaluates `rows.checked_mul(8)` before any row ceiling and returns
`KernelError::Overflow`; the C helper performs the multiplication in `u64`
and returns a nonzero allocation. The included Rust test explicitly expects
`Overflow`, while the contract requires `KernelError::Shape` and native zero
for every row above 65,536.

Required correction: reject `rows > 65_536` before `rows * TOP_K` in both
languages, use checked `u64` arithmetic for the remaining predicate and byte
formula, and prove identical `Shape`/zero behavior at 65,536/65,537 and the
`u32` maximum.

### BLOCKER 2: the required shared non-launching probe is absent

The r2 contract requires a 112-byte, 16-byte-aligned probe record containing
the exact metadata, CUTLASS workspace, required/allocated/headroom, semantic
output, device, and reserved fields. No probe record, exported probe
function, Rust representation, FFI declaration, validation routine, or CLI
route exists in the candidate.

The launch path still constructs `GroupedScratch` and CUTLASS arguments only
inside `prepare_grouped_control` and `enqueue_grouped_prepared`. Therefore a
future probe could drift from the launch instead of sharing the same metadata
layout and argument construction as the contract requires.

Required correction: factor one host sizing routine used immediately by both
probe and launch, freeze and statically assert the 112-byte record on both
sides, reject every nonzero flag/reserved field, and expose a Rust byte-exact
validator without enqueueing or mutating device memory.

### MAJOR 1: the qualification script cannot prove the new ABI

`scripts/cn4-phase-b.sh` checks the two FC2 control launch symbols but does
not require the scratch helper or the missing probe symbol. A library could
therefore omit or rename either new ABI and still pass the current symbol
gate before failing later or silently skipping the probe.

Required correction after design acceptance: bind both exact symbols and the
probe-record ABI into the CPU/native gate before any device launch.

### MAJOR 2: the CPU/native proof is materially incomplete

The new Rust test covers M1, the 170/171 crossover, row zero, `1 x 9`, and
`u32::MAX x 8`. It does not cover:

- rows 65,536/65,537;
- assignments 65,535/65,536;
- all required decode rows and boundary prefill rows across Rust/native;
- the exact 112-byte probe layout and reserved/flag rejection;
- independent metadata, CUTLASS workspace, required, allocated, and headroom
  mutations;
- distinct persistent allocation ranges and the temporal output-plane reuse;
  or
- preservation of every pre-existing workspace term.

The focused test
`abi::tests::fc2_workspace_includes_deterministic_scatter_state` passes, but
that is evidence only for its existing assertions and does not cover these
requirements.

### MAJOR 3: positive headroom remains unproved

The allocation floor changes to 4 MiB and the launch rechecks
`metadata_bytes + cutlass_workspace_bytes <= scratch_bytes`, which is the
right fail-closed inequality. However, no shared probe publishes the actual
required size or positive headroom for each FC2 route. The implementation
therefore cannot satisfy the contract's pre-launch matrix gate or distinguish
a later CUTLASS workspace increase from another opaque `Driver(-3)`.

## Correct elements retained

The candidate does correctly reproduce the central arithmetic already
specified by r2:

- M1 scratch is 4,194,304 bytes;
- the aggregate M1 grouped workspace is 4,554,820 bytes;
- the 4 MiB floor holds through row 170;
- row 171 produces 4,202,496 bytes;
- the grouped workspace replaces, rather than double-counts, the old semantic
  output extent; and
- the actual launch rechecks metadata plus CUTLASS workspace before
  initialization and preserves same-stream reuse before reduction.

Those points do not overcome the blockers above.

## Exit condition

Do not send this candidate to cn4. After the r2 design token arrives, the
implementation candidate must be corrected, independently reviewed, and pass
the complete CPU/native proof before a fresh immutable Phase B. The first
device sequence remains non-launching probe matrix, FC1 M1 regression, FC2
M1 twice, then M8/full FC2 matrix and leak/idleness checks.
