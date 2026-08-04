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
for one real target K3 expert on rank 0 at all three production projection
shapes. It supplies a full-hash-gated payload for the later scalar-versus-
staged SM120 comparison.

It does not admit the globally sharded checkpoint, implement K4, prove other
experts/ranks/layers, launch CUDA, establish kernel performance, replay a TP4
layer, run the checkpoint, measure quality, serve requests, or prove KV
capacity. K4 and the mixed 192:64 program remain behind their separate review
and CPU-proof gates.
