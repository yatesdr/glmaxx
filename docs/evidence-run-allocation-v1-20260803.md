# Evidence run allocation v1

Date: 2026-08-03

Status: local tooling proof; no GPU or checkpoint result

## Result

`scripts/new-evidence-run.sh` now allocates a unique evidence run directory
from one UTC clock read. It atomically claims the basename with `mkdir`, uses
`-01` through `-99` collision suffixes, never reuses an existing path, and
writes a `READY` marker only after all start-time and identity receipts exist.
Initialization errors and trappable signals leave `INCOMPLETE` state. The root
must be absolute, existing, non-symlink, and not `/`; the slug is restricted to
a bounded lowercase ASCII alphabet. An untrappable process death may leave an
unfinished directory, but it can never publish `READY` after that death.

`scripts/begin-evidence-run.sh` is the matching single-consumer gate. It
validates every allocator receipt and exact line framing, atomically creates a
runner claim, rejects replays and concurrent losers, records a second
single-read UTC identity, and publishes `RUNNING` only after the claim receipts
are complete. Any trapped post-claim failure leaves `INCOMPLETE`; an
untrappable death retains the claim and therefore remains non-replayable.

`scripts/finish-evidence-run.sh` atomically publishes one `COMPLETE` or
`FAILED` terminal receipt from `RUNNING`; terminal races choose one winner and
cannot rewrite the result. `scripts/cn4-phase-b-evidence.sh` composes all three
state gates around the unchanged reviewed Phase-B prepare or qualification
runner. It gives that runner a fresh, nonexistent `payload/` path inside the
claimed UTC run and records the wrapped runner hash.

Source identities:

- `scripts/new-evidence-run.sh`:
  `ccba2a97d97e86fa2b73b58071be2bdd10f19fd4faeef2d5ab65af27a7b98c0a`
- `scripts/begin-evidence-run.sh`:
  `43a788ebfde95dfe2fbe4f57352d585d02bb0ddf9800fd07030ed69e97ea23d6`
- `scripts/finish-evidence-run.sh`:
  `2c3f29c6a92772a2cf9e98c74190d6bb2c555d0d7fa1334e3e41da346ea4ebe9`
- `scripts/cn4-phase-b-evidence.sh`:
  `28f8bc8c3a6d818d99f474988bcc256babfca2f3dd591802d51da38934346894`
- `scripts/new-evidence-run-selftest.sh`:
  `7b5e26487e4d9311dd38f8cc8f292a5e00c8de226b49011b65781032e32df41a`

The final self-test command was run twenty consecutive times:

```text
bash -n scripts/new-evidence-run.sh scripts/begin-evidence-run.sh \
  scripts/finish-evidence-run.sh scripts/cn4-phase-b-evidence.sh \
  scripts/new-evidence-run-selftest.sh
for run in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
  bash scripts/new-evidence-run-selftest.sh
done
```

Each run returned:

```text
evidence-run-selftest=pass allocations=14 concurrent=8 runner-claims=4 terminal-claims=4 rejection-cases=10
```

The proof covers distinct sequential and eight-way concurrent allocation,
same-second sequence receipts when the clock values coincide, canonical-root
confinement, compact/RFC-3339/epoch receipt consistency, basename identity,
single-winner concurrent claiming, replay rejection, receipt-tamper rejection,
exact line framing, `COMPLETE`/`FAILED` terminal publication, a competing
terminal-state race, invalid terminal-state rejection, and rejection of an
invalid slug, symlink root, regular-file root, relative root, and filesystem
root.

## Integration boundary

The current reviewed cn4 Phase-B scripts intentionally require
`GLMAXX_EVIDENCE_DIR` not to exist and create it themselves. The wrapper now
satisfies that contract with the uncreated `payload/` child while the parent
run directory owns allocation, runner, and terminal receipts. The next cn4
prepare or qualification invocation must use the wrapper, not pass a manually
predicted timestamp to the old runner. The pinned runner is unchanged; its
already-required FC2 helper/probe symbol revision remains a separate reviewed
cut.

This closes the local cause of the earlier future-looking evidence basename,
and provides the cn4 integration path. The wrapper itself has not yet run on
cn4, so this does not claim a new kernel launch or model result.
