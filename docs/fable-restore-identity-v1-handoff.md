# Fable handoff: asynchronous restore identity v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`dc16273b019cf3a3dd8eb810cf9caeb26c99bced`

Required result path:
`fable-restore-identity-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`restore-identity-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/residency.rs` | `74e7dd8077d7ce1db082b6b2501debfcf07d39f0c444e5e355bdb5385ac29770` |
| `crates/glm-cache/src/store.rs` | `d37a1400dc0c393b26c121f72694945bef78c28eda29796abf41a2ed713a17ac` |
| `crates/glm-serving/src/cache.rs` | `786c7c7e5ce2f417749a78e8c48aa8a7d0a5cb617e0883e960a8e7c17d781720` |
| `docs/cache-lifecycle-proof-v1.md` | `11ad4936fea7cd0887e660911f50778d5b0918c21a6cebaca1a98a244b2e2de1` |
| `docs/restore-identity-proof-v1.md` | `16c44adf52c8fa0ad40b1656f7774bbea8072673fdddf5763f1ff33a3b4db256` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-restore-identity-v1-handoff.md
cargo test --offline -p glm-cache
cargo test --offline -p glm-serving
cargo clippy --offline -p glm-cache --all-targets -- -D warnings
```

## Review boundary

This review covers only request/result identity in the existing CPU
asynchronous restore and residency simulation. It does not accept the
pending direct-I/O design, real HBM/DRAM/NVMe transfers, io_uring, CUDA,
prefix transaction integration, or performance.

## Required adversarial questions

1. Before the candidate, could a completion with the same page key,
   namespace, and generation but a wrong request ID, wrong ordinal, or
   altered `TierRecord` be adopted?
2. Does `PendingRestoreIdentity { request_id, page_ordinal }` uniquely bind
   each pending page when one serving request restores multiple pages using
   the same request ID?
3. Can results for two pages on the same rank be swapped and still pass?
   Consider identical namespace/generation and distinct ordinals.
4. Does requiring complete `TierRecord` equality cover piece set, offsets,
   lengths, hashes, MTP posture, tier, generation, namespace, and page key?
5. Are zero request IDs, ranks outside four, and rank/ordinal ownership
   mismatches rejected before any residency or pending-identity mutation?
6. On wrong ID, ordinal, or record, does completion leave both residency and
   pending identity intact so correct completion or explicit abort remains
   possible?
7. Is pending identity cleared on every successful completion and abort, and
   absent from registration, HBM, DRAM, and NVMe states?
8. Can submission failure, cancellation, corruption, timeout, worker close,
   or serving rollback strand a pending identity under the pinned caller
   paths?
9. Does adding `ResidencyError::Request` preserve fail-closed caller
   behavior without allowing partial prefix admission?
10. Are the new tests independent and capable of failing on the previous
    implementation, or can they pass without exercising the identity checks?
11. Are the 232-test claim and all GPU/direct-I/O/model non-claims accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- begin-restore validation and mutation ordering are accepted;
- pending identity is sufficient and correctly scoped;
- exact completion-record validation is accepted;
- abort/success/caller cleanup behavior is accepted; and
- the CPU proof and its non-claims are accurate.

Only if all five answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, swappable completion,
partial mutation, stranded identity, incomplete record binding, or a test
that does not distinguish the old behavior.

The token accepts only this CPU correction. It does not open cn4, direct I/O,
checkpoint conversion, or model execution.
