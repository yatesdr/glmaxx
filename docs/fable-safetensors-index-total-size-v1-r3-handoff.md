# Fable handoff: safetensors index `total_size` accounting v1 r3

Date: 2026-08-04

Status: corrective adversarial design review requested

GPU authorization conveyed by this handoff: none

Do not launch CUDA, create a CUDA context, read checkpoint payloads, or modify
cn4. Read-only verification may stat the two pinned sources and read only each
index, eight-byte safetensors prefix, and padded JSON header. Require the r2
index hashes before any optional real-source check.

Review candidate commit:
`9c772e1227784ab7f62fcf9f90a787e8cb37c424`

Required result path:
`docs/reviews/fable-safetensors-index-total-size-v1-r3.md`

Requested acceptance token, only if every blocker and major is resolved:
`safetensors-index-total-size-v1-r3-design-accepted`

R3 supersedes the r1 and r2 handoffs for implementation authority. Review the
three design documents as one contract, but do not issue an older token. An
older result may be cited only as review history.

## Required provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Any mismatch withholds the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |
| `docs/safetensors-index-total-size-v1.md` | `350bbf1c52b7065276933cb97011930d9b3404c67eb3cb088303670bc38e66f6` |
| `docs/safetensors-index-total-size-v1-r2.md` | `6ec170c93c612d65866dcaec8555637cfb6763246bb45e78e2cf931109e2581c` |
| `docs/safetensors-index-total-size-v1-r3.md` | `ebf07483075a5dbe169570caa723feced60e5ac61870dcd60292587c80bc4386` |
| `docs/fable-safetensors-index-total-size-v1-handoff.md` | `a7937fdaf98a3e5a6a6fe43c588a54594b87e6a016b8cc709c6aa4b94fb13e10` |
| `docs/fable-safetensors-index-total-size-v1-r2-handoff.md` | `0c1d95e2013400d1dbb63d06ed173a875c127d2b1025ec45c89f53c2694b53e8` |
| `docs/safetensors-total-size-r2-implementation-readiness-20260803.md` | `3b1faefe0930e41f08d0a45cdb579fa8467cb6390ed837b2ee202abe30e23e4b` |
| `docs/cn4-integration-checkpoint-admission-6e073f3-20260804.md` | `90fd65d52e2eff8f920177308d49dad71db372d2d0ba0423d9773e458f193475` |
| `crates/glm-format/src/safetensors.rs` | `f15097989389dc8eebfad95bf7aa71977f1a43d5688c2c87273b047a2876149e` |
| `crates/glm-format/src/lib.rs` | `3d527c9c185d58c176350daf0880676fdcad28b39a320d08c2cd4c1e5dbc7576` |
| `crates/glm-format/src/checkpoint.rs` | `12777f070e56674599ce662326552cda7c28c2b36e5155d3e8daf7718577aa18` |
| `crates/glm-cli/src/main.rs` | `8f10c02b7ab859fc835a7526aab7cdfba8d5a1f4f834691bc92ff15cd68edb50` |
| `docs/checkpoint-ingest.md` | `186ce5985ca1adbf280a011a7692ae07780736f842c786dccb599ed8a458d07d` |
| `scripts/local-checks.sh` | `1675ca5bac9bda032ab6db206629bc0052ad6afc5cb6f59a7b6697e4e5c779d0` |

Run and record the complete CPU-only gate:

```text
./scripts/local-checks.sh
```

The green gate contains no r3 parser/accounting implementation and is not
acceptance evidence by itself.

## Retained r2 decisions

Repeat all eleven r2 decisions against the combined contract. Independently
rederive both exact real-source rows and confirm that contiguous coverage makes
the payload and complete-file interpretations mutually exclusive. Confirm the
typed result, checked arithmetic, no-payload-I/O boundary, and separation from
publisher/content authentication remain intact.

## R3 decision 1: diagnosis and honest boundary

Determine whether r2's per-shard post-header check actually permits an early
shard to change while later shards are opening. Confirm r3 neither calls its
sequential sweep an atomic snapshot nor weakens fail-closed retained-descriptor
use. Attack any wording that still implies arbitrary mutable files become
immutable through metadata sampling.

## R3 decision 2: exact index parsing

Attack BOMs, trailing data, duplicate top-level, metadata, `total_size`, and
weight-map keys, missing/empty maps, unknown top-level fields, and every JSON
numeric edge. Confirm duplicate keys cannot be collapsed by a generic map
before validation and only an integer representable as `u64` is accepted.

## R3 decision 3: retained-source transaction

Inspect the current source and determine whether r3 now specifies every change
needed to place index and shard opens behind no-follow, matching pathname/open-
descriptor pre-read fingerprints, exact header validation, post-read checks,
alias rejection, checked accounting, and retained descriptors. Confirm later
payload operations cannot be redirected by reopening a pathname.

## R3 decision 4: publication lease

Schedule an early-shard mutation after its local validation and before the
canonical publication sweep. Confirm it is rejected when observed, while a
post-sweep mutation correctly revokes the lease at the next validation rather
than being claimed retroactively impossible. Check every listed accessor,
streaming-reader boundary, hash, conversion publication, retry, and common-
failure rule.

## R3 decision 5: directory membership

Independently serialize the membership-digest preimage. Attack add, remove,
rename, replace, relink, non-UTF-8, nested, unsafe, alias, symlink, hard-link,
empty-set, and enumeration-order cases. Confirm directory identity is checked
on both sides of the second enumeration, membership is byte-identical, and the
mode remains diagnostic `None/Unspecified` with no conversion authority.

## R3 decision 6: production immutability

Determine whether retained fingerprints plus pinned content hashes and the
exact read-only-mount posture close the mutable-source boundary for production
admission. Check every retained descriptor with `fstatvfs(ST_RDONLY)`, mount-
namespace/effective-capability evidence, missing `CAP_SYS_ADMIN`, before/after
load receipts, and common failure. Confirm permission bits or a path-only
mount claim cannot substitute.

## R3 decision 7: proof sequence

Confirm explicit test barriers make every race deterministic, both real rows
remain metadata-only with read-only mounts and no CUDA device, and a separate
implementation review remains required before production admission. No design
token may be treated as checkpoint, CUDA, quality, capacity, or speed evidence.

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then answer
every retained r2 question and these decisions separately with an unqualified
`YES` or `NO`:

1. Is r3's diagnosis correct and its supersession fail-closed?
2. Is the exact parser implementable without pre-validation key collapse?
3. Is the retained index/shard transaction complete and non-redirectable?
4. Is the publication sweep honestly scoped and the revocable lease complete?
5. Is directory membership exact, stable at its declared checks, and still
   diagnostic only?
6. Is the production read-only/source-authentication boundary exact and
   sufficient for the stated threat model?
7. Does the amended proof matrix deterministically cover the new failures?
8. Is the combined r1+r2+r3 contract accepted for Rust CPU implementation?

Only if every retained and r3 answer is `YES`, attest the candidate commit and
all fourteen exact input hashes, then end with the requested acceptance token
declared above as the only bare acceptance line.

Acceptance opens only the Rust parser, accounting, retained-source lease, and
CPU proof plus a separate metadata-only real-source proof. It does not accept
their implementation, authenticate either checkpoint, authorize CUDA or
conversion, or establish checkpoint, quality, capacity, cold-start, or speed
evidence.
