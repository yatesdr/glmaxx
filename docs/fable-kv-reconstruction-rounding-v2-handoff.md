# Fable handoff: dynamic MLA KV reconstruction rounding and ABI v2

Date: 2026-07-30

Status: adversarial numerical/semantic-ABI design review requested

Review candidate commit:
`1212fe9bf39f690401df8e49dcaca44708502a20`

Required result path:
`docs/reviews/fable-kv-reconstruction-rounding-v2.md`

Requested acceptance token, only for an unqualified design pass:
`kv-reconstruction-rounding-v2-design-accepted`

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch GPU, container, storage-device,
or network work for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if needed. Hash every input at review start and finish. Any mismatch
must withhold the token as a stale candidate.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/kv-reconstruction-rounding-v2.md` | `1bf0e69b0920e0ed2e13e92a9a51648f9587ea9c756930d97ed7963542f0a2cd` |
| `crates/glm-cache/src/kv.rs` | `fe5f4b8e07c8a32c6534f6217d62057f3ddd7c4b1abfcc00489c550a39660721` |
| `crates/glm-format/src/float.rs` | `e2f547b3ec5efae0d9fdb975136164f557e24a93770a5791c4ca7d7359e7e1de` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `crates/glm-reference/src/manifest.rs` | `dc8076f90632ac556cf718053c82231ad2bd95d4871fc3ba23444a9574975403` |
| `docs/kv-finite-reconstruction-proof-v1.md` | `7c76c8b4690ab08e13a3814e63ba6fe4a2c23253ea29dc7ce7c8b3bfde716af2` |
| `docs/production-punchlist.md` | `5c7fa7630db16517d7360650ad3f2ebdd9ee8262fa7e7f54dd8ac9dd401e90f8` |
| `scripts/local-checks.sh` | `56f728cdf3f047f9633509a57341d25a977efa802f0d5b371c9716830517db59` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-kv-reconstruction-rounding-v2-handoff.md
git diff --check 1212fe9bf39f690401df8e49dcaca44708502a20^ \
  1212fe9bf39f690401df8e49dcaca44708502a20
cargo test --offline -p glm-cache kv::tests
cargo test --offline -p glm-format float::tests
```

The handoff itself is coordination metadata added after the candidate and is
not a candidate input.

## Review purpose

The earlier finite-reconstruction implementation correctly rejects
non-finite products, but it also changed the NoPE multiplication association
without changing the advertised semantic ABI. This review is not being
asked to waive that drift.

The candidate selects the current group-scale-first arithmetic as the future
oracle, requires an explicit `dynamic-token-v2` identity and cold prefix
namespace boundary, freezes a one-ULP distinguishing vector, and requires a
separate CPU implementation/proof gate after acceptance.

The reviewer must decide whether this is a complete and internally
consistent correction. Do not accept current code, specs, manifests, or
cache namespaces as v2-complete.

## Review boundary

Acceptance covers only:

- the exact RN32 association selected for NoPE, RoPE, and indexer decode;
- encoder/decoder intermediate-scale consistency;
- the v1-to-v2 semantic identity decision;
- unchanged physical layout/capacity;
- cold rejection of v1 namespace content; and
- the proposed CPU proof/mutation plan.

Acceptance does not accept:

- an implementation or spec amendment;
- v1 cache migration, relabelling, or deletion;
- CUDA math or SM120 evidence;
- KV transfer/storage implementation;
- attention/model correctness, logits, KLD, downstream quality, retrieval,
  1M execution, capacity, or performance;
- K01, K02, K04, Q01, or Q04 beyond existing evidence; or
- cn4 access.

## Required adversarial questions

1. Do all ten candidate-input hashes match at review start and finish in a
   detached worktree?
2. Independently compute the fixed vector. Are `g=0x39400002`,
   `x_v2=0x39900002`, and `x_v1=0x39900001` exact?
3. Can you find at least one additional finite record whose association
   changes the output bits?
4. Does the selected v2 decoder reconstruct through exactly the rounded
   group scale used by the encoder's E2M1 normalization?
5. Are both binary32 products required to round separately with
   reassociation, contraction, extended precision, fast math, and FTZ
   forbidden?
6. Are the RoPE and indexer expressions unaffected except for making their
   existing single multiply explicit?
7. Do engine section 19 and format section 24 require a new semantic ABI
   even when every physical byte and offset is unchanged?
8. Does `dynamic-token-v2` propagate to target/draft ABI hashes, prefix
   namespaces, operation manifest, combined sidecar binding, and capability
   identity?
9. Is any identity-bearing consumer or persistent record type missing from
   the candidate's propagation list?
10. Can a v2 runtime reject v1 manifest and cache identities without
    implementing a production v1 decoder?
11. Does the optional offline migration remain optional, preserve old bytes,
    publish a new namespace, and avoid equivalence claims?
12. Are layout, padding, code tables, geometry, and exact 1M byte arithmetic
    unchanged?
13. Does the proposed exact-vector test kill the left-associated mutation
    rather than merely compare within tolerance?
14. Do the remaining exhaustive/adversarial and stale-identity mutations
    close every realistic CPU semantic-drift path?
15. Does selecting encoder consistency avoid making an unsupported
    model-quality claim?
16. Is a second implementation review correctly required before CUDA or
    model evidence?
17. Are all exclusions accurate?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately:

1. the v2 rounding definition is exact and unambiguous;
2. encoder and decoder use one bit-identical rounded group scale;
3. a semantic ABI-v2 and cold namespace boundary are mandatory and complete;
4. physical layout and capacity do not change;
5. the CPU proof/mutation plan distinguishes the prior drift; and
6. the scope and quality/GPU exclusions are accurate.

Only if all seventeen questions and all six statements are unqualified
`YES`, end with:

```text
kv-reconstruction-rounding-v2-design-accepted
```

Withhold for stale provenance, incorrect fixed bits, ambiguous arithmetic,
missing identity propagation, silent v1 reuse, physical-layout drift,
nondistinguishing proof, or any implementation/quality/GPU overstatement.
