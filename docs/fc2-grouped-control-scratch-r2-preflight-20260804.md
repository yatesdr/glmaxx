# FC2 grouped-control scratch r2 preflight

Date: 2026-08-04

Status: exact design candidate and retained-evidence preflight passed;
adversarial acceptance and implementation remain pending

## Candidate and local gate

The exact candidate
`419c2b0832723f5ffaeecbbc39c9ad6fd8652be7` was checked in detached worktree
`/tmp/glmaxx-fc2-scratch-preflight-419c`. All twelve hashes pinned by
`docs/fable-fc2-grouped-control-scratch-r2-handoff.md` matched at the start
and finish, and the worktree remained clean.

`./scripts/local-checks.sh` returned zero. Its release matrix passed 413 tests:
122 cache, 17 CLI, 11 CUDA-ABI, 98 engine, 72 format, three NVFP4 proof, 22
reference, 16 scheduler, 42 serving, and ten tokenizer tests. Clippy, release
proofs, deterministic profile validation, review provenance, engine, serving,
and cache-lifecycle proofs also passed. The candidate review proof verified
138 handoffs and 39/120 configured results, with 39 accepted and none withheld
at that historical commit.

## Independent arithmetic

For M1, eight assignments, 6,144 hidden elements, and 512 local intermediate
elements, the pre-existing FC2 terms independently sum to 356,420 bytes:

```text
activation values          32,768
activation scales           4,096
activation global scales       32
assignment FP32 output     196,608
materialized BF16 output    98,304
token FP32 output           24,576
slot assignment                 32
validation                       4
```

Replacing the 4,096-byte global SFA plane with the 32,768-byte grouped SFA
capacity, then replacing the 24,576-byte token-output term with the 4,194,304
byte scratch floor, yields exactly 4,554,820 bytes. The scratch helper's
`max(rows * 6,144 * 4, 4 MiB)` is 4,194,304 bytes at row 170 and 4,202,496
bytes at row 171.

The pinned non-launching probe's independent CUTLASS construction reports
3,072 bytes of grouped metadata and 144,384 bytes of CUTLASS workspace. Their
147,456-byte sum exceeds the historical 24,576-byte capacity while metadata
alone fits, proving the observed `-3` was the combined scratch check rather
than the earlier metadata-only hypothesis.

## Retained cn4 evidence

Read-only verification of
`/home/derek/glmaxx/evidence/20260803T191700Z-fc2-scratch-probe-c25e558`
reproduced manifest SHA-256
`44efef29ecfabd552345368d22785c054d79df702f616ae86630276b9396bda7`.
All fourteen listed records passed `sha256sum -c`. The retained source includes
the exact accepted FC2 translation unit, constructs the same `GroupedScratch`,
CUTLASS arguments, hardware record, and `get_workspace_size` call, and its
output records 188 SMs and the byte figures above. The stored exit status is
zero and verdict is `FC2_GROUPED_SCRATCH_PROBE_PASS`.

This preflight only read the historical evidence directory. It did not compile
on cn4, create a CUDA context, query a GPU, execute the probe, or launch a
kernel.

## Gate boundary

This record found no new contradiction in the r2 design, but it is not an
adversarial review. The exact token
`fc2-grouped-control-scratch-r2-design-accepted` remains mandatory before the
shared Rust/native helper, 112-byte non-launching probe ABI, and qualification
script changes may land. The already-audited uncommitted implementation is
not sufficient: it lacks the row-65,536 ceiling and shared probe and must not
be sent to cn4.
