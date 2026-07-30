# EXL3 warp-staging CPU proof v2

Date: 2026-07-30

Status: CPU proof candidate; independent adversarial review required before
CUDA implementation

Accepted design:
`docs/exl3-sm120-warp-decode-v2.md`

Accepted design SHA-256:
`67fb3bcb5b839cc50f3462990f1ef6056ca7c9d991851efc5e668fed9d0b3325`

GPU evidence: none

## Scope

This proof implements gate 2 of the accepted EXL3 warp-staged decode
sequence. It does not implement or authorize the CUDA entry point.

The deterministic Rust proof is
`glm_format::prove_exl3_warp_staging_v2`. Its canonical output is
`fixtures/exl3-warp-staging-proof-v2.json`.

Run it with:

```text
cargo run --release --offline -p glm-cli --bin glmaxx -- \
  exl3-warp-proof /tmp/exl3-warp-staging-proof-v2.json
cmp fixtures/exl3-warp-staging-proof-v2.json \
  /tmp/exl3-warp-staging-proof-v2.json
```

The proof also runs as a `glm-format` unit test and the fixture comparison is
part of `scripts/local-checks.sh`.

## Independent paths

The scalar side calls the retained source-order `decode_native_at` function.
That function derives `(lane, weight)` from the inverse mapping and forms each
U32 source word from the two original U16 halves.

The staged side does not call that decoder:

1. it constructs a 16 by 16 slot table from the forward scatter;
2. it proves that every cell is written exactly once and cross-checks the
   completed table against the inverse mapping;
3. it forms a little-endian U32 view of the source;
4. it simulates threads 0 through 191 with the accepted
   `tile = thread / 24`, `word = thread % 24` mapping;
5. it loads one 8 by 24 U32 stage using the real
   `k_tile * n_tiles + n_tile` source address; and
6. it decodes only from that stage using the forward-derived slot table.

The two paths share only the accepted MCG codebook transform after each has
independently produced the same cyclic 16-bit window.

## Exhaustive coverage

The proof runs both production projection geometries:

| Family | K | N | K tiles | N tiles | 8-tile stages |
|---|---:|---:|---:|---:|---:|
| gate/up | 6,144 | 512 | 384 | 32 | 48 |
| down | 512 | 6,144 | 32 | 384 | 4 |

For each geometry it visits every N tile, stage, K tile, local K position,
and local N position. This compares exactly 3,145,728 weights per geometry,
or 6,291,456 total. Each shape schedules exactly 1,179,648 source-trellis
bytes, matching the source plane byte count.

For every output column the proof also applies the scalar and staged weights
to eight deterministic FP16 activation rows. Both sides use the same
ascending-K multiply-then-add recurrence with an explicit product boundary.
It compares every FP32 accumulator bit pattern before the FP16 projection
store and hashes the resulting FP16 plane.

The CTA ownership simulator separately checks row counts 1 through 8. Active
ownership is exactly 16 threads per row, every active `(row, column)` has one
owner, every inactive row has none, and all 256 threads arrive at both stage
barriers in every row case.

## Canonical result

The canonical fixture SHA-256 is:

```text
d0b1aaa375247d5ecfa7f889780e87ac47f9df3bef0e3ff603ff18075e290602
```

The release and debug binaries emitted identical fixture bytes on the Phase A
host.

Key results:

| Check | Result |
|---|---|
| 256-thread load mapping | bijective over 192 staged U32 words; 64 idle load threads |
| stage size | 768 bytes |
| row ownership | exact for rows 1 through 8 |
| barrier arrivals | 512 per CTA simulation for every row count |
| gate/up weight hashes | scalar and staged both `b94b4940e85d67dee8954c01eda53b7d522dc55a962f3fa074170496a53b8854` |
| down weight hashes | scalar and staged both `d9eda644ed48546a03a3219e040da335f27c4604242edb2178765efbd2e22944` |
| gate/up FP16 projection hashes | scalar and staged both `e5a418f505909da940b201863011b781acfe086f775294b91ad8b5cc6fd5b000` |
| down FP16 projection hashes | scalar and staged both `59a51359929d6cacaa84008ee221a1866f97fb9c15105875fbeb2dffa1ed1170` |

The canonical verdict is:

```text
EXHAUSTIVE_STAGED_SOURCE_AND_ASCENDING_K_BITWISE_PASS
```

## Claim boundary

This proves the accepted CPU address schedule, cyclic staged decode, row
ownership, barrier totality, source traffic count, and ascending-K numerical
equivalence for deterministic synthetic source bytes.

It does not prove CUDA barrier behavior, SM120 compilation, SASS, resource
counts, device bitwise equality, real-checkpoint equality, occupancy, memory
traffic, latency, routed execution, prefill, one-layer execution, checkpoint
smoke, model quality, or serving performance. Those remain later gates in the
accepted sequence.
