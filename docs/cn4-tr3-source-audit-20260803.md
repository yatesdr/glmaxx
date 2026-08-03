# cn4 TR3 3.25-bpw source audit

Date: 2026-08-03

Status: model payloads verified; production source admission remains blocked

## Scope and isolation

The checkpoint was read only from
`/home/derek/models/GLM-5.2-EXL3-TR3-3.25bpw`. Work and evidence remained
under `/home/derek/glmaxx/`. cn4 had no vLLM, GLMAXX, SGLang, Ray model, or
other GPU compute process before or after the run; all four GPUs remained at
0% utilization with 2/2/2/10 MiB reported used. No CUDA device was passed and
no model, vLLM, container, cache, shared-memory, or checkpoint state was
modified.

## Full manifest result

The authoritative run was:

```text
cd /home/derek/models/GLM-5.2-EXL3-TR3-3.25bpw
nice -n 19 ionice -c 3 sha256sum --check --strict MANIFEST.sha256
```

It ran from `2026-08-03T13:02:47Z` through `2026-08-03T13:14:51Z`, or 724
seconds. The stale `.manifest_verified` marker was not consulted.

The manifest has 97 entries. Ninety-four matched, including all 81
safetensors files, `model.safetensors.index.json`, `tier_bitmap.json`, the
configuration, tokenizer, calibration manifest, and scripts. Three
non-weight deployment-metadata rows prevented a passing exit:

| File | Manifest SHA-256 | Observed SHA-256 | Result |
|---|---|---|---|
| `README.md` | `69523e1a1af7e34165678f0b05040aab9cd13d1894370e12c2354994646be6e6` | `5e02ff95713b019267f0ad298f59b542c4fe1c61230b51437a00906d25e6b8d5` | mismatch |
| `docker-compose.yaml` | `766c8c7a851612063e314df419227ceb17de4b311b2beb9c042b2d9c592acb39` | `eeebf4a0ef7639f842bcb072c47babb8cf1439685f930c0d50801335cda9a83f` | mismatch |
| `docker-compose.yml` | `504e02f85352cc21f8e902a9fbf44600e70c18fdc998d4d2c733e90c91697145` | absent | missing |

The Hugging Face local-download metadata consistently names revision
`e2b03576cd103e6ad322a1e091e5d0e2d0529073`. An independent fetch of these
three files plus `MANIFEST.sha256` from the inferred
`brandonmusic/GLM-5.2-EXL3-TR3-3.0bpw` repository at that revision returned
HTTP 404 for every path. The root-owned cached tree record was unreadable by
the unprivileged cn4 account. Therefore this run does not authenticate the
observed metadata bytes against an immutable publisher revision and does not
authorize new manifest exceptions.

Production admission remains fail-closed until the publisher corrects and
re-signs the manifest, supplies the missing file, or a later adversarially
reviewed audit binds every non-model discrepancy to the actual immutable
publisher revision. The 81 model payload matches may be used as source
evidence but do not turn the overall exit into a pass.

## Raw evidence

Authoritative evidence root:

```text
/home/derek/glmaxx/evidence/20260803T130218Z-tr3-manifest-r2
```

| Record | SHA-256 |
|---|---|
| `manifest-check.txt` | `a431c377e565c84f7e687928ca2928bfde2ed0f6cd26ad2ee770ffbd0ac3332f` |
| `input-sha256.txt` | `49c2deeea3cca47d4462b5756e803a85c81fd53d0b257e199ffa9ef00c8717f1` |
| `run.txt` | `0864e361975ec364ad63c24e85321aa0be59bdd4924195eba4de7f3253622a28` |
| `start-utc.txt` | `99c8cf64c373964156727ca451b79e42ae39fefefa67436b8676e5cd449586b4` |
| `end-utc.txt` | `0576d32ed8f9ef14ff570ab8e4a7bb53bf5dc9e16b8b8619e37990af6fab6350` |
| `exit-status.txt` | `4355a46b19d348dc2f57c046f8ef63d4538ebb936000f3c9ee954a27460dd865` |

The preserved failed setup attempt at
`/home/derek/glmaxx/evidence/20260803T130007Z-tr3-manifest` exited 127 because
cn4 lacks `/usr/bin/time`; it performed no checksum work and is superseded by
the authoritative run above.
