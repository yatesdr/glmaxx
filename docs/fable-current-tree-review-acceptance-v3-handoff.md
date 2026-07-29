# Fable handoff: current-tree review acceptance gate v3

Date: 2026-07-29

Status: adversarial design review requested

GPU authorization conveyed by this handoff: none

cn4 posture: released to another workload; do not connect to cn4 or launch
CUDA for this review

Review candidate commit:
`60311cfa6ec61c80fb0a1544dfe8121e3c3e0c7b`

Required result path:
`fable-current-tree-review-acceptance-v3.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`current-tree-review-acceptance-v3-design-accepted`

## Provenance

Review the exact candidate in a detached worktree. Run `review-proof`, hash
every input at review start and finish, and report a stale candidate without
the token if either hash set differs.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/current-tree-review-acceptance-v3.md` | `4e81c38120044c609679498b665bd77061efd4f5c9b3707369e69a487d36ff40` |
| `docs/review-provenance-verifier-v2.md` | `933cf94825dc6b672ef260fb18069c865da8691c92c0ab18a2d5bec1cdbadb95` |
| `crates/glm-cli/src/review.rs` | `0319926f24f7ce84f4404f2a34ac595b8e1dc4a1ae853668145f95f9118c3196` |
| `crates/glm-cli/src/main.rs` | `6198f95898dcbb8d7af11986de33bc5bcf26267f91820934fb3311158d69feda` |
| `scripts/cn4-phase-b.sh` | `9ace5b4d4b0e8d2d1ee048bc32295cf86d7393b8420c5653b7d2f9faca23dd6d` |
| `scripts/cn4-exl3-phase-b.sh` | `4f8baac179d34bad89c565487b5954c31ceddfa51cda23494f57dc85d7b4bd35` |
| `scripts/cn4-phase-c-baseline.sh` | `86cbfb429222922755189babbd704248c0d6fc619da89f0c69e9e2351d3aa2b8` |
| `docs/fable-manifest-abi-v022-r2-handoff.md` | `d13839b369b22b0614fea641836c755cb544411c8343ca2ab6f78cc0a603f0e0` |
| `docs/fable-exl3-source-projection-v1-r2-handoff.md` | `22619d22148d5ee70ac08fb7d7baeaff08ea8f154420738a5953e37ad4f7d37d` |
| `docs/fable-exl3-warp-decode-v2-r2-handoff.md` | `1865d13ea97aa39dccbe47e26825ff00864e52f987aa4e3cd32dc21bf51dbd2b` |
| `profiles/profile-budget-v0.json` | `cdbe4eaad9465181b2ba60b3656fe5207eee54467abfbf8d9bc398c3e68c23e0` |
| `docs/checkpoint-ingest.md` | `186ce5985ca1adbf280a011a7692ae07780736f842c786dccb599ed8a458d07d` |

Run:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-current-tree-review-acceptance-v3-handoff.md
```

## Independently verify the live gap

Do not trust the design's diagnosis. Confirm from the pinned bytes that:

1. `review-proof.v2` hashes handoff inputs at the candidate commit and does
   not compare those paths with current working-tree bytes;
2. the manifest and EXL3 Phase-B scripts manually bind only a subset of their
   handoff input tables;
3. the manifest r2 handoff pins profile hash
   `028516adc04d454317e1b76a3147be4807c3ed3ce371e1d43aead3396270400d`,
   while the current profile hash is
   `cdbe4eaad9465181b2ba60b3656fe5207eee54467abfbf8d9bc398c3e68c23e0`;
4. Phase C requires `fable-manifest-abi-v022.md`, whose existing result is
   withheld, instead of the corrective r2 result path; and
5. `convert-pinned-exl3` checks exact token/named hash lines but does not
   parse the handoff, require the declared result path, or attest every
   reviewed input.

The current routes are fail-closed. The issue is that adding the missing r2
token would not by itself create a complete review-to-current-build binding.

## Required adversarial questions

1. Does current-worktree hashing of every declared handoff input close the
   reviewed-candidate-versus-built-HEAD gap without requiring unrelated
   later commits to be discarded?
2. Are tracked, repository-contained, single-link regular-file checks for
   handoff, review, and every input sufficient against symlink, hard-link,
   untracked-file, and path-substitution attacks?
3. Must `review-acceptance` itself require a globally clean tree, or is the
   design's separation—input binding in the command, global clean-tree checks
   in device scripts—safer and more reusable?
4. Can a handoff's declared input set be treated as the reviewed
   build-transitive closure, provided the reviewer explicitly accepts its
   completeness? If not, specify an implementable mechanical closure rule.
5. Is it safe to permit unrelated later committed files while requiring
   every reviewed input to remain byte-identical?
6. Does the command's success record contain enough identity to audit and
   reproduce the acceptance decision without trusting terminal output?
7. Is the required order strong enough to prove all rejection paths occur
   before GPU inventory, CUDA compilation, context creation, or persistent
   evidence mutation?
8. Does the Phase-B/Phase-C chain bind correctness and timing to identical
   source, handoff, result, acceptance record, and container identities?
9. Should Phase C rerun acceptance, verify the Phase-B acceptance record, or
   both? The design requires both; identify any circular or unstable field.
10. Is the conversion integration sufficient when combined with the
    existing embedded-source-commit and blocked-profile checks?
11. Is the re-pin sequence non-circular? In particular, can an implemented
    script point to a handoff that later hashes that script without a
    self-referential byte digest?
12. Does the CPU proof matrix cover accepted, withheld, stale, dirty,
    substituted-path, cross-phase-drift, and pre-device ordering failures?
13. Are the non-claims exact, and does design acceptance correctly open only
    implementation rather than any GPU or conversion route?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
state separately whether:

- the live review-to-build gap is real and accurately scoped;
- the `review-acceptance` command contract is accepted;
- the handoff coverage and clean-tree split are accepted;
- Phase-B/EXL3/Phase-C integration is accepted;
- conversion integration is accepted;
- the re-pin sequence is accepted; and
- the CPU proof matrix and non-claims are accepted.

Only if all seven answers are unqualified `YES`, end with the requested
token. Withhold it for a conditional pass, stale input, an unbound current
byte, an incomplete identity chain, a self-referential re-pin, or any
rejection path that can touch a GPU first.

The token accepts only this design. It does not accept the later
implementation, any r2/r3 kernel gate, checkpoint conversion, cn4 access, or
device evidence.
