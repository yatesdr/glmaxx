# cn4 real TR3 K3 CPU source proof

Date: 2026-08-04

Status: real-payload K3 CPU source proof passed; no CUDA claim

Source commit: `1c8459e3d8597cc45eea344f2518d52df66c9507`

## Result

The accepted EXL3 source-v1 Rust path read gate, up, and down for layer 3,
expert 0, rank 0 directly from the checkpoint's owning safetensors shard. The
tier map identifies that expert as K3 and independently reports the expected
192 K3 plus 64 K4 experts for the layer.

The run freshly hashed the complete 4,376,993,600-byte shard and matched its
publisher manifest row:

```text
31bc19eabf05d0782e33103672094f1d8aca2a8bb9fb5b88a502cd6caab61bd0  model-layer-003.safetensors
```

All twelve component names were resolved through the authenticated index to
that shard before proof execution. Each projection was imported and fully
reconstructed twice. The two complete JSON reports were byte-identical for
every projection.

| Projection | Source payload SHA-256 | Reconstructed FP16 SHA-256 |
|---|---|---|
| gate | `68e96700af31debf63c42be271595df75c523f40177e6b6f48c0bab4b24a0ec4` | `a13c295c381993da35eaef392c412024e70dd3d80c28612f71fb24cd17a74d13` |
| up | `a9ae3c42b7d42cea855efe5f090da0b72f162e535f0416e62cf2f8b55e92d7c3` | `407fdb277e75a855e3e29a45780a98f7ad0e16946de2f7f535e03713ba2d30ed` |
| down | `d3b387d38d4aa83a07bc911d3736c1fda61a8beb79f0fb501e9d509869056b35` | `855ceff4802447d39ce227690921d510f57248fa1b2ddbe841eb377e99053bf8` |

Each projection consumed 1,192,964 source bytes and reconstructed 6,291,456
FP16 bytes. The first three-projection pass, including container startup,
took 0.619 seconds; the repeated pass took 0.653 seconds.

## Expert and rank boundary cross-check

A second no-GPU run repeated the complete proof at the opposite semantic
boundaries: layer 3, expert 255, rank 3. The tier map also identifies expert
255 as K3, and the index maps all twelve boundary component names to the same
freshly rehashed owning shard. Both passes were byte-identical.

| Projection | Source payload SHA-256 | Reconstructed FP16 SHA-256 |
|---|---|---|
| gate | `35320be2a483254664b3efcbe64cf81d7194b71cb422161fdff4e0e80677f33a` | `675f7c23a869ed712aa5a31606ee4e76ee421fbfd8ae42474f1b49184a6c8f1c` |
| up | `4c0c0386bfe3525abeae896b54641f0885dfa649c6356257c48f9ece0c835ea6` | `1078be2dfd5b397805630b3f19e9b8b242c1f5293f94b037372bfdbbcdc7d6f4` |
| down | `56e6f1ccae07a34bd8db05061eab8fb6ac874c4269c883a834679e315bb0ec89` | `86a784f00bad0c2296c189e44a9c74452b430b86feeae35e2836d5bc656e4259` |

Boundary evidence:
`/home/derek/glmaxx/evidence/20260804T045640Z-tr3-k3-boundary-cpu-1c8459e-r1`.
Its artifact-manifest SHA-256 is
`06008d4e75e5f67d91223992194cb12100a45ed39ce1c9c98e2344fd58e5390a`.
The gate/up/down report hashes are respectively
`6701f76359085a3c4532c20f34770e26965c4d6c4c1b4be9b617e0b333566422`,
`42c0f23983b0b48b4ae0b347766425cfc1386161130abc9e542b701a3f576c11`,
and `0bc2fb8425d947cd035b8abc9e8e513d02584de5c174658ec0c6dc63aa54bd07`.

## Recurrent draft source cross-check

The real recurrent draft layer was checked separately at layer 78, expert 0,
ranks 0 and 3. Its tier record has no `k` field, no NVFP4 membership, and a
complete `tail_tr3` list from expert 0 through 255. The physical component
shapes passed the K3 source-v1 validator for gate, up, and down on both ranks.

The complete 4,244,751,352-byte owning shard freshly matched its publisher
manifest digest:

```text
5448b63a32e394e8cbff5a4737fb50b40fc53c6c3c41305f1a7ee540c4d9a6e3  model-layer-078.safetensors
```

