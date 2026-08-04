# NF3/ModelOpt source-and-kernel r2 preflight

Date: 2026-08-04

Status: exact design-candidate and independent-layout preflight passed;
adversarial acceptance and CPU implementation remain pending

## Candidate and local gate

The exact candidate
`2b8785907c11d2b58d8c5fa7f782845fae03e3ad` was checked in detached worktree
`/tmp/glmaxx-nf3-source-preflight-2b87`. All twelve hashes pinned by
`docs/fable-nf3-nvfp4-hybrid-source-and-kernel-v1-r2-handoff.md` matched at
the start and finish, and the worktree remained clean.

`./scripts/local-checks.sh` returned zero. Its release matrix passed 413 tests:
122 cache, 17 CLI, 11 CUDA-ABI, 98 engine, 72 format, three NVFP4 proof, 22
reference, 16 scheduler, 42 serving, and ten tokenizer tests. Clippy, release
proofs, deterministic profile validation, review provenance, engine, serving,
and cache-lifecycle proofs also passed. The candidate review proof verified
140 handoffs and 39/122 configured results, with 39 accepted and none withheld
at that historical commit.

## Independent Rust enumeration

A temporary Rust verifier was written independently from the candidate's
future converter and native inverse. Its source SHA-256 is
`9130c983d32605572053f937de504e5560d88bc8ceb9c3bc15c13e3a7f76bea7`;
the optimized executable and output hash to
`14c20aebf7706eea90a94091084a4983d837106db1b659594d1316064ab72090`
and `8511af360e99401d5f26e9a204fbef9cf4808f1a9eeb47dace91f3003e19791f`.

It proved:

- all 16,777,216 possible little-endian 24-bit NF3 source words extract and
  reconstruct eight exact three-bit codes;
- the low-two/high-one fragment split reconstructs all eight codes for every
  possible source word, while all 256 ModelOpt bytes preserve low-K/high-K
  nibble order;
- the r2 NF3 forward coordinate stream covers every logical value exactly once
  at `[1024,6144]` and `[6144,512]`, with no duplicate or out-of-range index;
- the NF3 8x8 scale transpose is bijective at both actual shapes;
- the ModelOpt `0x1201` scale permutation is bijective at both actual shapes;
- source scale bytes `0x00..0x7e` form exactly 127 canonical values, while all
  129 remaining encodings—including negative zero `0x80` and signed NaN
  `0xff`—are rejected; and
- the exact BF16 codebook bits are
  `bf80,bf1b,beb6,be03,3e03,3eb6,3f1b,3f80`.

The actual-shape byte results were:

| Rank-local record | Values | NF3 code | NF3 scale | ModelOpt value | ModelOpt scale |
|---|---:|---:|---:|---:|---:|
| fused gate/up `[1024,6144]` | 6,291,456 | 2,359,296 | 196,608 | 3,145,728 | 393,216 |
| down `[6144,512]` | 3,145,728 | 1,179,648 | 98,304 | 1,572,864 | 196,608 |

Thus one NF3 expert is 3,833,856 payload bytes per rank and one ModelOpt
expert is 5,308,416. The 14,400 target NF3, 4,800 target ModelOpt, and 256
draft ModelOpt assignments total 82,046,877,696 routed payload bytes per rank;
two 192-byte records per expert add 7,471,104 bytes, for 82,054,348,800 routed
bytes per rank before protected/shared tensors and arena alignment.

## Gate boundary

No cn4 connection, checkpoint payload read, conversion, CUDA context, or GPU
launch occurred. This preflight found no new contradiction in the r2 contract,
but it is not Fable acceptance and does not implement either codec. The exact
token `nf3-modelopt-nvfp4-source-kernel-v1-r2-design-accepted` remains
mandatory before the full CPU parser, metadata, converter, and numerical proof
may land. W4A4 CUTLASS evidence remains ineligible for the W4A16 profile.
