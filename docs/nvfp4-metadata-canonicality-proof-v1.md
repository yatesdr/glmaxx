# NVFP4 metadata and padding canonicality proof v1

Date: 2026-07-29

Implementation commit:
`232e503b47f6310b5b88c57e56ffa0bd769047a2`

Status: CPU format correction passed; independent review pending

GPU claim: none

## Corrected acceptance holes

The frozen 128-byte NVFP4 metadata schema assigns exact values to:

- rounding mode at byte 33;
- scale dtype at byte 34;
- global-scale mode at byte 35;
- reserved bytes 52–55; and
- reserved bytes 124–127.

The prior decoder checked nibble order at byte 32 but accepted arbitrary
values in all five remaining fields after CRC and container hashes were
recomputed. It also accepted:

- negative-zero `global_amax`;
- a finite positive `global_scale` inconsistent with the recorded
  `global_amax`;
- nonzero E2M1 codes whose block scale was positive zero; and
- nonzero value or scale padding outside the logical tensor.

Those cases are noncanonical alternate byte representations. Cryptographic
hashes detect accidental mutation but cannot reject an attacker or producer
that recomputes every hash over invalid bytes.

## Implemented validation

`Nvfp4Metadata::decode` now requires:

- bytes 32–35 to equal the four frozen mode IDs;
- both reserved ranges to be all zero;
- finite, nonnegative, positive-zero-only `global_amax`; and
- bit-exact `global_scale = global_amax / (448 * 6)`, with the all-zero
  special case exactly FP32 `1.0`.

The container reader now validates the value and scale planes together after
descriptor/metadata agreement:

- every E4M3 scale code is canonical finite positive;
- a zero scale permits only positive-zero E2M1 nibbles;
- every padded E2M1 nibble is positive zero;
- a 1D padded row has only zero scales;
- a scale group wholly beyond logical K has only zero scales; and
- 2D's final partial 16-row tile may repeat its shared nonzero scale into
  padded rows, while those rows' E2M1 values remain zero.

The last rule preserves the frozen 2D 16×16 sharing semantics rather than
incorrectly applying the 1D padded-row rule.

## Regressions

The unit proof:

- mutates every byte 33–35, 52–55, and 124–127 independently;
- recomputes metadata CRC32C;
- rejects every resigned record as unsupported;
- rejects a one-ULP global-scale lie;
- rejects negative-zero amax;
- rejects a nonzero code behind a zero scale;
- rejects nonzero value padding;
- rejects nonzero 1D scale padding; and
- retains deterministic randomized 1D and 2D packing for logical shape
  257×193, including the final partial 2D tile.

The container proof mutates representative mode and both reserved regions,
then recomputes:

- metadata CRC32C;
- descriptor-local metadata SHA-256;
- complete metadata-region SHA-256;
- descriptor-region SHA-256;
- file UUID;
- header CRC32C; and
- all affected enclosing identities.

`RankFile::read` still rejects those fully resigned containers through the
NVFP4 semantic decoder.

Verification:

```text
cargo test -p glm-format
cargo clippy -p glm-format --all-targets -- -D warnings
./scripts/local-checks.sh
```

Results:

- 63 `glm-format` unit tests passed;
- 3 external NVFP4 proof tests passed;
- doc tests passed; and
- targeted Clippy passed with warnings denied;
- the complete workspace gate passed 289 Rust tests;
- workspace Clippy and CUDA-FFI host checks passed with warnings denied;
- CPU, matrix, manifest, pack/inspect, budget, ABI, engine, serving, and cache
  proof commands passed;
- every generated checked-in fixture comparison passed; and
- 72 review handoffs were provenance-verified with 0 of 53 configured review
  results present.

The local host did not have `GLMAXX_TOKENIZER_DIR` set, so the pinned tokenizer
bundle proof was skipped. It also had no `nvcc`, so this run did not compile or
launch CUDA. Those exclusions do not weaken the CPU format result and are not
claimed as satisfied hardware gates.

Implementation hashes:

```text
spec/format-v0.md
619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a

crates/glm-format/src/nvfp4.rs
785d2daeb13517893fa6604c5a1424079111ac822821b1d16ec23ce6fde0e440

crates/glm-format/src/container.rs
802cd4eee7090ebcad9cce11127bc09271038614466198a84e5045271bdeeb25

crates/glm-format/tests/nvfp4_proof.rs
74b312d65566db5414dd012c2d9b5222aa39808dfb5e979b11c6dadb7c45c734
```

## Exclusions

This correction proves canonical CPU decoding of existing NVFP4 bytes. It
does not alter the packed layout, quantization policy, kernel ABI, tensor
membership, profile fit, or checkpoint.

It does not establish CUDA loading, SM120 execution, block-scaled MMA,
operator correctness, model quality, capacity, or performance. The complete
manifest/EXL3 current-tree review and authorized hardware gates remain open.
