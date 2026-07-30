# Fable handoff: page reuse quarantine and in-place commit v1

Date: 2026-07-29

Status: adversarial integrated CPU review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`832bf9784ae67b2db4891bb17dcb8fc2647cf53a`

Required result path:
`fable-page-reuse-quarantine-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`page-reuse-quarantine-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `crates/glm-cache/src/lib.rs` | `bc3f31265e26638afd40307262afa1947d5cc2e88cfea96a18399d9fcee1cf7d` |
| `crates/glm-cache/src/sequence.rs` | `8c0491d4f2d3e50da12e15961c8ac65a2fe5449a3527d40a38cdaa5ef27d644e` |
| `crates/glm-cache/src/delta.rs` | `71ac2da15e869a6f2470c3551a7cd6ec4ff387850a23240e9a44ad96a538ff16` |
| `crates/glm-engine/src/worker.rs` | `39a0c0b917921149869d2afc5d652815986bf776eca5ddc9b7abee41b4892652` |
| `crates/glm-serving/src/lib.rs` | `362312a48e1269f09f2f3f6e090dffcf896a8b6c688b65d6060e6b505aae0bae` |
| `docs/serving-page-transaction-v1.md` | `31983cce95ee01a5968213d5daf12c7a855f75f8735314700f2b4a9e55625d1a` |
| `docs/page-reuse-quarantine-proof-v1.md` | `94b6c39ee57fafa926d6bc375bf2841c00f8586c38fe99700d54e9b86065d84c` |
| `docs/offline-serving-spine.md` | `500628e6da720a760a242034678e402ab7fb0e78bd479c901254e6603cd35c99` |
| `docs/production-punchlist.md` | `9a9f5c37f366f6beda67a68fa0ced3cf89e6fb9fd31b9c894b947108642048bb` |
| `docs/results-index.md` | `7d31ba6c66f6d5362e717a9894ed706ad0ff92fe3062cf3fbfd15bbfa416f07c` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-page-reuse-quarantine-v1-handoff.md
cargo test --offline -p glm-cache sequence::tests
cargo test --offline -p glm-serving
cargo clippy --workspace --all-targets --offline -- -D warnings
scripts/local-checks.sh
```

## Review boundary

This review covers the CPU active-table allocator, in-place tentative commit,
target/draft retirement, generation binding, exact four-rank CPU
acknowledgement ordering in `ServingCoordinator`, fail-stop worker retirement,
and the stated regression/gate counts.

It does not accept fixed-capacity storage, CUDA-visible page tables, upload
events, stream dependencies, device payload zeroization, device-arena
teardown, `CACHE_ONLY`, direct tier I/O, checkpoint execution, model output,
quality, capacity with live payloads, or performance.

## Required adversarial questions

1. Does tentative commit retain every accepted target/draft physical ID in
   place rather than free and deterministically reacquire it?
2. Are only pages beyond the exact committed position retired, including
   cross-page MTP rejection at all tail occupancies?
3. Can any released target or draft ID appear simultaneously in the active
   physical map, free set, or quarantine?
4. Does a shared prefix ID enter quarantine only when its final active
   reference is released?
5. Can an allocator consume a quarantined ID before binding, while binding
   is pending, after a wrong-generation acknowledgement, or after a failed
   rank update?
6. Does binding require one nonzero generation, freeze every table mutator,
   and reject rebinding?
7. Does exact acknowledgement atomically move every owner-rank target/draft
   ID to the corresponding free set and clear the binding?
8. Does serving bind the complete rejected/removal set before transmitting
   the exact successor delta?
9. Does ordinary cleanup acknowledge reuse only after
   `apply_page_delta` has validated all four generation/global/local
   receipts?
10. Can terminal removal, cancellation, accepted EOS, length completion, or
    late rollback publish host allocator reuse before mirror removal?
11. Does a fatal worker generation avoid a forged receipt and leave the host
    quarantine unusable by any subsequent mutation?
12. Are failed mutations atomic across active pages, reference counts,
    prefixes, free sets, quarantine sets, and tentative state?
13. Are the ABA, wrong-generation, in-place identity, rejected suffix,
    shared-prefix, MTP0/MTP6, and 1M regressions distinguishing rather than
    tautological?
14. Are the 286-test, 69-handoff, formatting, Clippy, FFI, deterministic
    proof, and skip claims reproducible?
15. Are clone allocation, missing fixed undo/device receipt/`CACHE_ONLY`,
    and every model/quality/capacity/performance exclusion accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- accepted pages preserve exact target/draft identity;
- rejected and removed pages cannot ABA before exact receipt;
- generation binding and mutation freeze are fail-closed;
- all four rank receipts precede ordinary allocator reuse;
- fatal worker retirement cannot become a false acknowledgement;
- rollback and shared-prefix reference behavior remain atomic;
- all regressions and gate counts are accurate; and
- every device/model/performance exclusion is accurate.

Only if all eight answers are unqualified `YES`, end with the requested
token. Withhold it for a conditional pass, stale input, accepted-page
free/reacquire, early reuse, target/draft asymmetry, missing rank receipt,
wrong generation, mutable bound table, forged fatal cleanup, partial
rollback, nondistinguishing regression, incorrect gate count, or overstated
device/model claim.

The token accepts only this CPU quarantine and receipt-ordering milestone. It
does not open cn4, authorize CUDA work, or accept production serving.
