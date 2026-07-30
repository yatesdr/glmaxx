# Fable handoff: durable journal transaction sequence v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`397c76c8e0b8e04e43c3f4ed19f1ac55ec730018`

Required result path:
`fable-durable-journal-transaction-sequence-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`durable-journal-transaction-sequence-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/store.rs` | `0a2cd6f96bceb3ed352e5ade9fca302ed5f1498e0280de59a4b57286672dff0c` |
| `docs/durable-journal-data-presence-proof-v1.md` | `fc19414d706e317dd59491b2c284b9931c911161fc176e220fe121211c480b26` |
| `docs/durable-store-write-fail-stop-proof-v1.md` | `067c2b1f93a72ca0a5e661be02bad665da658154e3bfdf9ab33744137873dd09` |
| `docs/torn-journal-resume-proof-v1.md` | `2c0a5f131e72b41cf76f06e1127db7c9d492ba17332d9dec643392d301f85180` |
| `docs/journal-tail-corruption-proof-v1.md` | `d8bb0b738caba8afe5a5084ffbc969dacd9d01fa6eae298b9adb289cfa950a7d` |
| `docs/durable-catalog-extent-integrity-proof-v1.md` | `b8f38cf3ab3fde74d505ea7a118d063d3e235dd4049b6ce6e47c071099a2ea7d` |
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `docs/durable-journal-transaction-sequence-proof-v1.md` | `3c3a863c29246da7e2e4666872604aed3e031c2c1c3ab2e40f94b421899079bc` |
| `docs/production-punchlist.md` | `ad42562663ad015af60e804354d59a9d1dfa9f63aded965be7817bc87a2af210` |
| `docs/results-index.md` | `437a67cff6f2fa30ff188fef44323091d2b22cb48acf2a0e95f233ea9c747822` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-durable-journal-transaction-sequence-v1-handoff.md
cargo test --offline -p glm-cache \
  store::tests::missing_complete_transaction_group_fails_closed
cargo test --offline -p glm-cache store::tests
cargo clippy --offline -p glm-cache --all-targets -- -D warnings
```

## Review boundary

This review covers only transaction-sequence continuity in the retained CPU
journal decoder and its next-transaction derivation. It does not accept an
authenticated journal, redundant metadata, general salvage, operator repair,
direct I/O, online publication, segment cleaning, CUDA, checkpoint execution,
model output, or performance.

## Required adversarial questions

1. Does the retained single writer always start at transaction one and append
   every later begin at exactly the preceding transaction plus one?
2. Can a crash leave a group incomplete while reopen still appends only the
   next contiguous transaction, with no later event for the orphan?
3. Did the prior decoder merely take the maximum ID and therefore accept a
   journal whose first remaining transaction was two?
4. Did it also accept transactions one and three after deletion of the
   complete transaction-two group?
5. Does corrected decoding inspect every complete record in physical order
   and reject any changed transaction ID other than checked
   `current_transaction + 1`?
6. Must the first record at every changed transaction ID be `Begin`, including
   the first complete record in the file?
7. Does the same rule reject decreasing IDs, late events for an older
   transaction, and skipped IDs?
8. Does `TierJournal` replay still independently reject duplicate begins,
   missing or duplicate durable pieces, premature/duplicate publication, and
   content/generation conflicts?
9. Is writable `next_transaction` derived from the final validated contiguous
   ID rather than an arbitrary maximum?
10. Does the regression remove exact complete four-record groups without
    changing any retained record or CRC?
11. Does it cover both prefix and interior deletion through both writer and
    read-only snapshot construction, with exact `JournalSequence` results?
12. Would the prior implementation succeed and expose only page two in the
    prefix case and pages one/three in the interior case?
13. Do existing crash-phase tests prove that begin-only, data-synced, and
    piece-attested orphans can be followed by a valid contiguous publication?
14. Do valid short-tail repair, complete-record corruption, zero-history
    rejection, and catalog-integrity behavior remain intact?
15. Does the proof accurately exclude final-group deletion, which remains
    indistinguishable from an allowed crash without an independent durable
    high-water mark?
16. Are the 268-test claim, 62-handoff claim, and every
    GPU/direct-I/O/model/performance exclusion accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- the retained writer establishes the asserted contiguous sequence;
- the decoder enforces that sequence for every complete record;
- both prefix and interior complete-group deletion fail closed;
- legal crash-orphan continuation remains accepted;
- the regression distinguishes the prior silent catalog loss; and
- the CPU proof and all exclusions are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, a legal writer sequence that
the decoder rejects, any skipped/decreasing transaction it accepts, reader
and writer divergence, broken crash recovery, nondistinguishing regression,
or overstated production claim.

The token accepts only this retained CPU transaction-continuity correction.
It does not open cn4, authorize CUDA work, or accept online publication or
model execution.