| Rank | Projection | Source payload SHA-256 | Reconstructed FP16 SHA-256 |
|---:|---|---|---|
| 0 | gate | `be25094eb8d8de1e343011645fc950ac3b979f46daffaf8ae838a7ba5f72b8d1` | `e993ea39728048571b56684abb6bceef59c37f8f12e34aebfbb509d63f40f153` |
| 0 | up | `d31e070132aab01151807f945baab3a64a87badc8ecb626dffbde40cc25575b1` | `d128668be2f7ebc4517ede9daf96667650cedd7937af1c59a9c3f07324eadf7c` |
| 0 | down | `5c6d4360d30461ce2e84fc4fd7a1d7ffc9a7d43d71001ea3b95bdbd3622f8bc1` | `fbdf7f076f6ae61a690c81f77bbe4d0cac1045b922e9380ed691bb93b0e50848` |
| 3 | gate | `1bcfe298cf7828086d4241f2d7ebd81bf52eb21361dca017b67b190760c840f5` | `56bc100510b627e23b3de354531dd376ef11a81543842dadcd65db7cea67fd65` |
| 3 | up | `08a8a4517b1dead6e47288739b2ac2d1ebcbad802883c5c5ed45d64765241543` | `af06d1c618596ec523fe548546653140aa601a5d9326603f546b549ad1ab0409` |
| 3 | down | `1e908f8ae82fba6b934f2c34c33a475812e942fd7fb16bc6c74c076c76b2b9b4` | `6a2602ef623bbe9b5533df2849d50cee870c6a7cebcbf9dcf96b2177e79c0c47` |

Draft evidence:
`/home/derek/glmaxx/evidence/20260804T050038Z-tr3-draft-k3-cpu-1c8459e-r1`.
Its artifact-manifest SHA-256 is
`4f1b8ee13041167be75e0f497b9d041a691869cec02a4732270947cc7a8e633b`;
the run-script SHA-256 is
`ecc6dcdf15412c1552c2e3d48c9fa233096226d4b061d255276c8dcb818d6280`.

## Provenance

- Checkpoint:
  `/home/derek/models/GLM-5.2-EXL3-TR3-3.25bpw`, immutable publisher revision
  `e2b03576cd103e6ad322a1e091e5d0e2d0529073`.
- Evidence:
  `/home/derek/glmaxx/evidence/20260804T045218Z-tr3-k3-real-cpu-1c8459e-r1`.
- Evidence-manifest SHA-256:
  `e5e37a64e40b14a8005b8f1f038ce09d61beeaea95ce28538ce26eedf58ff54a`.
- CPU runner SHA-256:
  `076cfc9f1631b0cac2f986b4856b6972f4666687870b225ff23b88c4a41d6992`.
- Gate report SHA-256:
  `d507c17c3bb1f92f25dfdfb552242d873053e3dec3504becd00401e5b12d3011`.
- Up report SHA-256:
  `eee413605badba9a85f45f3ed2d7108ff2ad3b6278c06a70462ce1526be7d61a`.
- Down report SHA-256:
  `d36f0ae319d7c09aa0136d7933e062cb7bf55b579499ae931cc7b729c00974f5`.
- Run-script SHA-256:
  `26fe3e910b1dc1292b02b5d62a2aea2bb652f4bb5c7ee88d061ee5bc1aa4225a`.
- Container:
  `sha256:0b400cb8ba8dc58d8ae9729702260b5c3d1abaa063a8ef9e14380d72df773842`.

The proof containers used `NVIDIA_VISIBLE_DEVICES=void`. Host GPU state was
2/2/2/10 MiB before and after, with no compute processes in either record.

## Claim boundary

This establishes deterministic source import and complete CPU reconstruction
for two real target K3 experts spanning expert and rank boundaries plus the
real recurrent draft source on ranks 0 and 3, all at the three production
projection shapes. It supplies full-hash-gated payloads for the later
scalar-versus-staged SM120 comparison.

It does not admit the globally sharded checkpoint, implement K4, exhaustively
prove other experts or target layers, execute the recurrent draft or MTP,
launch CUDA, establish kernel performance, replay a TP4 layer, run the
checkpoint, measure quality, serve requests, or prove KV capacity. K4 and the
mixed 192:64 program remain behind their separate review and CPU-proof gates.
