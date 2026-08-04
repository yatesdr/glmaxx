# Fable handoff: EXL3 mixed-K source and kernel contract v1 r3

Date: 2026-08-04

Status: corrective adversarial design review requested

GPU authorization conveyed by this handoff: none

Do not launch CUDA, create a CUDA context, read checkpoint payloads, or modify
cn4. Read-only hash/schema verification of the pinned tier file is permitted.

Review candidate commit:
`b4dc806db89076ac015bd25d942b3d2ebfb724b3`

Required result path:
`docs/reviews/fable-exl3-mixed-k-source-and-kernel-v1-r3.md`

Requested acceptance token, only if every blocker and major is resolved:
`exl3-mixed-k-source-and-kernel-v1-r3-design-accepted`

R3 supersedes the r1 and r2 handoffs for implementation authority. Review
r1+r2+r3 as one contract, but do not issue either older token. An older result
may be cited only as review history.

## Required provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Any mismatch withholds the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `docs/exl3-mixed-k-source-and-kernel-v1.md` | `96d6ce4efc5782f0c1e3da613f4e067d1526e2eef6610196a2caaa543b866df9` |
| `docs/exl3-mixed-k-source-and-kernel-v1-r2.md` | `38b9ae4c9282c1550cc20fa7b8f4ac35fe38eb0fd3ea7e54c4a15a4c6cd741fe` |
| `docs/exl3-mixed-k-source-and-kernel-v1-r3.md` | `683bec3908a0650a4cef7d53075c5438f7d15473d631f62da1de3cd70d8e2866` |
| `docs/fable-exl3-mixed-k-source-and-kernel-v1-r2-handoff.md` | `6daeaed7ab25cddaee94ce15aa77053a3ddb58c726d099d1df554c08c69ff077` |
| `docs/cn4-tr3-tier-boundary-20260803.md` | `325f52710787eaf0eae15ceaa0bcb80310a7b4ca018d397b4c3ce1da139a1623` |
| `docs/exl3-mixed-k-r2-preflight-20260804.md` | `6612eaededcd35437f098d611bd41c4046acecf9fa1e6adc476b2da9ff6c60a0` |
| `docs/exl3-mixed-k-r2-implementation-readiness-20260803.md` | `a99d772892ea09ee6c0edd2fa633e671caeb60cd4f0487d75fd5eaefdacd16e9` |
| `docs/exl3-grouped-gate-up-sm120-v1-r2.md` | `b39d36644bba8c25e1ddc154f84c1a573a05589d86689fdb78defe2550509754` |
| `docs/target-layer-execution-v1.md` | `89c6cf7397a3dc6b0c01383e679dcc4b51e20e3c45057bef0928dc24b866a819` |
| `docs/target-layer-execution-v1-r2.md` | `808da35c2e54eb5692512996650839fb6f127cb91658603eb2fb5ce049c56ed2` |
| `docs/exl3-sm120-source-projection.md` | `20e4f6007969d42d78aba586e0a2b2496fc19483cd94563ed366bcd3e1b2b389` |
| `docs/exl3-sm120-warp-decode-v2.md` | `67fb3bcb5b839cc50f3462990f1ef6056ca7c9d991851efc5e668fed9d0b3325` |
| `crates/glm-format/src/exl3.rs` | `f6fa1b25311d78e13e22a0c7c908da7abca636948218fef1987c89850e974edb` |
| `crates/glm-format/src/safetensors.rs` | `f15097989389dc8eebfad95bf7aa71977f1a43d5688c2c87273b047a2876149e` |
| `crates/glm-format/src/checkpoint.rs` | `12777f070e56674599ce662326552cda7c28c2b36e5155d3e8daf7718577aa18` |
| `crates/glm-format/src/rank_manifest.rs` | `cb57a81ad3643a86992c4fd9c2166e9ee238cc7e66063732355d14e485f73410` |
| `crates/glm-engine/src/weight.rs` | `d658cefefc17757a28258bafd0e13f5309e8adcbf2b30c4d2bdc97be9899ca19` |
| `crates/glm-engine/src/graph.rs` | `c85ca1aa52ba42294fc6a43524e8f70357523977d343e8c2f212787e7754cd22` |
| `scripts/local-checks.sh` | `1675ca5bac9bda032ab6db206629bc0052ad6afc5cb6f59a7b6697e4e5c779d0` |

Run and record the complete CPU-only gate:

```text
./scripts/local-checks.sh
```

The green gate contains no r3 parser or partition implementation and is not
acceptance evidence by itself.

## Retained r2 decisions

