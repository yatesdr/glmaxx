# Fable handoff: direct-tier extent CPU proof v1

Date: 2026-07-30

Status: adversarial CPU-proof review requested

Review candidate commit:
`8c27c1e6082f35cc225a8ed76255bd2724c47c6c`

Required result path:
`docs/reviews/fable-direct-tier-extent-cpu-v1.md`

Requested acceptance token, only for an unqualified pass:
`direct-tier-extent-cpu-v1-accepted`

GPU authorization conveyed by this handoff: none

cn4 posture: do not connect to cn4 or launch GPU or NVMe work for this review

## Provenance

Review the exact candidate in a detached worktree. Copy this handoff into the
worktree if necessary, run `review-proof`, and hash every input at review
start and finish. A mismatch is a stale candidate and must withhold the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `Cargo.lock` | `72880aa9ec4a2bf9f42e4df9cb463272194b344590f1e7a839f08aaf2d3970e7` |
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `docs/direct-tier-extent-cpu-proof-v1.md` | `d54ad467e8f2219ec31638416ff5a0a74cf972a6077695b6eea7dd1b8eb859b1` |
| `crates/glm-cache/src/tier.rs` | `0a1541f13462bcdec92284911f96531b06869b60c7fe85fc5e9669c80fabe693` |
| `crates/glm-cache/src/store.rs` | `0a2cd6f96bceb3ed352e5ade9fca302ed5f1498e0280de59a4b57286672dff0c` |
| `crates/glm-cache/src/direct.rs` | `229379a5bf61e7f106187bbaea56549a61c6ac47226584ef763af80a15aaadee` |
| `crates/glm-cache/src/lib.rs` | `3412a9dca3dead256094d8ce1deb9054c30826d96d66324d954b5700242d1a98` |
| `crates/glm-cli/src/main.rs` | `9c8c96269f9af31a561d96559d03b42e0368993c7840a73992753b702b06d81d` |
| `fixtures/direct-tier-extent-proof-v1.json` | `eb5efc3faefc67a932ed4b86e1af29bee89b53cf0483b6a39c373c938b047d6c` |
| `scripts/local-checks.sh` | `b30afaea202f150b1cfe6034543c843649e33548a64984d7976d3a90a1e3bdb9` |
| `docs/production-punchlist.md` | `5be7440eb09e95deb43725d578624adf79c72c7517ad3bef03584a61ef059205` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-direct-tier-extent-cpu-v1-handoff.md
cargo test --offline -p glm-cache direct -- --nocapture
cargo clippy --offline -p glm-cache -p glm-cli --all-targets -- -D warnings
cargo run --release --offline -p glm-cli --bin glmaxx -- \
  direct-tier-proof /tmp/direct-tier-extent-proof-v1-release.json
cargo run --offline -p glm-cli --bin glmaxx -- \
  direct-tier-proof /tmp/direct-tier-extent-proof-v1-debug.json
cmp fixtures/direct-tier-extent-proof-v1.json \
  /tmp/direct-tier-extent-proof-v1-release.json
cmp /tmp/direct-tier-extent-proof-v1-debug.json \
  /tmp/direct-tier-extent-proof-v1-release.json
```

## Review boundary

This review covers only the canonical direct physical-extent codec and the
explicit fail-closed boundary with the retained blocking-store record.
Acceptance opens the separately bounded CPU buffer-generation and shared
restore-ticket/cancellation state-machine work.

It does not accept a durable metadata encoding, journal, catalog, restart
recovery, segment allocator or cleaner, `O_DIRECT`, `io_uring`, registered or
CUDA-pinned buffer, HBM copy, cn4 filesystem/device result, K03/K05,
checkpoint smoke, serving readiness, model quality, or performance claim.

The cleaner specifically remains blocked until the accepted design receives
the required relocation journal/checkpoint amendment.

## Required adversarial questions

1. Do all candidate hashes match at review start and finish in a detached
   worktree, even if `main` advances?
2. Does the implementation derive all three logical lengths from the retained
   `tier` constants rather than creating a second independent arithmetic
   source?
3. Independently re-derive all offsets, ends, logical totals, physical
   totals, and 493/501 block counts. Are they exact?
4. Does `DirectExtentRecord::validate` require the exact ordered piece set,
   offsets, lengths, capability, physical length, aligned file offset,
   nonzero durable identities, and nonzero digests?
5. Does the CPU allocator expose an exact-length 4,096-aligned subslice
   without unsafe code, and does withholding `Clone` avoid retaining an
   alignment displacement from a different allocation?
6. Does encode initialize the whole physical extent, copy only the logical
   ranges, require zero padding, and hash the physical bytes only after all
   materialization is complete?
7. Does decode validate the record, address/offset/length alignment, exact
   physical SHA, every padding interval, and every piece SHA before returning
   any view?
8. Do target-only and MTP round trips compare every logical byte exactly?
9. Does the padding mutation test visit every byte position in every padding
   interval for both capabilities?
10. After independently re-signing the physical SHA, does a nonzero padding
    byte still fail specifically as `Padding`?
11. After independently re-signing the physical SHA, does mutating each of
    target KV, target indexer, and draft sidecar still fail at its per-piece
    digest?
12. Can a whole-extent mismatch and each per-piece mismatch be observed
    independently, with no mutation silently becoming a passing decode?
13. Are metadata lies, address misalignment, file-offset misalignment, and
    wrong transfer length all fail-closed?
14. Does the direct MTP extent contain the accepted single combined
    token-major draft sidecar, rather than two separately hashed draft
    pieces?
15. Does `try_from_blocking_store` validate the legacy record but always
    reject reinterpretation as direct format with `MigrationRequired`?
16. Is that migration refusal sufficient for required CPU case 25's explicit
    boundary without claiming a cross-reader that does not exist?
17. Are debug and release fixture bytes identical and equal to the checked-in
    fixture with the documented SHA-256?
18. Can any proof failure be converted to a passing report, or does the CLI
    return an error before its PASS verdict?
19. Does the proof document faithfully carry forward all accepted design
    review findings, including the CQ arithmetic, ticket-scoped physical
    reservation, double memlock/DONTFORK/teardown rules, tail slack, W0
    starvation choice, added fault cases, and cleaner amendment?
20. Are K03 and K05 still unpassed, and are all absent production components
    stated as nonclaims?

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer each statement separately:

1. the target and MTP extent arithmetic and piece layout are exact;
2. aligned allocation and direct-I/O span validation are sound for this CPU
   scope;
3. zero padding, whole-extent SHA, and per-piece SHA boundaries are
   independently enforced;
4. metadata and payload mutations fail closed before a decoded view is
   returned;
5. the draft sidecar is one combined logical record at the accepted offset;
6. the legacy store has an explicit, lossless-migration-required boundary;
7. the canonical fixture and debug/release reproducibility claims are valid;
   and
8. the claim boundary is accurate and sufficient to open the next CPU
   state-machine slice.

Only if all twenty questions and all eight statements are unqualified `YES`,
end with:

```text
direct-tier-extent-cpu-v1-accepted
```

Withhold the token for stale provenance, duplicated or wrong arithmetic,
incorrect alignment, uninitialized or unchecked padding, missing digest
boundary, incorrect MTP sidecar representation, unsafe early view
publication, implicit legacy reinterpretation, nondeterministic fixture,
pass-on-error behavior, or evidence overstatement.
