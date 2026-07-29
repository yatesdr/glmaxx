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
- rejection when a handoff is supplied as its own review;
- rejection of token-plus-hash text that does not separately attest the
  candidate commit; and
- acceptance only after the exact candidate and every input hash are
  present.

The workspace gate must additionally run `review-proof-all` so every existing
handoff continues resolving against its historical candidate after the v2
parser change.
