# Fable request: repair machine attestations in four accepted reviews

Date: 2026-07-30

Status: administrative provenance correction required; substantive verdicts
are not being reopened

GPU authorization: none

## Outcome that triggered this request

Fable placed four substantively accepted reviews in
`docs/reviews/`:

- `fable-exl3-source-projection-v1-r2.md`;
- `fable-exl3-warp-decode-v2-r2.md`; and
- `fable-manifest-abi-v022-r2.md`; and
- `fable-restore-identity-v1.md`.

Their exact acceptance tokens and the gate-specific attestation subsets are
present. Sol copied the bytes without modification to the root result paths
required by the handoffs and verified each copy with `cmp` and SHA-256.

The repository-wide machine gate then rejected all four accepted artifacts.
`review-proof-all` requires an accepted result to contain the exact candidate
commit and every SHA-256 pinned by its handoff. The reviews state that all
inputs matched at review start and finish. The three kernel reviews do not
print every hash. The restore-identity review prints every hash, but its
acceptance token appears only inside prose rather than exactly once on a bare
line, so the verifier correctly classifies it as withheld.

The promoted root copies were removed again so the cn4 scripts remain
fail-closed. The operator-owned originals under `docs/reviews/` were not
modified. Do not issue a different substantive verdict unless rereading the
already-reviewed bytes changes it.

## Required repair

Reissue each review with its existing findings, answers, token, and
substantive conclusion unchanged, adding a machine provenance appendix that
contains:

1. the exact candidate commit as one 40-hex word;
2. every handoff input SHA-256 as one exact 64-hex word;
3. a statement that the candidate and all input hashes were verified at
   review start and finish; and
4. the existing acceptance token exactly once on a line by itself.

Write repaired files to these staging paths:

```text
docs/reviews/fable-exl3-source-projection-v1-r2-attested.md
docs/reviews/fable-exl3-warp-decode-v2-r2-attested.md
docs/reviews/fable-manifest-abi-v022-r2-attested.md
docs/reviews/fable-restore-identity-v1-attested.md
```

Sol will copy accepted repaired bytes verbatim to the original required root
result paths and will verify both byte identity and the machine gate. Do not
edit the original review files in place.

## EXL3 source projection v1 r2

Handoff:
`docs/fable-exl3-source-projection-v1-r2-handoff.md`

Candidate:
`0edfc8d796aeaeb969668005149bcb6286aa1e85`

The current result is missing these exact input attestations:

```text
c299371ec162f8d86acf323d5856657d99ebb0d81cea52401f2f128d43ed0298
4f8baac179d34bad89c565487b5954c31ceddfa51cda23494f57dc85d7b4bd35
b94156526800da97bc30f46e0315a64ab43c488c2007f510dedb86e71b3ec805
```

The other four input hashes and candidate are already present, but the
appendix should print all seven handoff input hashes so the attestation is
self-contained.

Acceptance token:

```text
exl3-source-projection-v1-accepted
```

## EXL3 warp-decode v2 r2

Handoff:
`docs/fable-exl3-warp-decode-v2-r2-handoff.md`

Candidate:
`0edfc8d796aeaeb969668005149bcb6286aa1e85`

The current result is missing these exact input attestations:

```text
943fc05276e3efe8fa31959c5ad872168ac46cb0ce257bda0c5042c5a137768b
20e4f6007969d42d78aba586e0a2b2496fc19483cd94563ed366bcd3e1b2b389
7c5bfa0795aa6b78646b00b029f8d4edc7c15491d69ea19e423f770701b5cdc3
c290960591295a1dd440818f97a7317fb42a061f9bc71c87a31b5bbb4cfd3647
241730ceaf629d01101629cb3f107e8d13fe92019444f4b635f9aa1d8cbc819d
b94156526800da97bc30f46e0315a64ab43c488c2007f510dedb86e71b3ec805
```

The design-document hash and candidate are already present, but the appendix
should print all seven handoff input hashes.

Acceptance token:

```text
exl3-warp-decode-v2-design-accepted
```

## Manifest ABI v0.2.2 gate / v0.2.3 bytes r2

Handoff:
`docs/fable-manifest-abi-v022-r2-handoff.md`

Candidate:
`0edfc8d796aeaeb969668005149bcb6286aa1e85`

The current result is missing these exact input attestations:

```text
505bf452895cde7598e8e03141bd8bd381729f31f5ee95c11c036d26c79c8d42
d5a189ae06f8f39e828400e589fbd31f94f0245cb90488881369d6a806bd6d1e
28905e69300a3a8c8105752ee9aaeb4d718cbe4387cab548139c257242ec68a4
23b4f7636b5930d6d7ef5c936b333fbcaca3c84705f37a29bd22e3895f2213f1
da233563c6bfe92885c1a3101bcafa20292365b12ab788afb4d32d44a3ed2472
dba61fd6bc34b659543f1b64a329603ab01406a1505954ae16bc9626b8f7ff94
b72fff75bf4b0ee0ef06bf65286bad73678e4d396b2bdaad72bc784da738bb31
c31b07d7f9054f3d51bc5d24c2c414b6c9a134d88f042502bc0f82e29cad500f
d37a1400dc0c393b26c121f72694945bef78c28eda29796abf41a2ed713a17ac
cda0df00f87041d2fa6a01b9fd43bc68706c1dfa30d398d3410a68ac3a068735
9ace5b4d4b0e8d2d1ee048bc32295cf86d7393b8420c5653b7d2f9faca23dd6d
b94156526800da97bc30f46e0315a64ab43c488c2007f510dedb86e71b3ec805
```

The four gate-specific identity hashes and candidate are already present,
but the appendix should print all sixteen handoff input hashes.

Acceptance token:

```text
manifest-abi-v0.2.2-accepted
```

## Asynchronous restore identity v1

Handoff:
`docs/fable-restore-identity-v1-handoff.md`

Candidate:
`dc16273b019cf3a3dd8eb810cf9caeb26c99bced`

The current result already contains the candidate and all six exact input
attestations:

```text
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
74e7dd8077d7ce1db082b6b2501debfcf07d39f0c444e5e355bdb5385ac29770
d37a1400dc0c393b26c121f72694945bef78c28eda29796abf41a2ed713a17ac
786c7c7e5ce2f417749a78e8c48aa8a7d0a5cb617e0883e960a8e7c17d781720
11ad4936fea7cd0887e660911f50778d5b0918c21a6cebaca1a98a244b2e2de1
16c44adf52c8fa0ad40b1656f7774bbea8072673fdddf5763f1ff33a3b4db256
```

Reissue the same unqualified verdict and append its existing token exactly
once on a line by itself:

```text
restore-identity-v1-accepted
```

## Sol verification after delivery

For each repaired result, Sol will:

1. compare the staged file byte-for-byte with its promoted root copy;
2. run `review-proof` with the exact handoff and root result;
3. run `review-proof-all`;
4. require the exact token to appear once; and
5. require the cn4 phase scripts to remain unauthorized until the operator
   grants a new GPU window.

The expected result is four accepted configured reviews with no missing
candidate, input attestation, or bare token. This repair does not itself
authorize implementation, a CUDA launch, cn4 access, or any
quality/performance claim.
