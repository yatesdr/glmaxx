# cn4 integration checkpoint admission diagnostic

Date: 2026-08-04

Status: current TR3 parser gap reproduced; hybrid structural admission passed

## Scope and provenance

Clean detached source commit:
`6e073f359d7e370b830bc4080180d372bf7c3e75`.

The run used the GLMAXX-only image
`sha256:0b400cb8ba8dc58d8ae9729702260b5c3d1abaa063a8ef9e14380d72df773842`
with no GPU devices exposed. The release binary SHA-256 was
`55af7cbac00543b955bab19c68721d6bd63ef3e95d2dc210409f3ae67fec1108`.
Both checkpoint directories were mounted read-only. No compute application
was present before or after the run.

Sealed evidence:

```text
/home/derek/glmaxx/evidence/20260804T081415Z-integration-admission-diagnostic-6e073f3-r4
```

Its `evidence-sha256.txt` SHA-256 is
`ab96339f80f735ea19c5e767aa32105be496599f4b6dd6d276582cdf6c7de3b3`.
`sha256sum --check evidence-sha256.txt` passed after sealing. The retained
runner SHA-256 is
`8c98f33db8cebbafd579efae348bf8ebabe1a69ed2c8e04d6cd284a9c502cc10`.

## Result

The current integration reader rejected the real TR3 3.25-bpw index after
2,654,272,889 ns with exit code 1 and exact stderr `glmaxx: Index`. Its pinned
index/config/tier hashes were respectively
`f5dcd976a64ca70808dd4d8bd3ad07e9610c8ca6c30e3a6ed77ddefdac4c1d21`,
`d83e2d8d96b6f36e94d896a05a104200d6674673daa02f788de710c1c0f94ba4`,
and `a287ffe816de5998fbc35a56a1ec05f69eb71087d5bbdfe631242c6b296b2a3d`.

The same binary admitted the real hybrid index in 535,547,434 ns and reported:

- structure SHA-256
  `6eb773222d932418dd0530c63aca498f86ef424da2a4526ccba76b59726da234`;
- 184 shards and 148,289 tensors;
- 365,968,736,768 tensor-payload bytes;
- 14,400 NF3 expert assignments are not established by this generic inventory
  result; that membership remains a separate tier-map/source gate.

The retained hybrid JSON SHA-256 is
`3545804f86cc7244815b6a937bb9040ee225c13184346d15bcdccd25e2e2f2f8`.

## Disposition

This reproduces the known `metadata.total_size` defect in executable current
integration code. `ShardedSafetensors::open_with_workers` still accepts only
`declared == actual_payload_bytes`, while the real TR3 publisher declares the
exact complete-shard-file total. The corrective contract is
`docs/safetensors-index-total-size-v1-r2.md`; implementation remains closed
until Fable returns the exact token
`safetensors-index-total-size-v1-r2-design-accepted`.

After that token, the required regression is stricter than merely making this
command return zero: the typed inventory must report declared, payload, file,
overhead, and interpretation fields for both real conventions, preserve all
open-descriptor anti-TOCTOU checks, and retain this exact paired real-source
test. This record authenticates metadata and structure only. It is not payload
authentication, conversion, CUDA, checkpoint execution, quality, capacity,
cold-start, or throughput evidence.

Three setup attempts and the first fail-fast current-parser attempt were also
sealed `FAILED`, rather than overwritten:

| Evidence directory suffix | Manifest SHA-256 | Cause |
|---|---|---|
| `20260804T080713Z-integration-admission-6e073f3` | `4e019e1cae2d671ce2f60475037cb65c8372b31eaa5a660067b06e4e7ede3dfb` | host Rust toolchain absent |
| `20260804T080815Z-integration-admission-container-6e073f3` | `cfa950413e129c17de5c649903af7fc3401d20b5fda7881b4cfbe677454de966` | nested command quoting rejected before admission |
| `20260804T081012Z-integration-admission-container-6e073f3-r2` | `f8754a131071cfce79be13440c635878c286e4d465a93bd48e7be1270420901b` | image lacks `/usr/bin/time`; build succeeded, admission did not start |
| `20260804T081202Z-integration-admission-container-6e073f3-r3` | `60d9c3b8e6e4e98fd2040bfa310fe86a57efee56a8b5971a9fcc2581effb671b` | fail-fast run stopped on the reproduced TR3 `Index` rejection |
