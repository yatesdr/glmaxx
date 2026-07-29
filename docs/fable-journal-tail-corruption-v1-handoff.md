# Fable handoff: complete journal-tail corruption v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`8612ec3a29421f707f0f231e3496a59bb81504b0`

Required result path:
`fable-journal-tail-corruption-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`journal-tail-corruption-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/store.rs` | `30a281fbd79bccd58ebecfdb906029985ace6df50907b2b84476f044586b8fc0` |
| `crates/glm-cache/src/tier.rs` | `0a1541f13462bcdec92284911f96531b06869b60c7fe85fc5e9669c80fabe693` |
| `docs/durable-content-dedup-proof-v1.md` | `75fd16886ef50e4509fb0a7b0701417a1469a0dad78809b39ce08e3e736a7514` |
| `docs/durable-store-write-fail-stop-proof-v1.md` | `067c2b1f93a72ca0a5e661be02bad665da658154e3bfdf9ab33744137873dd09` |
| `docs/journal-tail-corruption-proof-v1.md` | `d8bb0b738caba8afe5a5084ffbc969dacd9d01fa6eae298b9adb289cfa950a7d` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-journal-tail-corruption-v1-handoff.md
cargo test --offline -p glm-cache store::tests
cargo clippy --offline -p glm-cache --all-targets -- -D warnings
```

## Review boundary

This review covers only retained CPU journal framing at the final complete
record versus an incomplete trailing fragment. It does not accept journal
repair, compaction, redundant metadata, direct I/O, online publication,
CUDA, checkpoint execution, model output, or performance.

## Required adversarial questions

1. Did the prior `decode_journal` suppress every decoding error in the final
   complete 512-byte record?
2. Did that include CRC, magic, version, event, tier, piece-table, and record
   validation failures rather than only an incomplete write?
3. Could corruption of the final publish record therefore reopen
   successfully while silently dropping a previously published page?
4. Does corrected decoding require every complete record, including the last,
   to pass `decode_journal_event`?
5. Is only the sub-512-byte remainder still ignored as a crash tail?
6. Does one regression corrupt the real final publish record and prove both
   writer and read-only snapshot open return `JournalChecksum`?
7. Does that regression separately append a complete garbage record and
   prove it is also rejected?
8. Does the unchanged 113-byte-tail regression still reopen and restore the
   page, distinguishing the exact boundary?
9. Does the correction avoid automatic truncation, salvage, or mutation?
10. Are the retained CPU boundary, 254-test, 50-handoff, and all
    GPU/direct-I/O/model/performance exclusions stated accurately?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- no complete corrupt journal record can be silently ignored;
- final-publish corruption cannot become silent catalog loss;
- a genuinely short crash tail remains recoverable;
- writer and snapshot-reader construction use the same strict decoder;
- the regression distinguishes the previous last-record exception; and
- the CPU proof and all exclusions are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, any ignored full record,
silent page loss, short-tail regression, automatic unreviewed repair,
nondistinguishing test, or overstated production claim.

The token accepts only this retained CPU replay-boundary correction. It does
not open cn4, authorize CUDA work, or accept online publication or model
execution.
