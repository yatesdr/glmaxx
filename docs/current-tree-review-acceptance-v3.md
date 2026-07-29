# Current-tree review acceptance gate v3

Date: 2026-07-29

Status: design candidate; adversarial review required before implementation

GPU authorization conveyed by this document: none

## Problem

`review-proof.v2` correctly proves that:

- a handoff names an existing candidate commit;
- every handoff input hash matches that candidate;
- an accepted result is at the declared path;
- the result attests the candidate and every input hash; and
- the requested token appears exactly once.

It intentionally reads candidate blobs rather than the working tree. That is
the right behavior for provenance review, but it is insufficient as the
last gate before a device build: a later clean `HEAD` can retain the accepted
handoff and token while changing one of the reviewed kernel, ABI, script, or
contract inputs.

The current Phase-B scripts compensate with a few named current-file hash
checks, but those checks cover only a subset of each handoff table. The
manifest Phase-B script also pins an older profile-budget hash, and the
Phase-C timing script still names the original withheld manifest review
artifact rather than the corrective result path. The current state therefore
fails closed, but it cannot become a truthful accepted-review-to-build
contract merely by adding a token.

## Command

Version 3 adds:

```text
glmaxx review-acceptance HANDOFF REVIEW
```

The command SHALL:

1. perform every `review-proof.v2` handoff and result check;
2. require a configured acceptance token and `token_state=accepted`;
3. require `REVIEW` to be the exact declared result path;
4. require the handoff and review to be tracked, single-link regular files
   inside the repository;
5. read every provenance input from the current worktree, not only from the
   candidate commit;
6. require each current input to be a tracked, single-link regular file
   inside the repository, with no symlink traversal;
7. require the current SHA-256 of every input to equal its handoff SHA-256;
8. reject a missing, extra, duplicate, malformed, withheld, self-review, or
   stale artifact; and
9. exit nonzero before emitting an accepted verdict on any mismatch.

The successful record schema is:

```text
glmaxx.current-review-acceptance.v3
```

It SHALL include:

- current source commit;
- handoff path and SHA-256;
- reviewed candidate commit;
- review path and SHA-256;
- exact acceptance token;
- one sorted record per input with candidate, expected, and current SHA-256;
- input count; and
- exact verdict `CURRENT_TREE_REVIEW_ACCEPTED`.

`review-proof` and `review-proof-all` retain their v2 semantics. A withheld
review remains inspectable there. `review-acceptance` is deliberately
stricter and never succeeds for a withheld or unconfigured token.

## Clean-tree and coverage contract

The command binds every declared review input, but it cannot infer whether a
handoff omitted a transitive build input. Each device handoff SHALL therefore
include all of:

- engine/format/operation/profile contracts used by the route;
- Rust ABI, FFI, oracle, descriptor, launcher, and command sources;
- native header, CUDA source, CMake source, and compile definitions;
- qualification script and any invoked helper;
- pinned matrix/fixture inputs;
- relevant preparation/evidence record;
- container recipe and CUTLASS identity contract; and
- any source whose bytes affect validation, allocation, launch, comparison,
  or result classification.

The adversarial reviewer SHALL explicitly answer whether that set is
build-transitively complete.

Device scripts additionally require `git status --porcelain` to be empty
before acceptance verification and again after the run. This excludes
untracked or unrelated dirty-source ambiguity without forcing the current
commit to equal the older candidate commit. Unrelated, committed later
changes are permitted only when no reviewed input changed.

## Device-script integration

The corrected manifest and EXL3 qualification scripts SHALL:

1. validate operator authorization and required non-GPU arguments;
2. require a clean committed tree;
3. resolve a tracked exact handoff and exact result path;
4. run `review-acceptance` and capture its JSON outside Git;
5. require `CURRENT_TREE_REVIEW_ACCEPTED`;
6. retain gate-specific named-hash checks where they are stricter;
7. record the acceptance JSON SHA-256 with source/review hashes; and only
8. then inventory cn4 or create a CUDA context.

No device query, `nvidia-smi`, compiler invocation, container launch, or
evidence-directory mutation may precede acceptance.

The script may build `glmaxx` to run this CPU-only command. Its Cargo target
directory SHALL be external to Git and included in the run provenance.

## Phase-C correction

Phase C SHALL require the same corrective manifest handoff/result pair used
by the corresponding Phase-B run. It SHALL not accept the original withheld
`fable-manifest-abi-v022.md`.

Phase-B evidence SHALL record:

- source commit;
- handoff SHA-256;
- review SHA-256;
- current-tree acceptance-record SHA-256;
- container digest; and
- all built artifact hashes.

Phase C SHALL compare all five identities with its current source and
freshly recomputed `review-acceptance` record before timing. It already
requires the identical Phase-B source commit; v3 makes the review identity
equally explicit.

## Conversion integration

`convert-pinned-exl3` SHALL invoke the same strict acceptance verifier against
the exact conversion handoff/result pair before it opens a checkpoint or
creates an output directory. Token-line grep alone is insufficient.

This gate is necessary but not sufficient for conversion. The profile must
still be complete and `conversion_allowed=true`, the embedded converter
source commit must match the requested commit, and all source/checkpoint
checks remain mandatory.

## Re-pin sequence

There is no valid shortcut that edits a reviewed script while continuing to
use the old script hash. After this design is accepted:

1. implement and CPU-test `review-acceptance`;
2. update Phase B, EXL3 Phase B, Phase C, conversion, and local negative
   gates;
3. create new complete manifest/EXL3 implementation handoffs pinned to the
   resulting candidate;
4. obtain accepted result artifacts for those exact bytes; and
5. only then run a newly authorized cn4 qualification.

The r2 handoffs remain valid historical reviews of their pinned candidates,
but they cannot authorize a v3 script or a changed profile.

## CPU proof matrix

The implementation proof SHALL cover:

- accepted candidate with identical current inputs;
- one current input changed after the candidate;
- input changed and then restored;
- accepted token with missing candidate attestation;
- accepted token with one missing input attestation;
- withheld and unconfigured tokens;
- wrong result path and handoff-as-review;
- tracked regular file replaced by symlink, directory, or untracked file;
- dirty unrelated path rejected by each device script;
- Phase C supplied the original withheld artifact;
- Phase B/Phase C source, handoff, review, or acceptance hash mismatch;
- a result artifact changed during a run; and
- proof that every negative case exits before the mocked first GPU-inventory
  command.

Tests SHALL use temporary repositories and mocked device commands. They SHALL
not contact cn4 or require CUDA.

## Non-claims

This design does not accept any current Fable result, authorize cn4, qualify
a kernel, approve checkpoint conversion, or establish quality or
performance. It closes only the provenance link between accepted review
bytes and the bytes a later device/conversion command is about to consume.
