# Fable handoff: SM120 rank executor v1 corrective r4

Date: 2026-08-04

Status: superseding adversarial design and native-ABI review requested

GPU authorization conveyed by this handoff: none

Do not connect to cn4, launch CUDA, create a context, or modify a runtime
resource for this review.

Review candidate commit:
`688f5c7a4bfb79ed884693a968013a12a394d530`

Required result path:
`docs/reviews/fable-sm120-rank-executor-v1-r4.md`

Requested acceptance token, only if every blocker and major is resolved:
`sm120-rank-executor-v1-r4-design-accepted`

This handoff supersedes the unexecuted r2 and r3 handoffs. Do not issue their
tokens or authorize implementation from their header bytes. Review r1+r2+r3+
r4 together using the corrected header at this candidate.

The original withheld review remains a required operator-inbox input at
`docs/reviews/fable-sm120-rank-executor-v1.md`, SHA-256
`efe697b86235ac757fcfd9123d0f28b92a37f337a75f64267d8e96862620dd36`.
Hash it before using any closure claim and withhold the r4 token if it is
absent or differs.

## Required provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Any mismatch withholds the token.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/sm120-rank-executor-v1.md` | `e97c54b865ed50c40ff8b15f6580d0edc18dbd0783135bc1c17d11cc19986fd4` |
| `docs/sm120-rank-executor-v1-r2.md` | `4f40ea7652858b4cebbe4093dc81149cb30aa26bedc69edef72fa627c987df89` |
| `docs/sm120-rank-executor-v1-r3.md` | `1bdceee409ec871edc4e193d967848e401f965e6f45d7a99782a7e444352cee8` |
| `docs/sm120-rank-executor-v1-r4.md` | `4fa09dfd88329e3cf94190fa2084e9d7f863ff10a6ebbd02a7e0820cf35e3eb0` |
| `docs/sm120-rank-executor-native-abi-v1.h` | `b543e2a9fcc2cd30f385d174a5690ce73f09db373ea1c4a1bdea60b40f6daf13` |
| `docs/fable-sm120-rank-executor-v1-r2-handoff.md` | `71fc1d7d96fc52188ec97b97aa430b3178bd25310ebf40620b0ebe28863c5935` |
| `docs/fable-sm120-rank-executor-v1-r3-handoff.md` | `87100bca82c645f28040187fc9cd5466de1e551bf59de69d06ce896216e85aa7` |
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

Run:

```text
./scripts/local-checks.sh
clang++ -std=c++17 -x c++ \
  -include docs/sm120-rank-executor-native-abi-v1.h \
  -fsyntax-only /dev/null
clang -std=c11 -x c \
  -include docs/sm120-rank-executor-native-abi-v1.h \
  -fsyntax-only /dev/null
```

## Required retained review

Repeat all retained r2 questions and all five r3 decisions from the two pinned
handoffs against r1-r4. Do not inherit answers from an unexecuted review. In
particular, independently emit all C11/C++17/Rust field layouts, enum values,
and all 35 complete function signatures, including the corrected four-
argument validation-node entry.

## R4 validation-module decisions

1. Confirm that r3's dedicated validation-node route was unimplementable:
   the graph builder exposes no module-set handle, and the old entry supplied
   neither a validation module nor an unambiguous generation selector.
2. Confirm the corrected function takes the explicit nonzero module handle in
   C11, C++17, and the required Rust mirror without changing any struct byte.
3. Prove the module is owner-thread-local, same-context, loaded, adopted by
   the current generation, retained through graph instantiation, and has the
   unique `DEVICE_VALIDATION` capability bound by the module-set digest.
4. Attack zero, stale, unloaded, unadopted, foreign-context, wrong-family, and
   duplicate-family handles. Every case must fail before capture or enqueue.
5. Keep old and candidate hot-reload module generations resident together.
   Prove explicit binding prevents context scanning, newest-module selection,
   and cross-generation validation drift.
6. Confirm validation-module unload follows destruction of every borrowing
   graph and all r3 fatal/normal synchronization rules remain intact.
7. Confirm generic graph-node validation remains rejected, target/MTP module
   and collective-route meanings remain unchanged, and no built-in or
   rank-local fallback exists.

## Required answer

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Then
answer separately with an unqualified `YES` or `NO`:

1. Are all retained r2 and r3 closure decisions still accepted under r4?
2. Are all C11, C++17, and required Rust layouts and signatures exact?
3. Does the explicit validation-module argument close the otherwise missing
   native binding without introducing an implicit selection route?
4. Are module generation, capability, context, ownership, graph borrow, and
   destruction rules complete and fail-closed?
5. Are startup, memory, collectives, routes, tiering, deadlines,
   backpressure, synchronization, and MTP failure one implementable contract?
6. Is the combined r1-r4 design accepted for its independent CPU/mock proof?

Only if every answer is `YES`, attest the candidate and all nineteen exact
input hashes, then end with the requested acceptance token declared above as
the only bare acceptance line.

Acceptance opens only the coordinated CPU/mock proof. It does not accept the
current Rust workers, a native library, cn4, graph/collective execution,
checkpoint loading, target/MTP kernels, quality, KV capacity, concurrency, or
performance.
