# Evidence run allocation v1

Date: 2026-08-03

Status: local tooling proof; no GPU or checkpoint result

## Result

`scripts/new-evidence-run.sh` now allocates a unique evidence run directory
from one UTC clock read. It atomically claims the basename with `mkdir`, uses
`-01` through `-99` collision suffixes, never reuses an existing path, and
writes a `READY` marker only after all start-time and identity receipts exist.
Initialization errors and signals leave `INCOMPLETE` state. The root must be
absolute, existing, non-symlink, and not `/`; the slug is restricted to a
bounded lowercase ASCII alphabet.

Source identities:

- `scripts/new-evidence-run.sh`:
  `ccba2a97d97e86fa2b73b58071be2bdd10f19fd4faeef2d5ab65af27a7b98c0a`
- `scripts/new-evidence-run-selftest.sh`:
  `dc4b1b5dbdf83b9cb77d92a204da4a81d2860fc9f7f5c3d48da4f3c38b7149fa`

The self-test command was run five consecutive times:

```text
bash -n scripts/new-evidence-run.sh scripts/new-evidence-run-selftest.sh
for run in 1 2 3 4 5; do bash scripts/new-evidence-run-selftest.sh; done
```

Each run returned:

```text
evidence-run-selftest=pass allocations=10 concurrent=8 rejection-cases=5
```

The proof covers distinct sequential and eight-way concurrent allocation,
same-second sequence receipts when the clock values coincide, canonical-root
confinement, compact/RFC-3339/epoch receipt consistency, basename identity,
and rejection of an invalid slug, symlink root, regular-file root, relative
root, and filesystem root.

## Integration boundary

The current reviewed cn4 Phase-B scripts intentionally require
`GLMAXX_EVIDENCE_DIR` not to exist and create it themselves. This allocator
creates the directory as the atomic claim, so the old runner cannot consume it
yet. The already-required FC2 Phase-B revision must accept only an allocator
directory whose contract is `glmaxx-evidence-run-v1` and state is `READY`, then
change it to `RUNNING` before collecting any GPU evidence. That change belongs
in the next reviewed runner cut alongside the FC2 helper/probe symbol gates;
the pinned runner is not changed by this result.

This closes the local cause of the earlier future-looking evidence basename,
but does not claim cn4 integration, a kernel launch, or any model result.
