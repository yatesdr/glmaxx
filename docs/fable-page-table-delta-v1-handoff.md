# Fable handoff: canonical page-table delta v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`a1d4cb48331b229a683ffa90ba41a609d74ad261`

Required result path:
`fable-page-table-delta-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`page-table-delta-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/delta.rs` | `71ac2da15e869a6f2470c3551a7cd6ec4ff387850a23240e9a44ad96a538ff16` |
| `crates/glm-cache/src/sequence.rs` | `d48a93cbbbef67eaf2b1550cb1d20d6132bf10d0cf00c5e93d5b66d351981034` |
| `crates/glm-cache/src/lib.rs` | `a892febc0c979cfad3cc629aed005156639fa5aa1c27709207d9553d50575abc` |
| `docs/page-table-delta-proof-v1.md` | `e4174447b4c08c3252eec30bf5550bc9e0fe8936dc5bf6e1cd5bf89cee576f63` |
| `docs/serving-page-transaction-v1.md` | `04b5a1142dfc10aec0e2cde4178606ef173dbfb40f8358eb577f0dd6f0059b18` |
| `docs/offline-serving-spine.md` | `78e70d69a49c2d292bf1fc7ef48febec7c92f72546239327dab9b14151848318` |
| `docs/serving-active-page-transaction-proof-v1.md` | `073706cfe3c77afc42863cff9d3598ed74ef64e9ce1ea18d4dbeec4e5c147871` |
| `docs/production-punchlist.md` | `f62d24926e108d2693d1201d1fada02f6b1717f9d51038e7ff5bbacb4c702a85` |
| `docs/results-index.md` | `02d64a35b2e82fdf704fcdcf6483836888421fc7f63dd83d95fa935d835a3184` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-page-table-delta-v1-handoff.md
cargo test --offline -p glm-cache delta::tests
cargo test --offline -p glm-cache
cargo clippy --workspace --all-targets --offline -- -D warnings
scripts/local-checks.sh
```

## Review boundary

This review covers page-granular committed mutation, the canonical CPU delta,
global/rank-local digest construction, arena/owner validation, and atomic CPU
mirror reconstruction. It does not accept serving or worker delivery,
`StepInput` binding, fixed-allocation undo, rank/device acknowledgment,
physical-ID quarantine, CUDA-visible payloads, checkpoint execution, model
output, quality, live-tier capacity, or performance.

## Required adversarial questions

1. Does bulk committed append fill one mutable tail, allocate complete
   64-token pages, allocate at most one final partial page, and publish the
   checked committed count only after page mutation succeeds?
2. Does its clone-on-error wrapper restore the exact table on a late owner
   capacity or invariant failure?
3. Does the 1..257 regression compare one bulk append with repeated
   single-token appends across every page and DCP4 boundary?
4. Does the 64-tail by 7-depth regression exhaust all 448 tentative shapes
   and prove exact valid-position sums, page counts, per-page bounds, draft
   presence, and bit-identical rollback?
5. Does `PageTableDelta::between` require equal arena configurations and
   exact nonzero successor generations?
6. Are updates and removals sorted, bounded to 64 each, unique, disjoint, and
   prohibited from forming a no-op generation?
7. Does every update bind request ID, MTP posture, committed/tentative counts,
   final page count, first changed ordinal, and a complete changed suffix?
8. Is the common prefix based only on rank-visible fields, correctly omitting
   host reference/prefix bookkeeping?
9. Do changed pages require consecutive ordinals, deterministic DCP4 owner,
   nonzero valid count at most 64, exact draft posture, and an active HBM
   state?
10. Does the global digest domain-separate and hash generations, counts, all
    update invariants, removals, and every changed page in an unambiguous
    canonical order?
11. Does each rank-local digest bind its rank, global digest, all invariant
    fields, and exactly its owner-local changed pages?
12. Does `PageTableMirror` know target/draft arena bounds and apply to a clone
    before adoption?
13. After suffix reconstruction, does the mirror validate complete ordinal
    continuity, owner, committed plus tentative positions, page count/state,
    target/draft posture and bounds, duplicates, shared-physical consistency,
    and target/draft collisions?
14. Does stale generation, digest tampering, wrong owner, re-signed
    out-of-arena ID, malformed/no-op shape, or late collision leave the mirror
    bit-identical?
15. Does the compound regression reconstruct a tentative cross-page MTP
    update, removal, and new admission exactly, with four rank-local digests?
16. Does the suffix regression omit two unchanged sealed pages when a third
    page is appended?
17. Are the 278-test, 66-handoff, formatting, Clippy, FFI, and deterministic
    proof claims reproducible?
18. Are owned allocation, clone rollback, missing serving/worker input,
    missing acknowledgment/quarantine, CUDA, model, quality, tier, and
    performance exclusions accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- page-granular append is equivalent to the retained single-token path;
- every tentative tail/depth shape has exact reversible metadata;
- the global delta is canonical and complete for rank-visible state;
- rank-local digests are correctly bound to the global delta;
- the independent bounded mirror applies atomically and rejects stale or
  malformed input;
- the compound and suffix regressions distinguish missing/corrupt behavior;
- all gate counts are accurate; and
- every serving/device/model/performance exclusion is accurate.

Only if all eight answers are unqualified `YES`, end with the requested
token. Withhold it for a conditional pass, stale input, token/page
nonequivalence, an untested tail/depth, ambiguous hash, omitted rank-visible
field, wrong-owner acceptance, out-of-arena acceptance, partial mirror
mutation, collision acceptance, nondistinguishing regression, incorrect gate
count, or overstated integration/device/model claim.

The token accepts only this CPU page-granular mutation and delta/mirror
boundary. It does not open cn4, authorize CUDA work, or accept production
serving.
