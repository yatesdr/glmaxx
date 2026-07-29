# Fable handoff: active sequence page table v1

Date: 2026-07-29

Status: adversarial review request; token withheld by Sol

GPU authorization conveyed by this handoff: none

Review candidate commit:
`3404e070159be0d6932899111dda90865fdf2083`

## Provenance

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `docs/native-engine-plan.md` | `9662c82bea15c7b336ac3efa41a12a1e627a511a5f8da603ba466d5bcb6ae036` |
| `docs/offline-serving-spine.md` | `54c1450940b6131f3ad715a284eab1c1cc0fbcba36c4b8b9b8685934a53bcb4e` |
| `crates/glm-cache/src/sequence.rs` | `fe42a717a42b53f0c739b87f84303715a2a7b0c79c2efdf4af8691fe02e16b08` |
| `crates/glm-cache/src/page.rs` | `d32d70b46f8e09c31923b6fb574db07ef6a8a7dfc7489392b39785dd563217ed` |
| `crates/glm-cache/src/prefix.rs` | `2334d68914bf01ce1432bd7e4d07500ea9fc374e027deedeeb71972cc514fe68` |
| `crates/glm-cache/src/tier.rs` | `c31b07d7f9054f3d51bc5d24c2c414b6c9a134d88f042502bc0f82e29cad500f` |
| `crates/glm-cache/src/mtp.rs` | `1134213f9786eafab9dcb3dd0410f708e5b9addf083140676a523a586968a4b0` |
| `crates/glm-cache/src/budget.rs` | `14b563afbeea90fb2bc8897db1a73dab33c64f5427dacac83edd56a00e0eb8a7` |
| `crates/glm-cache/src/lib.rs` | `029b5038067c2ff60d9f7fffc346eb642e057a65563e612761adfadceac7e052` |

Hash every input at review start and finish. Review the exact candidate commit
in a worktree if `main` advances. Do not infer GPU authorization, HBM payload
allocation, or device qualification.

## Candidate

The candidate is a CPU metadata oracle for active DCP4 KV:

- independent bounded target and draft rank-local slots;
- deterministic 64-token page ownership;
- exact 1,048,576-token admission with 4,096 pages per rank;
- sealed shared prefixes and private copy-on-write mutable tails;
- an in-place target-prefix upgrade when a later MTP request restores the
  durable draft sidecar;
- MTP0–6 target/draft tentative reservation, commit, and rollback; and
- distinct configured target-only and MTP sequence-capacity reporting.

The full local gate passed at the candidate: 196 unit tests, Clippy with
warnings denied, CUDA FFI type checks, deterministic format/engine/serving
fixtures, and the external pinned tokenizer proof. The one-million-position
page-table test is deliberately a CPU allocation proof.

## Requested adversarial questions

1. Does striped allocation prove both exact one-million-token balance and
   fail-closed per-rank capacity under adversarial multi-sequence occupancy?
2. Can any admission, prefix reuse, fork, append, tentative operation, or
   removal leak or double-free a target or draft slot after a late error?
3. Is a chained prefix key sufficiently bound to logical page ordinal and
   owner, and can a duplicate/colliding key create an alias at another
   ordinal?
4. Is upgrading an existing shared target page with a draft slot correct
   when the incoming durable record has a valid combined draft sidecar?
   Identify the exact payload-transfer acknowledgment still required before
   a device executor may consume that attachment.
5. Do session forks share every legal sealed page, copy every mutable tail,
   preserve reference counts, and remain isolated under subsequent appends?
6. Across every tail occupancy from 0 through 63 and every MTP depth from 0
   through 6, do reserve, partial accept, full accept, rejection, capacity
   failure, and context-limit failure restore or commit the exact physical
   pages and valid-token counts?
7. Can the rollback-then-deterministic-reappend implementation of tentative
   commit ever select different physical IDs or invalidate already-written
   payload bytes?
8. Are target-only and MTP capacity statistics truthful when draft capacity
   is zero, smaller than target capacity, or already partly occupied?
9. Does the public API permit a caller to claim `has_draft = true` without a
   generation- and hash-validated sidecar? If so, define the serving-layer
   proof required before wiring this API to admission.
10. Which clone-on-error and per-token paths are acceptable only in this CPU
    oracle, and which must be replaced before the table can sit on the
    production scheduler hot path?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Withhold
any acceptance token unless every blocker and major is resolved. State
separately whether:

- this candidate is a valid CPU metadata proof;
- serving integration may proceed;
- any issue requires changing the active-page-table API; and
- any issue blocks independent GPU kernel work after its own gates pass.

