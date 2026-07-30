# Fable handoff: serving active-page transaction v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`326158a25f6ca0c68e1b543195984c5537542df4`

Required result path:
`fable-serving-active-page-transaction-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`serving-active-page-transaction-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/sequence.rs` | `e5902ffe36366916b728c54cd78f62331daf63136190d72cbc81d107e5150c36` |
| `crates/glm-cache/src/lib.rs` | `0d9d1fcdbb9c8350b1702d1c41263c24818861936d3ff37f4f4f73125cb6e269` |
| `crates/glm-serving/src/cache.rs` | `099bffde185307365f5932c84f14b15c1ccc4b4cfe29f00612265f69a46a9839` |
| `crates/glm-serving/src/lib.rs` | `d63508beaee3fdc5baed8d47f3435460c4f3143298c406d6e084babd02bf3da7` |
| `crates/glm-serving/src/backend.rs` | `a1dca883453d03e0e69a7896370f9d0b95cc1e7271443b6b91686a8d0d6e44e9` |
| `crates/glm-scheduler/src/lib.rs` | `5fd0c4506002c4da5679f1ca3bf96a880ca7b0b348d5f55ada26a2e06ae7ff4d` |
| `crates/glm-scheduler/src/compile.rs` | `220cf549c0b5882d109ebce4ebd646e9b28ebbab80a83fa579ef5a2c591a070a` |
| `crates/glm-cli/src/main.rs` | `2af7739f311520b60601b18b2d14d3617320df535de24ecd310596add7ac3ff4` |
| `fixtures/cpu-serving-proof-v1.json` | `c95e1049bc52f8a8aaacd5a2d704008df9e8cfe72c8f3486982568adbaa7b47e` |
| `docs/serving-active-page-transaction-proof-v1.md` | `073706cfe3c77afc42863cff9d3598ed74ef64e9ce1ea18d4dbeec4e5c147871` |
| `docs/serving-page-transaction-v1.md` | `266b4ca53a92be9a0ba77d367bac7f4da9d8500fd9437a0738c3e612e94e0b4b` |
| `docs/offline-serving-spine.md` | `008f7e72507d67a11269fb6c450bbde369ba4394cece1975774adefa5776175a` |
| `docs/active-prefix-record-binding-proof-v1.md` | `9bb87c359d78c340d740ef9723ac78ef23510af5fabf4b29b1630211499b4c12` |
| `docs/production-punchlist.md` | `a2374599452a4254357972671c54cbcbb95b8215bd1c8b4264d89672ee8d91dc` |
| `docs/results-index.md` | `079faf15bac2d1bf091a7e097f35daec2a63b2564526152fcef39aec528469cd` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-serving-active-page-transaction-v1-handoff.md
cargo test --offline -p glm-scheduler
cargo test --offline -p glm-serving
cargo clippy --workspace --all-targets --offline -- -D warnings
scripts/local-checks.sh
```

## Review boundary

This review covers the CPU coordinator's mandatory active table, admission,
step reservation/commit/failure transaction, MTP tail selection,
cancellation/terminal cleanup ordering, and exact 1M metadata boundary. It
does not accept a fixed-capacity hot path, rank page-table delta, device
acknowledgment, physical-ID quarantine, CUDA-visible payload, DRAM/NVMe
transfer, checkpoint execution, model output, quality, capacity under real
tiers, or performance.

## Required adversarial questions

1. Did the prior coordinator own only a generation counter, allowing
   scheduler progress and worker submission without an active capacity
   reservation?
2. Is `PageTableConfig` now mandatory and is one `SequencePageTable` owned by
   the coordinator?
3. Does admission reject `prompt + maximum_new > 1,048,576` before mutating
   scheduler or page state?
4. Does real token admission attach only the exact authoritative restored
   records, while the cached-position bypass is compiled only for tests?
5. Is active admission clone-based and atomic across prefix attachment,
   private cached positions, scheduler admission, generation, and event
   publication?
6. Before any rank submission, does every selected row require exact equality
   between active committed positions and scheduler prompt/generated
   progress?
7. Are prefill, decode, and verify reservations respectively the exact prompt
   count, one, and `K + 1`, on one all-row candidate table?
8. On a late page-capacity error, is the candidate discarded, every selected
   scheduler row failed and cleaned, and the rank call count unchanged?
9. Is the effective MTP depth the deepest captured verifier not exceeding
   both configured depth and `remaining_new - 1`, with MTP0 fallback?
10. Does admission require both a full configured-depth verifier and an MTP0
    graph so a partially accepting request cannot become stranded?
11. After four-rank consensus, does prefill require empty output and do
    decode/verify commit exactly the consensus output count on the candidate?
12. Are successful events and all releases preflighted before scheduler
    completion, after which active-table adoption occurs before prefix unpin?
13. On compile, worker, consensus, output, or capacity failure, is an
    uncommitted candidate unreachable and are selected active mappings removed
    from the last committed table?
14. Are cancellations applied at a collective-safe boundary and their active
    mappings/prefix pins released before a continuously runnable peer is
    selected?
15. Does the host generation advance once per published admission, successful
    step, or terminal mutation, with the proof explicitly withholding the
    future reserve/commit rank-generation claim?
16. Does the one-page capacity regression distinguish the old source by
    proving that the fifth TP4 step never reaches its workers?
17. Does the exact-boundary regression account 1,048,575 MTP0 positions as
    4,096 target pages per rank, execute the final token on four workers, and
    release all pages?
18. Does the same regression account an MTP6-capable sequence as 4,096 target
    plus 4,096 draft pages per rank, use MTP0 for its one-token tail, and
    release both arenas?
19. Is 1,048,577 rejected without leaving an active sequence?
20. Is the deterministic serving fixture's reduction from 13 to 11 steps
    explained by safe common MTP0 tail batching without changing its eleven
    emitted tokens?
21. Are the 273-test, 65-handoff, formatting, Clippy, FFI, and deterministic
    proof claims reproducible?
22. Are clone-on-step/per-token mutation, fixed undo, rank delta/digest,
    upload acknowledgment, ID quarantine, CUDA payload, live tiers, real
    model 1M execution, checkpoint, quality, and performance all accurately
    excluded?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- the prior missing active-capacity boundary is real;
- admission and every selected step are atomic in the retained CPU scope;
- no capacity-failed or malformed step can reach or partially publish rank
  work;
- MTP tail selection cannot reserve past the request/context budget;
- cancellation and terminal cleanup remove active mappings before prefix
  release or peer selection;
- the MTP0 and MTP6-capable exact-1M regressions account and release every
  page;
- the regressions distinguish the old missing integration; and
- all gate counts and device/model/performance exclusions are accurate.

Only if all eight answers are unqualified `YES`, end with the requested
token. Withhold it for a conditional pass, stale input, forged cached
progress, scheduler/page divergence, partial admission, any worker launch
before all-row reservation, partial table publication, request/context
overshoot, stranded MTP tail, active mapping surviving terminal release,
cleanup starvation, generation overclaim, nondistinguishing regression,
incorrect gate count, or overstated device/model claim.

The token accepts only this retained CPU serving active-page transaction. It
does not open cn4, authorize CUDA work, or accept production serving.
