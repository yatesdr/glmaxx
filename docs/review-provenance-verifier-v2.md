# Review provenance verifier v2

Date: 2026-07-29

Status: CPU-only implementation candidate

GPU authorization conveyed by this record: none

## Purpose

Version 1 proved that a handoff's candidate commit exists, that every pinned
input hash matches the blob at that commit, and that an optional review
contains exactly zero or one requested acceptance-token line. That prevented
working-tree drift from contaminating a review, but a token-only file could
still be classified as accepted without proving which candidate or input
bytes the reviewer considered.

Version 2 closes that acceptance gap. It does not try to decide whether a
reviewer's technical reasoning is sound.

## Accepted-review contract

For a review artifact whose token state is `accepted`, `review-proof` now
requires all of the following:

1. If the handoff declares `Required result path:`, the canonical
   repository-relative review path is exactly that path.
2. The review contains the exact 40-hex candidate commit as a complete
   hexadecimal word.
3. The review contains every pinned 64-hex input SHA-256 as a complete
   hexadecimal word.
4. The requested token still occurs exactly once as a complete line, and no
   other bare acceptance token occurs.
5. The review artifact is not the handoff itself. This closes the legacy
   format's self-acceptance shape, where the requested token was a bare line
   inside the handoff.

Hexadecimal word boundaries matter. A candidate-looking 40-character
substring inside a 64-character SHA-256 is not an attestation. A substring,
prefix, uppercase value, or surrounding hexadecimal characters cannot
satisfy the check.

Withheld reviews remain inspectable even if their prose omits one or more
hash literals. Their JSON reports whether the candidate was mentioned and
how many pinned hashes were present, but no withheld result opens a gate.

## Staged acceptance lint

Fable writes operator-owned review artifacts under `docs/reviews/`, while
older handoffs often require a root result path. Copying a malformed review
to the required path just to discover a missing hash or prose-wrapped token
creates avoidable coordination churn.

The staging-only command checks the proposed bytes in place:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-acceptance-lint HANDOFF STAGED_REVIEW
```

It verifies the pinned candidate blobs, requires the exact candidate and
every input SHA-256 in the staged review, and requires the requested token
exactly once on a bare line. It intentionally permits the staging path to
differ from the handoff's required result path.

A successful result uses schema
`glmaxx.review-staged-acceptance-proof.v1` and verdict
`STAGED_CONTENT_PASS_NOT_RECORDED`. It means only that an exact byte copy to
the required path would satisfy the content checks. It does not modify the
staged review, create the required result, increment any acceptance count,
or open a gate. `review-proof-all` remains the only repository-wide
acceptance inventory.

For a complete operator-owned review inbox, the batch form is:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-acceptance-lint-all docs/reviews [temporary-output]
```

It maps each regular staging file to a configured handoff by the required
result basename. Duplicate required basenames fail the whole command as
ambiguous. Every matched artifact uses the exact single-review verifier;
invalid artifacts are retained as structured per-review errors so one bad
file cannot hide the state of the rest. The command exits unsuccessfully
after writing the report when any matched artifact is rejected. Absent
reviews and unmatched status/README files are reported but do not fabricate
failures or acceptances.

The batch schema is `glmaxx.review-staged-acceptance-suite.v1`. Its only
passing verdict is `STAGED_CONTENT_PASS_NOT_RECORDED`; this still performs no
copy, promotion, acceptance-count update, or gate opening.

## Output

`glmaxx.review-provenance-proof.v2` and
`glmaxx.review-provenance-suite.v2` add:

- `required_result_path` to the handoff proof; and
- `candidate_commit_attested` plus `attested_input_hashes` to the optional
  review proof;
- automatic ingestion of a declared result path when that file exists; and
- suite counts for configured, present, accepted, and withheld result
  artifacts.

An absent declared result remains a pending gate and does not fail the suite.
Once the file appears, `review-proof-all` verifies it on every local run and
fails closed on a wrong path, self-review, malformed token, missing candidate,
or missing pinned hash. Handoffs that predate the declared-result convention
still require an explicit per-review command.

The candidate blobs are still read through `git cat-file` at the pinned
commit, never from the working tree. This verifier does not modify a handoff,
review, Git index, or GPU state.

## Gate boundary

This is repository evidence infrastructure. It does not:

- accept any existing Fable gate;
- infer acceptance from prose such as “pass with conditions”;
- prove that the reviewer hashed inputs at two different times;
- prove that the findings are technically correct;
- authorize CUDA, cn4 access, checkpoint conversion, or serving; or
- replace a gate-specific script's stronger named attestation lines.

An accepted review should be committed before a device qualification script
uses it. Device scripts retain their tracked-file, clean-tree, exact-result
path, named-hash, and immutable-run checks.

## CPU proof matrix

The unit proof covers:

- modern and legacy handoff parsing;
- required-result paths on the same or following line;
- duplicate and unsafe provenance paths;
- strict commit, SHA-256, and token shapes;
- withheld versus accepted token classification;
- unexpected and duplicate tokens;
- rejection of a prose-wrapped token by the staging-only acceptance lint;
- staging-path content validation without relaxing the required-path rule
  used by recorded results;
- batch discovery of ready, rejected, absent, and unmatched staged files;
- fail-closed rejection of duplicate required-result basenames;
- rejection when a handoff is supplied as its own review;
- rejection of token-plus-hash text that does not separately attest the
  candidate commit; and
- acceptance only after the exact candidate and every input hash are
  present.

The workspace gate must additionally run `review-proof-all` so every existing
handoff continues resolving against its historical candidate after the v2
parser change.
