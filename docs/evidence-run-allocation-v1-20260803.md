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

Source identities:

- `scripts/new-evidence-run.sh`:
  `ccba2a97d97e86fa2b73b58071be2bdd10f19fd4faeef2d5ab65af27a7b98c0a`
- `scripts/begin-evidence-run.sh`:
  `43a788ebfde95dfe2fbe4f57352d585d02bb0ddf9800fd07030ed69e97ea23d6`
- `scripts/new-evidence-run-selftest.sh`:
  `be77ed968213ac6e3e84da3ff0a07e98b31295ae910b2f0d46d8a741d1e33e46`

The final self-test command was run ten consecutive times:

```text
bash -n scripts/new-evidence-run.sh scripts/begin-evidence-run.sh \
  scripts/new-evidence-run-selftest.sh
for run in 1 2 3 4 5 6 7 8 9 10; do
  bash scripts/new-evidence-run-selftest.sh
done
```

Each run returned:

```text
evidence-run-selftest=pass allocations=12 concurrent=8 runner-claims=2 rejection-cases=8
```

The proof covers distinct sequential and eight-way concurrent allocation,
same-second sequence receipts when the clock values coincide, canonical-root
confinement, compact/RFC-3339/epoch receipt consistency, basename identity,
single-winner concurrent claiming, replay rejection, receipt-tamper rejection,
exact line framing, and rejection of an invalid slug, symlink root,
regular-file root, relative root, and filesystem root.

## Integration boundary

The current reviewed cn4 Phase-B scripts intentionally require
`GLMAXX_EVIDENCE_DIR` not to exist and create it themselves. This allocator
creates the directory as the atomic claim, so the old runner cannot consume it
yet. The already-required FC2 Phase-B revision must accept only an allocator
directory through `scripts/begin-evidence-run.sh` before collecting any GPU
evidence. That call belongs in the next reviewed runner cut alongside the FC2
helper/probe symbol gates; the pinned runner is not changed by this result.

This closes the local cause of the earlier future-looking evidence basename,
but does not claim cn4 integration, a kernel launch, or any model result.
