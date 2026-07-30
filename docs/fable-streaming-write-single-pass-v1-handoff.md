# Fable handoff: streaming tensor write single-pass v1

Date: 2026-07-30

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`5ff1f8541f0fdbb14b2923694d8cc4d444470b55`

Required result path:
`fable-streaming-write-single-pass-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`streaming-write-single-pass-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `crates/glm-format/src/container.rs` | `7ff63e753982716067207ecf6ba071995f00753273957af332cfa4bae42d182a` |
| `crates/glm-format/src/nvfp4.rs` | `af9211c7df2c74b446d234ed215580614ce58c415963a1038eb86df48ad8b11a` |
| `crates/glm-format/src/exl3.rs` | `c290960591295a1dd440818f97a7317fb42a061f9bc71c87a31b5bbb4cfd3647` |
| `crates/glm-format/src/stream.rs` | `a93e564bb29e823dbd9ddd922873b87080359f6004d872661ca8d800c1ec632d` |
| `docs/streaming-write-single-pass-proof-v1.md` | `f664c7443a9180479f1148aa18b4992bfadc4b2591736727af74d1e1e9235e3c` |
| `docs/production-punchlist.md` | `ddbcfed4b792e6383a9f3748169db6ddfa67c4a3f8e411e2be191cae05dcea0c` |
| `docs/results-index.md` | `5e9440dc3a7bef99bcb5c0bfa55fc92c7404ed572e8657819d9c9043e810d41b` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-streaming-write-single-pass-v1-handoff.md
```

## Review boundary

This review covers the CPU streaming writer's removal of the immediate
whole-plane staging-file reread for newly written plain, NVFP4, and EXL3
tensors while preserving codec validation, source-length checks,
descriptor-publication ordering, retry behavior, and resume validation.

It does not accept complete checkpoint conversion, conversion throughput, a
device sink, CUDA loading or execution, model quality, capacity, or serving
performance.

## Required adversarial questions

1. Does `copy_exact_at` present every sequential source chunk and its exact
   plane-relative offset to the selected validator before writing that chunk?
2. Do short sources and arithmetic failures stop the copy, and does the
   one-byte trailing-source probe still reject every overlong source before
   pending descriptor insertion?
3. Is the old `validate_planes(index)` call absent from the new-write path
   while remaining mandatory for every adopted completed descriptor during
   staging-file resume?
4. Does the plain validator preserve absolute coordinate and dtype-element
   semantics across arbitrary copy-buffer boundaries?
5. Does the NVFP4 validator still receive the complete value plane before
   the complete scale plane and enforce value padding, canonical scales,
   zero-scale coupling, scale padding, and 2D replica equality at finish?
6. Does the EXL3 validator preserve the exact primary and auxiliary bytes,
   reject skipped or reordered offsets, allocate fallibly, and invoke the
   canonical container-plane decoder before publication eligibility?
7. Are the EXL3 retained bytes bounded to one projection by validated
   descriptor/metadata geometry, and are the routed-expert rank-slab plane
   byte figures in the proof exact?
8. Can any plain, NVFP4, or EXL3 semantic error add the tensor to `pending`,
   publish a descriptor, or mark it complete?
9. After a late scale/aux/finalization failure, are partial payload bytes
   unadvertised and deterministically overwritten by a successful retry at
   the same fixed offsets?
10. Is payload-before-descriptor sync ordering unchanged for both immediate
    and deferred writes?
11. Are the three semantic-error regressions distinguishing, and do they
    prove the exact descriptor stays all zero rather than merely checking
    the in-memory completed count?
12. Do valid byte equality, deferred invisibility, resume, source-length,
    corruption, and atomic rank-set regressions still cover the surrounding
    contract?
13. Are the test counts, implementation hashes, host exclusions, and absence
    of checkpoint/device/model/quality/capacity/performance claims accurate?
14. Is the proof careful not to call EXL3 semantic decoding zero-copy or
    claim a speedup before matched conversion measurement?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- source chunk sequencing, exact lengths, and validation-before-write are
  sound;
- plain, NVFP4, and EXL3 validator state is complete and fail-closed;
- new writes avoid the immediate staging-file semantic reread;
- completed descriptors retain full hash and semantic resume validation;
- semantic failures cannot publish or complete a descriptor;
- retry and durable commit ordering remain crash-safe;
- scratch bounds and GLM-5.2 EXL3 byte arithmetic are accurate;
- the regressions are distinguishing; and
- proof results, exclusions, and lack of a speed claim are accurate.

Only if all nine answers are unqualified `YES`, end with the requested
token. Withhold it for a conditional pass, stale input, skipped or repeated
source region, validator-after-write ordering, short/trailing-source hole,
codec semantic regression, unchecked allocation, unbounded EXL3 retention,
new-write file reread, resume bypass, pending insertion after error,
descriptor publication before validation, unsafe retry, changed durable
ordering, nondistinguishing test, false arithmetic/count/hash, or overstated
checkpoint/device/model/quality/capacity/performance claim.

The token accepts only the CPU streaming writer correction. It does not open
cn4, authorize CUDA work, or accept production serving.
