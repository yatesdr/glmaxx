# Fable handoff: SM120 rank executor v1 corrective r3

Date: 2026-08-04

Status: superseding adversarial design and native-ABI review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4, launch CUDA, create a context, or modify any runtime
resource for this review.

Review candidate commit:
`236dab0e7fe6d8e674b1666e3906d1bc0c9bbc8f`

Required result path:
`docs/reviews/fable-sm120-rank-executor-v1-r3.md`

Requested acceptance token, only for an unqualified pass:
`sm120-rank-executor-v1-r3-design-accepted`

This handoff supersedes the unexecuted r2 handoff. Do not issue the r2 token
or authorize implementation from its header bytes. The r3 review must judge
r1+r2+r3 together using the corrected header at this candidate.

The original withheld review remains a required operator-inbox input at
`docs/reviews/fable-sm120-rank-executor-v1.md`, SHA-256
`efe697b86235ac757fcfd9123d0f28b92a37f337a75f64267d8e96862620dd36`.
Hash it before using the r2 closure claims. Withhold the r3 token if it is
absent or differs.

## Required provenance procedure

Review the exact candidate in a detached worktree. Hash every listed input at
review start and finish. Report a stale candidate and withhold the token if
either set differs. Do not substitute moving `main`, the later handoff commit,
or an untracked review-inbox source.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/sm120-rank-executor-v1.md` | `e97c54b865ed50c40ff8b15f6580d0edc18dbd0783135bc1c17d11cc19986fd4` |
| `docs/sm120-rank-executor-v1-r2.md` | `4f40ea7652858b4cebbe4093dc81149cb30aa26bedc69edef72fa627c987df89` |
| `docs/sm120-rank-executor-v1-r3.md` | `1bdceee409ec871edc4e193d967848e401f965e6f45d7a99782a7e444352cee8` |
| `docs/sm120-rank-executor-native-abi-v1.h` | `21ea614f1d8d140c322dadc2b4e851d6533d8611006d27f7c9c5467787819c5c` |
| `spec/engine-v0.md` | `52497e022bde5278372a9bce168e87a602fca9341c4cb4e019b4a3c7ce63179b` |
| `docs/target-layer-execution-v1.md` | `89c6cf7397a3dc6b0c01383e679dcc4b51e20e3c45057bef0928dc24b866a819` |
| `docs/target-layer-execution-v1-r2.md` | `808da35c2e54eb5692512996650839fb6f127cb91658603eb2fb5ce049c56ed2` |
| `docs/mtp-layer-execution-v1.md` | `5ad5bf01cdbd5e183b5e50aa0940344b5aabc09bf05a90c57d58e3e5b28dd3a7` |
| `docs/mtp-layer-execution-v1-r2.md` | `d75710b3b552f229cc3bef34a8977a7c30e5b03b4c4a268f27c0efb2a3d1f12c` |
| `docs/step-execution-abi-v3.md` | `1cde3bcabba0a0d861691b06ddb140cb64dfbefaab1129c8a04bc302c0ce609e` |
| `crates/glm-engine/src/worker.rs` | `52dbb32ef45bfa652ea113b7c3db7e4fb200bfd778015abb1aebceabaddf89d6` |
| `crates/glm-engine/src/startup.rs` | `1a5f1ac8aae94e6eb2aaf2cf4701dfc290604013103eacc1423046211609a5fc` |
| `crates/glm-engine/src/graph.rs` | `c85ca1aa52ba42294fc6a43524e8f70357523977d343e8c2f212787e7754cd22` |
| `crates/glm-engine/src/memory.rs` | `2131c999b6762a9b7e505cfe542c957877d95af4ee04056affa9d677156e9491` |
| `scripts/local-checks.sh` | `1675ca5bac9bda032ab6db206629bc0052ad6afc5cb6f59a7b6697e4e5c779d0` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

Run the complete local gate and both header compilers:

```text
./scripts/local-checks.sh

clang++ -std=c++17 -x c++ \
  -include docs/sm120-rank-executor-native-abi-v1.h \
  -fsyntax-only /dev/null

clang -std=c11 -x c \
  -include docs/sm120-rank-executor-native-abi-v1.h \
  -fsyntax-only /dev/null
