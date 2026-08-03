# cn4 NVFP4/NF3 hybrid source inventory

Date: 2026-08-03

Status: read-only source discovery; no GPU execution or checkpoint admission

## Isolation and provenance

The checkpoint was inspected read-only at
`/home/claude/LLM/GLM-5.2-hybrid`. All generated evidence was written under
the isolated GLMAXX root:

```text
/home/derek/glmaxx/evidence/20260803T133928Z-hybrid-format-inventory
```

The final `evidence.sha256` hashes as
`5f495512ae0d9ba261343450985993c1fc0692d6ae5a4df9231d847ff89cfc6d`.
No tensor payload was decoded and no CUDA context, GPU kernel, model process,
or container was started. At the final check all four GPUs reported 0%
utilization and no compute process; memory use was 2/2/2/10 MiB.

The Hugging Face cache metadata pins revision
`68babde27a97a4c980c2494e830dd424975cd5a3`. Decisive file hashes are:

| Input | SHA-256 |
|---|---|
| `config.json` | `254974797e9f455716a30ab5505ba68272181b20b58a3693e54f94fb8056f3ef` |
| `model.safetensors.index.json` | `6eb773222d932418dd0530c63aca498f86ef424da2a4526ccba76b59726da234` |
| `mxfp8_tier_nokvb.json` | `ebcd6087180033d4512fafa5f154f4fecfbc1ee5e5051448f34859cccc4430f0` |
| `README.md` | `ef4eb4e7f2e00b9767ef1251ab7387483bc77f60016c95a7a9742f5c208be518` |

## Exact observed inventory

The producer identifies `quant_algo=NVFP4`, `quant_method=modelopt`, and
ModelOpt `0.39.0.dev290+gf9d9a71de.d20260214`. The authenticated source
contract still has two routed-expert codecs:

- layers 3 through 77 each contain 192 NF3 experts and 64 NVFP4 experts;
- the target total is 14,400 NF3 and 4,800 NVFP4 expert assignments; and
- layer 78 is absent from the hybrid map and is uniformly NVFP4.

The index has 148,289 tensors, including 147,072 routed-expert tensors. Its
expert component counts are exactly:

| Component suffix | Count |
|---|---:|
| `weight_packed` | 43,200 |
| `weight` | 15,168 |
| `weight_scale` | 58,368 |
| `weight_scale_2` | 15,168 |
| `input_scale` | 15,168 |

The counts reconcile as 14,400 NF3 experts times three projections, plus
4,800 target and 256 draft NVFP4 experts times three projections.

Layer 3 expert 0 is representative NF3. Gate/up tensors have logical shape
`[2048,6144]`, `weight_packed` U8 shape `[2048,2304]`, and `weight_scale`
F8_E4M3 shape `[2048,192]`. Down has logical shape `[6144,2048]`, packed
shape `[6144,768]`, and scale shape `[6144,64]`. This is exactly three bits
per value plus one scale byte per K/32 group.

Layer 3 expert 6 is representative NVFP4. Gate/up `weight` U8 has shape
`[2048,3072]` and `weight_scale` F8_E4M3 has shape `[2048,384]`. Down uses
`[6144,1024]` and `[6144,128]`. Every projection also has scalar F32
`weight_scale_2` and `input_scale`. This is exactly four bits per value plus
one scale byte per K/16 group.

The representative header evidence hashes are
`1fb2dfb661e7ea18cb8dfdce953dbd39cf79fceeaee2e44a09f0e058934bb579`
for NF3 and
`132f5194a4a1b5740f8a757b052156978941cde2f99772894e7c2e51019ee207`
for NVFP4.

## NF3 reference discovery

A bounded read-only inspection of the existing staging exports recovered the
producer's missing NF3 math. No source file was copied into GLMAXX. The exports
contain no `.git` metadata, so the following complete file hashes, rather than
their directory names, are the only asserted source identity:

| Reference file | SHA-256 |
|---|---|
| SparkInfer `intrinsics.py` | `5177327722e1e4606dbbc57a7798575eae78c49abbbaebbf9ccac56053124117` |
| SparkInfer W4A16 `host.py` | `c6b41bf23b3d18024a1a8b4d19fff168fb51de164624a32c72c2e01571e9d2d4` |
| SparkInfer W4A16 `kernel.py` | `11a4dedeb1ff8eee01c13314582081776059e719658dc4189eb6cdc76eb68c4d` |
| SparkInfer W4A16 `prepare.py` | `b54175a861730662350a2ef5ee63989c8afafc907b6a3c13a3331928cfb9285f` |
| SparkInfer NF3 test | `07891aba04146ea1eb655765c8e493a7fecd36958568fab03ce5aa09c62edc49` |
| vLLM hybrid adapter | `a8b4e19c5e776ece1d6c7ff2c48da236d1bd4032a3399f7b6a9563955c99f61b` |

The derived NF3 contract uses codebook decimal values
`[-1,-0.6047,-0.3563,-0.1275,0.1275,0.3563,0.6047,1]`, which round to BF16
bit patterns
`[bf80,bf1b,beb6,be03,3e03,3eb6,3f1b,3f80]`. Source codes are eight
consecutive unsigned three-bit values in a little-endian 24-bit word. A weight
is the BF16 codebook value times its K/32 E4M3 scale, rounded to BF16.

## Nonclaims

This inventory does not authenticate every payload shard, admit the checkpoint,
prove the NF3 derivation independently, accept the staging source as a GLMAXX
dependency, qualify a native layout, or establish quality, fit, capacity, cold
start, or performance.
