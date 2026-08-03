# Current-tree review acceptance gate v3-r2

Date: 2026-08-03

Status: corrective design candidate; adversarial re-review required before
implementation

GPU authorization conveyed by this document: none

## Scope

This amendment resolves the findings in
`docs/reviews/fable-current-tree-review-acceptance-v3.md`. It is normative
over `docs/current-tree-review-acceptance-v3.md` where the two differ. The v3
command, record schema, exact-result-path rule, current-input hashing,
Phase-B/Phase-C chain, conversion integration, re-pin order, and non-claims
otherwise remain unchanged.

## Reviewed route closure

Every device or conversion handoff SHALL declare a nonempty, sorted, unique
`Route-relevant prefixes` table. Each entry is a repository-relative regular
file or directory prefix with no glob, absolute component, `.` or `..`.
Entries SHALL cover the Rust crates, CUDA sources, build definitions,
qualification scripts, container recipe, profiles, fixtures, and contracts
that can affect the reviewed route. The handoff itself and its result path
must not lie under a directory prefix merely to make the set self-referential.

The reviewer SHALL both accept the explicit input table as build-transitively
complete and confirm that the prefix table covers every route-relevant source
subtree. Human input-table review and the mechanical prefix check are both
required; neither substitutes for the other.

Before acceptance, each device/conversion script SHALL require both:

1. no committed addition, deletion, rename, or byte change beneath any route
   prefix between the reviewed candidate and current `HEAD`; and
2. an empty ignored-aware worktree status before the acceptance command and
   again after the complete run.

The first check uses the equivalent of:

```text
git diff --name-status --no-renames CANDIDATE..HEAD -- PREFIX...
```

The second uses the equivalent of:

```text
git status --porcelain=v1 --untracked-files=all --ignored=matching
```

and requires zero output. Thus tracked edits, ordinary untracked files, and
ignore-matched files such as in-tree `target/`, `build/`, `.env`, `.so`,
`.ptx`, `.cubin`, and `.fatbin` all fail closed. Build, cache, temporary, and
evidence directories must be outside the Git worktree. Later committed files
outside every declared route prefix remain permitted.

The script-level prefix and cleanliness checks are an independent shell
implementation. They SHALL parse the handoff's declared candidate and prefix
table rather than maintain a second handwritten subset.

## Acceptance record

Successful `glmaxx review-acceptance HANDOFF REVIEW` output retains schema
`glmaxx.current-review-acceptance.v3` and adds these deterministic fields:

- `worktree_clean: true`;
- SHA-256 of the exact verifier executable;
- the verifier source commit;
- the sorted route-prefix table; and
- an exact empty candidate-to-HEAD route-diff result.

The verifier executable must be a single-link regular artifact outside the
worktree, built from the accepted tracked source and identified by SHA-256.
That SHA-256 is the producer identity. The success record contains no wall
clock, hostname, absolute path, process ID, or other volatile field.
Phase C SHALL reuse the Phase-B verifier artifact or prove its SHA-256 equal,
so recomputing acceptance yields byte-identical JSON.

The command still binds every declared input independently. A handoff must
not list itself as an input. Handoff, result, input, and verifier path checks
reject a symlinked leaf, symlinked parent, hard link, directory, untracked
replacement, path escape, or identity change during the read.

## Pre-device ordering and evidence

Authorization, arguments, ignored-aware cleanliness, route-prefix closure,
exact handoff/result identity, and current-tree acceptance SHALL all pass
before the first GPU inventory, `nvidia-smi`, device compiler (`nvcc` or
CUDA CMake), container launch, CUDA context, or kernel command.

Compiling the CPU-only Rust verifier is permitted before acceptance only into
an external temporary target directory. The verifier JSON is written with
exclusive creation to a temporary path outside Git. After it reports
`CURRENT_TREE_REVIEW_ACCEPTED`, the script creates the unique evidence
directory, moves or copies that JSON as its first artifact, records its hash,
and may then inventory the GPUs. No pre-created evidence directory is valid.

Every negative path must terminate before a mocked first GPU-inventory
command. A later failure preserves the acceptance record and ordinary failure
evidence but cannot publish a successful manifest.

## Phase-B and Phase-C identity

Phase-B records six identity classes:

1. source commit;
2. handoff path and SHA-256;
3. result path and SHA-256;
4. acceptance-record and verifier SHA-256;
5. container digest; and
6. every built artifact path and SHA-256.

Before timing, Phase C recomputes acceptance with the byte-identical verifier
and compares all six classes: current source to the Phase-B source, current
handoff/result/record/verifier hashes to Phase B, its container digest to the
Phase-B digest, and every consumed binary/library hash to the Phase-B artifact
set. Any extra, absent, or changed identity fails closed.

## Threat model

This gate protects against accidental or unreviewed repository drift, stale
review artifacts, path substitution inside the checkout, and consuming a
different build artifact in Phase C. It assumes Git object integrity and an
honest host kernel/filesystem while the process runs. It does not defend
against an administrator replacing the running verifier, mounting a hostile
filesystem over the checkout, or rewriting Git objects. The independent
shell checks reduce a single-implementation error; they do not remove this
host trust boundary.

## Corrective CPU proof matrix

The implementation proof retains every v3 case and additionally requires:

- an ignored in-tree build artifact is rejected before GPU inventory;
- a changed, added, deleted, and renamed file under each route-prefix kind is
  rejected, including a change omitted from the explicit input table;
- a later committed file outside all route prefixes is accepted;
- malformed, empty, duplicate, overlapping, absolute, globbed, and escaping
  route prefixes are rejected;
- a hard-linked handoff, review, input, or verifier is rejected;
- a symlinked parent and a symlinked leaf are rejected independently;
- the result changed after acceptance is rejected before launch or timing;
- Phase C rejects each of the six identity classes independently; and
- two runs with the same source, verifier, handoff, result, and clean tree
  emit byte-identical acceptance records.

Tests use temporary repositories and mocked inventory/device commands. They
must not contact cn4 or require CUDA.

## Disposition of review findings

- MAJOR 1: closed by ignored-aware cleanliness plus mechanical route-prefix
  closure and corresponding negative tests.
- MINOR 2: `compiler` is narrowed to device compiler; CPU Rust verifier
  compilation is explicitly placed.
- MINOR 3: the six Phase-B identity classes and Phase-C comparisons are
  enumerated exactly.
- MINOR 4: successful records bind clean state and verifier identity while
  remaining free of volatile fields.
- MINOR 5: hard-link and symlinked-parent cases are mandatory.
- MINOR 6: the threat model and independent shell implementation are explicit.
- Questions 7 and 8: the acceptance JSON is the first evidence artifact, and
  route prefixes mechanically supplement reviewer completeness.

## Non-claims

This amendment accepts no review, implements no verifier, authorizes no cn4
operation, qualifies no kernel, and approves no checkpoint conversion. An
accepted re-review opens only the v3-r2 CPU implementation and re-pin sequence.