Repeat all thirteen r2 decisions against the combined contract. Independently
rederive 75 target layers, one recurrent draft layer, 172,800 target K3,
57,600 target K4, 3,072 draft K3, zero draft K4, 233,472 total descriptors,
5,662,310,400 K4-delta bytes/rank, and 75,293,233,152 routed source-plane
bytes/rank. Confirm descriptor-derived width remains authoritative and that
no average-bpw, caller width, rank-local fallback, or dense reconstructed
cache is introduced.

## R3 decision 1: diagnosed ambiguity and supersession

Determine whether r2 actually left the partition-hash serialization,
non-selection tier fields, duplicate JSON keys, and K4 functional route
undefined. Confirm r3 has clear precedence without weakening any retained r2
gate. Withhold the token if an older handoff can still open implementation.

## R3 decision 2: strict tier parsing

Attack BOMs, trailing data, duplicate root/nested keys, alternate layer-key
spellings, missing/extra fields, wrong array lengths, noninteger membership,
nonfinite or unrepresentable error values, reordered/incomplete tails, and
nonempty NVFP4 membership. Confirm the exact target/draft key sets match the
pinned TR3 profile while diagnostic floats never select execution width.

If cn4 remains available, read only
`/home/derek/models/GLM-5.2-EXL3-TR3-3.25bpw/tier_bitmap.json`, first requiring
SHA-256 `a287ffe816de5998fbc35a56a1ec05f69eb71087d5bbdfe631242c6b296b2a3d`.
Do not treat that raw hash as publisher authentication.

## R3 decision 3: canonical tier identity

Serialize the tier-plan preimage independently. Confirm its version, three
source identities, 76 ordered layer records, target/draft discriminator,
fixed expert count, and 19,456 descriptor-derived width bytes are exact and
address-free. Prove each width has twelve agreeing physical observations and
that target `k` or draft K3 disagreement fails before the digest exists.

Attack reordered layers/experts, a flipped kind or width, zero/stale source
identity, raw JSON reordering, and inclusion of TP rank or a device address.
Confirm the raw tier digest still binds diagnostic bytes outside the canonical
selection view.

## R3 decision 4: per-step partition receipt

Independently encode the 16-byte record and full digest preimage. Reconstruct
the complete router table from the K3 and K4 bins. Prove every original source
ordinal and unique `(token_row,route_slot)` destination appears exactly once,
bin order is the original compacted order, all fields are bounded, counts add
exactly, and draft K4 is impossible.

Attack consistent-looking count changes, reordered filters, renumbered
ordinals, duplicate or absent destinations, stale step/input/router/tier/
program/policy identity, and rank-local membership. Confirm the receipt is
rank-common while uploaded pointer bytes remain rank-local.

## R3 decision 5: minimum functional K4 route

Confirm source-projection v2 can execute gate, up, and down with separate
compile-time K3/K4 specializations and one width dispatch outside the weight
loop. Determine whether grouping by `(bits,expert)` plus source-ordinal scatter
preserves the target layer's FP16 projection, FP32 SwiGLU, route weighting,
scatter-add, and TP4 reduction boundaries.

Confirm both specialization capabilities and the route are selected commonly
before graph construction, any failure is common-step fatal, and the separately
gated grouped-K3 candidate is only an optional gate/up replacement. A K3-only
grouped result must not be accepted as a mixed-K target-layer result.

## R3 decision 6: implementation gate

Confirm the amended CPU matrix covers strict parsing, byte-exact hashes,
partition reconstruction and mutations, four-rank consensus with different
local addresses, K3/K4 controls, rows 1 through 8, and 3,072-row checked
arithmetic. Confirm CPU implementation review precedes any SM120 launch and
that K3/K4 target plus K3 draft layer replays precede checkpoint smoke.

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer every retained r2 question and these six r3 decisions separately with
an unqualified `YES` or `NO`:

1. Is r3's diagnosis correct and its supersession fail-closed?
2. Is the strict tier parser contract complete and compatible with the exact
   pinned TR3 schema?
3. Is the canonical tier-plan identity exact, address-free, and bound to all
   physical widths and authenticated source identities?
4. Is the per-step partition receipt byte-exact, reconstructible, bounded,
   and rank-common without comparing rank-local addresses?
5. Is the minimum K3/K4 functional route complete for gate, up, and down while
   keeping grouped K3 optional and honestly scoped?
6. Does the proof/gate sequence satisfy the required design-review-first
   progression?
7. Is the combined r1+r2+r3 contract accepted for its coordinated Rust CPU
   proof?

Only if every retained and r3 answer is `YES`, attest the candidate commit and
all twenty exact input hashes, then end with the requested acceptance token
declared above as the only bare acceptance line.

Acceptance opens only the Rust tier parser, tier plan, partition, accounting,
and CPU proof. It does not accept their implementation, a checkpoint, CUDA,
a target/draft layer, MTP, quality, KV capacity, reload, concurrency, or speed.
