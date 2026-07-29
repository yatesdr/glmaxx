# EXL3/Trellis CPU reconstruction candidate

Date: 2026-07-29

Status: CPU reconstruction proven; independent review and SM120 consuming
kernel remain required before serving load support.

## Pinned provenance

| Item | Identity |
|---|---|
| checkpoint | `brandonmusic/GLM-5.2-EXL3-TR3-3.0bpw` |
| checkpoint revision | `9297b9f1d53af5c67cffa01e30cc071a1ff7144b` |
| checkpoint config SHA-256 | `fcde001350291a0048318d4a1136e0732e31f829f804a57cfbb558903e54171a` |
| ExLlamaV3 version recorded by checkpoint | `0.0.43` |
| ExLlamaV3 `v0.0.43` tag commit | `c5d9c657966ffeeaa9353f0cc899f18629da4a13` |
| vLLM integration revision | `00787eeabebc11cee12cff12a823011b4e1a5ebc` |
| vLLM EXL3 source SHA-256 | `2f5d7546b1fc100b244b116b3d9b643dce7c89c5cf2da64ff820b0aa9be9584e` |
| audited SparkInfer worktree revision | `a4d72eff44f8c4ad741a6bc05efd8f8844e0a7ea` |
| CPU reconstruction oracle SHA-256 | `eed3f5ab80d2b02c4ce3db9973c21d0ae5e0113f27c5abdadef95436aee71d9d` |
| GPU decode-intrinsics SHA-256 | `7fdbeb0476bb2f2e2f314d6f596d97b81925f68e17350e9b1b97769a280a16f5` |
| GPU preparation/shape checks SHA-256 | `951f62c38a2684ba5d376d5153c52a73d8806bbeeebb575021cd2950e6d46999` |

The vLLM and SparkInfer integration sources are Apache-2.0. ExLlamaV3 and
its EXL3 work are MIT-licensed upstream. The Rust implementation is an
independent expression of the pinned numerical and byte-level contract;
these attributions remain required if later kernel code is adapted.

## Accepted GLM-5.2 schema

The checkpoint stores one logical matrix as four component tensors:

```text
model.layers.{L}.mlp.experts.{E}.{projection}.rank{R}.mcg
model.layers.{L}.mlp.experts.{E}.{projection}.rank{R}.suh
model.layers.{L}.mlp.experts.{E}.{projection}.rank{R}.svh
model.layers.{L}.mlp.experts.{E}.{projection}.rank{R}.trellis
```

The candidate accepts layers 3–78, experts 0–255, ranks 0–3, three bits,
and only MCG marker `0xCBAC1FED`. Target layers 3–77 use:

| Projection | Logical K×N | Trellis I16 shape | `suh` | `svh` |
|---|---:|---:|---:|---:|
| gate | 6,144×512 | 384×32×48 | 6,144 | 512 |
| up | 6,144×512 | 384×32×48 | 6,144 | 512 |
| down | 512×6,144 | 32×384×48 | 512 | 6,144 |

The layer-78 MTP overlay uses the same component schema but retains explicit
shape validation because its complete inventory must be generated from the
pinned checkpoint.

One projection occupies:

```text
4-byte MCG marker
2*K bytes of FP16 suh
2*N bytes of FP16 svh
2*(K/16)*(N/16)*(16*bits) trellis bytes
```

Each actual target projection is 1,192,964 source bytes. All three
projections are 3,578,892 bytes per expert per rank before the native
96-byte-per-projection record. The native policy accounts 3,579,180 bytes.

## Native trellis reconstruction

The trellis tensor is consumed directly in source order. For one 16×16 tile,
adjacent little-endian I16 halves form `8*bits` U32 words. For lane `l` in
0–31 and lane weight `w` in 0–7:

```text
end_bit   = (l*8 + w + 257) * bits
start_bit = end_bit - 16
first     = floor(start_bit / 32)
last      = floor((end_bit - 1) / 32)
shift     = (last + 1)*32 - end_bit
window    = (((word[first % width] << 32) |
               word[last % width]) >> shift) & 0xffff
```

The 16-bit window is decoded with wrapping U32 arithmetic:

```text
packed = (window * 0xCBAC1FED) mod 2^32
packed = (packed & 0x8FFF8FFF) XOR 0x3B603B60
value  = FP16(FP16(packed.low16) + FP16(packed.high16))
```

Lane values scatter into the 16×16 block using:

```text
row0   = (lane % 4) * 2
rows   = [row0, row0+1, row0+8, row0+9]
col0   = lane / 8
col1   = col0 + 4
parity = (lane >> 2) & 1

row = rows[weight % 4]
col = 2*(weight < 4 ? col0 : col1) + parity
```

Every K and N dimension is split into normalized 128-wide Walsh-Hadamard
blocks. The execution order is:

1. FP16-round the activation after its `suh` multiply;
2. apply normalized H128;
3. multiply by the reconstructed native matrix with FP32 accumulation and
   FP16-round the result;
4. apply normalized H128;
5. multiply by `svh`.

Gate and up have independent rotations. Their output-side vectors and the
down input-side vector are represented by the checkpoint's three
intermediate rotation segments. Persistent whole-matrix FP16 reconstruction
is forbidden in serving; the CPU implementation is an oracle.

## Real-payload proof

The proof extracted only the following rank-local source projection into
`/tmp`; no model data entered Git:

```text
model.layers.3.mlp.experts.0.gate_proj.rank0
```

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| concatenated `mcg+suh+svh+trellis` payload | 1,192,964 | `68e96700af31debf63c42be271595df75c523f40177e6b6f48c0bab4b24a0ec4` |
| reconstructed K×N FP16 byte matrix | 6,291,456 | `a13c295c381993da35eaef392c412024e70dd3d80c28612f71fb24cd17a74d13` |

The Rust decoder and an independent NumPy implementation of the audited CPU
oracle produced the same reconstruction digest. Re-run the Rust side with:

```text
cargo run -p glm-cli -- exl3-proof /external/path/source.payload
```

The external file must be exactly the four source components concatenated
in the order above.

## Remaining GPU gate

This proof defines the source bytes and reconstruction arithmetic. It does
not establish an SM120 kernel, an SM120 payload transformation, inclusive
operator speed, or model quality. The first GPU candidate must consume these
bytes without persistent expansion or runtime repack and must repeat parity
on actual gate, up, and down projections before codec `0x0200` can become a
healthy serving backend.