```

The compiler commands are CPU layout checks only. A green compile does not
answer ownership/lifecycle questions or authorize implementation.

## Required retained r2 review

Repeat all 24 adversarial questions in
`docs/fable-sm120-rank-executor-v1-r2-handoff.md` against r1+r2 as amended by
r3. Do not inherit an answer from an unexecuted r2 review. In particular,
recheck collective-before-graph order, HBM and pinned-host accounting,
strict escrow, reverse cleanup, communicator abort, route latches, tier
commands, absolute deadlines, no next-step H2D, nonblocking completion, and
emitted-but-unmaterialized MTP failure.

## Decision 1: C11/C++17/Rust ABI parity

Independently emit every struct size, alignment, and field offset in both C11
and C++17. Emit every enum value and the type/signature of all 35 functions.
Compare the two records and independently construct the expected Rust
`#[repr(C, align(16))]` layouts. Confirm all 18 size and all 18 alignment
assertions execute in each language mode rather than being hidden behind a
C++ guard.

Attack the C alignment macro, unsupported-compiler branch, implicit padding,
enum signedness, function `noexcept` boundary, context-synchronize signature,
and any field whose C and C++ offset differs. Withhold acceptance for syntax
success without the independent offset/signature record.

## Decision 2: flags, capability families, and arena roles

Enumerate every flags field and stream/event flags argument. Confirm v1
requires exact zero and still creates nonblocking streams. Re-derive the exact
SM120 integer encoding and reject every alternate encoding.

Prove the module-family set is exactly target, MTP, and validation; that the
adopted set contains exactly the posture-required unique records; and that no
unknown/missing/duplicate family reaches graph construction. Enumerate all 18
arena roles, verify the device-versus-pinned kind partition, and attack
duplicate IDs, unknown roles, uncharged bytes, kind swaps, missing required
roles, and post-plan changes.

## Decision 3: graph-node native object

Trace each node kind through construction and destruction. Confirm target and
MTP nodes accept only an adopted module handle with the matching capability,
collective accepts only an adopted route, status requires zero, and validation
uses only its dedicated entry. Prove no program handle, symbol handle, hidden
constructor, or implementation-selected alternative exists.

Determine whether `node_kind`, semantic `program_sha256`, module image hash,
ordered capability records, and module-set capability digest jointly bind the
one fixed SM120-native entry family without leaving kernel selection to a
rank-local decision.

## Decision 4: synchronization and destruction safety

Trace normal shutdown, startup qualification failure, asynchronous kernel
failure, collective failure, device loss, context-synchronize error, and a
native call that never returns. Confirm synchronization occurs before any
possibly borrowed graph/route/event/stream/arena/module resource is destroyed,
yet never enters a healthy hot path.

Verify communicator abort precedes fatal synchronization, the supervisor owns
the nonreturning-call deadline, and failure leads to leak-then-process-exit
rather than unsafe free or continued service. Check that the error record,
status taxonomy, owner-thread rule, and exception translation cover the new
entry point exactly.

## Decision 5: compatibility and gate sequence

Determine whether r3 closes the C-alignment, missing-synchronization,
unconstructible-program-handle, flags, family, and arena-role gaps without
regressing any r1/r2 lifecycle or numerical boundary. Identify every pending
downstream handoff pinned to the superseded r2 header and state that its token
cannot authorize implementation against r3 bytes without an explicit
compatibility rebind.

Confirm acceptance opens only the coordinated CPU/mock proof. It does not
accept current workers, native code, cn4, graph/collective execution,
checkpoint loading, target/MTP kernels, quality, KV capacity, concurrency, or
performance.

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately and unambiguously:

1. Are all retained r2 closure decisions still accepted under r3?
2. Are C11, C++17, and required Rust layouts/signatures exact and identical?
3. Are flags, capability families, SM120 encoding, and arena roles complete?
4. Is every graph node's native-object type unambiguous and rank invariant?
5. Is context synchronization sufficient, hot-path excluded, and cleanup safe?
6. Are ownership, startup, memory, collectives, routes, tiering, deadlines,
   backpressure, and MTP failure accepted as one implementable contract?
7. Is the combined design accepted for its independent CPU/mock proof?

Only if every answer is an unqualified `YES`, include the candidate commit
and all sixteen exact input SHA-256 values from the provenance table, then end
with the requested acceptance token as the only bare acceptance line.

Withhold the token for stale bytes, C-only alignment drift, an unasserted
layout, unspecified flag/role/family, a hidden program handle, missing drain
operation, hot-path synchronization, unsafe cleanup after device loss,
rank-local route choice, an unclosed r2 finding, or a downstream/GPU claim.

