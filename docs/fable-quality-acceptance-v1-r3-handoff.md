# Fable handoff: quality acceptance v1 r3

Date: 2026-08-04

Status: corrective adversarial design review requested

Review candidate commit:
`ab8dc028a708b2db78e276a8b01d9ad8c5d95703`

Required result path:
`fable-quality-acceptance-v1-r3.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`quality-acceptance-v1-r3-design-accepted`

GPU authorization conveyed by this handoff: none

Do not connect to cn4, launch CUDA, read checkpoint payloads, or execute any
vLLM resource. This is an immutable-source numerical and ABI design review.

## Provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Any mismatch or incomplete input withholds the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `docs/quality-acceptance-v1.md` | `705bb0611464bd5d76a08943b3122ecb8a78506e78f9c20a46d4e1ce24fc7be6` |
| `docs/quality-acceptance-v1-r3.md` | `44392c02bfc84b6813c90b8348033c953624bae436925acd220cf1a0ee6af0cd` |
| `docs/quality-corpus-manifest-v1.md` | `c74288b346dbb8d5f1c8bc87037207a8b6aa4d713efa1c134ec97f625b703c29` |
| `docs/fable-quality-acceptance-v1-r2-handoff.md` | `4788513838870418f6e6e889ead135ecd8b356204a1d98070c30a3f800f17782` |
| `docs/prior-cn4-kld-provenance-20260804.md` | `0bc7b3191d2df439323abdec3ce5982cebb82ea40a2dafc9c36ec8377f62094a` |
| `docs/distributed-sampling-abi-v1-r2.md` | `f2fb8ec8c81c63e76b7a0639fddc8c74719faff2a972bafcdf0b1d5de8db3db7` |
| `docs/matched-runtime-control-v1.md` | `446e25396e7eabd2fce85aa848c70318f964b1a9a7cf02a4945acc9917c02bf8` |
| `docs/benchmark-contract.md` | `024eda56d7bb7632c5023d4d0e8f095bd7b32a05ed241df1f8a13369ce5a3ebe` |
| `docs/native-engine-plan.md` | `493c0d218d93a3a8d7cf83da45a934fc44570fc190e85340c5eaba74edd50bdd` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `docs/tokenizer-serving-contract.md` | `9e9c82d2375d3f759b10568fdb33bc75d102f8a2f609ddbc6f980036c81b7527` |
| `docs/local-inference-lab-decode-bench-20260803.md` | `b0b43ee158ce665efc196a2b27b59ec572f0989abbd4f7858708e665941f02ea` |
| `docs/cn4-experiment-isolation-v1.md` | `aab1dc4860fd2dde21e19b067b211f842387436d3d92a48b2fb31037a945d735` |

Run `./scripts/local-checks.sh` in the pinned worktree and record its exit
status. A local host without CUDA must not claim device evidence.

## Immutable prior-procedure check

Do not inspect the dirty working-tree bytes of `../glm52-opt`. At exact commit
`38cba1091c043bdecd426a0d4625f58211f94e0c`, verify:

| External input | SHA-256 |
|---|---|
| `harness/run_glm52_tr3_dynamic_kld.sh` | `63c02bd1156ef8db49f9c0fc7d3d80fdbf46f9331f9499d7c334f37cca9ac55a` |
| `harness/prefill_kld_fallback_integrity.patch` | `19b183d48322ceb6d680e9234a1bf14a98343413d970853acf4f96347fe7e9f0` |
| `harness/prove_kld_repeat_determinism.py` | `690225fa94334ac830c8a1ae8b7d5789137ae439921792e76c12682b7786e39d` |
| `experiments/2026-07-28-v20-nvfp4-scaling-kld-n3/README.md` | `0bd879ea07d8b6be00271c2736b2c15b20cac9cf2ea27b5a3261f19beff56524` |
| `experiments/2026-07-28-cn4-tr3-qualification/README.md` | `c54bf499b8edb5d5886daa372ad34025ebc652f6545c45b353e1b389bdd09fff` |

Also verify the wrapper pins runner
`d1dc1a63b9889e881f3bd899638d0ec65a1a1079132f6a207a600d9cba845405`,
reference logits
`87f992a689c054a0548a4b3863da6c809f9239beacd5786d0401e45904fec063`,
reference manifest
`985120136741037918bcd4dc8da9813c1f6268b35a730302f99cf6b3eebb7606`,
and historical image
`sha256:a5608e0b4a2fcdaec476de79fbe5cf2f6e9ce2ecf30bf2dfe0c1314d97c6666e`.
Do not execute that image.

## Required independent work

1. Independently encode `QualityFamily.v1`, `QualityRun.v3`, and all three
   `QualityComparison.v1` kinds. Mutation-test every relation and prove the
   family digest is independent of weight policy and MTP depth.
2. Independently encode the exact 320-byte `PositionQuality.v3` record. Check
   every offset, raw float bit field, enum discriminant, digest, and terminal
   byte without importing an implementation helper from the candidate.
3. Reconstruct `row_identity_sha256` and `case_id_sha256` from their complete
   preimages. Prove a changed raw-row ordinal, token-history digest, cache
   posture, position, policy, or MTP depth cannot alias an accepted record.
4. Build a small independent binary32/binary64/MPFR-256 centered-error oracle.
   Exercise signed zero, ties, extreme finite logits, reordering,
   reassociation, contraction, nonfinite values, and MPFR flag failures.
5. Enumerate the exact 64-window/131,008-row corpus and the exact retrieval
   band/bin counts from the bound corpus manifest. Reject every one-off,
   reordered, shortened, extra, or calibration-overlapping mutation.
6. Independently implement the 10,000-replicate paired repetition bootstrap,
   including the SplitMix64 stream, sub-stratum preservation, binary64 order,
   value/ordinal sort, and one-based rank 9,500 endpoint. Exercise immediately
   below, at, and above 0.010.
7. Re-run the revision-2 Wilson, MPFR KLD, bootstrap, Holm, task-margin,
   retrieval, and control-selection derivations needed to prove this amendment
   preserved every threshold and did not create a conflicting identity.

## Required decisions

Answer each with an unqualified `YES` or `NO`:

1. Do all candidate and external hashes match at review start and finish?
2. Is `QualityFamily.v1` complete, deterministic, and invariant exactly where
   the three comparison kinds require it to be invariant?
3. Are `QualityRun.v3` and `QualityComparison.v1` domain separation, UUID/hash
   preimages, discriminants, and admission relations exact and non-ambiguous?
4. Is the 320-byte `PositionQuality.v3` layout total, padding-free, byte-exact,
   and sufficient to retain every per-position quality value required by the
   revision-2 gates?
5. Are row/case identity preimages complete enough to prevent wrong-row,
   wrong-policy, wrong-cache, wrong-history, or wrong-depth comparisons?
6. Is centered-error maximum/RMS arithmetic reproducible and fail-closed for
   every specified floating-point and MPFR edge?
7. Are exactly 64 windows, 131,008 paired rows, and every retrieval band/bin
   bound without an open-ended-count escape?
8. Is the repetition endpoint an exact, implementable one-sided paired
   bootstrap rather than an unspecified confidence bound?
9. Does revision 3 resolve all four stated revision-2 ambiguities without
   weakening or silently changing any numerical, task, retrieval, MTP,
   capacity, or performance threshold?
10. Does the combined contract remain consistent with sampling, tokenizer,
    benchmark, cache-posture, prior-cn4, and no-gather production rules?
11. Does acceptance open only CPU evaluator implementation/proof and preserve
    the required gate order before model or performance execution?
12. Are all implementation, CUDA, model, KLD-result, task, retrieval,
    capacity, cold-start, latency, throughput, and serving nonclaims accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, followed
by the independent derivations and all twelve decisions. Only if every answer
is `YES`, attest the candidate commit, all fourteen repository inputs, the
external commit, and all five external inputs, then end with the requested
token as the only bare acceptance line.

Acceptance does not accept an evaluator implementation, authorize cn4, run a
model, enable MTP, or establish any quality or performance result.
