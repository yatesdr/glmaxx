# Prior-cn4 KLD provenance pin

Date: 2026-08-04

Status: historical source and evidence pinned; no GLMAXX KLD result yet

This record separates the reproducible prior-cn4 compatibility diagnostic
from the revision-2 GLMAXX quality gate. The historical scalar is useful for
comparison, but it is not quality acceptance evidence.

## Immutable historical source

The only admitted source is the Git object at immutable `glm52-opt` commit
`38cba1091c043bdecd426a0d4625f58211f94e0c`. The following hashes were
recomputed directly from that commit rather than from its working tree:

| File | SHA-256 |
|---|---|
| `harness/run_glm52_tr3_dynamic_kld.sh` | `63c02bd1156ef8db49f9c0fc7d3d80fdbf46f9331f9499d7c334f37cca9ac55a` |
| `harness/prefill_kld_fallback_integrity.patch` | `19b183d48322ceb6d680e9234a1bf14a98343413d970853acf4f96347fe7e9f0` |
| `harness/prove_kld_repeat_determinism.py` | `690225fa94334ac830c8a1ae8b7d5789137ae439921792e76c12682b7786e39d` |
| `experiments/2026-07-28-v20-nvfp4-scaling-kld-n3/README.md` | `0bd879ea07d8b6be00271c2736b2c15b20cac9cf2ea27b5a3261f19beff56524` |
| `experiments/2026-07-28-cn4-tr3-qualification/README.md` | `c54bf499b8edb5d5886daa372ad34025ebc652f6545c45b353e1b389bdd09fff` |

The wrapper pins:

| Input | Identity |
|---|---|
| cleanup runner | `d1dc1a63b9889e881f3bd899638d0ec65a1a1079132f6a207a600d9cba845405` |
| BF16 reference logits | `87f992a689c054a0548a4b3863da6c809f9239beacd5786d0401e45904fec063` |
| reference manifest | `985120136741037918bcd4dc8da9813c1f6268b35a730302f99cf6b3eebb7606` |
| historical evaluator image | `sha256:a5608e0b4a2fcdaec476de79fbe5cf2f6e9ce2ecf30bf2dfe0c1314d97c6666e` |

The current `../glm52-opt` working-tree wrapper is not this artifact: it
hashed as `4d55ca740468be31745551d3a8ea2a106724d683785c207c608daf2b0fa1ac3f`
on 2026-08-04. It must never be substituted for the immutable Git object.
GLMAXX does not modify, execute from, or write into `../glm52-opt`.

## Exact compatibility operation

The legacy cell consumes one 2,048-token window and scores its 2,047
teacher-forced next-token rows. It uses TP4, DCP1, MTP0, eager execution,
exact sparse selection, one sequence, a 512-token prefill budget, and
32-row KLD chunks. The candidate and reference are physical FP32 arrays with
shape `[2047, 154880]`; the historical operation includes all physical rows.

For each 32-row chunk it computes PyTorch FP32
`log_softmax(reference)` and `log_softmax(candidate)`, then
`kl_div(log_candidate, log_reference, reduction="none",
log_target=true).sum(dim=-1)`. Each FP32 chunk sum is promoted to a Python
binary64 accumulator and the final sum is divided by 2,047. The direction is
`KL(BF16 reference || candidate)`.

This is intentionally a compatibility operation. It is not equivalent to
the logical-vocabulary-only, 256-bit MPFR operation in
`docs/quality-acceptance-v1.md`.

## R14 TR3-3.25 reference cell

The archived evidence is at:

```text
../glm52-opt/experiments/evidence/cn4/20260731/
  r14-tr3-325-qualification/kld-r14-v1/results/dynamic_per_token/run1/
```

The same four files remain on cn4 under
`/home/derek/glm52-r14-cn4-kld-20260731/evidence/kld-r14-v1/`; their hashes
were recomputed read-only on 2026-08-04 and match the archive exactly:

| File | SHA-256 |
|---|---|
| `config.json` | `024385953e6df222e4ff08d1212d41c1a063643cf65c45f6e0d291bd50612f52` |
| `summary.json` | `ab290141123afa8e548c9c608672f6f98753e19ef542d56263c436f0d81d0f55` |
| `writer-compile-proof.json` | `882733729de3507c66f143cfcf71bbab06e8a0cce2c7eddedccd913759828101` |
| `prefill_dcp1.log` | `f6b17783542d8ee0ebdee34ef9d69cb71309fafd112cc3de5a872ce24b7fab58` |

The cell binds the TR3-3.25 model revision
`d7d79c2d14599dfce7a5d12b85f7ad73f40e623d`, r14 image
`sha256:cb03f2079d8a74915f01cda15f6bdf505762d13cc3fff192f7ebdaaf6e318bf2`,
EXL3 weights, dynamic-per-token `nvfp4_ds_mla` KV, FP8 RoPE, BF16 compute,
seed zero, disabled prefix caching, and no speculative model. It reported:

```text
legacy_cn4_kld_mean = 0.09280776370609745
positions = 2047
physical_logits_shape = [2047, 154880]
```

The process log shows the `Salesforce/wikitext` fallback, tokenizer `/model`,
exactly 2,048 IDs, and first 16 IDs
`[284,8396,425,10960,465,284,14721,8396,425,10960,465,374,458,6364,4531,1154]`.
It does not retain the complete token vector, a complete-token hash,
candidate-logit hash, or per-position KLD. Its configuration explicitly says
`integrity_fingerprints=false`. Repeated equal means therefore cannot prove
bit-identical tokens, logits, or per-position values.

## NVFP4/NF3 historical cells

The immutable 2026-07-28 record contains two dynamic-per-token profiles with
the same reference, runner, TP4/DCP1/MTP0 posture, and single window:

| Weight membership | Per-run means | Aggregate mean |
|---|---|---:|
| current online MXFP8 policy | `0.13999698092705107`, `0.14038452243547964`, `0.13672544879717802` | `0.13903565` |
| quality-first MXFP8 policy | `0.13319050`, `0.13542133`, `0.13117311` | `0.13326164` |

The lower row is not automatically the matched control. GLMAXX may compare
against it only if the current NVFP4/NF3 checkpoint has the same protected
tensor and online-quantization membership. The weight-policy digest decides;
the favorable scalar does not. R17 TR3-3.36 results are excluded because
their weight precision is not the requested TR3-3.25 profile.

## Required closure before a GLMAXX compatibility result

The historical archive is insufficient to establish full-token identity by
itself. Before publishing `legacy_cn4_kld_mean`, GLMAXX must:

1. obtain the accepted revision-2 quality contract and pass its CPU evaluator
   proof;
2. copy the admitted reference payload into a content-addressed, read-only
   GLMAXX fixture without using a vLLM worktree, image, cache, or result path;
3. recover and retain the exact 2,048 token IDs from that frozen fixture; hash
   their canonical bytes and reject mere first-16 agreement. If the complete
   vector cannot be recovered, label the cell
   `LEGACY_PROCEDURE_UNVALIDATED` and publish no legacy mean;
4. validate a frozen synthetic row set against the historical FP32/chunked
   operation, including the complete per-position binary64 digest;
5. retain both full physical FP32 logit arrays, all 2,047 per-position legacy
   values, their digest, top-two/tie data, exact runtime identity, and the
   matched weight/cache posture; and
6. publish the result only as `legacy_cn4_kld_mean`, never as
   `kld_reference_to_candidate` or as a revision-2 gate result.

No GPU process was launched and no cn4 or `glm52-opt` file was modified while
preparing this provenance record.
