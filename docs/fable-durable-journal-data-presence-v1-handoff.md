# Fable handoff: durable journal/data presence v1

Date: 2026-07-29

Status: adversarial CPU implementation review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`f72437917bec6df3ab8382575f5521ce491d356d`

Required result path:
`fable-durable-journal-data-presence-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`durable-journal-data-presence-v1-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `crates/glm-cache/src/store.rs` | `7408e65a42e4e15598a761587dec31b63736c316f4f49a0d42c47cfd44884dff` |
| `docs/durable-store-write-fail-stop-proof-v1.md` | `067c2b1f93a72ca0a5e661be02bad665da658154e3bfdf9ab33744137873dd09` |
| `docs/torn-journal-resume-proof-v1.md` | `2c0a5f131e72b41cf76f06e1127db7c9d492ba17332d9dec643392d301f85180` |
| `docs/journal-tail-corruption-proof-v1.md` | `d8bb0b738caba8afe5a5084ffbc969dacd9d01fa6eae298b9adb289cfa950a7d` |
| `docs/durable-catalog-extent-integrity-proof-v1.md` | `b8f38cf3ab3fde74d505ea7a118d063d3e235dd4049b6ce6e47c071099a2ea7d` |
| `docs/direct-tier-io-v1.md` | `7e68d89481f38669b7e4d211e9e40d2f75a1ff82eebddfec96fa618d6ca08ae2` |
| `docs/durable-journal-data-presence-proof-v1.md` | `fc19414d706e317dd59491b2c284b9931c911161fc176e220fe121211c480b26` |
| `docs/production-punchlist.md` | `471030142d4bc80113d81ca18b875a5225725faa0f55129f19d88bb4609b5e4e` |
| `docs/results-index.md` | `5c8ea74bbba0b066b47c8053fbeb3941945ef6cceefcf59d35c46265e7d4e963` |
| `scripts/local-checks.sh` | `839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-durable-journal-data-presence-v1-handoff.md
cargo test --offline -p glm-cache \
  store::tests::nonempty_data_without_a_complete_journal_fails_closed
cargo test --offline -p glm-cache store::tests
cargo clippy --offline -p glm-cache --all-targets -- -D warnings
```

## Review boundary

This review covers only retained CPU startup rejection when physical page
data exists without any complete journal record. It does not accept
redundant metadata, general salvage, operator repair, direct I/O, online
publication, segment cleaning, CUDA, checkpoint execution, model output, or
performance.

## Required adversarial questions

1. Does `publish_prevalidated` append and synchronize a complete `Begin`
   record before its first seek or write against `pages.dat`?
2. Does any append or synchronization error return before payload writes, so
   a legitimate first torn `Begin` cannot accompany nonempty data?
3. After a successful begin synchronization, must at least one complete
   journal record remain under the retained crash-order contract even if
   later data, piece, or publish operations fail?
4. Could deletion, replacement, or truncation of the complete journal
   previously reopen nonempty page data as a successful empty cache?
5. Does the correction use
   `journal_len / JOURNAL_RECORD_BYTES * JOURNAL_RECORD_BYTES` rather than
   raw nonzero journal length, so a torn-only journal is treated as zero
   complete records?
6. Does the shared invariant return exactly `UnjournaledData` for nonzero
   physical data and zero complete journal bytes?
7. Do both the exclusive writer and read-only snapshot apply the invariant
   before replaying or exposing a catalog?
8. Does the regression cover both an empty journal and a 113-byte torn-only
   journal through both constructors?
9. Would the prior implementation return successful empty catalogs for all
   four distinguishing assertions?
10. Does a valid complete journal followed by a 113-byte tail remain
    readable, writer-repairable, and recoverable after a later publication?
11. Does a complete corrupt record still follow strict checksum/schema
    rejection rather than being classified as `UnjournaledData` or ignored?
12. Does the correction deliberately allow nonempty physical data after at
    least one valid record because append-only crash orphans can be
    legitimate?
13. Are the 267-test claim, 61-handoff claim, and every
    GPU/direct-I/O/model/performance exclusion accurate?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- retained write ordering makes zero complete records plus nonempty data
  unreachable without journal loss or corruption;
- both writer and snapshot reader fail closed on that state;
- the complete-record versus raw-tail boundary is exact;
- adjacent torn-tail and complete-corrupt-record behavior remains intact;
- the regression distinguishes all four prior successful-open paths; and
- the CPU proof and all exclusions are accurate.

Only if all six answers are unqualified `YES`, end with the requested token.
Withhold it for a conditional pass, stale input, any pre-`Begin` payload
write, a successful open with unjournaled data, raw-length misclassification,
reader/writer divergence, adjacent-boundary regression, nondistinguishing
test, or overstated production claim.

The token accepts only this retained CPU startup-integrity correction. It
does not open cn4, authorize CUDA work, or accept online publication or model
execution.
