# Fable handoff: current-tree review acceptance gate v3-r2

Date: 2026-08-03

Status: adversarial corrective-design re-review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4 or run CUDA for this review.

Review candidate commit:
`1efdf101f42439587b38c147f8a9ff5ee8b59ba5`

Required result path:
`fable-current-tree-review-acceptance-v3-r2.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`current-tree-review-acceptance-v3-r2-design-accepted`

## Provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Withhold the token for any mismatch or incomplete input.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/current-tree-review-acceptance-v3-r2.md` | `e0ca30b9038d5f9bc44d90f631e6bb660facb48a749b6a6787810fc6a3826db0` |
| `docs/current-tree-review-acceptance-v3.md` | `4e81c38120044c609679498b665bd77061efd4f5c9b3707369e69a487d36ff40` |
| `docs/fable-current-tree-review-acceptance-v3-handoff.md` | `ef1c4c9b522bc9864b8641a672d92304b0398915d89a7a0621ba0d9f108a8711` |
| `docs/review-provenance-verifier-v2.md` | `92ca18f910a6945069e73f7d65b979bb9834132b681932bc0055d05071056309` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

Run the complete CPU-only local gate and record its exit status:

```text
./scripts/local-checks.sh
```

## Required decisions

Independently verify that v3-r2 resolves every prior finding rather than
merely asserting a disposition:

1. Do ignored-aware status checks reject ignored build/config artifacts
   before GPU inventory and again after the run?
2. Does the sorted route-prefix table mechanically catch committed additions,
   deletions, renames, and changes omitted from the explicit input table while
   still permitting committed changes outside the route?
3. Are handoff/result/input/verifier regular-file, link-count, parent-path,
   containment, and identity checks sufficient and testable?
4. Is the acceptance record deterministic while binding clean state, producer
   binary, producer source, route prefixes, and an empty route diff?
5. Can Phase C reproduce byte-identical acceptance with the Phase-B verifier,
   and does it compare all six enumerated identity classes without circularity?
6. Is the first persistent evidence mutation correctly placed after
   acceptance, with every negative case before mocked GPU inventory?
7. Does the threat model accurately bound what the two independent
   implementations can prove?
8. Does the expanded CPU matrix cover ignored files, omitted transitive input,
   hard links, symlinked parents, route-prefix grammar, all cross-phase
   identities, and deterministic records?
9. Is the re-pin topology still non-self-referential, and are the inherited
   conversion and non-claim boundaries exact?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
answer each decision with an unqualified `YES` or `NO`. Only if every answer
is `YES`, attest the candidate and all five exact input hashes, then end with
the requested token as the only bare acceptance line.

Acceptance opens only the CPU implementation and re-pin work. It does not
accept any implementation, review result, kernel, conversion, cn4 run,
quality result, or performance claim.
