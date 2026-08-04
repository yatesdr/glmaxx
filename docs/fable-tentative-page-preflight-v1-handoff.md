# Fable handoff: tentative page transaction preflight v1

Date: 2026-08-04

Status: adversarial CPU implementation and matched-evidence review requested

Review candidate commit:
`c6ebdc97cf84a0349a689a1f816f82ada5e95203`

Required result path:
`docs/reviews/fable-tentative-page-preflight-v1.md`

Requested acceptance token, only for an unqualified pass:
`tentative-page-preflight-v1-accepted`

GPU authorization conveyed by this handoff: none

cn4 posture: read-only hash verification of the two named GLMAXX evidence
directories is permitted if access is available; do not launch CUDA, start or
stop a process, or modify any cn4 path

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at start and finish, and withhold the token for any drift.

| Input at candidate commit | SHA-256 |
|---|---|
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |
| `crates/glm-cache/src/lib.rs` | `331c724306021969f7f9174589b680ca93077017a3248c678a693358187ba4f4` |
| `crates/glm-cache/src/sequence.rs` | `9628b09f70aa3d9815082e480481e276c2a349ad0380f05355d3acc2b09ac99c` |
| `crates/glm-cache/src/delta.rs` | `71ac2da15e869a6f2470c3551a7cd6ec4ff387850a23240e9a44ad96a538ff16` |
| `crates/glm-serving/src/lib.rs` | `d5fe73c061e282a2e777d3b6faf0d2a25c00706e641ac2448f8405357afefff8` |
| `crates/glm-cli/src/page_profile.rs` | `78cb89da22ad22de8fbdf4a6f7836de27b44448aa64a8f0dc3e465f5be067aae` |
| `scripts/cn4-page-transaction-profile.sh` | `3e4eaa31c78831d70587f6be2853188549c431e28d8444ad02ff2326a60de78d` |
| `docs/tentative-page-preflight-v1.md` | `a2fee2787dc70b4076c36890399eaa9a44736bf36d1511d977314fcbec84e116` |
| `docs/cn4-page-transaction-preflight-d225904-20260804.md` | `d79304159e36ef77131b01ec20b6b7c8d57287a3dc986343b4b2732507b5382a` |
| `fable-page-reuse-quarantine-v1.md` | `d6871fb402dfca6d6a5470bae9682da2586054029e9a1814118f2339c7e81f4c` |
| `fable-serving-active-page-transaction-v1.md` | `8ce050d1a3a5531592063ea959a0f787dc026a9899685307b7dbed657bc79762` |
| `scripts/local-checks.sh` | `1675ca5bac9bda032ab6db206629bc0052ad6afc5cb6f59a7b6697e4e5c779d0` |

The matched cn4 evidence directories are:

```text
/home/derek/glmaxx/evidence/20260804T152243Z-page-transaction-profile-f03fc2c
/home/derek/glmaxx/evidence/20260804T153557Z-page-transaction-profile-d225904
```

Their sealed manifest SHA-256 values are respectively
`58519ad3a1185a50e8b23386caa9f8b94365a5ddf2b501c120af8cf1383aceba`
and
`4c1a985dbd2dee8c44bdc107b1c6cdb7f5cfa8c50a1df80afd5f3a3cdfeecc48`.

## Commands

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-tentative-page-preflight-v1-handoff.md
cargo test --offline -p glm-cache sequence::tests
cargo test --offline -p glm-serving
cargo test --offline -p glm-cli page_profile
cargo clippy --offline --workspace --all-targets -- -D warnings
scripts/local-checks.sh
```

For a semantics differential, check out parent `f03fc2c` and candidate code
`d225904` in separate detached worktrees. Reproduce every tail occupancy,
depth, accepted count, target/MTP posture, capacity failure, corrupt state,
and quarantine collision; compare snapshots, page deltas, global/local hashes,
errors, and quarantine receipts exactly. Do not compare only aggregate stats.

## Required adversarial questions

1. Do every candidate hash and both predecessor acceptance tokens verify at
   review start and finish?
2. Does reservation preflight cover the exact tail, state, reference, context,
   target capacity, draft capacity, owner, physical-ID, transition, and
   arithmetic conditions before the first mutation?
3. Given exclusive `&mut self`, can any caller-controlled input or concurrent
   action invalidate the plan before apply, or reach an assertion after a
   partial mutation?
4. Are one through seven tentative positions guaranteed to allocate at most
   one new 64-token page, including every old-tail occupancy?
5. Does commit preflight validate every retained entry and every retired
   target/draft ID before removing the tentative marker or changing a page?
6. Are accepted IDs retained in place and are rejected target/draft IDs moved
   only to owner-rank quarantine, never directly to free state?
7. Do capacity, corrupt-state, collision, overflow, invalid-depth, stale
   transaction, and bound-quarantine failures leave the complete table exactly
   unchanged and retryable where the predecessor was retryable?
8. Across every test posture, are snapshots, physical IDs, states, valid
   tokens, target/draft membership, reservation and commit deltas, global and
   rank-local digests, and error variants identical to parent behavior?
9. Do rollback, removal, shared-prefix, generation binding, four-rank
   acknowledgement, fatal-worker, and ABA behavior remain unchanged?
10. Are the two new mutation tests distinguishing, and do the exhaustive
    tail/depth, rejected-suffix, target/draft, capacity, 1M, and serving tests
    cover every claim rather than only the fast success case?
11. Does the profiler time exactly the coordinator transaction phases without
    charging its additional proof-only `verify()` calls, and retain every raw
    sample?
12. Do both sealed cn4 artifacts verify, use the same matrix/container/CPU
    affinity with only the child source change, and support all stated 1.55x
    through 21.29x speedups?
13. Are the 524,288 and 1,048,576 values explicitly metadata positions rather
    than physical KV capacity claims?
14. Are full delta scans still measured and honestly left to the separately
    gated fixed-page-transaction r2 work?
15. Are all CUDA, model, checkpoint, physical-capacity, quality, latency, and
    serving-throughput nonclaims accurate?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then state
separately whether:

- reservation and commit preflights are complete before mutation;
- exclusive application cannot drift or partially fail on caller input;
- observable success and error semantics match the parent exactly;
- accepted identities and rejected-page quarantine remain exact;
- target/draft, rollback, generation, acknowledgement, and ABA invariants hold;
- mutation and full-suite proofs are sufficient and reproducible;
- matched evidence and every speedup claim are valid; and
- the claim boundary is complete.

Only if all fifteen questions and all eight statements are unqualified `YES`,
end with:

```text
tentative-page-preflight-v1-accepted
```

Withhold the token for stale provenance, an assertion reachable by public
input, mutation before complete preflight, partial failure, target/draft
asymmetry, accepted-ID movement, early free reuse, delta/hash drift,
nondistinguishing tests, unmatched evidence, or any overstated claim.
